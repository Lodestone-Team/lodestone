//! Changing what a Minecraft instance runs: its Minecraft version and server software
//! (`change_version`, and a modpack update that moves to another Minecraft or loader
//! version), and the modpack it came from.

use color_eyre::eyre::{eyre, Context};
use tokio::sync::OwnedMutexGuard;
use tracing::{info, warn};

use crate::error::{Error, ErrorKind};
use crate::modpack::ModpackOrigin;
use crate::prelude::path_to_tmp;
use crate::traits::t_configurable::manifest::ConfigurableValue;
use crate::traits::t_server::State;
use crate::util::download_file;

use super::configurable::CmdArgSetting;
use super::util::{get_jre_major_version, get_server_jar_url};
use super::{
    ensure_jre, java_binary, run_server_installer, Flavour, MinecraftInstance, INSTALLER_JAR,
};

impl MinecraftInstance {
    /// Holds off `start()` until the guard is dropped. Fails if the server is not stopped,
    /// or another change holds it.
    pub async fn lock_stopped(&self, what: &str) -> Result<OwnedMutexGuard<()>, Error> {
        let guard = self
            .version_change_lock
            .clone()
            .try_lock_owned()
            .map_err(|_| Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("The server is busy with another change; try again when it is done"),
            })?;
        if *self.state.lock().await != State::Stopped {
            return Err(Error {
                kind: ErrorKind::BadRequest,
                source: eyre!("Stop the server before {what}"),
            });
        }
        Ok(guard)
    }

    /// The Minecraft version and the server software.
    pub async fn server_software(&self) -> (String, Flavour) {
        let config = self.config.lock().await;
        (config.version.clone(), config.flavour.clone())
    }

    /// The modpack the instance was created from, or last updated to.
    pub async fn modpack_origin(&self) -> Option<ModpackOrigin> {
        self.config.lock().await.modpack.clone()
    }

    pub async fn set_modpack_origin(&self, origin: Option<ModpackOrigin>) -> Result<(), Error> {
        self.config.lock().await.modpack = origin;
        self.write_config_to_file().await
    }

    /// Installs `flavour` (with its loader or build version set) for Minecraft `version`
    /// and saves it in the config: the Java it needs, then `server.jar`, or for Forge and
    /// NeoForge their installer, which is run. The caller holds [`Self::lock_stopped`].
    /// `progress` gets a message for each step.
    pub async fn install_server_software(
        &self,
        version: String,
        flavour: Flavour,
        progress: &(dyn Fn(String) + Send + Sync),
    ) -> Result<(), Error> {
        let flavour_name = flavour.to_string();
        let new_jre = get_jre_major_version(&version)
            .await
            .with_context(|| format!("Could not find the Java version for Minecraft {version}"))?;
        progress(format!("Getting Java {new_jre}"));
        ensure_jre(&self.path_to_runtimes, new_jre, &|_| {}).await?;
        let (url, resolved) = get_server_jar_url(&version, &flavour)
            .await
            .with_context(|| format!("Could not find a {flavour_name} server for {version}"))?;
        progress(format!("Downloading the {flavour_name} server"));
        let temp_dir = tempfile::tempdir_in(path_to_tmp()).context("Failed to create temp dir")?;
        let file_name = if resolved.uses_installer() {
            INSTALLER_JAR
        } else {
            "server.jar"
        };
        download_file(&url, temp_dir.path(), Some(file_name), &|_| {}, true).await?;
        crate::util::fs::rename(
            temp_dir.path().join(file_name),
            self.path_to_instance.join(file_name),
        )
        .await?;
        if resolved.uses_installer() {
            progress(format!("Installing the {flavour_name} server"));
            run_server_installer(
                &java_binary(&self.path_to_runtimes, new_jre),
                &self.path_to_instance,
                &flavour_name,
            )
            .await?;
        }
        info!(
            "[{}] Now runs {flavour_name} for Minecraft {version}",
            self.config.lock().await.name
        );
        self.set_server_software(version, resolved, new_jre).await
    }

    /// Saves a new Minecraft version and server software, and the Java it needs. The Java
    /// command moves to the new Java unless the user set their own.
    pub(super) async fn set_server_software(
        &self,
        version: String,
        flavour: Flavour,
        new_jre: u64,
    ) -> Result<(), Error> {
        let new_java_cmd = {
            let mut config = self.config.lock().await;
            let old_default = java_binary(&self.path_to_runtimes, config.jre_major_version)
                .to_string_lossy()
                .to_string();
            config.version = version;
            config.flavour = flavour;
            config.jre_major_version = new_jre;
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
}
