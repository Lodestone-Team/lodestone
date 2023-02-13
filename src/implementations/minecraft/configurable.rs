use std::{collections::HashMap, sync::atomic};

use async_trait::async_trait;
use color_eyre::eyre::{eyre, Context};
use tempdir::TempDir;

use crate::error::{Error, ErrorKind};
use crate::traits::t_configurable::TConfigurable;
use crate::traits::t_server::State;

use crate::types::InstanceUuid;
use crate::util::download_file;

use super::util::{get_fabric_jar_url, get_vanilla_jar_url, get_paper_jar_url};
use super::{BackupInstruction, MinecraftInstance};

#[async_trait]
impl TConfigurable for MinecraftInstance {
    async fn uuid(&self) -> InstanceUuid {
        self.config.uuid.clone()
    }

    async fn name(&self) -> String {
        self.config.name.clone()
    }

    async fn game_type(&self) -> String {
        self.config.game_type.clone()
    }

    async fn flavour(&self) -> String {
        self.config.flavour.to_string()
    }

    async fn cmd_args(&self) -> Vec<String> {
        self.config.cmd_args.clone()
    }

    async fn description(&self) -> String {
        self.config.description.clone()
    }

    async fn port(&self) -> u32 {
        self.config.port
    }

    async fn min_ram(&self) -> Result<u32, Error> {
        Ok(self.config.min_ram)
    }

    async fn max_ram(&self) -> Result<u32, Error> {
        Ok(self.config.max_ram)
    }

    async fn creation_time(&self) -> i64 {
        self.config.creation_time
    }

    async fn path(&self) -> std::path::PathBuf {
        self.config.path.clone()
    }

    async fn auto_start(&self) -> bool {
        self.config.auto_start
    }

    async fn restart_on_crash(&self) -> bool {
        self.config.restart_on_crash
    }

    async fn backup_period(&self) -> Result<Option<u32>, Error> {
        Ok(self.config.backup_period)
    }


    async fn set_name(&mut self, name: String) -> Result<(), Error> {
        if name.is_empty() {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Name cannot be empty"),
            });
        }
        if name.len() > 100 {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Name cannot be longer than 100 characters"),
            });
        }
        self.config.name = name;
        self.write_config_to_file().await?;
        Ok(())
    }

    async fn set_description(&mut self, description: String) -> Result<(), Error> {
        self.config.description = description;
        self.write_config_to_file().await?;
        Ok(())
    }

    async fn set_port(&mut self, port: u32) -> Result<(), Error> {
        self.config.port = port;
        *self
            .settings
            .lock()
            .await
            .entry("server-port".to_string())
            .or_insert_with(|| port.to_string()) = port.to_string();
        self.write_config_to_file()
            .await
            .and(self.write_properties_to_file().await)
    }

    async fn set_cmd_args(&mut self, cmd_args: Vec<String>) -> Result<(), Error> {
        self.config.cmd_args = cmd_args;
        self.write_config_to_file().await
    }

    async fn set_min_ram(&mut self, min_ram: u32) -> Result<(), Error> {
        self.config.min_ram = min_ram;
        self.write_config_to_file().await
    }

    async fn set_max_ram(&mut self, max_ram: u32) -> Result<(), Error> {
        self.config.max_ram = max_ram;
        self.write_config_to_file().await
    }

    async fn set_auto_start(&mut self, auto_start: bool) -> Result<(), Error> {
        self.config.auto_start = auto_start;
        self.auto_start.store(auto_start, atomic::Ordering::Relaxed);
        self.write_config_to_file().await
    }

    async fn set_restart_on_crash(&mut self, restart_on_crash: bool) -> Result<(), Error> {
        self.config.restart_on_crash = restart_on_crash;
        self.auto_start
            .store(restart_on_crash, atomic::Ordering::Relaxed);
        self.write_config_to_file().await
    }

    async fn set_backup_period(&mut self, backup_period: Option<u32>) -> Result<(), Error> {
        self.config.backup_period = backup_period;
        self.backup_sender
            .send(BackupInstruction::SetPeriod(backup_period))
            .unwrap();
        self.write_config_to_file().await
    }

    async fn set_field(&mut self, field: &str, value: String) -> Result<(), Error> {
        self.settings.lock().await.insert(field.to_string(), value);
        self.write_properties_to_file().await
    }

    async fn get_field(&self, field: &str) -> Result<String, Error> {
        Ok(self
            .settings
            .lock()
            .await
            .get(field)
            .ok_or(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Field {} not found", field),
            })?
            .to_string())
    }

    async fn change_version(&mut self, version: String) -> Result<(), Error> {
        if *self.state.lock().await != State::Stopped {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Cannot change version while server is running"),
            });
        }
        if version == self.config.version {
            return Ok(());
        }
        let url = match self.config.flavour {
            super::Flavour::Vanilla => get_vanilla_jar_url(&version).await.ok_or_else(|| {
                let error_msg =
                    format!("Cannot get the vanilla jar version for version {}", version);
                Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!(error_msg),
                }
            })?,
            super::Flavour::Fabric => get_fabric_jar_url(&version, None, None).await.ok_or_else(|| {
                let error_msg =
                    format!("Cannot get the fabric jar version for version {}", version);
                Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!(error_msg),
                }
            })?,
            super::Flavour::Paper => get_paper_jar_url(&version, None).await.ok_or_else(|| {
                let error_msg =
                    format!("Cannot get the paper jar version for version {}", version);
                Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!(error_msg),
                }
            })?,
            super::Flavour::Spigot => todo!(),
        };
        let temp_dir = TempDir::new("lodestone")
            .context("Cannot create temporary directory to download the server jar file")?;
        download_file(
            &url,
            temp_dir.path(),
            Some("server.jar"),
            &Box::new(|_| {}),
            true,
        )
        .await?;
        let jar_path = temp_dir.path().join("server.jar");
        tokio::fs::rename(jar_path, self.path().await.join("server.jar"))
            .await
            .context("Cannot move the downloaded server jar file to the server directory")?;
        self.config.version = version;
        self.write_config_to_file().await
    }

    async fn settings(&self) -> Result<HashMap<String, String>, Error> {
        Ok(self.settings.lock().await.clone())
    }
}
