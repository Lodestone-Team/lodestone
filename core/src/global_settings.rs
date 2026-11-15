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
    remote_access::RemoteAccess,
};

/// The message of every request refused because code execution is turned off.
pub const CODE_EXECUTION_DISABLED_MESSAGE: &str = "Code execution is disabled on this core";

/// The environment variable that locks code execution off, see
/// [`code_execution_lock_requested`].
pub const DISABLE_CODE_EXECUTION_ENV: &str = "LODESTONE_DISABLE_CODE_EXECUTION";

pub(crate) fn default_true() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct GlobalSettingsData {
    pub core_name: String,
    pub safe_mode: bool,
    pub domain: Option<String>,
    // Whether custom Java commands may run. Files written by older versions have no switch, so it defaults to on.
    #[serde(default = "default_true")]
    pub code_execution_enabled: bool,
    // Whether the administrator locked code execution off with LODESTONE_DISABLE_CODE_EXECUTION
    // or --disable-code-execution. Computed at startup: never read from the settings file or
    // a request, and never written to the file (see `stored_settings_json`).
    #[serde(default, skip_deserializing)]
    pub code_execution_locked: bool,
    // Whether other devices may use plain HTTP (otherwise only HTTPS). New installs start
    // with it off. Files written by older versions have no field and served plain HTTP to
    // everyone, so it defaults to on for them; it is saved once resolved, see `load_from_file`.
    #[serde(default = "default_true")]
    pub allow_lan_http: bool,
    // Whether instances the owner marks as listed are shown to anyone, without signing in,
    // at `GET /public/instances` (for Lodestone Launcher). Off unless the owner turns it on.
    #[serde(default)]
    pub list_instances_publicly: bool,
    // Whether a program on this computer (Lodestone Launcher) may claim owner access by
    // proving it can write to the core's data folder, see `crate::local_claim`. Anything
    // that can write there already controls the core, so it is on by default, also for
    // files written before it existed.
    #[serde(default = "default_true")]
    pub allow_local_claim: bool,
    // Whether other devices may use the core, see `crate::remote_access`. Off only for
    // Lodestone Host's core until its owner sets a password to share it. Files written
    // before it existed have no field: they keep serving other devices.
    #[serde(default = "default_true")]
    pub allow_remote_access: bool,
    // Whether the core looks for newer versions of the modpacks servers were made from, in
    // the background, see `crate::modpack::updates`. On by default, also for files written
    // before it existed.
    #[serde(default = "default_true")]
    pub check_modpack_updates: bool,
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
        warn!("Code execution is locked off ({reason}): custom Java commands are disabled");
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
            code_execution_enabled: true,
            code_execution_locked: false,
            allow_lan_http: false,
            list_instances_publicly: false,
            allow_local_claim: true,
            // a standalone core is set up from the dashboard with its setup key; Lodestone
            // Host's core turns this off at startup while it has no owner (`crate::run`)
            allow_remote_access: true,
            check_modpack_updates: true,
        }
    }
}

pub struct GlobalSettings {
    path_to_global_settings: PathBuf,
    _event_broadcaster: EventBroadcaster,
    global_settings_data: GlobalSettingsData,
    /// Mirrors `global_settings_data.safe_mode`, see [`GlobalSettings::safe_mode_flag`].
    safe_mode_flag: Arc<AtomicBool>,
    /// Mirrors `global_settings_data.allow_lan_http`, see
    /// [`GlobalSettings::allow_lan_http_flag`].
    allow_lan_http_flag: Arc<AtomicBool>,
    /// Mirrors `global_settings_data.allow_remote_access`, see
    /// [`GlobalSettings::remote_access`].
    remote_access: Arc<RemoteAccess>,
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
        let allow_lan_http_flag = Arc::new(AtomicBool::new(global_settings_data.allow_lan_http));
        let code_execution = CodeExecutionSwitch::new(global_settings_data.code_execution_enabled);
        let remote_access = RemoteAccess::new(global_settings_data.allow_remote_access);
        Self {
            path_to_global_settings,
            _event_broadcaster,
            global_settings_data,
            safe_mode_flag,
            allow_lan_http_flag,
            remote_access,
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
    /// Loads the settings, or the defaults when the file is new or empty. Settings that the
    /// file does not have yet (a new install, or `allow_lan_http` from an older version) are
    /// saved right away, so their resolved value no longer depends on the file's age.
    pub async fn load_from_file(&mut self) -> Result<(), Error> {
        let must_save;
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
            must_save = true;
        } else {
            let stored: serde_json::Value = serde_json::from_slice(
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
            must_save = stored.get("allow_lan_http").is_none()
                || stored.get("allow_remote_access").is_none();
            self.global_settings_data = serde_json::from_value(stored).context(format!(
                "Failed to parse global settings file at {}",
                self.path_to_global_settings.display()
            ))?;
        }
        self.safe_mode_flag
            .store(self.global_settings_data.safe_mode, Ordering::SeqCst);
        self.allow_lan_http_flag
            .store(self.global_settings_data.allow_lan_http, Ordering::SeqCst);
        self.remote_access
            .set(self.global_settings_data.allow_remote_access);
        self.sync_code_execution_switch();
        if must_save {
            // Not fatal: the value is the same on the next start, just resolved again.
            if let Err(e) = self.write_to_file().await {
                warn!("Failed to save the global settings: {}", e.source);
            }
        }
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

    /// Allows or refuses plain HTTP from other devices. Takes effect for new connections.
    pub async fn set_allow_lan_http(&mut self, allow_lan_http: bool) -> Result<(), Error> {
        let old_allow_lan_http = self.global_settings_data.allow_lan_http;
        self.global_settings_data.allow_lan_http = allow_lan_http;
        match self.write_to_file().await {
            Ok(_) => {
                self.allow_lan_http_flag
                    .store(allow_lan_http, Ordering::SeqCst);
                Ok(())
            }
            Err(e) => {
                self.global_settings_data.allow_lan_http = old_allow_lan_http;
                Err(e)
            }
        }
    }

    /// A shared flag that always holds the current `allow_lan_http` setting. The web server
    /// reads it for every new connection without locking the global settings.
    pub fn allow_lan_http_flag(&self) -> Arc<AtomicBool> {
        self.allow_lan_http_flag.clone()
    }

    pub fn allow_lan_http(&self) -> bool {
        self.global_settings_data.allow_lan_http
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

    /// Shows or hides the listed instances at `GET /public/instances`.
    pub async fn set_list_instances_publicly(&mut self, enabled: bool) -> Result<(), Error> {
        let old = self.global_settings_data.list_instances_publicly;
        self.global_settings_data.list_instances_publicly = enabled;
        match self.write_to_file().await {
            Ok(_) => Ok(()),
            Err(e) => {
                self.global_settings_data.list_instances_publicly = old;
                Err(e)
            }
        }
    }

    pub fn list_instances_publicly(&self) -> bool {
        self.global_settings_data.list_instances_publicly
    }

    pub async fn set_allow_local_claim(&mut self, enabled: bool) -> Result<(), Error> {
        let old = self.global_settings_data.allow_local_claim;
        self.global_settings_data.allow_local_claim = enabled;
        match self.write_to_file().await {
            Ok(_) => Ok(()),
            Err(e) => {
                self.global_settings_data.allow_local_claim = old;
                Err(e)
            }
        }
    }

    pub fn allow_local_claim(&self) -> bool {
        self.global_settings_data.allow_local_claim
    }

    /// Turns the background check for newer modpack versions on or off.
    pub async fn set_check_modpack_updates(&mut self, enabled: bool) -> Result<(), Error> {
        let old = self.global_settings_data.check_modpack_updates;
        self.global_settings_data.check_modpack_updates = enabled;
        match self.write_to_file().await {
            Ok(_) => Ok(()),
            Err(e) => {
                self.global_settings_data.check_modpack_updates = old;
                Err(e)
            }
        }
    }

    pub fn check_modpack_updates(&self) -> bool {
        self.global_settings_data.check_modpack_updates
    }

    /// Lets other devices use the core, or only this computer, see
    /// [`crate::remote_access`]. Takes effect at once. Turning it off stays in effect
    /// even when it can't be saved (the error says so): that is the safe side.
    pub async fn set_allow_remote_access(&mut self, allowed: bool) -> Result<(), Error> {
        let old = self.global_settings_data.allow_remote_access;
        self.global_settings_data.allow_remote_access = allowed;
        if !allowed {
            self.remote_access.set(false);
        }
        match self.write_to_file().await {
            Ok(_) => {
                self.remote_access.set(allowed);
                Ok(())
            }
            Err(e) => {
                if allowed {
                    self.global_settings_data.allow_remote_access = old;
                }
                Err(e)
            }
        }
    }

    pub fn allow_remote_access(&self) -> bool {
        self.global_settings_data.allow_remote_access
    }

    /// The setting, shared with the web server, which reads it for every request without
    /// locking the global settings.
    pub fn remote_access(&self) -> Arc<RemoteAccess> {
        self.remote_access.clone()
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

    #[tokio::test]
    async fn listing_instances_publicly_is_off_unless_turned_on() {
        use super::*;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("global_settings.json");
        // a file from before the setting existed
        std::fs::write(
            &path,
            r#"{"core_name":"c","safe_mode":true,"domain":null,"allow_lan_http":false}"#,
        )
        .unwrap();
        let (event_broadcaster, _) = EventBroadcaster::new(10);
        let mut settings = GlobalSettings::new(
            path.clone(),
            event_broadcaster,
            GlobalSettingsData::default(),
        );
        settings.load_from_file().await.unwrap();
        assert!(!settings.list_instances_publicly());
        assert!(!GlobalSettingsData::default().list_instances_publicly);
        settings.set_list_instances_publicly(true).await.unwrap();
        let (event_broadcaster, _) = EventBroadcaster::new(10);
        let mut again = GlobalSettings::new(path, event_broadcaster, GlobalSettingsData::default());
        again.load_from_file().await.unwrap();
        assert!(again.list_instances_publicly());
        assert!(again.client_view().list_instances_publicly);
    }

    /// Cores before Lodestone Connect saved `playit_enabled`. Such a file still loads with
    /// its other settings, and the old field is dropped the next time the file is saved.
    #[tokio::test]
    async fn settings_from_before_connect_still_load() {
        use super::*;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("global_settings.json");
        std::fs::write(
            &path,
            r#"{"core_name":"old","safe_mode":false,"domain":"example.com","playit_enabled":true,
                "code_execution_enabled":false,"allow_lan_http":true,"list_instances_publicly":true,
                "allow_local_claim":false}"#,
        )
        .unwrap();
        let (event_broadcaster, _) = EventBroadcaster::new(10);
        let mut settings = GlobalSettings::new(
            path.clone(),
            event_broadcaster,
            GlobalSettingsData::default(),
        );
        settings.load_from_file().await.unwrap();
        assert_eq!(settings.core_name(), "old");
        assert!(!settings.safe_mode());
        assert_eq!(settings.domain().as_deref(), Some("example.com"));
        assert!(settings.allow_lan_http());
        assert!(settings.list_instances_publicly());
        assert!(!settings.client_view().code_execution_enabled);
        assert!(!settings.client_view().allow_local_claim);
        settings.set_core_name("new".to_string()).await.unwrap();
        let stored: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(stored["core_name"], "new");
        assert!(stored.get("playit_enabled").is_none());
    }

    mod allow_lan_http {
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

        #[tokio::test]
        async fn new_install_refuses_lan_http_and_saves_it() {
            let dir = tempfile::TempDir::with_prefix("test_allow_lan_http_new").unwrap();
            let mut settings = settings_in(&dir);
            settings.load_from_file().await.unwrap();
            assert!(!settings.allow_lan_http());
            assert!(!settings.allow_lan_http_flag().load(Ordering::SeqCst));
            assert_eq!(stored_json(&dir)["allow_lan_http"], false);
        }

        #[tokio::test]
        async fn existing_file_without_the_field_allows_lan_http_and_saves_it() {
            let dir = tempfile::TempDir::with_prefix("test_allow_lan_http_old").unwrap();
            std::fs::write(
                dir.path().join("global_settings.json"),
                r#"{"core_name":"old core","safe_mode":true,"domain":null}"#,
            )
            .unwrap();
            let mut settings = settings_in(&dir);
            settings.load_from_file().await.unwrap();
            assert!(settings.allow_lan_http());
            assert!(settings.allow_lan_http_flag().load(Ordering::SeqCst));
            let stored = stored_json(&dir);
            assert_eq!(stored["allow_lan_http"], true);
            assert_eq!(stored["core_name"], "old core");
        }

        #[tokio::test]
        async fn stored_value_is_kept_and_setter_updates_flag_and_file() {
            let dir = tempfile::TempDir::with_prefix("test_allow_lan_http_set").unwrap();
            std::fs::write(
                dir.path().join("global_settings.json"),
                r#"{"core_name":"c","safe_mode":true,"domain":null,"allow_lan_http":true}"#,
            )
            .unwrap();
            let mut settings = settings_in(&dir);
            settings.load_from_file().await.unwrap();
            let flag = settings.allow_lan_http_flag();
            assert!(flag.load(Ordering::SeqCst));

            settings.set_allow_lan_http(false).await.unwrap();
            assert!(!flag.load(Ordering::SeqCst));
            assert!(!settings.client_view().allow_lan_http);
            assert_eq!(stored_json(&dir)["allow_lan_http"], false);

            let mut reloaded = settings_in(&dir);
            reloaded.load_from_file().await.unwrap();
            assert!(!reloaded.allow_lan_http());

            settings.set_allow_lan_http(true).await.unwrap();
            assert!(flag.load(Ordering::SeqCst));
            assert_eq!(stored_json(&dir)["allow_lan_http"], true);
        }
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
