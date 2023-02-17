use std::collections::BTreeMap;
use std::collections::HashMap;
pub use std::path::PathBuf;

use async_trait::async_trait;
use color_eyre::eyre::eyre;
pub use serde::{Deserialize, Serialize};
pub use serde_json;
use ts_rs::TS;

use crate::error::Error;
use crate::error::ErrorKind;
use crate::traits::GameInstance;
use crate::traits::MinecraftInstance;
use crate::types::InstanceUuid;

#[derive(Debug, Clone, Serialize, Deserialize, TS, PartialEq)]
pub enum ConfigurableSettingValue {
    String(String),
    Integer(i32),
    UnsignedInteger(u32),
    Float(f32),
    Boolean(bool),
    Enum(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConfigurableSettingValueType {
    String(Option<String>), // regex
    Integer { min: Option<i32>, max: Option<i32> },
    UnsignedInteger { min: Option<u32>, max: Option<u32> },
    Float { min: Option<f32>, max: Option<f32> },
    Boolean,
    Enum { options: Vec<String> },
}

impl ToString for ConfigurableSettingValueType {
    fn to_string(&self) -> String {
        match self {
            ConfigurableSettingValueType::String(_) => "string".to_string(),
            ConfigurableSettingValueType::Integer { .. } => "integer".to_string(),
            ConfigurableSettingValueType::UnsignedInteger { .. } => "unsigned integer".to_string(),
            ConfigurableSettingValueType::Float { .. } => "float".to_string(),
            ConfigurableSettingValueType::Boolean => "boolean".to_string(),
            ConfigurableSettingValueType::Enum { .. } => "enum".to_string(),
        }
    }
}

impl ConfigurableSettingValueType {
    pub fn type_check(&self, value: &Option<ConfigurableSettingValue>) -> Result<(), Error> {
        if let Some(value) = value {
            match (self, value) {
                (
                    ConfigurableSettingValueType::String(regex),
                    ConfigurableSettingValue::String(value),
                ) => {
                    if let Some(regex) = regex {
                        if let Ok(regex) = fancy_regex::Regex::new(regex) {
                            if let Ok(true) = regex.is_match(value) {
                                Ok(())
                            } else {
                                Err(Error {
                                    kind: ErrorKind::BadRequest,
                                    source: eyre!("Value does not match regex"),
                                })
                            }
                        } else {
                            Err(Error {
                                kind: ErrorKind::BadRequest,
                                source: eyre!("Invalid regex"),
                            })
                        }
                    } else {
                        Ok(())
                    }
                }
                (
                    ConfigurableSettingValueType::Integer { min, max },
                    ConfigurableSettingValue::Integer(value),
                ) => {
                    if let Some(min) = min {
                        if value < min {
                            return Err(Error {
                                kind: ErrorKind::BadRequest,
                                source: eyre!("Value is too small"),
                            });
                        }
                    }
                    if let Some(max) = max {
                        if value > max {
                            return Err(Error {
                                kind: ErrorKind::BadRequest,
                                source: eyre!("Value is too large"),
                            });
                        }
                    }
                    Ok(())
                }
                (
                    ConfigurableSettingValueType::UnsignedInteger { min, max },
                    ConfigurableSettingValue::UnsignedInteger(value),
                ) => {
                    if let Some(min) = min {
                        if value < min {
                            return Err(Error {
                                kind: ErrorKind::BadRequest,
                                source: eyre!("Value is too small"),
                            });
                        }
                    }
                    if let Some(max) = max {
                        if value > max {
                            return Err(Error {
                                kind: ErrorKind::BadRequest,
                                source: eyre!("Value is too large"),
                            });
                        }
                    }
                    Ok(())
                }
                (
                    ConfigurableSettingValueType::Float { min, max },
                    ConfigurableSettingValue::Float(value),
                ) => {
                    if let Some(min) = min {
                        if value < min {
                            return Err(Error {
                                kind: ErrorKind::BadRequest,
                                source: eyre!("Value is too small"),
                            });
                        }
                    }
                    if let Some(max) = max {
                        if value > max {
                            return Err(Error {
                                kind: ErrorKind::BadRequest,
                                source: eyre!("Value is too large"),
                            });
                        }
                    }
                    Ok(())
                }
                (ConfigurableSettingValueType::Boolean, ConfigurableSettingValue::Boolean(_)) => {
                    Ok(())
                }
                (
                    ConfigurableSettingValueType::Enum { options },
                    ConfigurableSettingValue::Enum(value),
                ) => {
                    if options.contains(value) {
                        Ok(())
                    } else {
                        Err(Error {
                            kind: ErrorKind::BadRequest,
                            source: eyre!("Value is not in enum"),
                        })
                    }
                }
                _ => Err(Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!("Type mismatch"),
                }),
            }
        } else {
            Ok(())
        }
    }
}

impl ToString for ConfigurableSettingValue {
    fn to_string(&self) -> String {
        match self {
            ConfigurableSettingValue::String(value) => value.to_string(),
            ConfigurableSettingValue::Integer(value) => value.to_string(),
            ConfigurableSettingValue::UnsignedInteger(value) => value.to_string(),
            ConfigurableSettingValue::Float(value) => value.to_string(),
            ConfigurableSettingValue::Boolean(value) => value.to_string(),
            ConfigurableSettingValue::Enum(value) => value.to_string(),
        }
    }
}

impl ConfigurableSettingValue {
    pub fn infer_type(&self) -> ConfigurableSettingValueType {
        match self {
            ConfigurableSettingValue::String(_) => ConfigurableSettingValueType::String(None),
            ConfigurableSettingValue::Integer(_) => ConfigurableSettingValueType::Integer {
                min: None,
                max: None,
            },
            ConfigurableSettingValue::UnsignedInteger(_) => {
                ConfigurableSettingValueType::UnsignedInteger {
                    min: None,
                    max: None,
                }
            }
            ConfigurableSettingValue::Float(_) => ConfigurableSettingValueType::Float {
                min: None,
                max: None,
            },
            ConfigurableSettingValue::Boolean(_) => ConfigurableSettingValueType::Boolean,
            ConfigurableSettingValue::Enum(_) => {
                ConfigurableSettingValueType::Enum { options: vec![] }
            }
        }
    }

    pub fn try_as_integer(&self) -> Result<i32, Error> {
        match self {
            ConfigurableSettingValue::Integer(value) => Ok(*value),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Expected integer, found {}", self.infer_type().to_string()),
            }),
        }
    }

    pub fn try_as_unsigned_integer(&self) -> Result<u32, Error> {
        match self {
            ConfigurableSettingValue::UnsignedInteger(value) => Ok(*value),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!(
                    "Expected unsigned integer, found {}",
                    self.infer_type().to_string()
                ),
            }),
        }
    }

    pub fn try_as_float(&self) -> Result<f32, Error> {
        match self {
            ConfigurableSettingValue::Float(value) => Ok(*value),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Expected float, found {}", self.infer_type().to_string()),
            }),
        }
    }

    pub fn try_as_enum(&self) -> Result<&String, Error> {
        match self {
            ConfigurableSettingValue::Enum(value) => Ok(value),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Expected enum, found {}", self.infer_type().to_string()),
            }),
        }
    }

    pub fn try_as_boolean(&self) -> Result<bool, Error> {
        match self {
            ConfigurableSettingValue::Boolean(b) => Ok(*b),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Expected boolean, found {}", self.infer_type().to_string()),
            }),
        }
    }

    pub fn try_as_string(&self) -> Result<&String, Error> {
        match self {
            ConfigurableSettingValue::String(s) => Ok(s),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Expected string, found {}", self.infer_type().to_string()),
            }),
        }
    }
}

// A ConfigurableSetting contains a unique identifier, a name and a description
// and a value
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigurableSetting {
    setting_id: String, // static, cannot change at runtime
    name: String,
    description: String,
    value: Option<ConfigurableSettingValue>,
    value_type: ConfigurableSettingValueType,
    default_value: Option<ConfigurableSettingValue>, // static, cannot change at runtime
    is_secret: bool,                                 // ??
    is_required: bool,                               // ??
    is_mutable: bool,                                // CAN change at runtime
}

impl ConfigurableSetting {
    pub fn get_value(&self) -> Option<&ConfigurableSettingValue> {
        self.value.as_ref()
    }
    pub fn get_identifier(&self) -> &String {
        &self.setting_id
    }
    /// # WARNING
    /// Will infer the type of the value from the value itself
    ///
    /// A number will be unbounded
    ///
    /// A string will have no regex
    ///
    /// An enum will have no options
    pub fn new_required_value(
        setting_id: String,
        name: String,
        description: String,
        value: ConfigurableSettingValue,
        default_value: Option<ConfigurableSettingValue>,
        is_secret: bool,
        is_mutable: bool,
    ) -> Self {
        Self {
            setting_id,
            name,
            description,
            value: Some(value.clone()),
            value_type: value.infer_type(),
            default_value,
            is_secret,
            is_required: true,
            is_mutable,
        }
    }
    pub fn new_optional_value(
        setting_id: String,
        name: String,
        description: String,
        value: Option<ConfigurableSettingValue>,
        value_type: ConfigurableSettingValueType,
        default_value: Option<ConfigurableSettingValue>,
        is_secret: bool,
        is_mutable: bool,
    ) -> Self {
        Self {
            setting_id,
            name,
            description,
            value,
            value_type,
            default_value,
            is_secret,
            is_required: false,
            is_mutable,
        }
    }

    pub fn new_value_with_type(
        setting_id: String,
        name: String,
        description: String,
        value: Option<ConfigurableSettingValue>,
        value_type: ConfigurableSettingValueType,
        default_value: Option<ConfigurableSettingValue>,
        is_secret: bool,
        is_mutable: bool,
    ) -> Result<Self, Error> {
        value_type.type_check(&value)?;
        Ok(Self {
            setting_id,
            name,
            description,
            is_required: value.is_some(),
            value,
            value_type,
            default_value,
            is_secret,
            is_mutable,
        })
    }

    fn set_value_type_safe(&mut self, value: ConfigurableSettingValue) -> Result<(), Error> {
        self.value_type
            .type_check(&Some(value.clone()))
            .map_err(|e| Error {
                kind: ErrorKind::BadRequest,
                source: eyre!(e),
            })
            .map(|_| {
                self.value = Some(value);
            })
    }

    pub fn set_value(&mut self, value: ConfigurableSettingValue) -> Result<(), Error> {
        if self.is_mutable {
            self.set_value_type_safe(value)
        } else {
            Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Setting is not mutable"),
            })
        }
    }

    pub fn set_optional_value(
        &mut self,
        value: Option<ConfigurableSettingValue>,
    ) -> Result<(), Error> {
        if self.is_mutable {
            if value.is_none() && self.is_required {
                Err(Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!("Setting is required"),
                })
            } else {
                self.value = value;
                Ok(())
            }
        } else {
            Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Setting is not mutable"),
            })
        }
    }
}

// A Setting section contains a name and a description (for UI)
// A Setting section contains a list of InstanceSetting
pub struct ConfigurableSection {
    section_id: String,
    name: String,
    description: String,
    settings: BTreeMap<String, ConfigurableSetting>,
}

impl ConfigurableSection {
    pub fn new(
        section_id: String,
        name: String,
        description: String,
        settings: BTreeMap<String, ConfigurableSetting>,
    ) -> Self {
        Self {
            section_id,
            name,
            description,
            settings,
        }
    }

    pub fn get_setting(&self, setting_id: &str) -> Option<&ConfigurableSetting> {
        self.settings.get(setting_id)
    }

    pub fn add_setting(&mut self, setting: ConfigurableSetting) -> Result<(), Error> {
        if self.settings.contains_key(setting.get_identifier()) {
            Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Setting already exists"),
            })
        } else {
            self.settings
                .insert(setting.get_identifier().clone(), setting);
            Ok(())
        }
    }

}

// A setting manifest indicates if the instance has implemented functionalities for smart, lodestone controlled feature
// A setting manifest has an ordered list of Setting Section
pub struct ConfigurableManifest {
    auto_start: bool,
    restart_on_crash: bool,
    start_on_connection: bool,
    timeout_last_left: bool,

    setting_sections: BTreeMap<String, ConfigurableSection>,
}

impl ConfigurableManifest {
    pub fn get_setting(
        &self,
        section_id: &str,
        setting_id: &str,
    ) -> Option<&ConfigurableSetting> {
        if let Some(section) = self.setting_sections.get(section_id) {
            section.settings.get(setting_id)
        } else {
            None
        }
    }

    fn get_setting_mut(
        &mut self,
        section_id: &str,
        setting_id: &str,
    ) -> Option<&mut ConfigurableSetting> {
        if let Some(section) = self.setting_sections.get_mut(section_id) {
            section.settings.get_mut(setting_id)
        } else {
            None
        }
    }

    pub fn get_section(&self, section_id: &str) -> Option<&ConfigurableSection> {
        self.setting_sections.get(section_id)
    }

    pub fn set_setting(
        &mut self,
        section_id: &str,
        setting_id: &str,
        value: Option<ConfigurableSettingValue>,
    ) -> Result<(), Error> {
        if let Some(setting) = self.get_setting_mut(section_id, setting_id) {
            setting.set_optional_value(value)
        } else {
            Err(Error {
                kind: ErrorKind::NotFound,
                source: eyre!("Setting not found"),
            })
        }
    }

    pub fn set_setting_mut(
        &mut self,
        section_id: &str,
        setting_id: &str,
        is_mutable: bool,
    ) -> Option<bool> {
        if let Some(setting) = self.get_setting_mut(section_id, setting_id) {
            let ret = setting.is_mutable;
            setting.is_mutable = is_mutable;
            Some(ret)
        } else {
            None
        }
    }
}

#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TConfigurable {
    // getters
    async fn uuid(&self) -> InstanceUuid;
    async fn name(&self) -> String;
    async fn flavour(&self) -> String;
    async fn game_type(&self) -> String;
    async fn cmd_args(&self) -> Vec<String>;
    async fn description(&self) -> String;
    async fn port(&self) -> u32;
    async fn min_ram(&self) -> Result<u32, Error>;
    async fn max_ram(&self) -> Result<u32, Error>;
    async fn creation_time(&self) -> i64;
    async fn path(&self) -> PathBuf;
    /// does start when lodestone starts
    async fn auto_start(&self) -> bool;
    async fn restart_on_crash(&self) -> bool;
    async fn backup_period(&self) -> Result<Option<u32>, Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support backup period"),
        })
    }
    // setters
    async fn set_name(&mut self, name: String) -> Result<(), Error>;
    async fn set_description(&mut self, description: String) -> Result<(), Error>;
    async fn set_port(&mut self, _port: u32) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting port"),
        })
    }
    async fn set_cmd_args(&mut self, _cmd_args: Vec<String>) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting cmd args"),
        })
    }
    async fn set_min_ram(&mut self, _min_ram: u32) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting ram"),
        })
    }
    async fn set_max_ram(&mut self, _max_ram: u32) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting ram"),
        })
    }
    async fn set_auto_start(&mut self, _auto_start: bool) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting auto start"),
        })
    }
    async fn set_restart_on_crash(&mut self, _restart_on_crash: bool) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting restart on crash"),
        })
    }
    async fn set_backup_period(&mut self, _backup_period: Option<u32>) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support setting backup period"),
        })
    }

    // server config files (server.properties)
    async fn set_field(&mut self, field: &str, value: String) -> Result<(), Error>;
    async fn get_field(&self, field: &str) -> Result<String, Error>;

    async fn change_version(&mut self, _version: String) -> Result<(), Error> {
        Err(Error {
            kind: ErrorKind::UnsupportedOperation,
            source: eyre!("This instance does not support changing version"),
        })
    }

    async fn settings(&self) -> Result<HashMap<String, String>, Error>;

    async fn get_configurable_manifest(&self) -> ConfigurableManifest;
}
