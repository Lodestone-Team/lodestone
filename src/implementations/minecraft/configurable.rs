use std::str::FromStr;
use std::{collections::HashMap, sync::atomic};

use async_trait::async_trait;
use axum::Server;
use color_eyre::eyre::{eyre, Context, ContextCompat};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;
use tempdir::TempDir;

use crate::error::{Error, ErrorKind};
use crate::traits::t_configurable::{ConfigurableManifest, TConfigurable};
use crate::traits::t_server::State;

use crate::types::InstanceUuid;
use crate::util::download_file;

use super::util::{get_fabric_jar_url, get_paper_jar_url, get_vanilla_jar_url};
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
        let (url, _) = match self.config.flavour {
            super::Flavour::Vanilla => get_vanilla_jar_url(&version).await.ok_or_else(|| {
                let error_msg =
                    format!("Cannot get the vanilla jar version for version {}", version);
                Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!(error_msg),
                }
            })?,
            super::Flavour::Fabric {..} => get_fabric_jar_url(&version, &None, &None).await.ok_or_else(|| {
                let error_msg =
                    format!("Cannot get the fabric jar version for version {}", version);
                Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!(error_msg),
                }
            })?,
            super::Flavour::Paper {..} => get_paper_jar_url(&version, &None).await.ok_or_else(|| {
                let error_msg =
                    format!("Cannot get the paper jar version for version {}", version);
                Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!(error_msg),
                }
            })?,
            super::Flavour::Spigot => todo!(),
            super::Flavour::Forge {..} => return Err(
                Error {
                    kind: ErrorKind::UnsupportedOperation,
                    source: eyre!("Changing versions is unsupported for forge servers"),
                }
            ),
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

    async fn get_configurable_manifest(&self) -> ConfigurableManifest {
        todo!()
    }
}

enum InstanceSetting {
    CmdArg(CmdArgSetting),
    ServerProperty(ServerPropertySetting),
}

impl InstanceSetting {
    pub fn get_identifier(&self) -> String {
        match self {
            InstanceSetting::CmdArg(setting) => setting.get_identifier().to_owned(),
            InstanceSetting::ServerProperty(setting) => setting.get_identifier(),
        }
    }
    pub fn get_name(&self) -> String {
        match self {
            InstanceSetting::CmdArg(setting) => setting.get_name().to_owned(),
            InstanceSetting::ServerProperty(setting) => setting.get_name(),
        }
    }
    pub fn get_description(&self) -> String {
        match self {
            InstanceSetting::CmdArg(setting) => setting.get_description().to_owned(),
            InstanceSetting::ServerProperty(setting) => setting.get_description(),
        }
    }
}

enum CmdArgSetting {
    MinRam(u32),
    MaxRam(u32),
    JavaCmd(String),
    Args(Vec<String>),
}

impl CmdArgSetting {
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
                "The minimum amount of RAM to allocate to the server instance"
            }
            CmdArgSetting::MaxRam(_) => {
                "The maximum amount of RAM to allocate to the server instance"
            }
            CmdArgSetting::JavaCmd(_) => "The command to use to run the java executable",
            CmdArgSetting::Args(_) => "The command line arguments to pass to the server",
        }
    }
    pub fn from_key_val(key: &str, val: &str) -> Result<Self, Error> {
        match key {
            "min_ram" => Ok(CmdArgSetting::MinRam(
                val.parse().context("Invalid value. Expected a u32")?,
            )),
            "max_ram" => Ok(CmdArgSetting::MaxRam(
                val.parse().context("Invalid value. Expected a u32")?,
            )),
            "java_cmd" => Ok(CmdArgSetting::JavaCmd(val.to_string())),
            "cmd_args" => Ok(CmdArgSetting::Args(
                val.split(' ').map(|s| s.to_string()).collect(),
            )),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Invalid key"),
            }),
        }
    }
    pub fn is_key_valid(key: &str) -> bool {
        matches!(key, "min_ram" | "max_ram" | "java_cmd" | "cmd_args")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum Gamemode {
    #[default]
    Survival,
    Creative,
    Adventure,
    Spectator,
}

impl FromStr for Gamemode {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "survival" => Ok(Gamemode::Survival),
            "creative" => Ok(Gamemode::Creative),
            "adventure" => Ok(Gamemode::Adventure),
            "spectator" => Ok(Gamemode::Spectator),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Invalid gamemode. The only valid gamemodes are: survival, creative, adventure, spectator"),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
enum Difficulty {
    #[default]
    Peaceful,
    Easy,
    Normal,
    Hard,
}

impl FromStr for Difficulty {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "peaceful" => Ok(Difficulty::Peaceful),
            "easy" => Ok(Difficulty::Easy),
            "normal" => Ok(Difficulty::Normal),
            "hard" => Ok(Difficulty::Hard),
            _ => Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Invalid difficulty. The only valid difficulties are: peaceful, easy, normal, hard"),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, EnumIter)]
enum ServerPropertySetting {
    EnableJmxMonitoring(bool),
    RconPort(u16),
    LevelSeed(Option<i64>),
    Gamemode(Gamemode),
    EnableCommandBlock(bool),
    EnableQuery(bool),
    GeneratorSettings(String),
    EnforceSecureProfile(bool),
    LevelName(String),
    Motd(String),
    QueryPort(u16),
    Pvp(bool),
    GenerateStructures(bool),
    MaxChainedNeighborUpdates(u32),
    Difficulty(Difficulty),
    NetworkCompressionThreshold(u32),
    RequireResourcePack(bool),
    MaxTickTime(u32),
    MaxPlayers(u32),
    UseNativeTransport(bool),
    OnlineMode(bool),
    EnableStatus(bool),
    AllowFlight(bool),
    InitialDisabledPacks(String),
    BroadcastRconToOps(bool),
    ViewDistance(u32),
    ResourcePackPrompt(String),
    ServerIp(String),
    AllowNether(bool),
    ServerPort(u16),
    EnableRcon(bool),
    SyncChunkWrites(bool),
    OpPermissionLevel(u32),
    PreventProxyConnections(bool),
    HideOnlinePlayers(bool),
    ResourcePack(String),
    EntityBroadcastRangePercentage(u32),
    SimulationDistance(u32),
    RconPassword(String),
    PlayerIdleTimeout(u32),
    ForceGamemode(bool),
    RateLimit(u32),
    Hardcore(bool),
    WhiteList(bool),
    BroadcastConsoleToOps(bool),
    PreviewsChat(bool),
    SpawnNpcs(bool),
    SpawnAnimals(bool),
    FunctionPermissionLevel(u32),
    InitialEnabledPacks(String),
    LevelType(String),
    TextFilteringConfig(String),
    SpawnMonsters(bool),
    EnforceWhitelist(bool),
    SpawnProtection(u32),
    ResourcePackSha1(String),
    MaxWorldSize(u32),
    Unknown(String, String),
}

impl ServerPropertySetting {
    fn get_identifier(&self) -> String {
        match self {
            Self::EnableJmxMonitoring(_) => "enable-jmx-monitoring",
            Self::RconPort(_) => "rcon.port",
            Self::LevelSeed(_) => "level-seed",
            Self::Gamemode(_) => "gamemode",
            Self::EnableCommandBlock(_) => "enable-command-block",
            Self::EnableQuery(_) => "enable-query",
            Self::GeneratorSettings(_) => "generator-settings",
            Self::EnforceSecureProfile(_) => "enforce-secure-profile",
            Self::LevelName(_) => "level-name",
            Self::Motd(_) => "motd",
            Self::QueryPort(_) => "query.port",
            Self::Pvp(_) => "pvp",
            Self::GenerateStructures(_) => "generate-structures",
            Self::MaxChainedNeighborUpdates(_) => "max-chained-neighbor-updates",
            Self::Difficulty(_) => "difficulty",
            Self::NetworkCompressionThreshold(_) => "network-compression-threshold",
            Self::RequireResourcePack(_) => "require-resource-pack",
            Self::MaxTickTime(_) => "max-tick-time",
            Self::MaxPlayers(_) => "max-players",
            Self::UseNativeTransport(_) => "use-native-transport",
            Self::OnlineMode(_) => "online-mode",
            Self::EnableStatus(_) => "enable-status",
            Self::AllowFlight(_) => "allow-flight",
            Self::InitialDisabledPacks(_) => "initial-disabled-packs",
            Self::BroadcastRconToOps(_) => "broadcast-rcon-to-ops",
            Self::ViewDistance(_) => "view-distance",
            Self::ResourcePackPrompt(_) => "resource-pack-prompt",
            Self::ServerIp(_) => "server-ip",
            Self::AllowNether(_) => "allow-nether",
            Self::ServerPort(_) => "server-port",
            Self::EnableRcon(_) => "enable-rcon",
            Self::SyncChunkWrites(_) => "sync-chunk-writes",
            Self::OpPermissionLevel(_) => "op-permission-level",
            Self::PreventProxyConnections(_) => "prevent-proxy-connections",
            Self::HideOnlinePlayers(_) => "hide-online-players",
            Self::ResourcePack(_) => "resource-pack",
            Self::EntityBroadcastRangePercentage(_) => "entity-broadcast-range-percentage",
            Self::SimulationDistance(_) => "simulation-distance",
            Self::RconPassword(_) => "rcon.password",
            Self::PlayerIdleTimeout(_) => "player-idle-timeout",
            Self::ForceGamemode(_) => "force-gamemode",
            Self::RateLimit(_) => "rate-limit",
            Self::Hardcore(_) => "hardcore",
            Self::WhiteList(_) => "white-list",
            Self::BroadcastConsoleToOps(_) => "broadcast-console-to-ops",
            Self::PreviewsChat(_) => "previews-chat",
            Self::SpawnNpcs(_) => "spawn-npcs",
            Self::SpawnAnimals(_) => "spawn-animals",
            Self::FunctionPermissionLevel(_) => "function-permission-level",
            Self::InitialEnabledPacks(_) => "initial-enabled-packs",
            Self::LevelType(_) => "level-type",
            Self::TextFilteringConfig(_) => "text-filtering-config",
            Self::SpawnMonsters(_) => "spawn-monsters",
            Self::EnforceWhitelist(_) => "enforce-whitelist",
            Self::SpawnProtection(_) => "spawn-protection",
            Self::ResourcePackSha1(_) => "resource-pack-sha1",
            Self::MaxWorldSize(_) => "max-world-size",
            Self::Unknown(key, _) => key,
        }
        .to_string()
    }

    // name to be displayed in the UI
    fn get_name(&self) -> String {
        if let Self::Unknown(key, _) = self {
            return key
                .to_string()
                .get_mut(0..1)
                .map(|c| c.to_uppercase())
                .unwrap_or_default();
        };
        match self {
            Self::EnableJmxMonitoring(_) => "Enable JMX Monitoring",
            Self::RconPort(_) => "Rcon Port",
            Self::LevelSeed(_) => "Level Seed",
            Self::Gamemode(_) => "Gamemode",
            Self::EnableCommandBlock(_) => "Enable Command Block",
            Self::EnableQuery(_) => "Enable Query",
            Self::GeneratorSettings(_) => "Generator Settings",
            Self::EnforceSecureProfile(_) => "Enforce Secure Profile",
            Self::LevelName(_) => "Level Name",
            Self::Motd(_) => "Motd",
            Self::QueryPort(_) => "Query Port",
            Self::Pvp(_) => "Pvp",
            Self::GenerateStructures(_) => "Generate Structures",
            Self::MaxChainedNeighborUpdates(_) => "Max Chained Neighbor Updates",
            Self::Difficulty(_) => "Difficulty",
            Self::NetworkCompressionThreshold(_) => "Network Compression Threshold",
            Self::RequireResourcePack(_) => "Require Resource Pack",
            Self::MaxTickTime(_) => "Max Tick Time",
            Self::MaxPlayers(_) => "Max Players",
            Self::UseNativeTransport(_) => "Use Native Transport",
            Self::OnlineMode(_) => "Online Mode",
            Self::EnableStatus(_) => "Enable Status",
            Self::AllowFlight(_) => "Allow Flight",
            Self::InitialDisabledPacks(_) => "Initial Disabled Packs",
            Self::BroadcastRconToOps(_) => "Broadcast Rcon To Ops",
            Self::ViewDistance(_) => "View Distance",
            Self::ResourcePackPrompt(_) => "Resource Pack Prompt",
            Self::ServerIp(_) => "Server Ip",
            Self::AllowNether(_) => "Allow Nether",
            Self::ServerPort(_) => "Server Port",
            Self::EnableRcon(_) => "Enable Rcon",
            Self::SyncChunkWrites(_) => "Sync Chunk Writes",
            Self::OpPermissionLevel(_) => "Op Permission Level",
            Self::PreventProxyConnections(_) => "Prevent Proxy Connections",
            Self::HideOnlinePlayers(_) => "Hide Online Players",
            Self::ResourcePack(_) => "Resource Pack",
            Self::EntityBroadcastRangePercentage(_) => "Entity Broadcast Range Percentage",
            Self::SimulationDistance(_) => "Simulation Distance",
            Self::RconPassword(_) => "Rcon Password",
            Self::PlayerIdleTimeout(_) => "Player Idle Timeout",
            Self::ForceGamemode(_) => "Force Gamemode",
            Self::RateLimit(_) => "Rate Limit",
            Self::Hardcore(_) => "Hardcore",
            Self::WhiteList(_) => "White List",
            Self::BroadcastConsoleToOps(_) => "Broadcast Console To Ops",
            Self::PreviewsChat(_) => "Previews Chat",
            Self::SpawnNpcs(_) => "Spawn Npcs",
            Self::SpawnAnimals(_) => "Spawn Animals",
            Self::FunctionPermissionLevel(_) => "Function Permission Level",
            Self::InitialEnabledPacks(_) => "Initial Enabled Packs",
            Self::LevelType(_) => "Level Type",
            Self::TextFilteringConfig(_) => "Text Filtering Config",
            Self::SpawnMonsters(_) => "Spawn Monsters",
            Self::EnforceWhitelist(_) => "Enforce Whitelist",
            Self::SpawnProtection(_) => "Spawn Protection",
            Self::ResourcePackSha1(_) => "Resource Pack Sha1",
            Self::MaxWorldSize(_) => "Max World Size",
            Self::Unknown(_, _) => unreachable!(),
        }
        .to_string()
    }

    // a short description of the property
    fn get_description(&self) -> String {
        if let Self::Unknown(key, val) = self {
            return format!(
                "Unknown property: {key} = {val} Please report this to the developers."
            );
        };
        match self {
            Self::EnableJmxMonitoring(_) => "Expose JMX metrics for monitoring.",
            Self::RconPort(_) => "Port for RCON connections.",
            Self::LevelSeed(_) => "Seed for the level. If left blank, a random seed will be used.",
            Self::Gamemode(_) => "Gamemode for the server. Applied when the player joined the server for the first time. Changing this value will not change the gamemode of existing players.",
            Self::EnableCommandBlock(_) => "Enable command blocks.",
            Self::EnableQuery(_) => "Enables GameSpy4 protocol server listener. Used to get information about server.",
            Self::GeneratorSettings(_) => "A JSON string that will be used to configure the world generator. If left blank, the default generator will be used.",
            Self::EnforceSecureProfile(_) => "Enforce the use of secure authentication.",
            Self::LevelName(_) => "The folder name for the world. Defaults to world.",
            Self::Motd(_) => "The message of the day.",
            Self::QueryPort(_) => "Port for GameSpy4 protocol server listener.",
            Self::Pvp(_) => "Enable PVP.",
            Self::GenerateStructures(_) => "Enable generation of structures.",
            Self::MaxChainedNeighborUpdates(_) => "Maximum number of chained neighbor updates.",
            Self::Difficulty(_) => "Difficulty of the server.",
            Self::NetworkCompressionThreshold(_) => "A threshold for network compression. If the size of a packet exceeds this value, it will be compressed.",
            Self::RequireResourcePack(_) => "Require clients to download resource packs.",
            Self::MaxTickTime(_) => "Maximum time in milliseconds that a single tick may take before the server watchdog stops the server with a timeout error.",
            Self::MaxPlayers(_) => "Maximum number of players that can be connected to the server at the same time.",
            Self::UseNativeTransport(_) => "Use native transport.",
            Self::OnlineMode(_) => "Enable online mode.",
            Self::EnableStatus(_) => "Enable status.",
            Self::AllowFlight(_) => "Allow flight.",
            Self::InitialDisabledPacks(_) => "A list of resource packs that will be disabled by default.",
            Self::BroadcastRconToOps(_) => "Send rcon console command outputs to all online operators.",
            Self::ViewDistance(_) => "Sets the amount of world data the server sends the client, measured in chunks in each direction of the player (radius, not diameter). It determines the server-side viewing distance.",
            Self::ResourcePackPrompt(_) => "Prompt clients to download resource packs.",
            Self::ServerIp(_) => "The IP address of the server. If left blank, the server will listen on all addresses.",
            Self::AllowNether(_) => "Allow nether.",
            Self::ServerPort(_) => "The port of the server.",
            Self::EnableRcon(_) => "Enables remote access to the server console. Not recommended to expose RCON to the internet.",
            Self::SyncChunkWrites(_) => "Sync chunk writes.",
            Self::OpPermissionLevel(_) => "The permission level for operators.",
            Self::PreventProxyConnections(_) => "Prevent connections from proxy servers.",
            Self::HideOnlinePlayers(_) => "Hide online players.",
            Self::ResourcePack(_) => "The URL of the resource pack that will be used by default.",
            Self::EntityBroadcastRangePercentage(_) => "The percentage of the world that entities can be broadcasted to.",
            Self::SimulationDistance(_) => "The distance in blocks that players will be able to interact with entities.",
            Self::RconPassword(_) => "The password for RCON connections.",
            Self::PlayerIdleTimeout(_) => "The number of minutes before a player is kicked for being idle.",
            Self::ForceGamemode(_) => "Force players to join in the specified gamemode.",
            Self::RateLimit(_) => "The maximum number of packets per second that the server will accept from a single IP address.",
            Self::Hardcore(_) => "Enable hardcore mode.",
            Self::WhiteList(_) => "Enable whitelist.",
            Self::BroadcastConsoleToOps(_) => "Send console command outputs to all online operators.",
            Self::PreviewsChat(_) => "Enable chat.",
            Self::SpawnNpcs(_) => "Enable spawning of NPCs.",
            Self::SpawnAnimals(_) => "Enable spawning of animals.",
            Self::FunctionPermissionLevel(_) => "The permission level for functions.",
            Self::InitialEnabledPacks(_) => "A list of resource packs that will be enabled by default.",
            Self::LevelType(_) => "The type of world to generate.",
            Self::TextFilteringConfig(_) => "The path to the text filtering configuration file.",
            Self::SpawnMonsters(_) => "Enable spawning of hostile mobs.",
            Self::EnforceWhitelist(_) => "Enforce the use of the whitelist.",
            Self::SpawnProtection(_) => "The radius of the spawn protection area.",
            Self::ResourcePackSha1(_) => "The SHA1 hash of the resource pack that will be used by default.",
            Self::MaxWorldSize(_) => "The maximum size of the world in blocks.",
            Self::Unknown(_, _) => unreachable!("Already handled above.")
        }.to_string()
    }

    pub fn from_key_val(key: &str, value: &str) -> Result<Self, Error> {
        match key {
            "enable-jmx-monitoring" => Ok(Self::EnableJmxMonitoring(
                value
                    .parse::<bool>()
                    .with_context(|| eyre!("Invalid value: {value}, expected bool"))?,
            )),
            "rcon.port" => {
                Ok(Self::RconPort(value.parse::<u16>().with_context(|| {
                    eyre!("Invalid value: {value}, expected u16")
                })?))
            }
            "level-seed" => Ok(Self::LevelSeed(value.parse::<i64>().ok())),
            "gamemode" => {
                Ok(Self::Gamemode(value.parse::<Gamemode>().with_context(
                    || eyre!("Invalid value: {value}, expected Gamemode"),
                )?))
            }
            "enable-command-block" => Ok(Self::EnableCommandBlock(
                value
                    .parse::<bool>()
                    .with_context(|| eyre!("Invalid value: {value}, expected bool"))?,
            )),
            "enable-query" => {
                Ok(Self::EnableQuery(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "generator-settings" => {
                Ok(Self::GeneratorSettings(value.parse().with_context(
                    || eyre!("Invalid value: {value}, expected string"),
                )?))
            }
            "enforce-secure-profile" => Ok(Self::EnforceSecureProfile(
                value
                    .parse::<bool>()
                    .with_context(|| eyre!("Invalid value: {value}, expected bool"))?,
            )),
            "level-name" => {
                Ok(Self::LevelName(value.parse().with_context(|| {
                    eyre!("Invalid value: {value}, expected string")
                })?))
            }
            "motd" => {
                Ok(Self::Motd(value.parse().with_context(|| {
                    eyre!("Invalid value: {value}, expected string")
                })?))
            }
            "query.port" => {
                Ok(Self::QueryPort(value.parse::<u16>().with_context(
                    || eyre!("Invalid value: {value}, expected u16"),
                )?))
            }
            "pvp" => {
                Ok(Self::Pvp(value.parse::<bool>().with_context(|| {
                    eyre!("Invalid value: {value}, expected bool")
                })?))
            }
            "generate-structures" => Ok(Self::GenerateStructures(
                value
                    .parse::<bool>()
                    .with_context(|| eyre!("Invalid value: {value}, expected bool"))?,
            )),
            "max-chained-neighbor-updates" => Ok(Self::MaxChainedNeighborUpdates(
                value
                    .parse::<u32>()
                    .with_context(|| eyre!("Invalid value: {value}, expected u32"))?,
            )),
            "difficulty" => {
                Ok(Self::Difficulty(value.parse::<Difficulty>().with_context(
                    || eyre!("Invalid value: {value}, expected Difficulty."),
                )?))
            }
            "network-compression-threshold" => Ok(Self::NetworkCompressionThreshold(
                value
                    .parse::<u32>()
                    .with_context(|| eyre!("Invalid value: {value}, expected u32"))?,
            )),
            "require-resource-pack" => Ok(Self::RequireResourcePack(
                value
                    .parse::<bool>()
                    .with_context(|| eyre!("Invalid value: {value}, expected bool"))?,
            )),
            "max-tick-time" => {
                Ok(Self::MaxTickTime(value.parse::<u32>().with_context(
                    || eyre!("Invalid value: {value}, expected u32"),
                )?))
            }
            "use-native-transport" => Ok(Self::UseNativeTransport(
                value
                    .parse::<bool>()
                    .with_context(|| eyre!("Invalid value: {value}, expected bool"))?,
            )),
            "max-players" => {
                Ok(Self::MaxPlayers(value.parse::<u32>().with_context(
                    || eyre!("Invalid value: {value}, expected u8"),
                )?))
            }

            "online-mode" => {
                Ok(Self::OnlineMode(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "enable-status" => {
                Ok(Self::EnableStatus(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "allow-flight" => {
                Ok(Self::AllowFlight(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "initial-disabled-packs" => Ok(Self::InitialDisabledPacks(value.to_string())),
            "broadcast-rcon-to-ops" => Ok(Self::BroadcastRconToOps(
                value
                    .parse::<bool>()
                    .with_context(|| eyre!("Invalid value: {value}, expected bool"))?,
            )),
            "view-distance" => {
                Ok(Self::ViewDistance(value.parse::<u32>().with_context(
                    || eyre!("Invalid value: {value}, expected u32"),
                )?))
            }
            "resource-pack-prompt" => Ok(Self::ResourcePackPrompt(value.to_string())),
            "server-ip" => Ok(Self::ServerIp(value.to_string())),
            "allow-nether" => {
                Ok(Self::AllowNether(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "server-port" => {
                Ok(Self::ServerPort(value.parse::<u16>().with_context(
                    || eyre!("Invalid value: {value}, expected u16"),
                )?))
            }
            "enable-rcon" => {
                Ok(Self::EnableRcon(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "sync-chunk-writes" => {
                Ok(Self::SyncChunkWrites(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "op-permission-level" => {
                Ok(Self::OpPermissionLevel(value.parse::<u32>().with_context(
                    || eyre!("Invalid value: {value}, expected u32"),
                )?))
            }
            "prevent-proxy-connections" => Ok(Self::PreventProxyConnections(
                value
                    .parse::<bool>()
                    .with_context(|| eyre!("Invalid value: {value}, expected bool"))?,
            )),
            "hide-online-players" => Ok(Self::HideOnlinePlayers(
                value
                    .parse::<bool>()
                    .with_context(|| eyre!("Invalid value: {value}, expected bool"))?,
            )),
            "resource-pack" => Ok(Self::ResourcePack(value.to_string())),
            "entity-broadcast-range-percentage" => Ok(Self::EntityBroadcastRangePercentage(
                value
                    .parse::<u32>()
                    .with_context(|| eyre!("Invalid value: {value}, expected u32"))?,
            )),
            "simulation-distance" => Ok(Self::SimulationDistance(
                value
                    .parse::<u32>()
                    .with_context(|| eyre!("Invalid value: {value}, expected u32"))?,
            )),
            "rcon.password" => Ok(Self::RconPassword(value.to_string())),
            "player-idle-timeout" => {
                Ok(Self::PlayerIdleTimeout(value.parse::<u32>().with_context(
                    || eyre!("Invalid value: {value}, expected u32"),
                )?))
            }
            "force-gamemode" => {
                Ok(Self::ForceGamemode(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "rate-limit" => {
                Ok(Self::RateLimit(value.parse::<u32>().with_context(
                    || eyre!("Invalid value: {value}, expected u32"),
                )?))
            }
            "hardcore" => {
                Ok(Self::Hardcore(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "white-list" => {
                Ok(Self::WhiteList(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "broadcast-console-to-ops" => Ok(Self::BroadcastConsoleToOps(
                value
                    .parse::<bool>()
                    .with_context(|| eyre!("Invalid value: {value}, expected bool"))?,
            )),
            "previews-chat" => {
                Ok(Self::PreviewsChat(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "spawn-npcs" => {
                Ok(Self::SpawnNpcs(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "spawn-animals" => {
                Ok(Self::SpawnAnimals(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "function-permission-level" => Ok(Self::FunctionPermissionLevel(
                value
                    .parse::<u32>()
                    .with_context(|| eyre!("Invalid value: {value}, expected u32"))?,
            )),
            "initial-enabled-packs" => Ok(Self::InitialEnabledPacks(value.to_string())),
            "level-type" => Ok(Self::LevelType(value.to_string())),
            "text-filtering-config" => Ok(Self::TextFilteringConfig(value.to_string())),
            "spawn-monsters" => {
                Ok(Self::SpawnMonsters(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }

            "enforce-whitelist" => {
                Ok(Self::EnforceWhitelist(value.parse::<bool>().with_context(
                    || eyre!("Invalid value: {value}, expected bool"),
                )?))
            }
            "spawn-protection" => {
                Ok(Self::SpawnProtection(value.parse::<u32>().with_context(
                    || eyre!("Invalid value: {value}, expected u32"),
                )?))
            }
            "resource-pack-sha1" => Ok(Self::ResourcePackSha1(value.to_string())),
            "max-world-size" => {
                Ok(Self::MaxWorldSize(value.parse::<u32>().with_context(
                    || eyre!("Invalid value: {value}, expected u32"),
                )?))
            }
            _ => Ok(Self::Unknown(key.to_string(), value.to_string())),
        }
    }
}

impl FromStr for ServerPropertySetting {
    type Err = Error;

    fn from_str(line: &str) -> Result<Self, Self::Err> {
        let mut split = line.split('=');
        let key = split
            .next()
            .with_context(|| eyre!("Invalid line, no key: {}", line))?;
        let value = split
            .next()
            .with_context(|| eyre!("Invalid line, no value: {}", line))?;

        Self::from_key_val(key, value)
    }
}

#[cfg(test)]
mod test {
    use std::io::BufRead;

    use super::*;

    #[test]
    fn test_parse_server_properties() {
        let properties =
            "enable-jmx-monitoring=false\nrcon.port=25575\nlevel-seed=\ndifficulty=easy";

        let mut res: Vec<ServerPropertySetting> = Vec::new();
        for (line_num, line) in properties.lines().enumerate() {
            if let Ok(entry) = ServerPropertySetting::from_str(line) {
                res.push(entry);
            } else {
                panic!("Failed to parse line: {} at {line_num}", line);
            }
        }

        assert_eq!(res[0], ServerPropertySetting::EnableJmxMonitoring(false));

        assert_eq!(res[1], ServerPropertySetting::RconPort(25575));

        assert_eq!(res[2], ServerPropertySetting::LevelSeed(None));

        assert_eq!(res[3], ServerPropertySetting::Difficulty(Difficulty::Easy));
    }

    #[test]
    fn test_exhausiveness() {
        let properties_file = std::io::BufReader::new(
            std::fs::File::open("src/testdata/sample_server.properties")
                .expect("Failed to open server.properties"),
        );
        for line in properties_file.lines() {
            let line = line.expect("Failed to read line");
            match ServerPropertySetting::from_str(&line) {
                Ok(v) => {
                    if let ServerPropertySetting::Unknown(_, _) = v {
                        panic!("Unknown property: {}", line);
                    }
                }
                Err(e) => panic!("Failed to parse line: {} with error: {}", line, e),
            }
        }
    }
}
