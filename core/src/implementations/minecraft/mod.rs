pub mod configurable;
pub mod fabric;
mod forge;
mod line_parser;
pub mod r#macro;
mod paper;
pub mod player;
mod players_manager;
pub mod properties;
pub mod server;
pub mod server_properties;
pub mod util;
mod vanilla;

use color_eyre::eyre::{eyre, Context};
use enum_kinds::EnumKind;
use indexmap::IndexMap;

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, LazyLock};
use std::time::Instant;
use tokio::process::{Child, Command};

use tokio::sync::Mutex;

use ::serde::{Deserialize, Serialize};
use serde_json::to_string_pretty;

use tracing::{error, warn};

use tokio;
use ts_rs::TS;

use crate::error::{Error, ErrorKind};
use crate::event_broadcaster::EventBroadcaster;
use crate::events::{Event, ProgressionEventID};
use crate::macro_executor::{MacroExecutor, MacroPID};
use crate::prelude::path_to_binaries;
use crate::traits::t_configurable::PathBuf;

use crate::traits::t_configurable::manifest::{
    ConfigurableManifest, ConfigurableValue, ConfigurableValueType, SectionManifest,
    SettingManifest, SetupManifest, SetupValue,
};

use crate::traits::t_macro::TaskEntry;
use crate::traits::t_server::State;
use crate::traits::TInstance;
use crate::types::{DotLodestoneConfig, InstanceUuid};
use crate::util::{
    dont_spawn_terminal, download_file, format_byte, format_byte_download, unzip_file_async,
    DownloadProgress, UnzipOption,
};

use self::configurable::CmdArgSetting;
use self::fabric::get_fabric_minecraft_versions;
use self::forge::get_forge_minecraft_versions;
use self::paper::get_paper_minecraft_versions;
use self::players_manager::PlayersManager;
use self::properties::PropertiesDocument;
use self::util::{get_jre_major_version, get_server_jar_url, jre_download_url};
use self::vanilla::get_vanilla_minecraft_versions;

#[derive(Debug, Clone, TS, Serialize, Deserialize, PartialEq)]
#[ts(export)]
pub struct FabricLoaderVersion(String);
impl FabricLoaderVersion {
    pub fn new(version: String) -> Self {
        Self(version)
    }
}
#[derive(Debug, Clone, TS, Serialize, Deserialize, PartialEq)]
#[ts(export)]
pub struct FabricInstallerVersion(String);
#[derive(Debug, Clone, TS, Serialize, Deserialize, PartialEq)]
#[ts(export)]
pub struct PaperBuildVersion(i64);
#[derive(Debug, Clone, TS, Serialize, Deserialize, PartialEq)]
#[ts(export)]
pub struct ForgeBuildVersion(String);

/// A parameter for constructor of `MinecraftInstance`
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, EnumKind)]
#[serde(rename_all = "snake_case")]
#[enum_kind(FlavourKind, derive(Serialize, Deserialize, TS))]
pub enum Flavour {
    Vanilla,
    Fabric {
        loader_version: Option<FabricLoaderVersion>,
        installer_version: Option<FabricInstallerVersion>,
    },
    Paper {
        build_version: Option<PaperBuildVersion>,
    },
    Spigot,
    Forge {
        build_version: Option<ForgeBuildVersion>,
    },
}

impl From<FlavourKind> for Flavour {
    fn from(kind: FlavourKind) -> Self {
        match kind {
            FlavourKind::Vanilla => Flavour::Vanilla,
            FlavourKind::Fabric => Flavour::Fabric {
                loader_version: None,
                installer_version: None,
            },
            FlavourKind::Paper => Flavour::Paper {
                build_version: None,
            },
            FlavourKind::Spigot => Flavour::Spigot,
            FlavourKind::Forge => Flavour::Forge {
                build_version: None,
            },
        }
    }
}

impl ToString for Flavour {
    fn to_string(&self) -> String {
        match self {
            Flavour::Vanilla => "vanilla".to_string(),
            Flavour::Fabric { .. } => "fabric".to_string(),
            Flavour::Paper { .. } => "paper".to_string(),
            Flavour::Spigot => "spigot".to_string(),
            Flavour::Forge { .. } => "forge".to_string(),
        }
    }
}

impl ToString for FlavourKind {
    fn to_string(&self) -> String {
        match self {
            FlavourKind::Vanilla => "vanilla".to_string(),
            FlavourKind::Fabric => "fabric".to_string(),
            FlavourKind::Paper => "paper".to_string(),
            FlavourKind::Spigot => "spigot".to_string(),
            FlavourKind::Forge => "forge".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SetupConfig {
    pub name: String,
    pub version: String,
    pub flavour: Flavour,
    pub port: u32,
    pub cmd_args: Vec<String>,
    pub description: Option<String>,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
    pub auto_start: Option<bool>,
    pub restart_on_crash: Option<bool>,
    pub backup_period: Option<u32>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RestoreConfig {
    pub name: String,
    pub version: String,
    pub flavour: Flavour,
    pub description: String,
    pub cmd_args: Vec<String>,
    pub java_cmd: Option<String>,
    pub port: u32,
    pub min_ram: u32,
    pub max_ram: u32,
    pub auto_start: bool,
    pub restart_on_crash: bool,
    // `backup_period` and `has_started` are not used by this version. They are still
    // written so that older Lodestone versions can read the config file.
    pub backup_period: Option<u32>,
    pub jre_major_version: u64,
    #[serde(default)]
    pub has_started: bool,
}
#[derive(Clone)]
pub struct MinecraftInstance {
    config: Arc<Mutex<RestoreConfig>>,
    uuid: InstanceUuid,
    creation_time: i64,
    state: Arc<Mutex<State>>,
    event_broadcaster: EventBroadcaster,
    // file paths
    path_to_instance: PathBuf,
    path_to_config: PathBuf,
    path_to_properties: PathBuf,

    // directory paths
    path_to_macros: PathBuf,
    path_to_runtimes: PathBuf,

    process: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    /// Set when a user stops or kills the instance, so that the exit is not seen as a crash
    stop_requested: Arc<AtomicBool>,
    /// Held while the version changes, so the instance can't start meanwhile
    version_change_lock: Arc<Mutex<()>>,
    /// Serializes read-modify-write cycles of server.properties
    properties_lock: Arc<Mutex<()>>,
    system: Arc<Mutex<sysinfo::System>>,
    players_manager: Arc<Mutex<PlayersManager>>,
    configurable_manifest: Arc<Mutex<ConfigurableManifest>>,
    macro_executor: MacroExecutor,
    rcon_conn: Arc<Mutex<Option<rcon::Connection<tokio::net::TcpStream>>>>,
    macro_name_to_last_run: Arc<Mutex<HashMap<String, i64>>>,
    pid_to_task_entry: Arc<Mutex<IndexMap<MacroPID, TaskEntry>>>,
}

#[tokio::test]
async fn test_setup_manifest() {
    let manifest = MinecraftInstance::setup_manifest(&FlavourKind::Fabric)
        .await
        .unwrap();
    let manifest_json_string = serde_json::to_string_pretty(&manifest).unwrap();
    println!("{manifest_json_string}");
}

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

/// Checks a name for a new instance. The name becomes part of the instance folder name.
pub(super) fn validate_instance_name(name: &str) -> Result<(), Error> {
    if name.trim().is_empty() {
        return Err(bad_request("Name cannot be empty"));
    }
    if name.chars().count() > 100 {
        return Err(bad_request("Name cannot be longer than 100 characters"));
    }
    if name.chars().any(|c| c.is_control()) {
        return Err(bad_request("Name cannot contain control characters"));
    }
    Ok(())
}

/// Checks a name for a new instance folder: also rejects path separators and characters
/// that Windows does not allow in file names.
pub(crate) fn validate_new_instance_name(name: &str) -> Result<(), Error> {
    validate_instance_name(name)?;
    const FORBIDDEN: &[char] = &['/', '\\', ':', '*', '?', '"', '<', '>', '|'];
    if name.chars().any(|c| FORBIDDEN.contains(&c)) {
        return Err(bad_request(
            "Name cannot contain any of these characters: / \\ : * ? \" < > |",
        ));
    }
    Ok(())
}

/// Splits command line arguments like a shell: whitespace separates arguments, and single
/// or double quotes group text with spaces. Inside double quotes, `\"` and `\\` are escapes.
/// Backslashes outside quotes are kept, so Windows paths work.
pub(super) fn split_args(input: &str) -> Result<Vec<String>, Error> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut in_arg = false;
    let mut quote: Option<char> = None;
    let mut chars = input.chars();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == q => quote = None,
            Some('"') if c == '\\' => match chars.next() {
                Some(next @ ('"' | '\\')) => current.push(next),
                Some(next) => {
                    current.push('\\');
                    current.push(next);
                }
                None => current.push('\\'),
            },
            Some(_) => current.push(c),
            None if c.is_whitespace() => {
                if in_arg {
                    args.push(std::mem::take(&mut current));
                    in_arg = false;
                }
            }
            None if c == '\'' || c == '"' => {
                quote = Some(c);
                in_arg = true;
            }
            None => {
                current.push(c);
                in_arg = true;
            }
        }
    }
    if quote.is_some() {
        return Err(bad_request("Command line arguments have an unclosed quote"));
    }
    if in_arg {
        args.push(current);
    }
    args.retain(|a| !a.is_empty());
    Ok(args)
}

/// Joins arguments so that `split_args` gives them back.
pub(super) fn join_args(args: &[String]) -> String {
    args.iter()
        .filter(|a| !a.is_empty())
        .map(|arg| {
            if !arg
                .chars()
                .any(|c| c.is_whitespace() || c == '\'' || c == '"')
            {
                arg.clone()
            } else if !arg.contains('\'') {
                format!("'{arg}'")
            } else {
                format!("\"{}\"", arg.replace('\\', "\\\\").replace('"', "\\\""))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The `java` binary of a JRE that Lodestone manages.
pub(super) fn java_binary(path_to_runtimes: &Path, jre_major_version: u64) -> PathBuf {
    path_to_runtimes
        .join("java")
        .join(format!("jre{jre_major_version}"))
        .join(if std::env::consts::OS == "macos" {
            "Contents/Home/bin"
        } else {
            "bin"
        })
        .join("java")
}

/// Downloads the JRE for a Java major version unless it is already there. Returns true if
/// it was downloaded. Concurrent calls for the same version wait for each other.
pub(super) async fn ensure_jre(
    path_to_runtimes: &Path,
    jre_major_version: u64,
    on_download: &(dyn Fn(DownloadProgress) + Send + Sync),
) -> Result<bool, Error> {
    static LOCKS: LazyLock<std::sync::Mutex<HashMap<u64, Arc<Mutex<()>>>>> =
        LazyLock::new(Default::default);
    let lock = LOCKS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(jre_major_version)
        .or_default()
        .clone();
    let _guard = lock.lock().await;

    let java_dir = path_to_runtimes.join("java");
    let target = java_dir.join(format!("jre{jre_major_version}"));
    if tokio::fs::try_exists(&target).await.unwrap_or(false) {
        return Ok(false);
    }
    tokio::fs::create_dir_all(&java_dir)
        .await
        .context("Could not create the Java runtime directory")?;
    // A private folder next to the target, so the final rename stays on one file system
    // and two downloads never share files.
    let work_dir = tempfile::Builder::new()
        .prefix(".jre-download-")
        .tempdir_in(&java_dir)
        .context("Could not create a temporary directory for the JRE download")?;
    let archive_name = if std::env::consts::OS == "windows" {
        "jre.zip"
    } else {
        "jre.tar.gz"
    };
    let archive = download_file(
        &jre_download_url(jre_major_version),
        work_dir.path(),
        Some(archive_name),
        on_download,
        true,
    )
    .await?;
    let extracted = unzip_file_async(
        &archive,
        UnzipOption::ToDir(work_dir.path().join("extracted")),
    )
    .await?;
    if extracted.len() != 1 {
        return Err(eyre!(
            "Expected only one file in the JRE archive, got {}",
            extracted.len()
        )
        .into());
    }
    let extracted = extracted
        .into_iter()
        .next()
        .ok_or_else(|| eyre!("The JRE archive is empty"))?;
    tokio::fs::rename(&extracted, &target)
        .await
        .context(format!(
            "Could not move the JRE from {} to {}",
            extracted.display(),
            target.display()
        ))?;
    Ok(true)
}

/// Writes a file by writing a temporary file in the same folder and renaming it, so the
/// file is never left half written.
pub(super) async fn write_file_atomically(path: &Path, bytes: Vec<u8>) -> Result<(), Error> {
    let path = path.to_owned();
    tokio::task::spawn_blocking(move || -> Result<(), Error> {
        let dir = path
            .parent()
            .ok_or_else(|| eyre!("{} has no parent directory", path.display()))?;
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let mut tmp = tempfile::Builder::new()
            .prefix(&format!(".{file_name}."))
            .suffix(".tmp")
            .tempfile_in(dir)
            .context(format!(
                "Failed to create a temporary file in {}",
                dir.display()
            ))?;
        tmp.write_all(&bytes)
            .context(format!("Failed to write {}", path.display()))?;
        if let Ok(metadata) = std::fs::metadata(&path) {
            let _ = tmp.as_file().set_permissions(metadata.permissions());
        }
        tmp.as_file()
            .sync_all()
            .context(format!("Failed to write {}", path.display()))?;
        tmp.persist(&path)
            .map_err(|e| e.error)
            .context(format!("Failed to replace {}", path.display()))?;
        Ok(())
    })
    .await
    .context("The file write task failed")?
}

async fn tail_of_file(path: &Path, lines: usize) -> String {
    match tokio::fs::read(path).await {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            let all: Vec<&str> = text.lines().collect();
            all[all.len().saturating_sub(lines)..].join("\n")
        }
        Err(e) => format!("(could not read {}: {e})", path.display()),
    }
}

impl MinecraftInstance {
    /// The versions a flavour supports, newest first. Cached for a few minutes, because
    /// both the setup form and instance creation need them.
    async fn minecraft_versions(flavour: &FlavourKind) -> Result<Vec<String>, Error> {
        static CACHE: LazyLock<Mutex<HashMap<String, (Instant, Vec<String>)>>> =
            LazyLock::new(Default::default);
        let key = flavour.to_string();
        if let Some((fetched_at, versions)) = CACHE.lock().await.get(&key) {
            if fetched_at.elapsed() < util::VERSION_CACHE_TTL {
                return Ok(versions.clone());
            }
        }
        let versions = match flavour {
            FlavourKind::Vanilla => get_vanilla_minecraft_versions().await,
            FlavourKind::Fabric => get_fabric_minecraft_versions().await,
            FlavourKind::Paper => get_paper_minecraft_versions().await,
            FlavourKind::Spigot => {
                return Err(Error {
                    kind: ErrorKind::UnsupportedOperation,
                    source: eyre!("Spigot servers are not supported"),
                })
            }
            FlavourKind::Forge => get_forge_minecraft_versions().await,
        }
        .context("Failed to get minecraft versions")?;
        CACHE
            .lock()
            .await
            .insert(key, (Instant::now(), versions.clone()));
        Ok(versions)
    }

    pub async fn setup_manifest(flavour: &FlavourKind) -> Result<SetupManifest, Error> {
        let versions = Self::minecraft_versions(flavour).await?;
        let newest = versions
            .first()
            .cloned()
            .ok_or_else(|| eyre!("The version list for {} is empty", flavour.to_string()))?;

        let version_setting = SettingManifest::new_value_with_type(
            "version".to_string(),
            "Version".to_string(),
            "The version of minecraft to use".to_string(),
            Some(ConfigurableValue::Enum(newest)),
            ConfigurableValueType::Enum { options: versions },
            None,
            false,
            true,
        );

        let port_setting = SettingManifest::new_value_with_type(
            "port".to_string(),
            "Port".to_string(),
            "The port to run the server on".to_string(),
            Some(ConfigurableValue::UnsignedInteger(25565)),
            ConfigurableValueType::UnsignedInteger {
                min: Some(1),
                max: Some(65535),
            },
            Some(ConfigurableValue::UnsignedInteger(25565)),
            false,
            true,
        );

        let min_ram_setting = SettingManifest::new_value_with_type(
            "min_ram".to_string(),
            "Minimum RAM".to_string(),
            "The minimum amount of RAM to allocate to the server, in MB".to_string(),
            Some(ConfigurableValue::UnsignedInteger(1024)),
            ConfigurableValueType::UnsignedInteger {
                min: Some(1),
                max: None,
            },
            Some(ConfigurableValue::UnsignedInteger(1024)),
            false,
            true,
        );

        let max_ram_setting = SettingManifest::new_value_with_type(
            "max_ram".to_string(),
            "Maximum RAM".to_string(),
            "The maximum amount of RAM to allocate to the server, in MB".to_string(),
            Some(ConfigurableValue::UnsignedInteger(2048)),
            ConfigurableValueType::UnsignedInteger {
                min: Some(1),
                max: None,
            },
            Some(ConfigurableValue::UnsignedInteger(2048)),
            false,
            true,
        );

        let command_line_args_setting = SettingManifest::new_optional_value(
            "cmd_args".to_string(),
            "Command Line Arguments".to_string(),
            "Command line arguments to pass to the server".to_string(),
            None,
            ConfigurableValueType::String { regex: None },
            None,
            false,
            true,
        );

        let mut section_1_map = IndexMap::new();

        section_1_map.insert("version".to_string(), version_setting);
        section_1_map.insert("port".to_string(), port_setting);

        let mut section_2_map = IndexMap::new();

        section_2_map.insert("min_ram".to_string(), min_ram_setting);

        section_2_map.insert("max_ram".to_string(), max_ram_setting);

        section_2_map.insert("cmd_args".to_string(), command_line_args_setting);

        let section_1 = SectionManifest::new(
            "section_1".to_string(),
            "Basic Settings".to_string(),
            "Basic settings for the server.".to_string(),
            section_1_map,
        );

        let section_2 = SectionManifest::new(
            "section_2".to_string(),
            "Advanced Settings".to_string(),
            "Advanced settings for your minecraft server.".to_string(),
            section_2_map,
        );

        let mut sections = IndexMap::new();

        sections.insert("section_1".to_string(), section_1);
        sections.insert("section_2".to_string(), section_2);

        Ok(SetupManifest {
            setting_sections: sections,
        })
    }

    pub async fn construct_setup_config(
        setup_value: SetupValue,
        flavour: FlavourKind,
    ) -> Result<SetupConfig, Error> {
        validate_new_instance_name(&setup_value.name)?;
        Self::setup_manifest(&flavour)
            .await?
            .validate_setup_value(&setup_value)?;

        let required = |id: &str| {
            setup_value
                .get_unique_setting(id)
                .and_then(|s| s.get_value())
                .ok_or_else(|| bad_request(format!("Missing required setting: {id}")))
        };

        let version = required("version")?.try_as_enum()?.clone();
        let port = required("port")?.try_as_unsigned_integer()?;
        let min_ram = required("min_ram")?.try_as_unsigned_integer()?;
        let max_ram = required("max_ram")?.try_as_unsigned_integer()?;
        if min_ram > max_ram {
            return Err(bad_request("Minimum RAM cannot be larger than maximum RAM"));
        }
        let cmd_args = match setup_value
            .get_unique_setting("cmd_args")
            .and_then(|s| s.get_value())
        {
            Some(value) => split_args(value.try_as_string()?)?,
            None => Vec::new(),
        };

        Ok(SetupConfig {
            name: setup_value.name.clone(),
            description: setup_value.description.clone(),
            version,
            port,
            min_ram: Some(min_ram),
            max_ram: Some(max_ram),
            cmd_args,
            flavour: flavour.into(),
            auto_start: Some(setup_value.auto_start),
            restart_on_crash: Some(setup_value.restart_on_crash),
            backup_period: None,
        })
    }

    fn init_configurable_manifest(
        restore_config: &RestoreConfig,
        java_cmd: String,
    ) -> ConfigurableManifest {
        let mut cmd_args_config_map = IndexMap::new();
        let cmd_args = CmdArgSetting::Args(restore_config.cmd_args.clone());
        cmd_args_config_map.insert(cmd_args.get_identifier().to_owned(), cmd_args.into());
        let min_ram = CmdArgSetting::MinRam(restore_config.min_ram);
        cmd_args_config_map.insert(min_ram.get_identifier().to_owned(), min_ram.into());
        let max_ram = CmdArgSetting::MaxRam(restore_config.max_ram);
        cmd_args_config_map.insert(max_ram.get_identifier().to_owned(), max_ram.into());
        let java_cmd = CmdArgSetting::JavaCmd(java_cmd);
        cmd_args_config_map.insert(java_cmd.get_identifier().to_owned(), java_cmd.into());

        let cmd_line_section_manifest = SectionManifest::new(
            CmdArgSetting::get_section_id().to_string(),
            "Command Line Settings".to_string(),
            "Settings are passed to the server and Java as command line arguments".to_string(),
            cmd_args_config_map,
        );

        let server_properties_section_manifest = SectionManifest::new(
            server_properties::SECTION_ID.to_string(),
            "Server Properties Settings".to_string(),
            "All settings in the server.properties file can be configured here".to_string(),
            IndexMap::new(),
        );

        let mut setting_sections = IndexMap::new();

        setting_sections.insert(
            CmdArgSetting::get_section_id().to_string(),
            cmd_line_section_manifest,
        );

        setting_sections.insert(
            server_properties::SECTION_ID.to_string(),
            server_properties_section_manifest,
        );

        ConfigurableManifest::new(false, false, setting_sections)
    }

    pub async fn new(
        config: SetupConfig,
        dot_lodestone_config: DotLodestoneConfig,
        path_to_instance: PathBuf,
        progression_event_id: &ProgressionEventID,
        event_broadcaster: EventBroadcaster,
        macro_executor: MacroExecutor,
    ) -> Result<MinecraftInstance, Error> {
        let path_to_config = path_to_instance.join(".lodestone_minecraft_config.json");
        let path_to_eula = path_to_instance.join("eula.txt");
        let path_to_macros = path_to_instance.join("macros");
        let path_to_resources = path_to_instance.join("resources");
        let path_to_properties = path_to_instance.join("server.properties");
        let path_to_runtimes = path_to_binaries().to_owned();

        // Step 1: Create Directories
        event_broadcaster.send(Event::new_progression_event_update(
            progression_event_id,
            "1/4: Creating directories",
            1.0,
        ));
        tokio::fs::create_dir_all(&path_to_instance)
            .await
            .and(tokio::fs::create_dir_all(&path_to_macros).await)
            .and(tokio::fs::create_dir_all(&path_to_resources.join("mods")).await)
            .and(tokio::fs::create_dir_all(&path_to_resources.join("worlds")).await)
            .and(tokio::fs::create_dir_all(&path_to_resources.join("defaults")).await)
            .and(tokio::fs::write(&path_to_eula, "#generated by Lodestone\neula=true").await)
            .and(
                tokio::fs::write(
                    &path_to_properties,
                    format!("server-port={}\n", config.port),
                )
                .await,
            )
            .context("Could not create some files or directories for instance")
            .map_err(|e| {
                error!("{e}");
                e
            })?;

        // Step 2: Download JRE
        let jre_major_version = get_jre_major_version(config.version.as_str())
            .await
            .context("Could not get JRE URL")?;
        let downloaded = ensure_jre(&path_to_runtimes, jre_major_version, {
            let event_broadcaster = event_broadcaster.clone();
            &move |dl| {
                if let Some(total) = dl.total {
                    event_broadcaster.send(Event::new_progression_event_update(
                        progression_event_id,
                        format!(
                            "2/4: Downloading JRE {}",
                            format_byte_download(dl.downloaded, total)
                        ),
                        (dl.step as f64 / total as f64) * 4.0,
                    ));
                }
            }
        })
        .await?;
        if !downloaded {
            event_broadcaster.send(Event::new_progression_event_update(
                progression_event_id,
                "2/4: JRE already downloaded",
                4.0,
            ));
        }

        // Step 3: Download server.jar
        let flavour_name = config.flavour.to_string();
        let (jar_url, flavour) = get_server_jar_url(config.version.as_str(), &config.flavour)
            .await
            .context(format!(
                "Could not find a {} server.jar for version {}",
                flavour_name, config.version
            ))?;
        let jar_name = match flavour {
            Flavour::Forge { .. } => "forge-installer.jar",
            _ => "server.jar",
        };

        download_file(
            jar_url.as_str(),
            &path_to_instance,
            Some(jar_name),
            {
                let event_broadcaster = event_broadcaster.clone();
                &move |dl| {
                    if let Some(total) = dl.total {
                        event_broadcaster.send(Event::new_progression_event_update(
                            progression_event_id,
                            format!(
                                "3/4: Downloading {} {} {}",
                                flavour_name,
                                jar_name,
                                format_byte_download(dl.downloaded, total),
                            ),
                            (dl.step as f64 / total as f64) * 3.0,
                        ));
                    } else {
                        event_broadcaster.send(Event::new_progression_event_update(
                            progression_event_id,
                            format!(
                                "3/4: Downloading {} {} {}",
                                flavour_name,
                                jar_name,
                                format_byte(dl.downloaded),
                            ),
                            0.0,
                        ));
                    }
                }
            },
            true,
        )
        .await?;
        let jre = java_binary(&path_to_runtimes, jre_major_version);
        // Step 3 (part 2): Forge Setup
        if let Flavour::Forge { .. } = flavour.clone() {
            event_broadcaster.send(Event::new_progression_event_update(
                progression_event_id,
                "3/4: Installing Forge Server",
                1.0,
            ));

            let log_path = path_to_instance.join("forge-installer.log");
            let log =
                std::fs::File::create(&log_path).context("Could not create forge-installer.log")?;
            let log_stderr = log
                .try_clone()
                .context("Could not create forge-installer.log")?;
            let status = dont_spawn_terminal(
                Command::new(&jre)
                    .arg("-jar")
                    .arg(path_to_instance.join("forge-installer.jar"))
                    .arg("--installServer")
                    .arg(&path_to_instance)
                    .current_dir(&path_to_instance),
            )
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_stderr))
            .stdin(Stdio::null())
            .spawn()
            .context("Failed to start forge-installer.jar")?
            .wait()
            .await
            .context("forge-installer.jar failed")?;
            if !status.success() {
                return Err(eyre!(
                    "Failed to install forge server ({status}). The end of {}:\n{}",
                    log_path.display(),
                    tail_of_file(&log_path, 20).await
                )
                .into());
            }

            tokio::fs::write(
                &path_to_instance.join("user_jvm_args.txt"),
                "# Generated by Lodestone\n# This file is ignored by Lodestone\n# Please set arguments using Lodestone",
            )
            .await
            .context("Could not create user_jvm_args.txt")?;
        }

        // Step 4: Finishing Up
        event_broadcaster.send(Event::new_progression_event_update(
            progression_event_id,
            "4/4: Finishing up",
            1.0,
        ));

        let restore_config = RestoreConfig {
            name: config.name,
            version: config.version,
            flavour,
            description: config.description.unwrap_or_default(),
            cmd_args: config.cmd_args,
            port: config.port,
            min_ram: config.min_ram.unwrap_or(2048),
            max_ram: config.max_ram.unwrap_or(4096),
            auto_start: config.auto_start.unwrap_or(false),
            restart_on_crash: config.restart_on_crash.unwrap_or(false),
            backup_period: config.backup_period,
            jre_major_version,
            has_started: false,
            java_cmd: Some(jre.to_string_lossy().to_string()),
        };
        // create config file
        write_file_atomically(
            &path_to_config,
            to_string_pretty(&restore_config)
                .context("Failed to serialize config to string. This is a bug, please report it.")?
                .into_bytes(),
        )
        .await
        .context(format!(
            "Failed to write config file at {}",
            path_to_config.display()
        ))?;
        MinecraftInstance::restore(
            path_to_instance,
            dot_lodestone_config,
            event_broadcaster,
            macro_executor,
        )
        .await
    }

    pub async fn restore(
        path_to_instance: PathBuf,
        dot_lodestone_config: DotLodestoneConfig,
        event_broadcaster: EventBroadcaster,
        macro_executor: MacroExecutor,
    ) -> Result<MinecraftInstance, Error> {
        let path_to_config = path_to_instance.join(".lodestone_minecraft_config.json");
        let config_bytes = tokio::fs::read(&path_to_config).await.context(format!(
            "Failed to open config file at {}",
            path_to_config.display()
        ))?;
        let restore_config: RestoreConfig = serde_json::from_slice(&config_bytes).context(
            "Failed to deserialize config from string. Was the config file modified manually?",
        )?;
        let path_to_macros = path_to_instance.join("macros");
        let path_to_properties = path_to_instance.join("server.properties");
        let path_to_runtimes = path_to_binaries().clone();
        // if the properties file doesn't exist, create it
        if !tokio::fs::try_exists(&path_to_properties)
            .await
            .unwrap_or(false)
        {
            tokio::fs::write(
                &path_to_properties,
                format!("server-port={}\n", restore_config.port),
            )
            .await
            .context(format!("Failed to create {}", path_to_properties.display()))?;
        };
        // a custom java command set by the user wins over the managed JRE
        let java_cmd = restore_config.java_cmd.clone().unwrap_or_else(|| {
            java_binary(&path_to_runtimes, restore_config.jre_major_version)
                .to_string_lossy()
                .to_string()
        });

        let configurable_manifest = Arc::new(Mutex::new(Self::init_configurable_manifest(
            &restore_config,
            java_cmd,
        )));

        let instance = MinecraftInstance {
            state: Arc::new(Mutex::new(State::Stopped)),
            uuid: dot_lodestone_config.uuid().clone(),
            creation_time: dot_lodestone_config.creation_time(),
            players_manager: Arc::new(Mutex::new(PlayersManager::new(
                event_broadcaster.clone(),
                dot_lodestone_config.uuid().clone(),
            ))),
            config: Arc::new(Mutex::new(restore_config)),
            path_to_instance,
            path_to_config,
            path_to_properties,
            path_to_macros,
            macro_executor,
            event_broadcaster,
            path_to_runtimes,
            process: Arc::new(Mutex::new(None)),
            stdin: Arc::new(Mutex::new(None)),
            stop_requested: Arc::new(AtomicBool::new(false)),
            version_change_lock: Arc::new(Mutex::new(())),
            properties_lock: Arc::new(Mutex::new(())),
            system: Arc::new(Mutex::new(crate::util::cpu_and_memory_system())),
            rcon_conn: Arc::new(Mutex::new(None)),
            configurable_manifest,
            macro_name_to_last_run: Arc::new(Mutex::new(HashMap::new())),
            pid_to_task_entry: Arc::new(Mutex::new(IndexMap::new())),
        };
        // A broken server.properties must not hide the instance.
        if let Err(e) = instance.refresh_properties_section().await {
            warn!(
                "Failed to read {}: {e}",
                instance.path_to_properties.display()
            );
        }
        Ok(instance)
    }

    async fn write_config_to_file(&self) -> Result<(), Error> {
        let content = to_string_pretty(&*self.config.lock().await)
            .context("Failed to serialize config to string, this is a bug, please report it")?;
        write_file_atomically(&self.path_to_config, content.into_bytes())
            .await
            .context(format!(
                "Failed to write config to file at {}",
                self.path_to_config.display()
            ))?;
        Ok(())
    }

    /// Reads server.properties. A missing file reads as empty; lines that can't be parsed
    /// are kept in the file and logged.
    pub(super) async fn load_properties(&self) -> Result<PropertiesDocument, Error> {
        let bytes = match tokio::fs::read(&self.path_to_properties).await {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                return Err(e)
                    .context(format!(
                        "Failed to read properties file at {}",
                        self.path_to_properties.display()
                    ))
                    .map_err(Into::into)
            }
        };
        let (doc, warnings) = PropertiesDocument::from_bytes(&bytes);
        for warning in warnings {
            warn!(
                "{} line {}: {}",
                self.path_to_properties.display(),
                warning.line_number,
                warning.message
            );
        }
        Ok(doc)
    }

    /// Rebuilds the server properties section of the manifest from the file, and syncs
    /// the port from it.
    pub(super) async fn refresh_properties_section(&self) -> Result<PropertiesDocument, Error> {
        let doc = self.load_properties().await?;
        let settings = server_properties::section_settings(&doc);
        self.configurable_manifest
            .lock()
            .await
            .replace_section_settings(server_properties::SECTION_ID, settings)?;
        self.sync_port_from(&doc).await;
        Ok(doc)
    }

    /// Keeps `config.port` (used by the port manager) equal to `server-port`, which can
    /// also be changed by editing server.properties by hand.
    pub(super) async fn sync_port_from(&self, doc: &PropertiesDocument) {
        let Some(port) = doc
            .get("server-port")
            .and_then(|p| p.trim().parse::<u16>().ok())
            .filter(|p| *p != 0)
        else {
            return;
        };
        let changed = {
            let mut config = self.config.lock().await;
            let changed = config.port != port as u32;
            config.port = port as u32;
            changed
        };
        if changed {
            if let Err(e) = self.write_config_to_file().await {
                warn!("Failed to save the new port to the instance config: {e}");
            }
        }
    }

    /// Validates `value` against the property's spec and writes it to server.properties.
    /// Only that line of the file changes.
    pub(super) async fn set_server_property(
        &self,
        key: &str,
        value: &ConfigurableValue,
        insert_if_missing: bool,
    ) -> Result<(), Error> {
        let raw = server_properties::serialize_value(key, value)?;
        {
            let _guard = self.properties_lock.lock().await;
            let mut doc = self.load_properties().await?;
            if !insert_if_missing && !doc.contains_key(key) {
                return Err(Error {
                    kind: ErrorKind::NotFound,
                    source: eyre!("Setting not found"),
                });
            }
            doc.set(key, &raw);
            write_file_atomically(&self.path_to_properties, doc.to_bytes()).await?;
        }
        self.refresh_properties_section().await?;
        Ok(())
    }

    pub fn get_rcon(&self) -> Arc<Mutex<Option<rcon::Connection<tokio::net::TcpStream>>>> {
        self.rcon_conn.clone()
    }

    pub async fn send_rcon(&self, cmd: &str) -> Result<String, Error> {
        let a = self
            .rcon_conn
            .lock()
            .await
            .as_mut()
            .ok_or_else(|| {
                eyre!("Failed to send rcon command, rcon connection is not initialized")
            })?
            .cmd(cmd)
            .await
            .context("Failed to send rcon command")?;
        Ok(a)
    }
}

impl TInstance for MinecraftInstance {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_names() {
        assert!(validate_new_instance_name("My Server 1.20").is_ok());
        assert!(validate_new_instance_name("Überserver ✨").is_ok());
        assert!(validate_new_instance_name("").is_err());
        assert!(validate_new_instance_name("   ").is_err());
        assert!(validate_new_instance_name("../../x").is_err());
        assert!(validate_new_instance_name("/tmp/x").is_err());
        assert!(validate_new_instance_name("a\\b").is_err());
        assert!(validate_new_instance_name("line\nbreak").is_err());
        assert!(validate_new_instance_name(&"é".repeat(100)).is_ok());
        assert!(validate_new_instance_name(&"é".repeat(101)).is_err());
        // renaming only affects the display name
        assert!(validate_instance_name("Survival: hard").is_ok());
    }

    #[test]
    fn argument_splitting() {
        let s = |input: &str| split_args(input).unwrap();
        assert_eq!(s(""), Vec::<String>::new());
        assert_eq!(s("  -Xss4M   -Dfoo=bar "), vec!["-Xss4M", "-Dfoo=bar"]);
        assert_eq!(
            s(r#"-Dname="My Server" '-Dother=a b'"#),
            vec!["-Dname=My Server", "-Dother=a b"]
        );
        assert_eq!(s(r#""say \"hi\"""#), vec![r#"say "hi""#]);
        assert_eq!(s(r"-Dpath=C:\Games\mc"), vec![r"-Dpath=C:\Games\mc"]);
        assert!(split_args("\"unclosed").is_err());
    }

    #[test]
    fn argument_join_round_trips() {
        for args in [
            vec!["-Xss4M", "-Dfoo=bar"],
            vec!["-Dname=My Server", "it's", r#"say "hi""#, r"C:\a b\c"],
        ] {
            let args: Vec<String> = args.into_iter().map(String::from).collect();
            assert_eq!(split_args(&join_args(&args)).unwrap(), args);
        }
        assert_eq!(join_args(&["".to_string()]), "");
    }

    #[tokio::test]
    async fn atomic_write_replaces_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.properties");
        std::fs::write(&path, "old").unwrap();
        write_file_atomically(&path, b"new".to_vec()).await.unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new");
        // no temporary files are left behind
        assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
    }
}
