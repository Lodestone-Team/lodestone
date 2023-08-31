use std::path::{Path, PathBuf};

use async_trait::async_trait;
use color_eyre::eyre::{eyre, Context};
use indexmap::IndexMap;

use crate::{
    error::Error,
    events::CausedBy,
    macro_executor::{DefaultWorkerOptionGenerator, MacroPID, SpawnResult},
    traits::t_macro::{HistoryEntry, MacroEntry, TMacro, TaskEntry},
};
use crate::error::ErrorKind;
use crate::macro_executor::MacroExecutor;
use crate::traits::t_configurable::manifest::{SettingLocalCache, SettingManifest};

use super::MinecraftInstance;

pub fn resolve_macro_invocation(path_to_macro: &Path, macro_name: &str) -> Option<PathBuf> {
    let ts_macro = path_to_macro.join(macro_name).with_extension("ts");
    let js_macro = path_to_macro.join(macro_name).with_extension("js");

    let macro_folder = path_to_macro.join(macro_name);

    if ts_macro.is_file() {
        return Some(ts_macro);
    } else if js_macro.is_file() {
        return Some(js_macro);
    } else if macro_folder.is_dir() {
        // check if index.ts exists
        let index_ts = macro_folder.join("index.ts");
        let index_js = macro_folder.join("index.js");
        if index_ts.exists() {
            return Some(index_ts);
        } else if index_js.exists() {
            return Some(index_js);
        }
    }
    None
}

#[async_trait]
impl TMacro for MinecraftInstance {
    async fn get_macro_list(&self) -> Result<Vec<MacroEntry>, Error> {
        let mut ret = Vec::new();
        for entry in
            (std::fs::read_dir(&self.path_to_macros).context("Failed to read macro dir")?).flatten()
        {
            // if the entry is a file, check if it has the .ts or .js extension
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "ts" || ext == "js" {
                        ret.push(MacroEntry {
                            last_run: self.macro_name_to_last_run.lock().await.get(&name).cloned(),
                            name,
                            path,
                        })
                    }
                }
            } else if path.is_dir() {
                // check if index.ts or index.js exists
                let index_ts = path.join("index.ts");
                let index_js = path.join("index.js");
                if index_ts.exists() || index_js.exists() {
                    ret.push(MacroEntry {
                        last_run: self.macro_name_to_last_run.lock().await.get(&name).cloned(),
                        name,
                        path,
                    })
                }
            }
        }
        ret.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(ret)
    }

    async fn get_task_list(&self) -> Result<Vec<TaskEntry>, Error> {
        let mut ret = Vec::new();
        for (pid, task_entry) in self.pid_to_task_entry.lock().await.iter() {
            if self.macro_executor.get_macro_status(*pid).await.is_none() {
                ret.push(task_entry.clone());
            }
        }
        ret.sort_by(|a, b| a.creation_time.cmp(&b.creation_time));
        Ok(ret)
    }

    async fn get_history_list(&self) -> Result<Vec<HistoryEntry>, Error> {
        let mut ret = Vec::new();
        for (pid, task_entry) in self.pid_to_task_entry.lock().await.iter() {
            if let Some(exit_status) = self.macro_executor.get_macro_status(*pid).await {
                ret.push(HistoryEntry {
                    task: task_entry.clone(),
                    exit_status,
                });
            }
        }
        ret.sort_by(|a, b| b.exit_status.time().cmp(&a.exit_status.time()));
        Ok(ret)
    }

    async fn delete_macro(&self, name: &str) -> Result<(), Error> {
        crate::util::fs::remove_file(self.path_to_macros.join(name)).await?;
        Ok(())
    }

    async fn create_macro(&self, name: &str, content: &str) -> Result<(), Error> {
        crate::util::fs::write_all(self.path_to_macros.join(name), content.as_bytes().to_vec())
            .await
    }

    async fn run_macro(
        &self,
        name: &str,
        args: Vec<String>,
        caused_by: CausedBy,
    ) -> Result<TaskEntry, Error> {
        let path_to_macro = resolve_macro_invocation(&self.path_to_macros, name)
            .ok_or_else(|| eyre!("Failed to resolve macro invocation for {}", name))?;

        let SpawnResult { macro_pid: pid, .. } = self
            .macro_executor
            .spawn(
                path_to_macro,
                args,
                caused_by,
                Box::new(DefaultWorkerOptionGenerator),
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

        Ok(entry)
    }

    async fn kill_macro(&self, pid: MacroPID) -> Result<(), Error> {
        self.macro_executor.abort_macro(pid)?;
        Ok(())
    }

    async fn get_macro_config(&self, name: &str) -> Result<IndexMap<String, SettingManifest>, Error> {
        let path_to_macro = resolve_macro_invocation(&self.path_to_macros, name)
            .ok_or_else(|| eyre!("Failed to resolve macro invocation for {}", name))?;
        MacroExecutor::get_config_manifest(&path_to_macro).await
    }

    async fn store_macro_config_to_local(
        &self,
        name: &str,
        config_to_store: &IndexMap<String, SettingManifest>,
    ) -> Result<(), Error> {
        let mut local_configs: IndexMap<String, SettingLocalCache> = IndexMap::new();
        config_to_store.iter().for_each(|(_, config)| {
           local_configs.insert(config.get_identifier().clone(), SettingLocalCache::from(config));
        });

        let config_file_path = self.path_to_macros.join(name).join(format!("{name}_config")).with_extension("json");
        std::fs::write(
            config_file_path,
            serde_json::to_string_pretty(&local_configs).unwrap(),
        ).context("failed to create the config file")?;

        Ok(())
    }

    async fn validate_local_config(
        &self,
        name: &str,
        config_to_validate: Option<&IndexMap<String, SettingManifest>>,
    ) -> Result<IndexMap<String, SettingLocalCache>, Error> {
        let path_to_macro = resolve_macro_invocation(&self.path_to_macros, name)
            .ok_or_else(|| eyre!("Failed to resolve macro invocation for {}", name))?;

        let is_config_needed = match config_to_validate {
            None => std::fs::read_to_string(path_to_macro).context("failed to read macro file")?.contains("LodestoneConfig"),
            Some(_) => true,
        };

        // if the macro does not need a config, pass the validation ("vacuously true")
        if !is_config_needed {
            return Ok(IndexMap::new());
        }

        let config_file_path = self.path_to_macros.join(name).join(format!("{name}_config")).with_extension("json");
        match std::fs::read_to_string(config_file_path) {
            Ok(config_string) => {
                let local_configs: IndexMap<String, SettingLocalCache> = serde_json::from_str(&config_string)
                    .context("failed to parse local config cache")?;

                let configs = match config_to_validate {
                    Some(config) => config.clone(),
                    None => self.get_macro_config(name).await?,
                };

                let validation_result = local_configs.iter().fold(true, |partial_result, (setting_id, local_cache)| {
                    if !partial_result {
                        return false;
                    }
                    local_cache.match_type(&configs[setting_id])
                });

                if !validation_result {
                    return Err(Error {
                        kind: ErrorKind::Internal,
                        source: eyre!("There is a mismatch between a config type and its locally-stored value"),
                    });
                }

                Ok(local_configs)
            },
            Err(_) => Err(Error {
                kind: ErrorKind::NotFound,
                source: eyre!("Local config cache is not found"),
            }),
        }
    }
}
