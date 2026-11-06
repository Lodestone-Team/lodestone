use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use color_eyre::eyre::Context;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::{error::Error, event_broadcaster::EventBroadcaster, persist::write_atomic};

#[derive(Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct GlobalSettingsData {
    pub core_name: String,
    pub safe_mode: bool,
    pub domain: Option<String>,
    #[serde(default)]
    pub playit_enabled: bool,
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
        }
    }
}

pub struct GlobalSettings {
    path_to_global_settings: PathBuf,
    _event_broadcaster: EventBroadcaster,
    global_settings_data: GlobalSettingsData,
    /// Mirrors `global_settings_data.safe_mode`, see [`GlobalSettings::safe_mode_flag`].
    safe_mode_flag: Arc<AtomicBool>,
}

impl GlobalSettings {
    pub fn new(
        path_to_global_settings: PathBuf,
        _event_broadcaster: EventBroadcaster,
        global_settings_data: GlobalSettingsData,
    ) -> Self {
        let safe_mode_flag = Arc::new(AtomicBool::new(global_settings_data.safe_mode));
        Self {
            path_to_global_settings,
            _event_broadcaster,
            global_settings_data,
            safe_mode_flag,
        }
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
        Ok(())
    }
    /// Writes the settings to a temporary file and renames it over the old one, so a crash
    /// during the write cannot leave a truncated file that stops Lodestone from starting.
    async fn write_to_file(&self) -> Result<(), Error> {
        let contents = serde_json::to_string_pretty(&self.global_settings_data)
            .context("Failed to serialize global settings data")?;
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
}

impl AsRef<GlobalSettingsData> for GlobalSettings {
    fn as_ref(&self) -> &GlobalSettingsData {
        &self.global_settings_data
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
}
