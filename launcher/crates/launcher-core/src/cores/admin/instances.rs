//! Instances: the list, power, creating them (every game the core offers, and
//! Modrinth modpacks), renaming, and what the Overview shows (sharing with friends,
//! linked players). Work package A (see `launcher/docs/host-parity.md`).

use std::time::Duration;

use http_client::Method;
use serde::{Deserialize, Deserializer, Serialize, de::DeserializeOwned};

use super::{AdminClient, models::InstanceInfo, segment};
use crate::cores::{CoreError, client::MAX_JSON_BYTES};

/// What the power menu does to an instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    Start,
    Stop,
    Restart,
    Kill,
}

impl Power {
    fn route(self) -> &'static str {
        match self {
            Power::Start => "start",
            Power::Stop => "stop",
            Power::Restart => "restart",
            Power::Kill => "kill",
        }
    }
}

impl AdminClient {
    /// `GET /instance/list`: the instances the user may see, in the core's order.
    pub async fn instances(&self) -> Result<Vec<InstanceInfo>, CoreError> {
        self.get("instance/list").await
    }

    /// `PUT /instance/{uuid}/{start|stop|restart|kill}`. The new state arrives as a
    /// `StateTransition` event.
    pub async fn power(&self, uuid: &str, power: Power) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!("instance/{}/{}", segment(uuid), power.route()),
            None::<()>,
        )
        .await
    }
}

// ---- Creating instances (§2.3) ----

/// A game the core can make servers for (`HandlerGameType`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HandlerGameType {
    MinecraftJavaVanilla,
    MinecraftFabric,
    MinecraftForge,
    MinecraftNeoForge,
    MinecraftPaper,
    MinecraftBedrock,
    /// A game a newer core offers. Not shown.
    #[serde(other)]
    Unknown,
}

impl HandlerGameType {
    /// The name in the core's routes.
    pub fn route(&self) -> Option<&'static str> {
        Some(match self {
            HandlerGameType::MinecraftJavaVanilla => "MinecraftJavaVanilla",
            HandlerGameType::MinecraftFabric => "MinecraftFabric",
            HandlerGameType::MinecraftForge => "MinecraftForge",
            HandlerGameType::MinecraftNeoForge => "MinecraftNeoForge",
            HandlerGameType::MinecraftPaper => "MinecraftPaper",
            HandlerGameType::MinecraftBedrock => "MinecraftBedrock",
            HandlerGameType::Unknown => return None,
        })
    }

    /// The game an instance of this type runs, for its title and icon.
    pub fn game(&self) -> super::models::Game {
        use super::models::{Game, MinecraftVariant};
        let java = |variant| Game::MinecraftJava { variant };
        match self {
            HandlerGameType::MinecraftJavaVanilla => java(MinecraftVariant::Vanilla),
            HandlerGameType::MinecraftFabric => java(MinecraftVariant::Fabric),
            HandlerGameType::MinecraftForge => java(MinecraftVariant::Forge),
            HandlerGameType::MinecraftNeoForge => java(MinecraftVariant::NeoForge),
            HandlerGameType::MinecraftPaper => java(MinecraftVariant::Paper),
            HandlerGameType::MinecraftBedrock => Game::MinecraftBedrock,
            HandlerGameType::Unknown => Game::Unknown,
        }
    }
}

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

/// What a setting may hold (`ConfigurableValueType`).
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
        #[serde(default)]
        options: Vec<String>,
    },
    /// A kind of setting a newer core has. Shown as unavailable.
    #[serde(other)]
    Unknown,
}

/// One setting of a manifest (`SettingManifest`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SettingManifest {
    pub setting_id: String,
    #[serde(default)]
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

/// A section of settings (`SectionManifest`), in the core's order.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SectionManifest {
    pub section_id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(deserialize_with = "ordered")]
    pub settings: Vec<SettingManifest>,
}

/// The form for a new instance (`SetupManifest`), sections in the core's order.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct SetupManifest {
    #[serde(deserialize_with = "ordered")]
    pub setting_sections: Vec<SectionManifest>,
}

/// A JSON object's values in the order the core sent them (the launcher's
/// `serde_json` keeps it).
pub(super) fn ordered<'de, D: Deserializer<'de>, T: DeserializeOwned>(
    deserializer: D,
) -> Result<Vec<T>, D::Error> {
    let map = serde_json::Map::<String, serde_json::Value>::deserialize(deserializer)?;
    map.into_iter()
        .map(|(_, value)| serde_json::from_value(value).map_err(serde::de::Error::custom))
        .collect()
}

/// What `POST /instance/create/{game}` takes (`SetupValue`). `setting_sections` is
/// `{section: {settings: {setting: {value}}}}`.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SetupValue {
    pub name: String,
    pub description: Option<String>,
    pub auto_start: bool,
    pub restart_on_crash: bool,
    pub setting_sections: serde_json::Map<String, serde_json::Value>,
    /// The user agreed to the Minecraft EULA. Only ever `true` after the user ticked
    /// the box themselves.
    pub accept_eula: bool,
}

impl SetupValue {
    /// The settings' values for `manifest`, from `value_of(section, setting)`.
    pub fn sections(
        manifest: &SetupManifest,
        value_of: impl Fn(&SectionManifest, &SettingManifest) -> Option<ConfigurableValue>,
    ) -> serde_json::Map<String, serde_json::Value> {
        let mut sections = serde_json::Map::new();
        for section in &manifest.setting_sections {
            let mut settings = serde_json::Map::new();
            for setting in &section.settings {
                settings.insert(
                    setting.setting_id.clone(),
                    serde_json::json!({ "value": value_of(section, setting) }),
                );
            }
            sections.insert(
                section.section_id.clone(),
                serde_json::json!({ "settings": settings }),
            );
        }
        sections
    }
}

/// Where a modpack comes from (`ModpackSource`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ModpackSource {
    Modrinth { version_id: String },
    Upload { upload_id: String },
}

/// `POST /instance/create_modpack` (`CreateModpackInstanceRequest`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct CreateModpackRequest {
    pub source: ModpackSource,
    pub name: String,
    pub port: u32,
    pub description: String,
    pub min_ram: u32,
    pub max_ram: u32,
    pub auto_start: bool,
    pub restart_on_crash: bool,
    /// Only ever `true` after the user ticked the box themselves.
    pub accept_eula: bool,
}

impl AdminClient {
    /// `GET /games`: the games the core makes servers for, in its order.
    pub async fn games(&self) -> Result<Vec<HandlerGameType>, CoreError> {
        self.get("games").await
    }

    /// `GET /setup_manifest/{game}`: the "Server settings" form for a game.
    pub async fn setup_manifest(&self, game: &HandlerGameType) -> Result<SetupManifest, CoreError> {
        let route = game.route().ok_or_else(|| {
            CoreError::Invalid("This core offers a game Lodestone doesn't know".into())
        })?;
        self.get(&format!("setup_manifest/{route}")).await
    }

    /// `POST /instance/create/{game}`. The core sets the instance up in the background:
    /// progress and the new instance arrive as progression events.
    pub async fn create_instance(
        &self,
        game: &HandlerGameType,
        value: &SetupValue,
    ) -> Result<(), CoreError> {
        let route = game.route().ok_or_else(|| {
            CoreError::Invalid("This core offers a game Lodestone doesn't know".into())
        })?;
        self.call(
            Method::POST,
            &format!("instance/create/{route}"),
            Some(value),
        )
        .await
    }

    /// `POST /instance/create_modpack`: the new instance's id. It is set up in the
    /// background, as above.
    pub async fn create_modpack_instance(
        &self,
        request: &CreateModpackRequest,
    ) -> Result<String, CoreError> {
        self.json(Method::POST, "instance/create_modpack", Some(request))
            .await
    }

    /// `PUT /instance/{uuid}/name`.
    pub async fn set_instance_name(&self, uuid: &str, name: &str) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!("instance/{}/name", segment(uuid)),
            Some(name),
        )
        .await
    }

    /// `PUT /instance/{uuid}/description`.
    pub async fn set_instance_description(
        &self,
        uuid: &str,
        description: &str,
    ) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!("instance/{}/description", segment(uuid)),
            Some(description),
        )
        .await
    }
}

// ---- The Overview (§3) ----

/// Sharing an instance with friends through Lodestone Connect (`ShareState`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareState {
    Starting,
    Sharing,
    Reconnecting,
    #[serde(other)]
    Unknown,
}

/// How a friend is connected (`ShareConnection`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShareConnection {
    SameComputer,
    LocalNetwork,
    Ipv6,
    Internet,
    #[serde(other)]
    Unknown,
}

/// A friend connected to a shared instance (`SharedFriend`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SharedFriend {
    pub id: u32,
    pub address: String,
    pub connection: ShareConnection,
    /// Unix seconds.
    #[serde(default)]
    pub connected_at: i64,
    #[serde(default)]
    pub rtt_ms: u32,
}

/// `GET /instance/{uuid}/share` (`InstanceShare`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceShare {
    pub sharing: bool,
    pub state: Option<ShareState>,
    /// `https://connect.lodestone.cc/j/...`, once known.
    pub link: Option<String>,
    pub stable_link: bool,
    pub joins: u32,
    pub friends: Vec<SharedFriend>,
    pub problem: Option<String>,
    pub hard_network: bool,
}

/// One CPU and memory sample of `/monitor/{uuid}` (`PerformanceReport`), every second.
#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct PerformanceReport {
    /// Bytes.
    pub memory_usage: Option<u64>,
    /// Percent of one core's worth, as the core measures it.
    pub cpu_usage: Option<f32>,
    /// Unix seconds.
    pub start_time: Option<u64>,
}

impl PerformanceReport {
    /// A frame of the monitor stream.
    pub fn parse(frame: &str) -> Option<Self> {
        serde_json::from_str(frame).ok()
    }
}

/// A Minecraft account linked to a Lodestone user, for the player list.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct LinkedPlayer {
    pub minecraft_uuid: String,
    pub minecraft_name: String,
    /// The Lodestone user. Empty in `/user/minecraft_links` (the user's own).
    #[serde(default)]
    pub username: String,
}

impl AdminClient {
    /// `GET /instance/{uuid}/share`. Fails for users who can't change the instance's
    /// settings, and on cores without sharing.
    pub async fn share(&self, uuid: &str) -> Result<InstanceShare, CoreError> {
        self.get(&format!("instance/{}/share", segment(uuid))).await
    }

    /// `POST /instance/{uuid}/share`: answers once the link is known (a few seconds).
    pub async fn start_share(&self, uuid: &str) -> Result<InstanceShare, CoreError> {
        let path = format!("instance/{}/share", segment(uuid));
        let bytes = self
            .send_slow(Method::POST, &path, None, MAX_JSON_BYTES, SHARE_TIMEOUT)
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// `DELETE /instance/{uuid}/share`. The link stops working for good.
    pub async fn stop_share(&self, uuid: &str) -> Result<InstanceShare, CoreError> {
        self.json(
            Method::DELETE,
            &format!("instance/{}/share", segment(uuid)),
            None::<()>,
        )
        .await
    }

    /// The Lodestone users of linked Minecraft accounts: every link for the owner
    /// (`GET /minecraft_links`), the user's own for others (`GET /user/minecraft_links`,
    /// with `me` as their name). Empty on a core without links.
    pub async fn linked_players(
        &self,
        is_owner: bool,
        me: &str,
    ) -> Result<Vec<LinkedPlayer>, CoreError> {
        let path = if is_owner {
            "minecraft_links"
        } else {
            "user/minecraft_links"
        };
        match self.get::<Vec<LinkedPlayer>>(path).await {
            Ok(mut links) => {
                if !is_owner {
                    for link in &mut links {
                        link.username = me.to_string();
                    }
                }
                Ok(links)
            }
            Err(CoreError::NotFound(_)) => Ok(Vec::new()),
            Err(err) => Err(err),
        }
    }
}

/// How long starting to share may take: the core waits for the link.
const SHARE_TIMEOUT: Duration = Duration::from_secs(60);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_setup_manifest_in_order() {
        let json = serde_json::json!({
            "setting_sections": {
                "zeta": {
                    "section_id": "zeta",
                    "name": "Z",
                    "description": "",
                    "settings": {
                        "version": {
                            "setting_id": "version",
                            "name": "Version",
                            "description": "",
                            "value": null,
                            "value_type": { "type": "Enum", "options": ["1.21", "1.20.1"] },
                            "default_value": { "type": "Enum", "value": "1.21" },
                            "is_secret": false,
                            "is_required": true,
                            "is_mutable": true
                        },
                        "port": {
                            "setting_id": "port",
                            "name": "Port",
                            "description": "",
                            "value": { "type": "UnsignedInteger", "value": 25565 },
                            "value_type": { "type": "UnsignedInteger", "min": 0, "max": 65535 },
                            "default_value": null,
                            "is_secret": false,
                            "is_required": true,
                            "is_mutable": true
                        }
                    }
                },
                "alpha": { "section_id": "alpha", "name": "A", "description": "", "settings": {} }
            }
        });
        let manifest: SetupManifest = serde_json::from_value(json).unwrap();
        let ids: Vec<_> = manifest
            .setting_sections
            .iter()
            .map(|s| s.section_id.as_str())
            .collect();
        assert_eq!(ids, ["zeta", "alpha"], "the core's order");
        let settings = &manifest.setting_sections[0].settings;
        assert_eq!(settings[0].setting_id, "version");
        assert_eq!(
            settings[1].value,
            Some(ConfigurableValue::UnsignedInteger(25565))
        );
        let sections = SetupValue::sections(&manifest, |_, setting| setting.value.clone());
        assert_eq!(
            serde_json::Value::Object(sections),
            serde_json::json!({
                "zeta": { "settings": {
                    "version": { "value": null },
                    "port": { "value": { "type": "UnsignedInteger", "value": 25565 } },
                } },
                "alpha": { "settings": {} },
            })
        );
    }

    #[test]
    fn unknown_values_dont_fail() {
        let games: Vec<HandlerGameType> =
            serde_json::from_str(r#"["MinecraftFabric","Terraria"]"#).unwrap();
        assert_eq!(games[1], HandlerGameType::Unknown);
        assert!(games[1].route().is_none());
        let kind: ConfigurableValueType = serde_json::from_str(r#"{"type":"Colour"}"#).unwrap();
        assert_eq!(kind, ConfigurableValueType::Unknown);
    }

    #[test]
    fn reads_a_share() {
        let share: InstanceShare = serde_json::from_value(serde_json::json!({
            "sharing": true,
            "state": "sharing",
            "link": "https://connect.lodestone.cc/j/ABCDEFGHJK#aa.bb",
            "stable_link": true,
            "joins": 3,
            "friends": [{ "id": 1, "address": "1.2.3.4", "connection": "internet",
                          "connected_at": 1700000000, "rtt_ms": 42 }],
            "problem": null,
            "hard_network": false,
        }))
        .unwrap();
        assert_eq!(share.state, Some(ShareState::Sharing));
        assert_eq!(share.friends[0].connection, ShareConnection::Internet);
        let off: InstanceShare = serde_json::from_str(r#"{"sharing":false}"#).unwrap();
        assert!(!off.sharing && off.link.is_none());
    }

    #[test]
    fn writes_requests_as_the_core_reads_them() {
        let request = CreateModpackRequest {
            source: ModpackSource::Upload {
                upload_id: "u1".into(),
            },
            name: "Pack".into(),
            port: 25565,
            description: String::new(),
            min_ram: 1024,
            max_ram: 4096,
            auto_start: false,
            restart_on_crash: false,
            accept_eula: false,
        };
        let json = serde_json::to_value(&request).unwrap();
        assert_eq!(
            json["source"],
            serde_json::json!({"type":"upload","upload_id":"u1"})
        );
        assert_eq!(
            serde_json::to_value(ConfigurableValue::Boolean(true)).unwrap(),
            serde_json::json!({"type":"Boolean","value":true})
        );
        let report = PerformanceReport::parse(
            r#"{"memory_usage":1073741824,"disk_usage":null,"cpu_usage":12.5,"start_time":1}"#,
        )
        .unwrap();
        assert_eq!(report.cpu_usage, Some(12.5));
    }
}
