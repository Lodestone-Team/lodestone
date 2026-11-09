//! An instance's Settings tab: work package C (see `launcher/docs/host-parity.md`).
//!
//! - `GET /instance/{uuid}/settings -> ConfigurableManifest`
//! - `PUT /instance/{uuid}/settings/{section}/{setting}, body: ConfigurableValue`
//! - `PUT /instance/{uuid}/auto_start`, `PUT /instance/{uuid}/restart_on_crash` (JSON bool)
//! - (the public listing's routes are in [`super::core_settings`]: Core › General
//!   shows them too)
//! - `GET/PUT /instance/{uuid}/resource_pack`, `POST .../resource_pack/hash`
//! - `GET/PUT /instance/{uuid}/eula` (the Minecraft EULA: only ever accepted by the
//!   user's own click)
//! - `DELETE /instance/{uuid}` (the danger zone)

use std::{fmt, marker::PhantomData};

use http_client::Method;
use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor},
};

use super::{AdminClient, segment};
use crate::cores::CoreError;

/// A setting's value (`ConfigurableValue`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value")]
pub enum ConfigurableValue {
    String(String),
    Integer(i64),
    UnsignedInteger(u64),
    Float(f64),
    Boolean(bool),
    Enum(String),
}

impl ConfigurableValue {
    /// The value as a text field shows it.
    pub fn to_text(&self) -> String {
        match self {
            ConfigurableValue::String(text) | ConfigurableValue::Enum(text) => text.clone(),
            ConfigurableValue::Integer(n) => n.to_string(),
            ConfigurableValue::UnsignedInteger(n) => n.to_string(),
            ConfigurableValue::Float(n) => n.to_string(),
            ConfigurableValue::Boolean(b) => b.to_string(),
        }
    }
}

/// What kind of value a setting holds, and its limits (`ConfigurableValueType`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ConfigurableValueType {
    String {
        #[serde(default)]
        regex: Option<String>,
    },
    Integer {
        #[serde(default)]
        min: Option<i64>,
        #[serde(default)]
        max: Option<i64>,
    },
    UnsignedInteger {
        #[serde(default)]
        min: Option<u64>,
        #[serde(default)]
        max: Option<u64>,
    },
    Float {
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
    },
    Boolean,
    Enum {
        options: Vec<String>,
    },
    #[serde(other)]
    Unknown,
}

/// One setting of an instance (`SettingManifest`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingManifest {
    pub setting_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub value: Option<ConfigurableValue>,
    pub value_type: ConfigurableValueType,
    #[serde(default)]
    pub default_value: Option<ConfigurableValue>,
    #[serde(default)]
    pub is_secret: bool,
    #[serde(default)]
    pub is_required: bool,
    #[serde(default = "yes")]
    pub is_mutable: bool,
}

fn yes() -> bool {
    true
}

/// A section of an instance's settings (`SectionManifest`), its settings in the
/// core's order.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SectionManifest {
    pub section_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, deserialize_with = "ordered")]
    pub settings: Vec<(String, SettingManifest)>,
}

/// An instance's settings (`ConfigurableManifest`), its sections in the core's order.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct ConfigurableManifest {
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub restart_on_crash: bool,
    #[serde(default, deserialize_with = "ordered")]
    pub setting_sections: Vec<(String, SectionManifest)>,
}

impl ConfigurableManifest {
    /// Sets a setting's value, after it was saved.
    pub fn set(&mut self, section: &str, setting: &str, value: ConfigurableValue) {
        if let Some((_, section)) = self.setting_sections.iter_mut().find(|(id, _)| id == section)
            && let Some((_, setting)) = section.settings.iter_mut().find(|(id, _)| id == setting)
        {
            setting.value = Some(value);
        }
    }
}

/// A JSON object as its entries, in the order the core wrote them (the core keeps its
/// settings in an `IndexMap`).
fn ordered<'de, D, T>(deserializer: D) -> Result<Vec<(String, T)>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct Entries<T>(PhantomData<T>);
    impl<'de, T: Deserialize<'de>> Visitor<'de> for Entries<T> {
        type Value = Vec<(String, T)>;
        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("an object")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0));
            while let Some(entry) = map.next_entry()? {
                entries.push(entry);
            }
            Ok(entries)
        }
    }
    deserializer.deserialize_map(Entries(PhantomData))
}

/// The server resource pack lines of `server.properties` (`ServerResourcePack`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ServerResourcePack {
    /// Empty for none.
    pub url: String,
    /// Lowercase hex, or empty.
    pub sha1: String,
    pub required: bool,
    pub prompt: String,
}

/// `POST .../resource_pack/hash` (`ResourcePackHash`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct ResourcePackHash {
    pub sha1: String,
    /// Bytes downloaded.
    #[serde(default)]
    pub size: u64,
}

/// Whether the user accepted the Minecraft EULA for an instance (`EulaStatus`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct EulaStatus {
    pub accepted: bool,
}

/// Where the Minecraft EULA is: shown beside every place the user can accept it.
pub const MINECRAFT_EULA_URL: &str = "https://aka.ms/MinecraftEULA";

/// Why `url` can't be a resource pack URL, or `None` when it can (or is empty): the
/// core's rules (`check_resource_pack`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PackUrlError {
    Spaces,
    NotAUrl,
    Scheme,
}

pub fn pack_url_error(url: &str) -> Option<PackUrlError> {
    let url = url.trim();
    if url.is_empty() {
        return None;
    }
    if url.chars().any(char::is_whitespace) {
        return Some(PackUrlError::Spaces);
    }
    match url::Url::parse(url) {
        Err(_) => Some(PackUrlError::NotAUrl),
        Ok(parsed) if parsed.host_str().is_none() => Some(PackUrlError::NotAUrl),
        Ok(parsed) if !matches!(parsed.scheme(), "http" | "https") => Some(PackUrlError::Scheme),
        Ok(_) => None,
    }
}

/// Whether `sha1` is empty or 40 hexadecimal characters.
pub fn is_sha1(sha1: &str) -> bool {
    let sha1 = sha1.trim();
    sha1.is_empty() || (sha1.len() == 40 && sha1.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Settings that choose the program an instance runs, or its arguments: changing them
/// needs the owner (or global file writes with safe mode off), and code execution on.
pub const COMMAND_LINE_SETTINGS: [&str; 2] = ["java_cmd", "cmd_args"];

impl AdminClient {
    /// `GET /instance/{uuid}/settings`.
    pub async fn instance_settings(&self, uuid: &str) -> Result<ConfigurableManifest, CoreError> {
        self.get(&format!("instance/{}/settings", segment(uuid)))
            .await
    }

    /// `PUT /instance/{uuid}/settings/{section}/{setting}`.
    pub async fn set_instance_setting(
        &self,
        uuid: &str,
        section: &str,
        setting: &str,
        value: &ConfigurableValue,
    ) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!(
                "instance/{}/settings/{}/{}",
                segment(uuid),
                segment(section),
                segment(setting)
            ),
            Some(value),
        )
        .await
    }

    /// `PUT /instance/{uuid}/auto_start`.
    pub async fn set_auto_start(&self, uuid: &str, on: bool) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!("instance/{}/auto_start", segment(uuid)),
            Some(on),
        )
        .await
    }

    /// `PUT /instance/{uuid}/restart_on_crash`.
    pub async fn set_restart_on_crash(&self, uuid: &str, on: bool) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!("instance/{}/restart_on_crash", segment(uuid)),
            Some(on),
        )
        .await
    }

    /// `GET /instance/{uuid}/resource_pack`.
    pub async fn resource_pack(&self, uuid: &str) -> Result<ServerResourcePack, CoreError> {
        self.get(&format!("instance/{}/resource_pack", segment(uuid)))
            .await
    }

    /// `PUT /instance/{uuid}/resource_pack`; the pack as written. An empty URL removes it.
    pub async fn set_resource_pack(
        &self,
        uuid: &str,
        pack: &ServerResourcePack,
    ) -> Result<ServerResourcePack, CoreError> {
        self.json(
            Method::PUT,
            &format!("instance/{}/resource_pack", segment(uuid)),
            Some(pack),
        )
        .await
    }

    /// `POST /instance/{uuid}/resource_pack/hash`: the core downloads `url` and hashes it.
    pub async fn hash_resource_pack(
        &self,
        uuid: &str,
        url: &str,
    ) -> Result<ResourcePackHash, CoreError> {
        self.json(
            Method::POST,
            &format!("instance/{}/resource_pack/hash", segment(uuid)),
            Some(serde_json::json!({ "url": url })),
        )
        .await
    }

    /// `GET /instance/{uuid}/eula`.
    pub async fn eula(&self, uuid: &str) -> Result<EulaStatus, CoreError> {
        self.get(&format!("instance/{}/eula", segment(uuid))).await
    }

    /// `PUT /instance/{uuid}/eula`: the user agrees to the Minecraft EULA. Call it only
    /// from the user's own click on an "I agree" button that links to the EULA.
    pub async fn accept_eula(&self, uuid: &str) -> Result<EulaStatus, CoreError> {
        self.json(
            Method::PUT,
            &format!("instance/{}/eula", segment(uuid)),
            None::<()>,
        )
        .await
    }

    /// `DELETE /instance/{uuid}`: deletes the instance and its files. It leaves the
    /// list through an `InstanceDelete` progression event.
    pub async fn delete_instance(&self, uuid: &str) -> Result<(), CoreError> {
        self.call(
            Method::DELETE,
            &format!("instance/{}", segment(uuid)),
            None::<()>,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_manifest_in_the_cores_order() {
        let json = r#"{
            "auto_start": true, "restart_on_crash": false,
            "setting_sections": {
                "zeta": { "section_id": "zeta", "name": "Z", "description": "", "settings": {
                    "port": { "setting_id": "port", "name": "Port", "description": "",
                        "value": { "type": "UnsignedInteger", "value": 25565 },
                        "value_type": { "type": "UnsignedInteger", "min": 0, "max": 65535 },
                        "default_value": null, "is_secret": false, "is_required": true, "is_mutable": true },
                    "difficulty": { "setting_id": "difficulty", "name": "Difficulty", "description": "",
                        "value": { "type": "Enum", "value": "easy" },
                        "value_type": { "type": "Enum", "options": ["peaceful", "easy"] },
                        "default_value": null, "is_secret": false, "is_required": true, "is_mutable": true }
                } },
                "alpha": { "section_id": "alpha", "name": "A", "description": "", "settings": {} }
            }
        }"#;
        let mut manifest: ConfigurableManifest = serde_json::from_str(json).unwrap();
        let ids: Vec<_> = manifest.setting_sections.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(ids, ["zeta", "alpha"]);
        let settings = &manifest.setting_sections[0].1.settings;
        assert_eq!(settings[0].0, "port");
        assert_eq!(
            settings[0].1.value_type,
            ConfigurableValueType::UnsignedInteger {
                min: Some(0),
                max: Some(65535)
            }
        );
        manifest.set("zeta", "difficulty", ConfigurableValue::Enum("peaceful".into()));
        assert_eq!(
            manifest.setting_sections[0].1.settings[1].1.value,
            Some(ConfigurableValue::Enum("peaceful".into()))
        );
    }

    #[test]
    fn values_are_tagged_as_the_core_expects() {
        assert_eq!(
            serde_json::to_string(&ConfigurableValue::Boolean(true)).unwrap(),
            r#"{"type":"Boolean","value":true}"#
        );
    }

    #[test]
    fn pack_checks_follow_the_core() {
        assert_eq!(pack_url_error(""), None);
        assert_eq!(pack_url_error("https://x.com/p.zip"), None);
        assert_eq!(pack_url_error("https://x.com/a b"), Some(PackUrlError::Spaces));
        assert_eq!(pack_url_error("ftp://x.com/p"), Some(PackUrlError::Scheme));
        assert_eq!(pack_url_error("nope"), Some(PackUrlError::NotAUrl));
        assert!(is_sha1(""));
        assert!(is_sha1(&"a".repeat(40)));
        assert!(!is_sha1("abc"));
    }
}
