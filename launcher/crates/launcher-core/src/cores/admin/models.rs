//! The core's types as the dashboard's bindings (`dashboard/src/bindings`) describe
//! them, for what more than one area uses. An area's own types live in its file.
//!
//! Every type tolerates what a newer core adds: unknown fields are ignored, and enums
//! have an `Unknown` (or `Other`) variant where the core may grow new values.

use serde::{Deserialize, Serialize};

use super::super::models::{CoreProduct, LaunchedBy};

/// An instance's state (`InstanceState`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum InstanceState {
    Starting,
    Running,
    Stopping,
    #[default]
    Stopped,
    Error,
    #[serde(other)]
    Unknown,
}

impl InstanceState {
    /// Whether the server process is up (or coming up, or going down).
    pub fn is_live(self) -> bool {
        matches!(
            self,
            InstanceState::Starting | InstanceState::Running | InstanceState::Stopping
        )
    }
}

/// The Minecraft server software (`MinecraftVariant`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MinecraftVariant {
    Vanilla,
    Forge,
    NeoForge,
    Fabric,
    Paper,
    Spigot,
    Other {
        name: String,
    },
    #[serde(other)]
    Unknown,
}

/// What an instance runs (`Game`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Game {
    MinecraftJava {
        variant: MinecraftVariant,
    },
    MinecraftBedrock,
    /// A Docker container managed as an instance. An older core's atom (scripted)
    /// instances read as this too, and show as any non-Minecraft server.
    Generic {
        game_name: String,
        game_display_name: String,
    },
    #[serde(other)]
    Unknown,
}

impl Game {
    /// The game's name as the dashboard shows it: "Fabric (Minecraft)", "Minecraft".
    pub fn title(&self) -> String {
        match self {
            Game::MinecraftJava { variant } => match variant {
                MinecraftVariant::Vanilla => "Minecraft".into(),
                MinecraftVariant::Forge => "Forge (Minecraft)".into(),
                MinecraftVariant::NeoForge => "NeoForge (Minecraft)".into(),
                MinecraftVariant::Fabric => "Fabric (Minecraft)".into(),
                MinecraftVariant::Paper => "Paper (Minecraft)".into(),
                MinecraftVariant::Spigot => "Spigot (Minecraft)".into(),
                MinecraftVariant::Other { name } => format!("{name} (Minecraft)"),
                MinecraftVariant::Unknown => "Minecraft".into(),
            },
            Game::MinecraftBedrock => "Minecraft Bedrock".into(),
            Game::Generic {
                game_display_name, ..
            } => game_display_name.clone(),
            Game::Unknown => "Unknown game".into(),
        }
    }
}

/// A player on a server (`Player`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Player {
    MinecraftPlayer {
        name: String,
        #[serde(default)]
        uuid: Option<String>,
    },
    /// Anything else, such as the `GenericPlayer` of an older core's atom instance.
    #[serde(other)]
    Unknown,
}

impl Player {
    pub fn name(&self) -> &str {
        match self {
            Player::MinecraftPlayer { name, .. } => name,
            Player::Unknown => "",
        }
    }
}

/// Where an instance made from a modpack came from (`ModpackOrigin`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModpackOrigin {
    /// Empty for an uploaded pack.
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub version_id: String,
    pub name: String,
    #[serde(default)]
    pub version_number: Option<String>,
    /// Only ever a `https://cdn.modrinth.com/` URL.
    #[serde(default)]
    pub icon_url: Option<String>,
}

/// An instance as `GET /instance/list` describes it (`InstanceInfo`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceInfo {
    pub uuid: String,
    pub name: String,
    pub game_type: Game,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub version: String,
    pub port: u32,
    /// Unix seconds.
    #[serde(default)]
    pub creation_time: i64,
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub auto_start: bool,
    #[serde(default)]
    pub restart_on_crash: bool,
    pub state: InstanceState,
    #[serde(default)]
    pub player_count: Option<u32>,
    #[serde(default)]
    pub max_player_count: Option<u32>,
    #[serde(default)]
    pub player_list: Option<Vec<Player>>,
    /// The modpack's icon (a Modrinth CDN URL).
    #[serde(default)]
    pub icon_url: Option<String>,
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

impl InstanceInfo {
    /// A Docker container managed as an instance (its id starts `DOCKER-`).
    pub fn is_docker(&self) -> bool {
        self.uuid.starts_with("DOCKER-")
    }
}

/// What a user may do (`UserPermission`). The per-instance lists hold instance ids.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UserPermission {
    pub can_view_instance: Vec<String>,
    pub can_start_instance: Vec<String>,
    pub can_stop_instance: Vec<String>,
    pub can_access_instance_console: Vec<String>,
    pub can_access_instance_setting: Vec<String>,
    pub can_read_instance_resource: Vec<String>,
    pub can_write_instance_resource: Vec<String>,
    pub can_access_instance_macro: Vec<String>,
    pub can_read_instance_file: Vec<String>,
    pub can_write_instance_file: Vec<String>,
    pub can_create_instance: bool,
    pub can_delete_instance: bool,
    pub can_read_global_file: bool,
    pub can_write_global_file: bool,
    pub can_manage_permission: bool,
    pub can_install_extension: bool,
}

/// A permission that applies to some instances.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstancePermission {
    View,
    Start,
    Stop,
    Console,
    Settings,
    ReadResource,
    WriteResource,
    ReadFiles,
    WriteFiles,
}

/// A permission that applies to the whole core.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CorePermission {
    CreateInstance,
    DeleteInstance,
    ReadGlobalFile,
    WriteGlobalFile,
    ManagePermission,
}

/// A user as the core shows them (`PublicUser`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicUser {
    pub uid: String,
    pub username: String,
    #[serde(default)]
    pub is_owner: bool,
    #[serde(default)]
    pub is_admin: bool,
    #[serde(default)]
    pub permissions: UserPermission,
    /// Whether they can sign in with a password. Only Lodestone Host's owner has none,
    /// until they set one to share the core. Older cores don't say: everyone has one.
    #[serde(default = "yes")]
    pub has_password: bool,
}

fn yes() -> bool {
    true
}

impl PublicUser {
    /// Whether the user may do `permission` with `instance`, as the dashboard decides
    /// (`isUserAuthorized`): the owner may do everything. The core checks again.
    pub fn may(&self, permission: InstancePermission, instance: &str) -> bool {
        if self.is_owner {
            return true;
        }
        let p = &self.permissions;
        let list = match permission {
            InstancePermission::View => &p.can_view_instance,
            InstancePermission::Start => &p.can_start_instance,
            InstancePermission::Stop => &p.can_stop_instance,
            InstancePermission::Console => &p.can_access_instance_console,
            InstancePermission::Settings => &p.can_access_instance_setting,
            InstancePermission::ReadResource => &p.can_read_instance_resource,
            InstancePermission::WriteResource => &p.can_write_instance_resource,
            InstancePermission::ReadFiles => &p.can_read_instance_file,
            InstancePermission::WriteFiles => &p.can_write_instance_file,
        };
        list.iter().any(|id| id == instance)
    }

    /// Whether the user may do `permission` on the core. The owner may do everything.
    pub fn may_core(&self, permission: CorePermission) -> bool {
        if self.is_owner {
            return true;
        }
        let p = &self.permissions;
        match permission {
            CorePermission::CreateInstance => p.can_create_instance,
            CorePermission::DeleteInstance => p.can_delete_instance,
            CorePermission::ReadGlobalFile => p.can_read_global_file,
            CorePermission::WriteGlobalFile => p.can_write_global_file,
            CorePermission::ManagePermission => p.can_manage_permission,
        }
    }
}

/// The core's settings (`GlobalSettingsData`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalSettingsData {
    pub core_name: String,
    pub safe_mode: bool,
    pub domain: Option<String>,
    pub code_execution_enabled: bool,
    pub code_execution_locked: bool,
    pub allow_lan_http: bool,
    pub list_instances_publicly: bool,
    pub allow_local_claim: bool,
    /// Whether other devices may use the core ("Allow access from other devices"). Off
    /// for Lodestone Host's core until its owner sets up a password to share it. Older
    /// cores don't say: they always allow it.
    #[serde(default = "yes")]
    pub allow_remote_access: bool,
    /// Whether the core looks for newer versions of servers' modpacks in the background.
    /// Older cores don't have the check.
    #[serde(default = "yes")]
    pub check_modpack_updates: bool,
}

/// Where the core's HTTPS certificate comes from (`TlsMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TlsMode {
    SelfSigned,
    UserProvided,
    #[default]
    None,
    #[serde(other)]
    Unknown,
}

/// The core's HTTPS state (`TlsInfo`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TlsInfo {
    pub mode: TlsMode,
    pub fingerprint_sha256: Option<String>,
}

/// The core's optional features (`CoreCapabilities`). A core that doesn't say has
/// them all. The core still sends `macros` and `generic_instances`, now always false;
/// nothing reads them since macros and atom instances were removed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CoreCapabilities {
    pub docker: bool,
}

impl Default for CoreCapabilities {
    fn default() -> Self {
        Self { docker: true }
    }
}

/// Everything `GET /info` says (the dashboard's `CoreInfo`). Needs no sign-in.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct FullCoreInfo {
    pub version: String,
    pub is_setup: bool,
    pub os: String,
    pub arch: String,
    pub cpu: String,
    pub cpu_count: u32,
    pub total_ram: u64,
    pub total_disk: u64,
    pub host_name: String,
    pub uuid: String,
    pub core_name: String,
    /// Unix seconds.
    pub up_since: i64,
    pub tls: TlsInfo,
    pub capabilities: CoreCapabilities,
    pub product: CoreProduct,
    pub launched_by: LaunchedBy,
    /// Whether other devices may use the core. Older cores don't say: they always allow
    /// it.
    #[serde(default = "yes")]
    pub remote_access: bool,
}

/// `POST /user/login` and `POST /setup/{key}` (`LoginReply`). No `Debug`: it holds
/// the token.
#[derive(Clone, Deserialize)]
pub struct LoginReply {
    pub token: String,
    pub user: PublicUser,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Older cores don't say whether other devices may use them, or who has a password:
    /// they always allow it, and everyone has one.
    #[test]
    fn older_cores_allow_remote_access() {
        let info: FullCoreInfo =
            serde_json::from_value(serde_json::json!({ "uuid": "c" })).unwrap();
        assert!(info.remote_access);
        let settings: GlobalSettingsData =
            serde_json::from_value(serde_json::json!({ "core_name": "Home" })).unwrap();
        assert!(settings.allow_remote_access);
        let user: PublicUser =
            serde_json::from_value(serde_json::json!({ "uid": "1", "username": "u" })).unwrap();
        assert!(user.has_password);
        let local: FullCoreInfo =
            serde_json::from_value(serde_json::json!({ "remote_access": false })).unwrap();
        assert!(!local.remote_access);
        let owner: PublicUser = serde_json::from_value(
            serde_json::json!({ "uid": "1", "username": "u", "has_password": false }),
        )
        .unwrap();
        assert!(!owner.has_password);
    }

    #[test]
    fn reads_an_instance_from_the_list() {
        let json = serde_json::json!({
            "uuid": "abc",
            "name": "SMP",
            "game_type": { "type": "MinecraftJava", "variant": { "type": "Fabric" } },
            "description": "",
            "version": "1.20.1",
            "port": 25565,
            "creation_time": 1_700_000_000,
            "path": "/x",
            "auto_start": false,
            "restart_on_crash": true,
            "state": "Running",
            "player_count": 1,
            "max_player_count": 20,
            "player_list": [{ "type": "MinecraftPlayer", "name": "Steve", "uuid": null }],
            "icon_url": null,
            "modpack": null,
            "loader_version": "0.16.14",
            "min_ram": 1024,
            "max_ram": 4096,
            "something_new": true,
        });
        let info: InstanceInfo = serde_json::from_value(json).unwrap();
        assert_eq!(info.state, InstanceState::Running);
        assert_eq!(info.game_type.title(), "Fabric (Minecraft)");
        assert_eq!(info.player_list.as_ref().unwrap()[0].name(), "Steve");
    }

    /// An older core may still list an atom (scripted) instance: it reads as a generic,
    /// non-Minecraft one.
    #[test]
    fn reads_an_older_cores_atom_instance() {
        let info: InstanceInfo = serde_json::from_value(serde_json::json!({
            "uuid": "atom-1", "name": "Bot", "port": 0, "state": "Stopped",
            "game_type": { "type": "Generic", "game_name": "bot", "game_display_name": "Bot" },
        }))
        .unwrap();
        assert!(matches!(info.game_type, Game::Generic { .. }));
        assert!(!info.is_docker());
        // its players, which only atom instances had, don't fail the list
        let players: Vec<Player> = serde_json::from_value(serde_json::json!([
            { "type": "GenericPlayer", "id": "p1", "name": "Bot" },
            { "type": "MinecraftPlayer", "name": "Steve", "uuid": null },
        ]))
        .unwrap();
        assert_eq!(players[0], Player::Unknown);
        assert_eq!(players[1].name(), "Steve");
    }

    #[test]
    fn unknown_values_dont_fail() {
        let state: InstanceState = serde_json::from_str("\"Hibernating\"").unwrap();
        assert_eq!(state, InstanceState::Unknown);
        let game: Game = serde_json::from_str(r#"{"type":"Terraria"}"#).unwrap();
        assert_eq!(game, Game::Unknown);
        let info: FullCoreInfo = serde_json::from_str(r#"{"uuid":"u","version":"0.5.1"}"#).unwrap();
        assert!(
            info.capabilities.docker,
            "an older core has every capability"
        );
        // a core's capabilities with the macro and atom fields it still sends
        let info: FullCoreInfo = serde_json::from_str(
            r#"{"capabilities":{"macros":false,"generic_instances":false,"docker":false}}"#,
        )
        .unwrap();
        assert!(!info.capabilities.docker);
        // settings from a core before Lodestone Connect, with its playit.gg switch
        let settings: GlobalSettingsData = serde_json::from_str(
            r#"{"core_name":"Home","safe_mode":true,"domain":null,"playit_enabled":true,
                "code_execution_enabled":true,"code_execution_locked":false,
                "allow_lan_http":false,"list_instances_publicly":true,"allow_local_claim":true}"#,
        )
        .unwrap();
        assert_eq!(settings.core_name, "Home");
        assert!(settings.list_instances_publicly);
    }

    #[test]
    fn the_owner_may_do_everything() {
        let mut user = PublicUser {
            uid: "1".into(),
            username: "u".into(),
            is_owner: false,
            is_admin: false,
            permissions: UserPermission {
                can_start_instance: vec!["a".into()],
                ..Default::default()
            },
            has_password: true,
        };
        assert!(user.may(InstancePermission::Start, "a"));
        assert!(!user.may(InstancePermission::Start, "b"));
        assert!(!user.may_core(CorePermission::CreateInstance));
        user.is_owner = true;
        assert!(user.may(InstancePermission::Stop, "b"));
        assert!(user.may_core(CorePermission::CreateInstance));
    }
}
