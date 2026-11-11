use async_trait::async_trait;
use color_eyre::eyre::{eyre, Context};
use tracing::{info, warn};

use crate::error::{Error, ErrorKind};
use crate::prelude::path_to_tmp;
use crate::traits::t_configurable::manifest::{
    ConfigurableManifest, ConfigurableValue, ConfigurableValueType, SettingManifest,
};
use crate::traits::t_configurable::{Game, TConfigurable};
use crate::traits::t_server::State;

use crate::types::InstanceUuid;
use crate::util::download_file;

use super::util::{
    get_fabric_jar_url, get_jre_major_version, get_paper_jar_url, get_vanilla_jar_url,
};
use super::{
    ensure_jre, java_binary, join_args, server_properties, split_args, validate_instance_name,
    Flavour, MinecraftInstance,
};

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

#[async_trait]
impl TConfigurable for MinecraftInstance {
    async fn uuid(&self) -> InstanceUuid {
        self.uuid.clone()
    }

    async fn name(&self) -> String {
        self.config.lock().await.name.clone()
    }

    async fn game_type(&self) -> Game {
        self.config.lock().await.flavour.clone().into()
    }

    async fn version(&self) -> String {
        self.config.lock().await.version.clone()
    }

    async fn description(&self) -> String {
        self.config.lock().await.description.clone()
    }

    async fn port(&self) -> u32 {
        self.config.lock().await.port
    }

    async fn creation_time(&self) -> i64 {
        self.creation_time
    }

    async fn path(&self) -> std::path::PathBuf {
        self.path_to_instance.clone()
    }

    async fn auto_start(&self) -> bool {
        self.config.lock().await.auto_start
    }

    async fn restart_on_crash(&self) -> bool {
        self.config.lock().await.restart_on_crash
    }

    async fn set_name(&self, name: String) -> Result<(), Error> {
        validate_instance_name(&name)?;
        self.config.lock().await.name = name;
        self.write_config_to_file().await?;
        Ok(())
    }

    async fn set_description(&self, description: String) -> Result<(), Error> {
        self.config.lock().await.description = description;
        self.write_config_to_file().await?;
        Ok(())
    }

    async fn set_auto_start(&self, auto_start: bool) -> Result<(), Error> {
        self.config.lock().await.auto_start = auto_start;
        self.write_config_to_file().await
    }

    async fn set_restart_on_crash(&self, restart_on_crash: bool) -> Result<(), Error> {
        self.config.lock().await.restart_on_crash = restart_on_crash;
        self.write_config_to_file().await
    }

    async fn change_version(&self, version: String) -> Result<(), Error> {
        // start() refuses to run while this lock is held
        let _guard = self.version_change_lock.lock().await;
        if *self.state.lock().await != State::Stopped {
            return Err(bad_request("Cannot change version while server is running"));
        }
        let (old_version, old_flavour, old_jre) = {
            let config = self.config.lock().await;
            (
                config.version.clone(),
                config.flavour.clone(),
                config.jre_major_version,
            )
        };
        if version == old_version {
            return Ok(());
        }
        let not_found = |e: Error| Error {
            kind: ErrorKind::BadRequest,
            source: e.source.wrap_err(format!(
                "Cannot get the {} jar for version {version}",
                old_flavour.to_string()
            )),
        };
        // Look up new loader/build versions too: the old ones belong to the old version.
        let (url, new_flavour) = match old_flavour {
            Flavour::Vanilla => get_vanilla_jar_url(&version).await.map_err(not_found)?,
            Flavour::Fabric { .. } => get_fabric_jar_url(&version, &None, &None)
                .await
                .map_err(not_found)?,
            Flavour::Paper { .. } => get_paper_jar_url(&version, &None)
                .await
                .map_err(not_found)?,
            Flavour::Spigot => {
                return Err(Error {
                    kind: ErrorKind::UnsupportedOperation,
                    source: eyre!("Spigot servers are not supported"),
                })
            }
            Flavour::Forge { .. } => {
                return Err(Error {
                    kind: ErrorKind::UnsupportedOperation,
                    source: eyre!("Changing versions is unsupported for forge servers"),
                })
            }
        };

        // A newer Minecraft version may need a newer Java.
        let new_jre = get_jre_major_version(&version).await.context(format!(
            "Could not find the Java version for Minecraft {version}"
        ))?;
        if new_jre != old_jre {
            info!(
                "[{}] Minecraft {version} needs Java {new_jre}, downloading it if needed",
                self.name().await
            );
            ensure_jre(&self.path_to_runtimes, new_jre, &|_| {}).await?;
        }

        let lodestone_tmp = path_to_tmp().clone();
        let temp_dir = tempfile::tempdir_in(lodestone_tmp).context("Failed to create temp dir")?;
        download_file(
            &url,
            temp_dir.path(),
            Some("server.jar"),
            &Box::new(|_| {}),
            true,
        )
        .await?;
        let jar_path = temp_dir.path().join("server.jar");
        crate::util::fs::rename(jar_path, self.path().await.join("server.jar")).await?;

        let new_java_cmd = {
            let mut config = self.config.lock().await;
            config.version = version;
            config.flavour = new_flavour;
            config.jre_major_version = new_jre;
            // Only move to the new JRE if the user did not set a custom Java command.
            let old_default = java_binary(&self.path_to_runtimes, old_jre)
                .to_string_lossy()
                .to_string();
            let uses_default = config
                .java_cmd
                .as_ref()
                .is_none_or(|cmd| *cmd == old_default);
            if uses_default {
                let new_default = java_binary(&self.path_to_runtimes, new_jre)
                    .to_string_lossy()
                    .to_string();
                config.java_cmd = Some(new_default.clone());
                Some(new_default)
            } else {
                None
            }
        };
        if let Some(java_cmd) = new_java_cmd {
            if let Err(e) = self
                .configurable_manifest
                .lock()
                .await
                .update_setting_value(
                    CmdArgSetting::get_section_id(),
                    "java_cmd",
                    ConfigurableValue::String(java_cmd),
                )
            {
                warn!("Failed to show the new Java command in the settings: {e}");
            }
        }
        self.write_config_to_file().await
    }

    async fn configurable_manifest(&self) -> ConfigurableManifest {
        // The file may have been edited by hand, so read it again.
        if let Err(e) = self.refresh_properties_section().await {
            warn!("Failed to read {}: {e}", self.path_to_properties.display());
        }
        self.configurable_manifest.lock().await.clone()
    }

    async fn update_configurable(
        &self,
        section_id: &str,
        setting_id: &str,
        value: ConfigurableValue,
    ) -> Result<(), Error> {
        if section_id == server_properties::SECTION_ID {
            self.set_server_property(setting_id, &value, false).await
        } else if section_id == CmdArgSetting::get_section_id() {
            self.update_cmd_arg(setting_id, value).await
        } else {
            Err(Error {
                kind: ErrorKind::NotFound,
                source: eyre!("Section not found"),
            })
        }
    }
}

fn as_ram(value: &ConfigurableValue) -> Result<u32, Error> {
    let ram = match value {
        ConfigurableValue::UnsignedInteger(n) => *n,
        ConfigurableValue::Integer(n) if *n >= 0 => *n as u32,
        _ => return Err(bad_request("Expected a positive whole number of MB")),
    };
    if ram == 0 {
        return Err(bad_request("RAM must be at least 1 MB"));
    }
    Ok(ram)
}

impl MinecraftInstance {
    /// Updates one setting of the command line section and the instance config.
    async fn update_cmd_arg(
        &self,
        setting_id: &str,
        value: ConfigurableValue,
    ) -> Result<(), Error> {
        {
            // held until the change is applied, so two RAM updates can't pass the check
            let mut config = self.config.lock().await;
            let change = match setting_id {
                "min_ram" | "max_ram" => {
                    let ram = as_ram(&value)?;
                    let (min, max, change) = if setting_id == "min_ram" {
                        (ram, config.max_ram, CmdArgSetting::MinRam(ram))
                    } else {
                        (config.min_ram, ram, CmdArgSetting::MaxRam(ram))
                    };
                    if min > max {
                        return Err(bad_request(format!(
                            "Minimum RAM ({min} MB) cannot be larger than maximum RAM ({max} MB)"
                        )));
                    }
                    change
                }
                // the command line can run any program, so it is part of code execution
                "java_cmd" | "cmd_args" if !self.code_execution.is_enabled() => {
                    return Err(crate::global_settings::code_execution_disabled_error());
                }
                "java_cmd" => {
                    let cmd = value.try_as_string()?.trim().to_string();
                    if cmd.is_empty() {
                        return Err(bad_request("The Java command cannot be empty"));
                    }
                    CmdArgSetting::JavaCmd(cmd)
                }
                "cmd_args" => CmdArgSetting::Args(split_args(value.try_as_string()?)?),
                _ => {
                    return Err(Error {
                        kind: ErrorKind::NotFound,
                        source: eyre!("Setting not found"),
                    })
                }
            };
            let manifest_value = match &change {
                CmdArgSetting::MinRam(ram) | CmdArgSetting::MaxRam(ram) => {
                    ConfigurableValue::UnsignedInteger(*ram)
                }
                CmdArgSetting::JavaCmd(cmd) => ConfigurableValue::String(cmd.clone()),
                CmdArgSetting::Args(args) => ConfigurableValue::String(join_args(args)),
            };
            self.configurable_manifest
                .lock()
                .await
                .update_setting_value(
                    CmdArgSetting::get_section_id(),
                    setting_id,
                    manifest_value,
                )?;
            match change {
                CmdArgSetting::MinRam(ram) => config.min_ram = ram,
                CmdArgSetting::MaxRam(ram) => config.max_ram = ram,
                CmdArgSetting::JavaCmd(cmd) => config.java_cmd = Some(cmd),
                CmdArgSetting::Args(args) => config.cmd_args = args,
            }
        }
        self.write_config_to_file().await
    }
}

#[derive(Debug)]
pub(super) enum CmdArgSetting {
    MinRam(u32),
    MaxRam(u32),
    JavaCmd(String),
    Args(Vec<String>),
}

impl CmdArgSetting {
    pub fn get_section_id() -> &'static str {
        "cmd_args_section"
    }
    pub fn get_identifier(&self) -> &'static str {
        match self {
            CmdArgSetting::MinRam(_) => "min_ram",
            CmdArgSetting::MaxRam(_) => "max_ram",
            CmdArgSetting::JavaCmd(_) => "java_cmd",
            CmdArgSetting::Args(_) => "cmd_args",
        }
    }
    pub fn get_name(&self) -> &'static str {
        match self {
            CmdArgSetting::MinRam(_) => "Minimum RAM",
            CmdArgSetting::MaxRam(_) => "Maximum RAM",
            CmdArgSetting::JavaCmd(_) => "Java command",
            CmdArgSetting::Args(_) => "Command line arguments",
        }
    }
    pub fn get_description(&self) -> &'static str {
        match self {
            CmdArgSetting::MinRam(_) => {
                "The minimum amount of RAM to allocate to the server, in MB"
            }
            CmdArgSetting::MaxRam(_) => {
                "The maximum amount of RAM to allocate to the server, in MB"
            }
            CmdArgSetting::JavaCmd(_) => "The command to use to run the java executable",
            CmdArgSetting::Args(_) => {
                "The command line arguments to pass to Java. Use quotes for arguments that contain spaces"
            }
        }
    }
}

impl From<CmdArgSetting> for SettingManifest {
    fn from(value: CmdArgSetting) -> Self {
        let ram_type = ConfigurableValueType::UnsignedInteger {
            min: Some(1),
            max: None,
        };
        let (setting_value, value_type) = match &value {
            CmdArgSetting::MinRam(ram) | CmdArgSetting::MaxRam(ram) => {
                (ConfigurableValue::UnsignedInteger(*ram), ram_type)
            }
            CmdArgSetting::JavaCmd(java_cmd) => (
                ConfigurableValue::String(java_cmd.to_owned()),
                ConfigurableValueType::String { regex: None },
            ),
            CmdArgSetting::Args(args) => (
                ConfigurableValue::String(join_args(args)),
                ConfigurableValueType::String { regex: None },
            ),
        };
        // a RAM value of 0 from an old config must not panic the type check
        let setting_value = match setting_value {
            ConfigurableValue::UnsignedInteger(0) => ConfigurableValue::UnsignedInteger(1),
            other => other,
        };
        SettingManifest::new_optional_value(
            value.get_identifier().to_owned(),
            value.get_name().to_owned(),
            value.get_description().to_owned(),
            Some(setting_value),
            value_type,
            None,
            false,
            true,
        )
    }
}

#[cfg(test)]
mod test {
    use std::io::BufRead;

    use super::*;
    use crate::implementations::minecraft::properties::PropertiesDocument;

    /// Every key of the older sample file is known and parses to its spec type.
    #[test]
    fn test_sample_server_properties() {
        let text = std::fs::read_to_string("testdata/sample_server.properties")
            .expect("Failed to open server.properties");
        let (doc, warnings) = PropertiesDocument::parse(&text);
        assert!(warnings.is_empty());
        for line in std::io::Cursor::new(&text).lines() {
            let line = line.unwrap();
            let key = line.split('=').next().unwrap();
            assert!(
                server_properties::spec_for(key).is_some(),
                "Unknown property: {line}"
            );
        }
        let settings = server_properties::section_settings(&doc);
        let get = |key: &str| settings[key].get_value().unwrap().clone();
        assert_eq!(
            get("enable-jmx-monitoring"),
            ConfigurableValue::Boolean(false)
        );
        assert_eq!(get("rcon.port"), ConfigurableValue::UnsignedInteger(25575));
        assert_eq!(
            get("resource-pack"),
            ConfigurableValue::String(String::new())
        );
        assert_eq!(
            get("difficulty"),
            ConfigurableValue::Enum("hard".to_string())
        );
        assert_eq!(
            get("level-type"),
            ConfigurableValue::String("minecraft:normal".to_string())
        );
    }

    #[test]
    fn cmd_arg_manifest() {
        let setting: SettingManifest =
            CmdArgSetting::Args(vec!["-Dname=My Server".to_string(), "-Xss4M".to_string()]).into();
        assert_eq!(
            setting.get_value(),
            Some(&ConfigurableValue::String(
                "'-Dname=My Server' -Xss4M".to_string()
            ))
        );
        let setting: SettingManifest = CmdArgSetting::MinRam(0).into();
        assert_eq!(
            setting.get_value(),
            Some(&ConfigurableValue::UnsignedInteger(1))
        );
    }

    #[test]
    fn ram_values() {
        assert_eq!(
            as_ram(&ConfigurableValue::UnsignedInteger(1024)).unwrap(),
            1024
        );
        assert_eq!(as_ram(&ConfigurableValue::Integer(512)).unwrap(), 512);
        assert!(as_ram(&ConfigurableValue::Integer(-1)).is_err());
        assert!(as_ram(&ConfigurableValue::UnsignedInteger(0)).is_err());
        assert!(as_ram(&ConfigurableValue::String("1G".to_string())).is_err());
    }
}
