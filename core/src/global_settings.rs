use std::path::PathBuf;

use color_eyre::eyre::Context;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use ts_rs::TS;

use crate::{error::Error, event_broadcaster::EventBroadcaster};

#[derive(Serialize, Deserialize, Clone, TS)]
#[ts(export)]
pub struct GlobalSettingsData {
    pub core_name: String,
    pub safe_mode: bool,
    pub domain: Option<String>,
}

impl Default for GlobalSettingsData {
    fn default() -> Self {
        Self {
            core_name: format!("{}'s Lodestone Core", whoami::realname()),
            safe_mode: true,
            domain: None,
        }
    }
}

pub struct GlobalSettings {
    path_to_global_settings: PathBuf,
    _event_broadcaster: EventBroadcaster,
    global_settings_data: GlobalSettingsData,
}

impl GlobalSettings {
    pub fn new(
        path_to_global_settings: PathBuf,
        _event_broadcaster: EventBroadcaster,
        global_settings_data: GlobalSettingsData,
    ) -> Self {
        Self {
            path_to_global_settings,
            _event_broadcaster,
            global_settings_data,
        }
    }
    pub async fn load_from_file(&mut self) -> Result<(), Error> {
        if tokio::fs::OpenOptions::new()
            .read(true)
            .create(true)
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
        Ok(())
    }
    async fn write_to_file(&self) -> Result<(), Error> {
        let mut file = tokio::fs::File::create(&self.path_to_global_settings)
            .await
            .context(format!(
                "Failed to create global settings file at {}",
                self.path_to_global_settings.display()
            ))?;
        file.write_all(
            serde_json::to_string_pretty(&self.global_settings_data)
                .context("Failed to serialize global settings data")?
                .as_bytes(),
        )
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
            Ok(_) => Ok(()),
            Err(e) => {
                self.global_settings_data.safe_mode = old_safe_mode;
                Err(e)
            }
        }
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
        let temp_dir = tempdir::TempDir::new("test_global_settings").unwrap();

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
