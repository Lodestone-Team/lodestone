//! The settings an image carries, typed and by scope.
//!
//! | Scope | Fields |
//! | --- | --- |
//! | shared | the image's `game` (Minecraft version, loader, loader version) |
//! | server | [`ServerSettings`]: port, `server.properties` values, ops, whitelist, flavour |
//! | client | [`ClientSettings`]: recommended memory, resource pack order, shader, icon, background |
//! | local | Java path, Rosetta, JVM and game arguments, window, play time |
//!
//! Local settings have no field here at all, and fields this version doesn't know are
//! dropped when a manifest is read, so a Java path or JVM arguments can never be written
//! into an image, whoever made the manifest. Each view keeps only its side's settings.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::hex::is_sha256;
use crate::strings::open_enum;
use crate::Side;

/// `server.properties` keys that hold secrets. They are never put in an image's settings
/// ([`ServerSettings::from_properties`] drops them, and validation refuses them).
pub const SECRET_PROPERTIES: [&str; 2] = ["rcon.password", "management-server-secret"];
/// An icon or background is at most this large (256 MiB): anything bigger isn't one.
pub const MAX_STORED_FILE: u64 = 256 << 20;

/// Every setting an image may carry, by side. A `null` anywhere in the settings reads as
/// the field's default, as everywhere in a manifest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client: Option<ClientSettings>,
}

impl Settings {
    /// Only `side`'s settings, for that side's view.
    pub fn for_side(&self, side: Side) -> Settings {
        match side {
            Side::Server => Settings {
                server: self.server.clone(),
                client: None,
            },
            Side::Client => Settings {
                server: None,
                client: self.client.clone(),
            },
        }
    }

    /// Sorts the lists whose order means nothing (ops, whitelist), so equal settings
    /// serialize alike. The resource pack order means something and is kept. A side whose
    /// settings are all empty becomes no settings for that side, so "none recorded" has one
    /// spelling and gives one id.
    pub fn normalize(&mut self) {
        if let Some(server) = &mut self.server {
            server.ops.sort();
            server.ops.dedup();
            server.whitelist.sort();
            server.whitelist.dedup();
        }
        if self.server.as_ref() == Some(&ServerSettings::default()) {
            self.server = None;
        }
        if self.client.as_ref() == Some(&ClientSettings::default()) {
            self.client = None;
        }
    }

    /// Problems a manifest from elsewhere might have: secrets in the properties, and stored
    /// files (icon, background) that aren't named by a SHA-256 or are impossibly large.
    /// Those are store keys that garbage collection and exports act on.
    pub fn validate(&self) -> Result<(), String> {
        if let Some(server) = &self.server {
            if let Some(key) = server
                .properties
                .keys()
                .find(|key| SECRET_PROPERTIES.contains(&key.as_str()))
            {
                return Err(format!("settings.server.properties has the secret {key:?}"));
            }
        }
        if let Some(client) = &self.client {
            for (name, stored) in [("icon", &client.icon), ("background", &client.background)] {
                let Some(stored) = stored else { continue };
                if !is_sha256(&stored.sha256) {
                    return Err(format!(
                        "settings.client.{name}.sha256 isn't 64 lowercase hex characters"
                    ));
                }
                if stored.size > MAX_STORED_FILE {
                    return Err(format!(
                        "settings.client.{name} is larger than {MAX_STORED_FILE} bytes"
                    ));
                }
            }
        }
        Ok(())
    }
}

open_enum! {
    /// The server software, from the core's flavours. It decides the world layout.
    pub enum ServerFlavour {
        Vanilla => "vanilla",
        Fabric => "fabric",
        Quilt => "quilt",
        Forge => "forge",
        NeoForge => "neoforge",
        Paper => "paper",
        Spigot => "spigot",
        Bukkit => "bukkit",
        Purpur => "purpur",
        Folia => "folia",
    }
}

impl ServerFlavour {
    /// Whether this server splits its level the Paper way (`<level>_nether`,
    /// `<level>_the_end`); see [`crate::layout`].
    pub fn splits_worlds(&self) -> bool {
        matches!(
            self,
            ServerFlavour::Paper
                | ServerFlavour::Spigot
                | ServerFlavour::Bukkit
                | ServerFlavour::Purpur
                | ServerFlavour::Folia
        )
    }
}

/// Settings only a server uses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    /// `server.properties` values, without [`SECRET_PROPERTIES`].
    #[serde(default, deserialize_with = "crate::manifest::setting_map")]
    pub properties: BTreeMap<String, String>,
    #[serde(default, deserialize_with = "crate::manifest::setting_list")]
    pub ops: Vec<Op>,
    #[serde(default, deserialize_with = "crate::manifest::setting_list")]
    pub whitelist: Vec<Player>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flavour: Option<ServerFlavour>,
}

impl ServerSettings {
    /// The properties of a `server.properties` file, without its secrets.
    pub fn from_properties(
        properties: impl IntoIterator<Item = (String, String)>,
    ) -> BTreeMap<String, String> {
        properties
            .into_iter()
            .filter(|(key, _)| !SECRET_PROPERTIES.contains(&key.as_str()))
            .collect()
    }
}

/// A player on the whitelist (`whitelist.json`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Player {
    pub uuid: String,
    pub name: String,
}

/// An operator (`ops.json`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Op {
    pub uuid: String,
    pub name: String,
    pub level: u8,
    #[serde(default, deserialize_with = "crate::manifest::null_as_default")]
    pub bypasses_player_limit: bool,
}

/// Settings only a client uses.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientSettings {
    /// The memory the pack recommends, in MiB. A recommendation: the player's own memory
    /// setting is local and never comes from an image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recommended_memory_mb: Option<u32>,
    /// Resource packs in use, in order (first is lowest), as Minecraft writes them in
    /// `options.txt` (`file/<name>`, `vanilla`…).
    #[serde(default, deserialize_with = "crate::manifest::setting_list")]
    pub resource_packs: Vec<String>,
    /// The shader pack in use, by file name in `shaderpacks/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shader: Option<String>,
    /// The instance's icon, stored as an object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<StoredFile>,
    /// The Play page's background, stored as an object.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<StoredFile>,
}

/// A file kept as an object in the store but not placed in the game folder (an icon).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredFile {
    pub sha256: String,
    pub size: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_fields_are_never_representable() {
        // A manifest that tries to carry local settings: they are dropped on reading and
        // never written back.
        let json = r#"{
            "server": {"port": 25565, "java_path": "/usr/bin/java", "jvm_args": ["-Xmx8G"]},
            "client": {"recommended_memory_mb": 4096, "java_path": "C:\\java.exe",
                       "rosetta": true, "window": {"width": 800}, "play_time": 99,
                       "game_args": ["--demo"]},
            "local": {"java_path": "/x"}
        }"#;
        let settings: Settings = serde_json::from_str(json).unwrap();
        let written = serde_json::to_string(&settings).unwrap();
        for local in [
            "java",
            "jvm",
            "rosetta",
            "window",
            "play_time",
            "game_args",
            "local",
        ] {
            assert!(!written.contains(local), "{local} in {written}");
        }
        assert_eq!(settings.server.as_ref().unwrap().port, Some(25565));
        assert_eq!(
            settings.client.as_ref().unwrap().recommended_memory_mb,
            Some(4096)
        );
    }

    #[test]
    fn for_side_keeps_one_side() {
        let settings = Settings {
            server: Some(ServerSettings {
                port: Some(25565),
                ..Default::default()
            }),
            client: Some(ClientSettings {
                shader: Some("bsl.zip".into()),
                ..Default::default()
            }),
        };
        let server = settings.for_side(Side::Server);
        assert!(server.client.is_none());
        assert_eq!(server.server, settings.server);
        let client = settings.for_side(Side::Client);
        assert!(client.server.is_none());
        assert_eq!(client.client, settings.client);
    }

    #[test]
    fn secrets_are_dropped_and_refused() {
        let properties = ServerSettings::from_properties([
            ("motd".to_string(), "hi".to_string()),
            ("rcon.password".to_string(), "hunter2".to_string()),
        ]);
        assert_eq!(properties.len(), 1);
        assert!(properties.contains_key("motd"));

        let mut settings = Settings::default();
        let mut server = ServerSettings::default();
        server.properties.insert("rcon.password".into(), "x".into());
        settings.server = Some(server);
        let error = settings.validate().unwrap_err();
        assert!(error.contains("rcon.password"));
        assert!(!error.contains("\"x\""));
    }

    #[test]
    fn normalize_sorts_sets_but_not_resource_packs() {
        let mut settings = Settings {
            server: Some(ServerSettings {
                whitelist: vec![
                    Player {
                        uuid: "b".into(),
                        name: "B".into(),
                    },
                    Player {
                        uuid: "a".into(),
                        name: "A".into(),
                    },
                    Player {
                        uuid: "a".into(),
                        name: "A".into(),
                    },
                ],
                ..Default::default()
            }),
            client: Some(ClientSettings {
                resource_packs: vec!["file/z.zip".into(), "vanilla".into()],
                ..Default::default()
            }),
        };
        settings.normalize();
        let server = settings.server.as_ref().unwrap();
        assert_eq!(server.whitelist.len(), 2);
        assert_eq!(server.whitelist[0].uuid, "a");
        assert_eq!(
            settings.client.as_ref().unwrap().resource_packs,
            ["file/z.zip", "vanilla"]
        );
    }

    #[test]
    fn stored_files_must_be_store_keys() {
        let with_icon = |sha256: &str, size: u64| Settings {
            server: None,
            client: Some(ClientSettings {
                icon: Some(StoredFile {
                    sha256: sha256.into(),
                    size,
                }),
                ..Default::default()
            }),
        };
        assert_eq!(with_icon(&"a".repeat(64), 10).validate(), Ok(()));
        for (sha256, size) in [
            ("../../etc", 1),
            (&*"A".repeat(64), 1),
            (&*"a".repeat(63), 1),
            (&*"a".repeat(64), MAX_STORED_FILE + 1),
        ] {
            let error = with_icon(sha256, size).validate().unwrap_err();
            assert!(error.contains("icon"), "{error}");
        }
        let mut background = Settings::default();
        background.client = Some(ClientSettings {
            background: Some(StoredFile {
                sha256: "x".into(),
                size: 1,
            }),
            ..Default::default()
        });
        assert!(background.validate().unwrap_err().contains("background"));
    }

    #[test]
    fn empty_sides_are_no_settings() {
        let mut settings = Settings {
            server: Some(ServerSettings::default()),
            client: Some(ClientSettings::default()),
        };
        settings.normalize();
        assert_eq!(settings, Settings::default());
        let mut kept = Settings {
            server: Some(ServerSettings {
                port: Some(25565),
                ..Default::default()
            }),
            client: None,
        };
        kept.normalize();
        assert!(kept.server.is_some());
    }

    #[test]
    fn flavours() {
        assert!(ServerFlavour::Paper.splits_worlds());
        assert!(ServerFlavour::parse("purpur").splits_worlds());
        assert!(!ServerFlavour::Fabric.splits_worlds());
        assert!(!ServerFlavour::parse("someday").splits_worlds());
        assert_eq!(
            serde_json::to_string(&ServerFlavour::NeoForge).unwrap(),
            r#""neoforge""#
        );
    }
}
