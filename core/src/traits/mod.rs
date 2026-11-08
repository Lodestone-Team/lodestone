use std::collections::HashSet;

use async_trait::async_trait;

use serde::{Deserialize, Serialize};

use ts_rs::TS;

use self::t_configurable::Game;
use self::t_player::Player;
use self::t_server::State;
use self::{
    t_configurable::TConfigurable, t_macro::TMacro, t_player::TPlayerManagement, t_server::TServer,
};

pub mod t_configurable;
pub mod t_macro;
pub mod t_player;
pub mod t_server;

#[derive(Serialize, Deserialize, Clone, Debug, TS, PartialEq)]
#[ts(export)]
pub struct InstanceInfo {
    pub uuid: InstanceUuid,
    pub name: String,
    pub game_type: Game,
    pub description: String,
    pub version: String,
    pub port: u32,
    pub creation_time: i64,
    pub path: String,
    pub auto_start: bool,
    pub restart_on_crash: bool,
    pub state: State,
    pub player_count: Option<u32>,
    pub max_player_count: Option<u32>,
    pub player_list: Option<HashSet<Player>>,
    /// The icon of the modpack the instance was created from (a Modrinth CDN URL).
    #[serde(default)]
    pub icon_url: Option<String>,
    /// The modpack the instance was created from.
    #[serde(default)]
    pub modpack: Option<ModpackOrigin>,
    /// The Fabric loader version, Forge build or Paper build, when known.
    #[serde(default)]
    pub loader_version: Option<String>,
    /// In MB, for Minecraft instances.
    #[serde(default)]
    pub min_ram: Option<u32>,
    #[serde(default)]
    pub max_ram: Option<u32>,
}

/// The parts of [`InstanceInfo`] that only some instance types have.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InstanceDetails {
    pub icon_url: Option<String>,
    pub modpack: Option<ModpackOrigin>,
    pub loader_version: Option<String>,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
}

impl InstanceInfo {
    /// Sets the fields from `details`.
    pub fn with_details(self, details: InstanceDetails) -> Self {
        InstanceInfo {
            icon_url: details.icon_url,
            modpack: details.modpack,
            loader_version: details.loader_version,
            min_ram: details.min_ram,
            max_ram: details.max_ram,
            ..self
        }
    }
}
use crate::generic::GenericInstance;
use crate::minecraft::MinecraftInstance;
use crate::modpack::ModpackOrigin;
use crate::prelude::GameInstance;
use crate::types::InstanceUuid;
#[async_trait]
#[enum_dispatch::enum_dispatch]
pub trait TInstance: TConfigurable + TMacro + TPlayerManagement + TServer + Clone {
    /// The details only some instance types have. None by default.
    async fn details(&self) -> InstanceDetails {
        InstanceDetails::default()
    }

    async fn get_instance_info(&self) -> InstanceInfo {
        InstanceInfo {
            uuid: self.uuid().await,
            name: self.name().await,
            game_type: self.game_type().await,
            description: self.description().await,
            version: self.version().await,
            port: self.port().await,
            creation_time: self.creation_time().await,
            path: self.path().await.display().to_string(),
            auto_start: self.auto_start().await,
            restart_on_crash: self.restart_on_crash().await,
            state: self.state().await,
            player_count: self.get_player_count().await.ok(),
            max_player_count: self.get_max_player_count().await.ok(),
            player_list: self.get_player_list().await.ok(),
            icon_url: None,
            modpack: None,
            loader_version: None,
            min_ram: None,
            max_ram: None,
        }
        .with_details(self.details().await)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traits::t_configurable::MinecraftVariant;

    fn plain_info() -> InstanceInfo {
        InstanceInfo {
            uuid: InstanceUuid::default(),
            name: "Pack server".into(),
            game_type: Game::MinecraftJava {
                variant: MinecraftVariant::Fabric,
            },
            description: String::new(),
            version: "1.20.1".into(),
            port: 25565,
            creation_time: 0,
            path: "/x".into(),
            auto_start: false,
            restart_on_crash: false,
            state: State::Stopped,
            player_count: None,
            max_player_count: None,
            player_list: None,
            icon_url: None,
            modpack: None,
            loader_version: None,
            min_ram: None,
            max_ram: None,
        }
    }

    #[test]
    fn serializes_the_new_fields() {
        let origin = ModpackOrigin {
            project_id: "p".into(),
            version_id: "v".into(),
            name: "Pack".into(),
            version_number: None,
            icon_url: Some("https://cdn.modrinth.com/i.png".into()),
        };
        let info = plain_info().with_details(InstanceDetails {
            icon_url: origin.icon_url.clone(),
            modpack: Some(origin),
            loader_version: Some("0.15.11".into()),
            min_ram: Some(1024),
            max_ram: Some(4096),
        });
        let json = serde_json::to_value(&info).unwrap();
        assert_eq!(json["icon_url"], "https://cdn.modrinth.com/i.png");
        assert_eq!(json["modpack"]["name"], "Pack");
        assert_eq!(json["modpack"]["project_id"], "p");
        assert_eq!(json["loader_version"], "0.15.11");
        assert_eq!(json["min_ram"], 1024);
        assert_eq!(json["max_ram"], 4096);

        // without details they are null
        let json = serde_json::to_value(plain_info()).unwrap();
        for field in [
            "icon_url",
            "modpack",
            "loader_version",
            "min_ram",
            "max_ram",
        ] {
            assert!(json[field].is_null(), "{field}");
        }
    }

    #[test]
    fn reads_info_without_the_new_fields() {
        let original = plain_info();
        let mut json = serde_json::to_value(&original).unwrap();
        for field in [
            "icon_url",
            "modpack",
            "loader_version",
            "min_ram",
            "max_ram",
        ] {
            json.as_object_mut().unwrap().remove(field);
        }
        let read: InstanceInfo = serde_json::from_value(json).unwrap();
        assert_eq!(read, original);
    }
}
