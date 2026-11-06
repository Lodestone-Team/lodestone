use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use async_trait::async_trait;
use color_eyre::eyre::{eyre, Context};
use tracing::{debug, error, warn};
use url::Url;

use self::{bridge::procedure_call::ProcedureCallInner, r#macro::GenericMainWorkerGenerator};
use crate::{
    error::{Error, ErrorKind},
    event_broadcaster::EventBroadcaster,
    events::{CausedBy, ProgressionEventID},
    macro_executor::{MacroExecutor, MacroPID, SpawnResult},
    traits::{
        t_configurable::{manifest::SetupValue, TConfigurable},
        t_player::TPlayerManagement,
        t_server::TServer,
        InstanceInfo, TInstance,
    },
    types::DotLodestoneConfig,
};

mod bridge;
pub mod configurable;
mod r#macro;
pub mod player;
pub mod server;

/// The procedure bridge every atom runs (`js/main/mod.ts` and the files it imports), pinned
/// to a commit of the public Lodestone repository so that a push to a branch cannot change
/// the code atoms run.
///
/// To update it after changing the bridge or the files it imports, push the change, find
/// the new commit with
/// `git ls-remote https://github.com/Lodestone-Team/lodestone.git refs/heads/main`
/// and replace the SHA below. Existing atoms switch to the new pin on their next restore.
const ATOM_BRIDGE_MODULE: &str = "https://raw.githubusercontent.com/Lodestone-Team/lodestone/39f107baa22840a9f480a6576729763fe6d16513/core/src/implementations/generic/js/main/mod.ts";

/// Name of the bootstrap module written into every atom instance directory.
const BOOTSTRAP_FILE: &str = "run.ts";
/// The line of the bootstrap that imports the atom implementation.
const ATOM_IMPORT_PREFIX: &str = "import * as a from ";

#[derive(Clone)]
pub struct GenericInstance {
    dot_lodestone_config: DotLodestoneConfig,
    procedure_bridge: bridge::procedure_call::ProcedureBridge,
    path: PathBuf,
    /// Aborts the core macro once the last clone of the instance is dropped.
    _drop_guard: Arc<GenericDropGuard>,
}

/// RAII guard for dropping a generic instance
///
/// Will abort the core macro process if the instance is dropped
struct GenericDropGuard {
    core_macro_pid: MacroPID,
    macro_executor: MacroExecutor,
}

impl Drop for GenericDropGuard {
    fn drop(&mut self) {
        debug!("Dropping generic instance, aborting core macro process");
        let _ = self.macro_executor.abort_macro(self.core_macro_pid);
    }
}

/// Turns the source given when creating an atom (an http(s) or file URL, or an absolute
/// path, of the atom's directory or of its `main.ts`) into a URL.
fn parse_atom_location(source: &str) -> Result<Url, Error> {
    let bad_source = |reason: &str| Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("Invalid atom source {source:?}: {reason}"),
    };
    match Url::parse(source) {
        Ok(url) if matches!(url.scheme(), "http" | "https" | "file") => Ok(url),
        // anything else (including Windows paths such as C:\atom, which parse as a URL with
        // scheme "c") must be a filesystem path
        _ => {
            let path = Path::new(source);
            if !path.is_absolute() {
                return Err(bad_source(
                    "expected an http(s) or file URL, or an absolute path",
                ));
            }
            Url::from_file_path(path).map_err(|_| bad_source("not a valid file path"))
        }
    }
}

/// URL of the `main.ts` module of the atom whose directory is at `source`.
fn atom_module_url(source: &str) -> Result<Url, Error> {
    let mut dir = parse_atom_location(source)?;
    // `Url::join` replaces the last path segment unless the path ends with '/'
    if !dir.path().ends_with('/') {
        let path = format!("{}/", dir.path());
        dir.set_path(&path);
    }
    dir.join("main.ts").map_err(|e| Error {
        kind: ErrorKind::BadRequest,
        source: eyre!("Invalid atom source {source:?}: {e}"),
    })
}

/// A JavaScript string literal for `value`.
fn js_string_literal(value: &str) -> String {
    // a JSON string is a valid JS string literal
    serde_json::Value::from(value).to_string()
}

/// The bootstrap module that loads the atom at `atom_module` and runs the procedure bridge.
fn bootstrap_code(atom_module: &Url) -> String {
    include_str!("js/main/bootstrap.ts")
        .replacen(
            "__ATOM_MODULE__",
            &js_string_literal(atom_module.as_str()),
            1,
        )
        .replacen(
            "__BRIDGE_MODULE__",
            &js_string_literal(ATOM_BRIDGE_MODULE),
            1,
        )
}

/// Reads the atom module URL back from a bootstrap written by this or an older version.
fn atom_module_from_bootstrap(bootstrap: &str) -> Option<Url> {
    let literal = bootstrap
        .lines()
        .find_map(|line| line.trim().strip_prefix(ATOM_IMPORT_PREFIX))?
        .trim()
        .strip_suffix(';')?
        .trim();
    // Current bootstraps hold a JSON string; older ones pasted the URL between quotes.
    let module = serde_json::from_str::<String>(literal).ok().or_else(|| {
        literal
            .strip_prefix('"')
            .and_then(|l| l.strip_suffix('"'))
            .map(str::to_owned)
    })?;
    parse_atom_location(&module).ok()
}

/// Rewrites an existing instance's bootstrap if it predates the current bridge pin (older
/// versions imported the bridge from the unpinned `dev` branch).
async fn refresh_bootstrap(path_to_bootstrap: &Path) {
    let current = match tokio::fs::read_to_string(path_to_bootstrap).await {
        Ok(v) => v,
        Err(e) => {
            // spawning the macro reports the problem
            warn!("Failed to read {}: {e}", path_to_bootstrap.display());
            return;
        }
    };
    let Some(atom_module) = atom_module_from_bootstrap(&current) else {
        warn!(
            "Not updating {}: cannot find the atom it loads",
            path_to_bootstrap.display()
        );
        return;
    };
    let updated = bootstrap_code(&atom_module);
    if updated == current {
        return;
    }
    debug!("Updating atom bootstrap {}", path_to_bootstrap.display());
    if let Err(e) = tokio::fs::write(path_to_bootstrap, updated).await {
        warn!("Failed to update {}: {e}", path_to_bootstrap.display());
    }
}

impl GenericInstance {
    /// Starts the atom macro behind an instance and waits until its procedure bridge is
    /// ready. The macro is aborted again if anything after the spawn fails.
    async fn start_core_macro(
        path_to_bootstrap: PathBuf,
        dot_lodestone_config: &DotLodestoneConfig,
        core_macro_executor: &MacroExecutor,
    ) -> Result<
        (
            bridge::procedure_call::ProcedureBridge,
            Arc<GenericDropGuard>,
        ),
        Error,
    > {
        let procedure_bridge = bridge::procedure_call::ProcedureBridge::new();
        let SpawnResult {
            macro_pid: core_macro_pid,
            detach_future,
            exit_future,
        } = core_macro_executor
            .spawn(
                path_to_bootstrap,
                Vec::new(),
                CausedBy::System,
                Box::new(GenericMainWorkerGenerator::new(procedure_bridge.clone())),
                None,
                None,
                Some(dot_lodestone_config.uuid().clone()),
            )
            .await?;
        // from here on, every early return aborts the macro
        let drop_guard = Arc::new(GenericDropGuard {
            core_macro_pid,
            macro_executor: core_macro_executor.clone(),
        });
        procedure_bridge.close_on_exit(exit_future);
        // The bridge loop reports ready right before it detaches; fails if the macro exits
        // first (e.g. a bad atom URL or an exception in the atom code).
        tokio::select! {
            ready = procedure_bridge.wait_ready() => ready?,
            _ = detach_future => {}
        }
        Ok((procedure_bridge, drop_guard))
    }

    pub async fn new(
        path_to_source: PathBuf,
        path: PathBuf,
        dot_lodestone_config: DotLodestoneConfig,
        setup_value: SetupValue,
        progression_event_id: &ProgressionEventID,
        _event_broadcaster: EventBroadcaster,
        core_macro_executor: MacroExecutor,
    ) -> Result<Self, Error> {
        let source = path_to_source.to_str().ok_or_else(|| Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("Atom source is not valid UTF-8"),
        })?;
        let atom_module = atom_module_url(source)?;
        tokio::fs::create_dir_all(&path).await.context(format!(
            "Failed to create directory for instance at {}",
            path.display()
        ))?;
        let path_to_config = path.join(".lodestone_config");

        let path_to_bootstrap = path.join(BOOTSTRAP_FILE);
        tokio::fs::write(&path_to_bootstrap, bootstrap_code(&atom_module))
            .await
            .context(format!(
                "Failed to write bootstrap to {}",
                path_to_bootstrap.display()
            ))?;
        tokio::fs::write(
            &path_to_config,
            serde_json::to_string_pretty(&dot_lodestone_config).context(
                "Failed to serialize config to string. This is a bug, please report it.",
            )?,
        )
        .await
        .context(format!(
            "Failed to write config to {}",
            path_to_config.display()
        ))?;

        let (procedure_bridge, drop_guard) = Self::start_core_macro(
            path_to_bootstrap,
            &dot_lodestone_config,
            &core_macro_executor,
        )
        .await?;
        procedure_bridge
            .call(ProcedureCallInner::SetupInstance {
                dot_lodestone_config: dot_lodestone_config.clone(),
                setup_value,
                progression_event_id: progression_event_id.inner(),
                path: path.clone(),
            })
            .await?;
        Ok(GenericInstance {
            dot_lodestone_config,
            procedure_bridge,
            path,
            _drop_guard: drop_guard,
        })
    }

    pub async fn restore(
        path_to_instance: PathBuf,
        dot_lodestone_config: DotLodestoneConfig,
        _event_broadcaster: EventBroadcaster,
        core_macro_executor: MacroExecutor,
    ) -> Result<Self, Error> {
        let path_to_bootstrap = path_to_instance.join(BOOTSTRAP_FILE);
        refresh_bootstrap(&path_to_bootstrap).await;
        let (procedure_bridge, drop_guard) = Self::start_core_macro(
            path_to_bootstrap,
            &dot_lodestone_config,
            &core_macro_executor,
        )
        .await?;
        procedure_bridge
            .call(ProcedureCallInner::RestoreInstance {
                dot_lodestone_config: dot_lodestone_config.clone(),
                path: path_to_instance.clone(),
            })
            .await?;
        Ok(GenericInstance {
            dot_lodestone_config,
            procedure_bridge,
            path: path_to_instance,
            _drop_guard: drop_guard,
        })
    }

    /// Will notify the typescript side that the instance is being destructed
    pub async fn destruct(self) {
        let _ = self
            .procedure_bridge
            .call(ProcedureCallInner::DestructInstance)
            .await
            .map_err(|e| {
                error!("Generic instance destructor raised an error: {}", e);
            });
    }
}

#[async_trait]
impl TInstance for GenericInstance {
    async fn get_instance_info(&self) -> InstanceInfo {
        // issued together so the round trips to the atom overlap
        let (
            name,
            game_type,
            description,
            version,
            port,
            auto_start,
            restart_on_crash,
            state,
            player_count,
            max_player_count,
            player_list,
        ) = tokio::join!(
            self.name(),
            self.game_type(),
            self.description(),
            self.version(),
            self.port(),
            self.auto_start(),
            self.restart_on_crash(),
            self.state(),
            self.get_player_count(),
            self.get_max_player_count(),
            self.get_player_list(),
        );
        InstanceInfo {
            uuid: self.uuid().await,
            name,
            game_type,
            description,
            version,
            port,
            creation_time: self.creation_time().await,
            path: self.path().await.display().to_string(),
            auto_start,
            restart_on_crash,
            state,
            player_count: player_count.ok(),
            max_player_count: max_player_count.ok(),
            player_list: player_list.ok(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atom_module_url_joins_main_ts() {
        for (source, expected) in [
            (
                "https://example.com/atoms/my_atom",
                "https://example.com/atoms/my_atom/main.ts",
            ),
            (
                "https://example.com/atoms/my_atom/",
                "https://example.com/atoms/my_atom/main.ts",
            ),
            // backslashes are path separators in http URLs
            (
                "https://example.com/atoms\\my_atom",
                "https://example.com/atoms/my_atom/main.ts",
            ),
        ] {
            assert_eq!(atom_module_url(source).unwrap().as_str(), expected);
        }
        #[cfg(unix)]
        assert_eq!(
            atom_module_url("/srv/atoms/my atom").unwrap().as_str(),
            "file:///srv/atoms/my%20atom/main.ts"
        );
        for bad in [
            "relative/atom",
            "ftp://example.com/atom",
            "javascript:alert(1)",
            "",
        ] {
            assert!(atom_module_url(bad).is_err(), "{bad} was accepted");
        }
    }

    #[test]
    fn bootstrap_quotes_the_atom_url() {
        // a quote in the source must not end the string literal
        let url = atom_module_url("https://example.com/a\";globalThis.pwned=1;\"").unwrap();
        let code = bootstrap_code(&url);
        let import = code
            .lines()
            .find(|l| l.starts_with(ATOM_IMPORT_PREFIX))
            .unwrap();
        assert_eq!(import.matches('"').count(), 2, "{import}");
        assert!(code.contains(&js_string_literal(ATOM_BRIDGE_MODULE)));
        assert!(!code.contains("__ATOM_MODULE__") && !code.contains("__BRIDGE_MODULE__"));
        // and it can be read back for later restores
        assert_eq!(atom_module_from_bootstrap(&code), Some(url));
    }

    #[test]
    fn old_bootstraps_are_recognized() {
        let old = r#"// This file "links" a concrete implementation of the Atom instance to the procedure bridge

import * as a from "https://example.com/atoms/my_atom/main.ts";
import { run } from "https://raw.githubusercontent.com/Lodestone-Team/lodestone/dev/core/src/implementations/generic/js/main/mod.ts";
const instance = new a.default();
run(instance);"#;
        let module = atom_module_from_bootstrap(old).unwrap();
        assert_eq!(module.as_str(), "https://example.com/atoms/my_atom/main.ts");
        let updated = bootstrap_code(&module);
        assert!(!updated.contains("/lodestone/dev/"));
        assert!(updated.contains(ATOM_BRIDGE_MODULE));
        assert_eq!(atom_module_from_bootstrap("console.log(1);"), None);
    }

    #[test]
    fn bridge_is_pinned_to_a_commit() {
        let rest = ATOM_BRIDGE_MODULE
            .strip_prefix("https://raw.githubusercontent.com/Lodestone-Team/lodestone/")
            .unwrap();
        let (sha, path) = rest.split_once('/').unwrap();
        assert_eq!(sha.len(), 40);
        assert!(sha.chars().all(|c| c.is_ascii_hexdigit()));
        assert_eq!(path, "core/src/implementations/generic/js/main/mod.ts");
    }
}
