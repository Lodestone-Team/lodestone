use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use ts_rs::TS;

use crate::{
    error::{Error, ErrorKind},
    event_broadcaster::EventBroadcaster,
    persist::write_atomic,
};

/// The message of every request refused because code execution is turned off.
pub const CODE_EXECUTION_DISABLED_MESSAGE: &str = "Code execution is disabled on this core";

/// The environment variable that locks code execution off, see
/// [`code_execution_lock_requested`].
pub const DISABLE_CODE_EXECUTION_ENV: &str = "LODESTONE_DISABLE_CODE_EXECUTION";

fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct GlobalSettingsData {
    pub core_name: String,
    pub safe_mode: bool,
    pub domain: Option<String>,
    #[serde(default)]
    pub playit_enabled: bool,
    // Whether macros, atom instances, extensions, instance scripts and custom Java commands
    // may run. Files written by older versions have no switch, so it defaults to on.
    #[serde(default = "default_true")]
    pub code_execution_enabled: bool,
    // Whether the administrator locked code execution off with LODESTONE_DISABLE_CODE_EXECUTION
    // or --disable-code-execution. Computed at startup: never read from the settings file or
    // a request, and never written to the file (see `stored_settings_json`).
    #[serde(default, skip_deserializing)]
    pub code_execution_locked: bool,
}

/// A lock-free view of the code execution kill switch, shared by everything that runs
/// user-supplied code. It is off whenever the setting is off or the switch is locked.
#[derive(Clone, Debug)]
pub struct CodeExecutionSwitch {
    enabled: Arc<AtomicBool>,
}

impl CodeExecutionSwitch {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// Fails with [`CODE_EXECUTION_DISABLED_MESSAGE`] (403) while code execution is off.
    pub fn check(&self) -> Result<(), Error> {
        if self.is_enabled() {
            Ok(())
        } else {
            Err(code_execution_disabled_error())
        }
    }

    /// Only [`GlobalSettings`] changes the switch outside of tests.
    pub(crate) fn set(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
    }
}

pub fn code_execution_disabled_error() -> Error {
    Error {
        kind: ErrorKind::PermissionDenied,
        source: eyre!(CODE_EXECUTION_DISABLED_MESSAGE),
    }
}

/// Whether the administrator asked to lock code execution off: with the
/// `--disable-code-execution` flag, or with [`DISABLE_CODE_EXECUTION_ENV`] set to `1`, `true`
/// or `yes` (in any case).
pub fn code_execution_lock_requested(cli_flag: bool, env_value: Option<&str>) -> bool {
    cli_flag
        || env_value.is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
}

/// Reads whether code execution must be locked off from the CLI flag and the environment,
/// and logs the result.
pub fn code_execution_lock_from_env(cli_flag: bool) -> bool {
    let env_value = std::env::var(DISABLE_CODE_EXECUTION_ENV).ok();
    let locked = code_execution_lock_requested(cli_flag, env_value.as_deref());
    if locked {
        let reason = if cli_flag {
            "--disable-code-execution".to_string()
        } else {
            format!("{DISABLE_CODE_EXECUTION_ENV} is set")
        };
        warn!(
            "Code execution is locked off ({reason}): macros, atom instances, extensions, \
             instance scripts and custom Java commands are disabled"
        );
    } else if env_value.is_some() {
        info!("Ignoring {DISABLE_CODE_EXECUTION_ENV}: it is not 1, true or yes");
    }
    locked
}

/// The JSON written to the settings file. `code_execution_locked` is computed at startup, so
/// it is left out.
fn stored_settings_json(data: &GlobalSettingsData) -> Result<String, Error> {
    let mut value =
        serde_json::to_value(data).context("Failed to serialize global settings data")?;
    if let Some(object) = value.as_object_mut() {
        object.remove("code_execution_locked");
    }
    Ok(serde_json::to_string_pretty(&value).context("Failed to serialize global settings data")?)
}

impl Default for GlobalSettingsData {
    fn default() -> Self {
        Self {
            core_name: format!(
                "{}'s Lodestone Core",
                whoami::realname()
                    .or_else(|_| whoami::username())
                    .unwrap_or_else(|_| "Unknown".to_string())
            ),
            safe_mode: true,
            domain: None,
            playit_enabled: true,
            code_execution_enabled: true,
            code_execution_locked: false,
        }
    }
}

pub struct GlobalSettings {
    path_to_global_settings: PathBuf,
    _event_broadcaster: EventBroadcaster,
    global_settings_data: GlobalSettingsData,
    /// Mirrors `global_settings_data.safe_mode`, see [`GlobalSettings::safe_mode_flag`].
    safe_mode_flag: Arc<AtomicBool>,
    /// On when `global_settings_data.code_execution_enabled` is on and the switch is not
    /// locked, see [`GlobalSettings::code_execution_switch`].
    code_execution: CodeExecutionSwitch,
    /// Code execution was locked off at startup and cannot be turned on through the API.
    code_execution_locked: bool,
}

impl GlobalSettings {
    pub fn new(
        path_to_global_settings: PathBuf,
        _event_broadcaster: EventBroadcaster,
        global_settings_data: GlobalSettingsData,
    ) -> Self {
        let safe_mode_flag = Arc::new(AtomicBool::new(global_settings_data.safe_mode));
        let code_execution = CodeExecutionSwitch::new(global_settings_data.code_execution_enabled);
        Self {
            path_to_global_settings,
            _event_broadcaster,
            global_settings_data,
            safe_mode_flag,
            code_execution,
            code_execution_locked: false,
        }
    }

    /// Locks code execution off for the lifetime of this core: the switch stays off and
    /// [`GlobalSettings::set_code_execution_enabled`] refuses to turn it on. The stored
    /// setting is kept, so it applies again once the lock is removed and the core restarts.
    pub fn lock_code_execution(&mut self) {
        self.code_execution_locked = true;
        self.sync_code_execution_switch();
    }

    fn sync_code_execution_switch(&self) {
        self.code_execution
            .set(self.global_settings_data.code_execution_enabled && !self.code_execution_locked);
    }
    pub async fn load_from_file(&mut self) -> Result<(), Error> {
        if tokio::fs::OpenOptions::new()
            .read(true)
            .create(true)
            .truncate(false)
            .write(true)
            .open(&self.path_to_global_settings)
            .await
            .context(format!(
                "Failed to open global settings file at {}",
                self.path_to_global_settings.display()
            ))?
            .metadata()
            .await
            .context(format!(
                "Failed to get metadata for global settings file at {}",
                self.path_to_global_settings.display()
            ))?
            .len()
            == 0
        {
            self.global_settings_data = GlobalSettingsData::default();
        } else {
            self.global_settings_data = serde_json::from_slice(
                &tokio::fs::read(&self.path_to_global_settings)
                    .await
                    .context(format!(
                        "Failed to read global settings file at {}",
                        self.path_to_global_settings.display()
                    ))?,
            )
            .context(format!(
                "Failed to parse global settings file at {}",
                self.path_to_global_settings.display()
            ))?;
        }
        self.safe_mode_flag
            .store(self.global_settings_data.safe_mode, Ordering::SeqCst);
        self.sync_code_execution_switch();
        Ok(())
    }
    /// Writes the settings to a temporary file and renames it over the old one, so a crash
    /// during the write cannot leave a truncated file that stops Lodestone from starting.
    async fn write_to_file(&self) -> Result<(), Error> {
        let contents = stored_settings_json(&self.global_settings_data)?;
        write_atomic(&self.path_to_global_settings, contents, None)
            .await
            .context(format!(
                "Failed to write to global settings file at {}",
                self.path_to_global_settings.display()
            ))?;
        Ok(())
    }
    pub async fn set_core_name(&mut self, name: String) -> Result<(), Error> {
        let old_name = self.global_settings_data.core_name.clone();
        self.global_settings_data.core_name = name;
        match self.write_to_file().await {
            Ok(_) => Ok(()),
            Err(e) => {
                self.global_settings_data.core_name = old_name;
                Err(e)
            }
        }
    }

    pub fn core_name(&self) -> String {
        self.global_settings_data.core_name.clone()
    }

    pub async fn set_safe_mode(&mut self, safe_mode: bool) -> Result<(), Error> {
        let old_safe_mode = self.global_settings_data.safe_mode;
        self.global_settings_data.safe_mode = safe_mode;
        match self.write_to_file().await {
            Ok(_) => {
                self.safe_mode_flag.store(safe_mode, Ordering::SeqCst);
                Ok(())
            }
            Err(e) => {
                self.global_settings_data.safe_mode = old_safe_mode;
                Err(e)
            }
        }
    }

    /// A shared flag that always holds the current safe mode setting. Reading it needs no
    /// lock on the global settings.
    pub fn safe_mode_flag(&self) -> Arc<AtomicBool> {
        self.safe_mode_flag.clone()
    }

    pub fn safe_mode(&self) -> bool {
        self.global_settings_data.safe_mode
    }

    pub async fn set_domain(&mut self, domain: Option<String>) -> Result<(), Error> {
        let old_domain = self.global_settings_data.domain.clone();
        self.global_settings_data.domain = domain;
        match self.write_to_file().await {
            Ok(_) => Ok(()),
            Err(e) => {
                self.global_settings_data.domain = old_domain;
                Err(e)
            }
        }
    }

    pub fn domain(&self) -> Option<String> {
        self.global_settings_data.domain.clone()
    }

    pub async fn set_playit_enabled(&mut self, playit_enabled: bool) -> Result<(), Error> {
        let old_playit_enabled = self.global_settings_data.playit_enabled;
        self.global_settings_data.playit_enabled = playit_enabled;
        match self.write_to_file().await {
            Ok(_) => Ok(()),
            Err(e) => {
                self.global_settings_data.playit_enabled = old_playit_enabled;
                Err(e)
            }
        }
    }

    pub fn playit_enabled(&self) -> bool {
        self.global_settings_data.playit_enabled
    }

    /// Turns code execution on or off and saves the setting. Turning it on fails with 403
    /// while the switch is locked. After turning it off, the caller must stop the code that
    /// is still running.
    pub async fn set_code_execution_enabled(&mut self, enabled: bool) -> Result<(), Error> {
        if enabled && self.code_execution_locked {
            return Err(Error {
                kind: ErrorKind::PermissionDenied,
                source: eyre!(
                    "Code execution is locked off by the server administrator \
                     ({DISABLE_CODE_EXECUTION_ENV} or --disable-code-execution), so it cannot \
                     be turned on here"
                ),
            });
        }
        let old_enabled = self.global_settings_data.code_execution_enabled;
        self.global_settings_data.code_execution_enabled = enabled;
        match self.write_to_file().await {
            Ok(_) => {
                self.sync_code_execution_switch();
                Ok(())
            }
            Err(e) => {
                self.global_settings_data.code_execution_enabled = old_enabled;
                Err(e)
            }
        }
    }

    /// Whether code execution is on: the setting is on and the switch is not locked.
    pub fn code_execution_enabled(&self) -> bool {
        self.code_execution.is_enabled()
    }

    pub fn code_execution_locked(&self) -> bool {
        self.code_execution_locked
    }

    /// A shared, lock-free view of the code execution switch.
    pub fn code_execution_switch(&self) -> CodeExecutionSwitch {
        self.code_execution.clone()
    }

    /// The settings as clients see them: with `code_execution_locked` filled in, and
    /// `code_execution_enabled` off while locked.
    pub fn client_view(&self) -> GlobalSettingsData {
        let mut data = self.global_settings_data.clone();
        data.code_execution_enabled = data.code_execution_enabled && !self.code_execution_locked;
        data.code_execution_locked = self.code_execution_locked;
        data
    }
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn test_global_settings() {
        use super::*;
        use std::path::PathBuf;

        // create a temporary directory
        let temp_dir = tempfile::TempDir::with_prefix("test_global_settings").unwrap();

        // create a global settings object

        let (event_broadcaster, _) = EventBroadcaster::new(10);

        let mut global_settings = GlobalSettings::new(
            PathBuf::from(temp_dir.path()).join("global_settings.json"),
            event_broadcaster,
            GlobalSettingsData::default(),
        );

        // load the global settings from the file

        global_settings.load_from_file().await.unwrap();

        // check that the default values are correct

        assert!(global_settings.safe_mode());

        // the lock-free flag follows the setting
        let flag = global_settings.safe_mode_flag();
        assert!(flag.load(Ordering::SeqCst));
        global_settings.set_safe_mode(false).await.unwrap();
        assert!(!flag.load(Ordering::SeqCst));
        global_settings.set_safe_mode(true).await.unwrap();
        assert!(flag.load(Ordering::SeqCst));

        // set the core name

        global_settings
            .set_core_name("test_core_name".to_string())
            .await
            .unwrap();

        // check that the core name was set correctly

        assert_eq!(global_settings.core_name(), "test_core_name");

        drop(global_settings);

        // create a new global settings object

        let (event_broadcaster, _) = EventBroadcaster::new(10);

        let mut global_settings = GlobalSettings::new(
            PathBuf::from(temp_dir.path()).join("global_settings.json"),
            event_broadcaster,
            GlobalSettingsData::default(),
        );

        // load the global settings from the file

        global_settings.load_from_file().await.unwrap();

        // check that the core name was set correctly

        assert_eq!(global_settings.core_name(), "test_core_name");
    }

    mod code_execution {
        use super::super::*;

        fn settings_in(dir: &tempfile::TempDir) -> GlobalSettings {
            let (event_broadcaster, _) = EventBroadcaster::new(10);
            GlobalSettings::new(
                dir.path().join("global_settings.json"),
                event_broadcaster,
                GlobalSettingsData::default(),
            )
        }

        fn stored_json(dir: &tempfile::TempDir) -> serde_json::Value {
            serde_json::from_slice(&std::fs::read(dir.path().join("global_settings.json")).unwrap())
                .unwrap()
        }

        #[test]
        fn lock_is_requested_by_flag_or_env() {
            for value in ["1", "true", "yes", "TRUE", "Yes", " 1 "] {
                assert!(code_execution_lock_requested(false, Some(value)), "{value}");
            }
            for value in ["", "0", "false", "no", "on", "2"] {
                assert!(
                    !code_execution_lock_requested(false, Some(value)),
                    "{value}"
                );
            }
            assert!(!code_execution_lock_requested(false, None));
            assert!(code_execution_lock_requested(true, None));
            assert!(code_execution_lock_requested(true, Some("0")));
        }

        #[test]
        fn old_settings_files_default_to_enabled_and_lock_is_ignored_on_input() {
            let data: GlobalSettingsData = serde_json::from_str(
                r#"{"core_name":"c","safe_mode":true,"domain":null,"code_execution_locked":true}"#,
            )
            .unwrap();
            assert!(data.code_execution_enabled);
            assert!(!data.code_execution_locked);
            let data: GlobalSettingsData = serde_json::from_str(
                r#"{"core_name":"c","safe_mode":true,"domain":null,"code_execution_enabled":false}"#,
            )
            .unwrap();
            assert!(!data.code_execution_enabled);
        }

        #[tokio::test]
        async fn switch_follows_setting_and_is_saved() {
            let dir = tempfile::TempDir::with_prefix("test_code_execution").unwrap();
            let mut settings = settings_in(&dir);
            settings.load_from_file().await.unwrap();
            let switch = settings.code_execution_switch();
            assert!(switch.is_enabled());
            assert!(switch.check().is_ok());

            settings.set_code_execution_enabled(false).await.unwrap();
            assert!(!switch.is_enabled());
            assert!(!settings.code_execution_enabled());
            let err = switch.check().unwrap_err();
            assert!(matches!(err.kind, ErrorKind::PermissionDenied));
            assert_eq!(err.source.to_string(), CODE_EXECUTION_DISABLED_MESSAGE);
            assert_eq!(stored_json(&dir)["code_execution_enabled"], false);

            // the setting survives a restart
            let mut reloaded = settings_in(&dir);
            reloaded.load_from_file().await.unwrap();
            assert!(!reloaded.code_execution_enabled());
            assert!(!reloaded.code_execution_switch().is_enabled());

            settings.set_code_execution_enabled(true).await.unwrap();
            assert!(switch.is_enabled());
            assert_eq!(stored_json(&dir)["code_execution_enabled"], true);
        }

        #[tokio::test]
        async fn lock_forces_switch_off_and_is_never_written() {
            let dir = tempfile::TempDir::with_prefix("test_code_execution_lock").unwrap();
            let mut settings = settings_in(&dir);
            settings.lock_code_execution();
            settings.load_from_file().await.unwrap();
            let switch = settings.code_execution_switch();
            assert!(!switch.is_enabled());
            assert!(settings.code_execution_locked());

            let view = settings.client_view();
            assert!(!view.code_execution_enabled);
            assert!(view.code_execution_locked);

            // cannot be turned on through the API
            let err = settings.set_code_execution_enabled(true).await.unwrap_err();
            assert!(matches!(err.kind, ErrorKind::PermissionDenied));
            assert!(!switch.is_enabled());

            // turning it off is allowed and saved; the lock itself is not
            settings.set_code_execution_enabled(false).await.unwrap();
            settings.set_core_name("locked".to_string()).await.unwrap();
            let stored = stored_json(&dir);
            assert!(stored.get("code_execution_locked").is_none());
            assert_eq!(stored["code_execution_enabled"], false);

            // the client view of an unlocked core reports the stored value
            let mut unlocked = settings_in(&dir);
            unlocked.load_from_file().await.unwrap();
            let view = unlocked.client_view();
            assert!(!view.code_execution_locked);
            assert!(!view.code_execution_enabled);
        }

        #[tokio::test]
        async fn lock_keeps_the_stored_setting() {
            let dir = tempfile::TempDir::with_prefix("test_code_execution_keep").unwrap();
            let mut settings = settings_in(&dir);
            settings.load_from_file().await.unwrap();
            settings.set_core_name("x".to_string()).await.unwrap();
            assert_eq!(stored_json(&dir)["code_execution_enabled"], true);

            let mut locked = settings_in(&dir);
            locked.lock_code_execution();
            locked.load_from_file().await.unwrap();
            assert!(!locked.code_execution_enabled());
            locked.set_core_name("y".to_string()).await.unwrap();
            // the stored setting is still on, so removing the lock restores it
            assert_eq!(stored_json(&dir)["code_execution_enabled"], true);
            assert!(stored_json(&dir).get("code_execution_locked").is_none());
        }
    }
}
