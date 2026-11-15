//! Setting up a new server's software: which server Lodestone installs for a Minecraft
//! version and loader, and installing it (Java, the server jar, a loader's installer).
//!
//! Every route that makes a server goes through [`ServerSetup`] (`AppState::server_setup`),
//! so the core's tests make servers with [`FakeSetup`] and never reach the network,
//! download a server jar or accept the EULA. [`Installers`] is the real one.

use std::path::PathBuf;

use async_trait::async_trait;
use color_eyre::eyre::eyre;

use crate::error::{Error, ErrorKind};
use crate::event_broadcaster::EventBroadcaster;
use crate::events::ProgressionEventID;
use crate::global_settings::CodeExecutionSwitch;
use crate::implementations::minecraft::util::get_server_jar_url;
use crate::implementations::minecraft::{
    FabricLoaderVersion, Flavour, FlavourKind, ForgeBuildVersion, MinecraftInstance,
    NeoForgeBuildVersion, SetupConfig,
};
use crate::types::DotLodestoneConfig;

/// Picks and installs a new server's software.
#[async_trait]
pub trait ServerSetup: Send + Sync {
    /// The software to install for Minecraft `version` with `kind`: `loader_version` when
    /// it can still be downloaded, else the newest for the version. A clear `BadRequest`
    /// when Lodestone can't make such a server at all, before anything is made.
    async fn resolve(
        &self,
        version: &str,
        kind: FlavourKind,
        loader_version: Option<&str>,
    ) -> Result<Flavour, Error>;

    /// Installs a new server into the existing folder `path` (Java, the server jar, a
    /// loader's installer, `eula.txt` as `config.accept_eula` says, `server.properties`
    /// with the port, Lodestone's settings) and reads it as an instance. Reports progress
    /// on `event_id`.
    async fn install(
        &self,
        config: SetupConfig,
        dot_lodestone_config: DotLodestoneConfig,
        path: PathBuf,
        event_id: &ProgressionEventID,
        event_broadcaster: EventBroadcaster,
        code_execution: CodeExecutionSwitch,
    ) -> Result<MinecraftInstance, Error>;
}

/// The real setup: Lodestone's managed Java and each server's own download or installer.
pub struct Installers;

#[async_trait]
impl ServerSetup for Installers {
    async fn resolve(
        &self,
        version: &str,
        kind: FlavourKind,
        loader_version: Option<&str>,
    ) -> Result<Flavour, Error> {
        let newest: Flavour = kind.into();
        if let Some(loader) = loader_version.filter(|v| !v.is_empty()) {
            let asked = flavour_with_loader(kind, Some(loader));
            if asked != newest && get_server_jar_url(version, &asked).await.is_ok() {
                return Ok(asked);
            }
        }
        get_server_jar_url(version, &newest).await.map_err(|e| {
            bad_request(format!(
                "Lodestone can't set up a {} server for Minecraft {version}: {}",
                newest.to_string(),
                e.source
            ))
        })?;
        Ok(newest)
    }

    async fn install(
        &self,
        config: SetupConfig,
        dot_lodestone_config: DotLodestoneConfig,
        path: PathBuf,
        event_id: &ProgressionEventID,
        event_broadcaster: EventBroadcaster,
        code_execution: CodeExecutionSwitch,
    ) -> Result<MinecraftInstance, Error> {
        MinecraftInstance::new(
            config,
            dot_lodestone_config,
            path,
            event_id,
            event_broadcaster,
            code_execution,
        )
        .await
    }
}

fn bad_request(message: String) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message),
    }
}

/// `kind` with the loader version to install, when one is asked for.
pub fn flavour_with_loader(kind: FlavourKind, loader_version: Option<&str>) -> Flavour {
    let loader = loader_version.map(str::to_owned);
    match kind {
        FlavourKind::Fabric => Flavour::Fabric {
            loader_version: loader.map(FabricLoaderVersion::new),
            installer_version: None,
        },
        FlavourKind::Forge => Flavour::Forge {
            build_version: loader.map(ForgeBuildVersion::new),
        },
        FlavourKind::NeoForge => Flavour::NeoForge {
            build_version: loader.map(NeoForgeBuildVersion::new),
        },
        other => other.into(),
    }
}

/// A [`ServerSetup`] for tests: no network, no Java, no real server jar. `install` writes
/// what an install leaves that Lodestone reads (its settings with no Java command, a stub
/// `server.jar`, `eula.txt` from `accept_eula`, `server.properties` with the port), the
/// `forge-installer.log` Forge's and NeoForge's installer writes, and
/// [`FakeSetup::extra_files`], and reads the instance; `resolve` takes the asked loader
/// version, else [`FakeSetup::NEWEST`].
#[cfg(test)]
#[derive(Default)]
pub struct FakeSetup {
    /// Makes every install fail, as a download that fails would.
    pub fail: std::sync::atomic::AtomicBool,
    /// The configs installs were asked for.
    pub installed: std::sync::Mutex<Vec<SetupConfig>>,
    /// More files every install writes (a path from the server folder, its bytes), as a
    /// loader's installer may.
    pub extra_files: std::sync::Mutex<Vec<(String, Vec<u8>)>>,
}

#[cfg(test)]
impl FakeSetup {
    /// The loader version `resolve` gives when none is asked for.
    pub const NEWEST: &'static str = "9.9.9";
}

#[cfg(test)]
#[async_trait]
impl ServerSetup for FakeSetup {
    async fn resolve(
        &self,
        _version: &str,
        kind: FlavourKind,
        loader_version: Option<&str>,
    ) -> Result<Flavour, Error> {
        if kind == FlavourKind::Spigot {
            return Err(bad_request(
                "Lodestone can't set up a spigot server".to_string(),
            ));
        }
        let loader = loader_version.filter(|v| !v.is_empty()).or(match kind {
            FlavourKind::Fabric | FlavourKind::Forge | FlavourKind::NeoForge => Some(Self::NEWEST),
            _ => None,
        });
        Ok(flavour_with_loader(kind, loader))
    }

    async fn install(
        &self,
        config: SetupConfig,
        dot_lodestone_config: DotLodestoneConfig,
        path: PathBuf,
        _event_id: &ProgressionEventID,
        event_broadcaster: EventBroadcaster,
        code_execution: CodeExecutionSwitch,
    ) -> Result<MinecraftInstance, Error> {
        use crate::implementations::minecraft::RestoreConfig;
        self.installed.lock().unwrap().push(config.clone());
        if self.fail.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(eyre!("The server jar couldn't be downloaded (a test's failure)").into());
        }
        MinecraftInstance::create_instance_files(&path, &config).await?;
        tokio::fs::write(path.join("server.jar"), b"fake server jar")
            .await
            .map_err(|e| eyre!("{e}"))?;
        let mut more = self.extra_files.lock().unwrap().clone();
        if matches!(
            config.flavour,
            Flavour::Forge { .. } | Flavour::NeoForge { .. }
        ) {
            more.push((
                "forge-installer.log".into(),
                b"the installer's log".to_vec(),
            ));
        }
        for (file, bytes) in more {
            let to = path.join(file);
            if let Some(dir) = to.parent() {
                tokio::fs::create_dir_all(dir)
                    .await
                    .map_err(|e| eyre!("{e}"))?;
            }
            tokio::fs::write(to, bytes)
                .await
                .map_err(|e| eyre!("{e}"))?;
        }
        let restore = RestoreConfig {
            name: config.name,
            version: config.version,
            flavour: config.flavour,
            description: config.description.unwrap_or_default(),
            cmd_args: config.cmd_args,
            java_cmd: None,
            port: config.port,
            min_ram: config.min_ram.unwrap_or(2048),
            max_ram: config.max_ram.unwrap_or(4096),
            auto_start: config.auto_start.unwrap_or(false),
            restart_on_crash: config.restart_on_crash.unwrap_or(false),
            backup_period: config.backup_period,
            jre_major_version: 21,
            has_started: false,
            modpack: config.modpack,
        };
        tokio::fs::write(
            path.join(".lodestone_minecraft_config.json"),
            serde_json::to_vec_pretty(&restore).map_err(|e| eyre!("{e}"))?,
        )
        .await
        .map_err(|e| eyre!("{e}"))?;
        MinecraftInstance::restore(
            path,
            dot_lodestone_config,
            event_broadcaster,
            code_execution,
        )
        .await
    }
}
