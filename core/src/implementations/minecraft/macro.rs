use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use color_eyre::eyre::{eyre, Context};
use futures::FutureExt;
use indexmap::IndexMap;

use crate::error::ErrorKind;
use crate::macro_executor::MacroExecutor;
use crate::traits::t_configurable::manifest::{
    ConfigurableValue, SettingLocalCache, SettingManifest,
};
use crate::{
    error::Error,
    events::CausedBy,
    macro_executor::{DefaultWorkerOptionGenerator, MacroPID, SpawnResult},
    traits::t_macro::{HistoryEntry, MacroEntry, TMacro, TaskEntry},
};

use super::MinecraftInstance;

/// How many finished macro runs are kept in the history
const MAX_FINISHED_TASKS: usize = 100;

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

/// Macro names come from URLs and are joined to the macros folder, so they must be a
/// single plain path component.
pub(super) fn validate_macro_name(name: &str) -> Result<(), Error> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', '\0'])
        || name.chars().any(|c| c.is_control())
    {
        return Err(bad_request(format!("Invalid macro name: {name:?}")));
    }
    Ok(())
}

/// True for names that can be used as JavaScript variable names.
fn is_js_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_' || c == '$')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

/// Builds the code that defines the macro's config object. Values are written as JSON
/// literals, so quotes or newlines in them can't change the code.
fn config_injection_code(
    config_map: &IndexMap<String, SettingLocalCache>,
) -> Result<Option<String>, Error> {
    let Some((_, first)) = config_map.get_index(0) else {
        return Ok(None);
    };
    let config_var_name = first.get_identifier().split('|').next().unwrap_or_default();
    if !is_js_identifier(config_var_name) {
        return Err(bad_request(format!(
            "Invalid macro config name: {config_var_name:?}"
        )));
    }
    let mut code = format!("let {config_var_name} = {{\r\n");
    for (var_name, meta) in config_map {
        if !is_js_identifier(var_name) {
            return Err(bad_request(format!(
                "Invalid macro config variable name: {var_name:?}"
            )));
        }
        let value_code = match meta.get_value() {
            Some(ConfigurableValue::String(s) | ConfigurableValue::Enum(s)) => {
                serde_json::to_string(s).context("Failed to encode a macro config value")?
            }
            Some(ConfigurableValue::Boolean(b)) => b.to_string(),
            Some(ConfigurableValue::Integer(n)) => n.to_string(),
            Some(ConfigurableValue::UnsignedInteger(n)) => n.to_string(),
            Some(ConfigurableValue::Float(f)) if f.is_finite() => f.to_string(),
            Some(ConfigurableValue::Float(_)) => "NaN".to_string(),
            None => "undefined".to_string(),
        };
        code.push_str(&format!("  {var_name}: {value_code},\r\n"));
    }
    code.push_str("};\r\n");
    Ok(Some(code))
}

async fn is_file(path: &Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .map(|m| m.is_file())
        .unwrap_or(false)
}

async fn is_dir(path: &Path) -> bool {
    tokio::fs::metadata(path)
        .await
        .map(|m| m.is_dir())
        .unwrap_or(false)
}

/// Finds `<name>.ts`, `<name>.js`, `<name>/index.ts` or `<name>/index.js`.
pub async fn resolve_macro_invocation(path_to_macro: &Path, macro_name: &str) -> Option<PathBuf> {
    if validate_macro_name(macro_name).is_err() {
        return None;
    }
    let ts_macro = path_to_macro.join(macro_name).with_extension("ts");
    let js_macro = path_to_macro.join(macro_name).with_extension("js");
    let macro_folder = path_to_macro.join(macro_name);

    if is_file(&ts_macro).await {
        return Some(ts_macro);
    } else if is_file(&js_macro).await {
        return Some(js_macro);
    } else if is_dir(&macro_folder).await {
        let index_ts = macro_folder.join("index.ts");
        let index_js = macro_folder.join("index.js");
        if is_file(&index_ts).await {
            return Some(index_ts);
        } else if is_file(&index_js).await {
            return Some(index_js);
        }
    }
    None
}

impl MinecraftInstance {
    fn config_file_path(&self, name: &str) -> PathBuf {
        self.path_to_macros
            .join(name)
            .join(format!("{name}_config"))
            .with_extension("json")
    }

    async fn resolve_macro(&self, name: &str) -> Result<PathBuf, Error> {
        validate_macro_name(name)?;
        resolve_macro_invocation(&self.path_to_macros, name)
            .await
            .ok_or_else(|| Error {
                kind: ErrorKind::NotFound,
                source: eyre!("Failed to resolve macro invocation for {}", name),
            })
    }

    /// Keeps running macros and the newest finished ones.
    async fn prune_task_history(&self) {
        let entries: Vec<(MacroPID, i64)> = self
            .pid_to_task_entry
            .lock()
            .await
            .iter()
            .map(|(pid, entry)| (*pid, entry.creation_time))
            .collect();
        let mut finished = Vec::new();
        for (pid, creation_time) in entries {
            if self.macro_executor.get_macro_status(pid).await.is_some() {
                finished.push((pid, creation_time));
            }
        }
        if finished.len() <= MAX_FINISHED_TASKS {
            return;
        }
        finished.sort_by_key(|(_, creation_time)| std::cmp::Reverse(*creation_time));
        let mut tasks = self.pid_to_task_entry.lock().await;
        for (pid, _) in finished.into_iter().skip(MAX_FINISHED_TASKS) {
            tasks.shift_remove(&pid);
        }
    }
}

#[async_trait]
impl TMacro for MinecraftInstance {
    async fn get_macro_list(&self) -> Result<Vec<MacroEntry>, Error> {
        let mut ret = Vec::new();
        let mut dir = tokio::fs::read_dir(&self.path_to_macros)
            .await
            .context("Failed to read macro dir")?;
        let last_run = self.macro_name_to_last_run.lock().await.clone();
        while let Some(entry) = dir.next_entry().await.context("Failed to read macro dir")? {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            // a macro is a folder with index.ts or index.js
            if is_dir(&path).await
                && (is_file(&path.join("index.ts")).await || is_file(&path.join("index.js")).await)
            {
                ret.push(MacroEntry {
                    last_run: last_run.get(&name).cloned(),
                    name,
                    path,
                })
            }
        }
        ret.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(ret)
    }

    async fn get_task_list(&self) -> Result<Vec<TaskEntry>, Error> {
        // don't hold the lock while asking the executor
        let entries: Vec<TaskEntry> = self
            .pid_to_task_entry
            .lock()
            .await
            .values()
            .cloned()
            .collect();
        let mut ret = Vec::new();
        for entry in entries {
            if self
                .macro_executor
                .get_macro_status(entry.pid)
                .await
                .is_none()
            {
                ret.push(entry);
            }
        }
        ret.sort_by_key(|entry| entry.creation_time);
        Ok(ret)
    }

    async fn get_history_list(&self) -> Result<Vec<HistoryEntry>, Error> {
        let entries: Vec<TaskEntry> = self
            .pid_to_task_entry
            .lock()
            .await
            .values()
            .cloned()
            .collect();
        let mut ret = Vec::new();
        for entry in entries {
            if let Some(exit_status) = self.macro_executor.get_macro_status(entry.pid).await {
                ret.push(HistoryEntry {
                    task: entry,
                    exit_status,
                });
            }
        }
        ret.sort_by_key(|entry| std::cmp::Reverse(entry.exit_status.time()));
        Ok(ret)
    }

    /// Deletes a macro: its folder, or its single `.ts`/`.js` file.
    async fn delete_macro(&self, name: &str) -> Result<(), Error> {
        validate_macro_name(name)?;
        let folder = self.path_to_macros.join(name);
        if is_dir(&folder).await {
            return crate::util::fs::remove_dir_all(folder).await;
        }
        for extension in ["ts", "js"] {
            let file = folder.with_extension(extension);
            if is_file(&file).await {
                return crate::util::fs::remove_file(file).await;
            }
        }
        Err(Error {
            kind: ErrorKind::NotFound,
            source: eyre!("Macro {name} not found"),
        })
    }

    /// Creates a macro as `<name>/index.ts`, the layout that the macro list shows.
    async fn create_macro(&self, name: &str, content: &str) -> Result<(), Error> {
        validate_macro_name(name)?;
        let folder = self.path_to_macros.join(name);
        crate::util::fs::create_dir_all(&folder).await?;
        crate::util::fs::write_all(folder.join("index.ts"), content.as_bytes().to_vec()).await
    }

    async fn run_macro(
        &self,
        name: &str,
        args: Vec<String>,
        configs: Option<IndexMap<String, SettingLocalCache>>,
        caused_by: CausedBy,
    ) -> Result<TaskEntry, Error> {
        let path_to_macro = self.resolve_macro(name).await?;

        let config_code = match configs {
            Some(config_map) => config_injection_code(&config_map)?,
            None => None,
        };

        let SpawnResult { macro_pid: pid, .. } = self
            .macro_executor
            .spawn(
                path_to_macro,
                args,
                caused_by,
                Box::new(DefaultWorkerOptionGenerator),
                config_code,
                None,
                Some(self.uuid.clone()),
            )
            .await?;
        let entry = TaskEntry {
            pid,
            name: name.to_string(),
            creation_time: chrono::Utc::now().timestamp(),
        };
        self.pid_to_task_entry
            .lock()
            .await
            .insert(pid, entry.clone());
        self.macro_name_to_last_run
            .lock()
            .await
            .insert(name.to_string(), chrono::Utc::now().timestamp());
        self.prune_task_history().await;

        Ok(entry)
    }

    async fn kill_macro(&self, pid: MacroPID) -> Result<(), Error> {
        self.macro_executor.abort_macro(pid)?;
        Ok(())
    }

    async fn get_macro_config(
        &self,
        name: &str,
    ) -> Result<IndexMap<String, SettingManifest>, Error> {
        let path_to_macro = self.resolve_macro(name).await?;
        // The config parser runs on user-written macro source. Until it stops panicking
        // on unusual input, turn a panic into an error for this request.
        AssertUnwindSafe(MacroExecutor::get_config_manifest(&path_to_macro))
            .catch_unwind()
            .await
            .unwrap_or_else(|_| {
                Err(Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!("Failed to parse the config of macro {name}"),
                })
            })
    }

    async fn store_macro_config_to_local(
        &self,
        name: &str,
        config_to_store: &IndexMap<String, SettingManifest>,
    ) -> Result<(), Error> {
        self.resolve_macro(name).await?;
        let local_configs: IndexMap<String, SettingLocalCache> = config_to_store
            .iter()
            .map(|(var_name, config)| (var_name.clone(), SettingLocalCache::from(config)))
            .collect();
        // reject what run_macro would refuse to inject
        config_injection_code(&local_configs)?;

        let content = serde_json::to_string_pretty(&local_configs)
            .context("Failed to serialize the macro config")?;
        tokio::fs::write(self.config_file_path(name), content)
            .await
            .context("failed to create the config file")?;

        Ok(())
    }

    async fn validate_local_config(
        &self,
        name: &str,
        config_to_validate: Option<&IndexMap<String, SettingManifest>>,
    ) -> Result<IndexMap<String, SettingLocalCache>, Error> {
        let path_to_macro = self.resolve_macro(name).await?;

        let is_config_needed = match config_to_validate {
            None => tokio::fs::read_to_string(path_to_macro)
                .await
                .context("failed to read macro file")?
                .contains("LodestoneConfig"),
            Some(_) => true,
        };

        // if the macro does not need a config, pass the validation ("vacuously true")
        if !is_config_needed {
            return Ok(IndexMap::new());
        }

        match tokio::fs::read_to_string(self.config_file_path(name)).await {
            Ok(config_string) => {
                let local_configs: IndexMap<String, SettingLocalCache> =
                    serde_json::from_str(&config_string)
                        .context("failed to parse local config cache")?;

                let configs = match config_to_validate {
                    Some(config) => config.clone(),
                    None => self.get_macro_config(name).await?,
                };

                let validation_result = local_configs.len() == configs.len()
                    && local_configs.iter().all(|(setting_id, local_cache)| {
                        local_cache.validate_type(configs.get(setting_id))
                    });

                if !validation_result {
                    return Err(Error {
                        kind: ErrorKind::Internal,
                        source: eyre!("There is a mismatch between a config type and its locally-stored value"),
                    });
                }

                Ok(local_configs)
            }
            Err(_) => Err(Error {
                kind: ErrorKind::NotFound,
                source: eyre!("Local config cache is not found"),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cache(id: &str, value: ConfigurableValue) -> SettingLocalCache {
        let manifest = SettingManifest::new_optional_value(
            id.to_string(),
            String::new(),
            String::new(),
            Some(value.clone()),
            value.infer_type(),
            None,
            false,
            true,
        );
        SettingLocalCache::from(&manifest)
    }

    #[test]
    fn macro_names() {
        assert!(validate_macro_name("backup").is_ok());
        assert!(validate_macro_name("my.macro-1").is_ok());
        for bad in ["", ".", "..", "../x", "a/b", "a\\b", "a\0b"] {
            assert!(validate_macro_name(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn config_values_are_json_literals() {
        let mut map = IndexMap::new();
        map.insert(
            "message".to_string(),
            cache(
                "config|message",
                ConfigurableValue::String("it's \"x\"\n'); evil();//".into()),
            ),
        );
        map.insert(
            "count".to_string(),
            cache("config|count", ConfigurableValue::Integer(-3)),
        );
        map.insert(
            "flag".to_string(),
            cache("config|flag", ConfigurableValue::Boolean(true)),
        );
        let code = config_injection_code(&map).unwrap().unwrap();
        assert_eq!(
            code,
            "let config = {\r\n  message: \"it's \\\"x\\\"\\n'); evil();//\",\r\n  count: -3,\r\n  flag: true,\r\n};\r\n"
        );
    }

    #[test]
    fn config_names_must_be_identifiers() {
        let mut map = IndexMap::new();
        map.insert(
            "x".to_string(),
            cache("a = 1; evil(); let b|x", ConfigurableValue::Boolean(true)),
        );
        assert!(config_injection_code(&map).is_err());

        let mut map = IndexMap::new();
        map.insert(
            "x: 1, y".to_string(),
            cache("config|x", ConfigurableValue::Boolean(true)),
        );
        assert!(config_injection_code(&map).is_err());

        assert_eq!(config_injection_code(&IndexMap::new()).unwrap(), None);
    }
}
