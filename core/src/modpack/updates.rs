//! Checking for newer versions of the modpacks servers were made from.
//!
//! [`watch`] checks every server whose modpack is on Modrinth soon after the core starts
//! and then every [`CHECK_EVERY`], while the `check_modpack_updates` setting is on (it is
//! by default). "Check for an update" ([`check_instance`]) works either way. A newer version is one for the server's loader that
//! is newer than the server's (alphas aren't offered). The first time a version is
//! found, a `ModpackUpdateAvailable` event says so; [`NOTIFIED_FILE`] remembers it, so
//! a restart doesn't say it again.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tracing::{info, warn};
use ts_rs::TS;

use super::ModpackOrigin;
use crate::error::Error;
use crate::events::{CausedBy, Event, EventInner, InstanceEvent, InstanceEventInner};
use crate::implementations::minecraft::{Flavour, MinecraftInstance};
use crate::modrinth::ModrinthApi;
use crate::prelude::GameInstance;
use crate::traits::t_configurable::TConfigurable;
use crate::types::InstanceUuid;
use crate::AppState;
use lodestone_minecraft::modrinth::types::ModrinthVersionType;

/// How often the servers are checked.
pub const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// How long after the core starts the first check runs.
const FIRST_CHECK_AFTER: Duration = Duration::from_secs(2 * 60);
/// The version the last notification was about, in the instance folder.
pub const NOTIFIED_FILE: &str = ".lodestone_modpack/notified.json";

/// A newer version of a server's modpack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AvailableUpdate {
    pub version_id: String,
    pub version_number: String,
    /// The version's title on Modrinth.
    pub name: String,
    pub game_versions: Vec<String>,
    /// As Modrinth gives it (RFC 3339).
    pub date_published: String,
    /// What the modpack's author wrote about it (Markdown), if anything.
    pub changelog: Option<String>,
}

/// The newest versions found, by instance.
#[derive(Default)]
pub struct UpdateChecks {
    found: Mutex<HashMap<InstanceUuid, AvailableUpdate>>,
}

impl UpdateChecks {
    pub fn get(&self, uuid: &InstanceUuid) -> Option<AvailableUpdate> {
        self.found
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(uuid)
            .cloned()
    }

    fn set(&self, uuid: &InstanceUuid, update: Option<AvailableUpdate>) {
        let mut found = self.found.lock().unwrap_or_else(|e| e.into_inner());
        match update {
            Some(update) => found.insert(uuid.clone(), update),
            None => found.remove(uuid),
        };
    }
}

/// The Modrinth loader of a server's software, for the servers modpacks make.
fn modrinth_loader(flavour: &Flavour) -> Option<&'static str> {
    match flavour {
        Flavour::Fabric { .. } => Some("fabric"),
        Flavour::Forge { .. } => Some("forge"),
        Flavour::NeoForge { .. } => Some("neoforge"),
        _ => None,
    }
}

/// The newest version of `origin`'s modpack for the server's loader and its Minecraft
/// version `game_version` (or a newer one), if it is newer than the server's. A pack
/// that keeps a line for an older Minecraft version (14.x for 26.2 beside 15.x for
/// 26.3) never offers that line to a server on the newer one.
pub async fn newest_version(
    api: &dyn ModrinthApi,
    origin: &ModpackOrigin,
    flavour: &Flavour,
    game_version: &str,
) -> Result<Option<AvailableUpdate>, Error> {
    if origin.project_id.is_empty() {
        return Ok(None);
    }
    let Some(loader) = modrinth_loader(flavour) else {
        return Ok(None);
    };
    let versions = api
        .project_versions(&origin.project_id, &[loader.to_string()], &[])
        .await?;
    let for_this_or_newer = |v: &&lodestone_minecraft::modrinth::ModrinthVersion| {
        v.game_versions.iter().any(|g| {
            lodestone_minecraft::versions::compare_versions(g, game_version)
                != std::cmp::Ordering::Less
        })
    };
    let Some(newest) = versions
        .iter()
        .filter(for_this_or_newer)
        .find(|v| v.version_type != ModrinthVersionType::Alpha)
    else {
        return Ok(None);
    };
    if newest.id == origin.version_id {
        return Ok(None);
    }
    // the server's version may not be in the list (another loader, an alpha)
    let current_date = match versions.iter().find(|v| v.id == origin.version_id) {
        Some(current) => Some(current.date_published.clone()),
        None if origin.version_id.is_empty() => None,
        None => api
            .version(&origin.version_id)
            .await
            .ok()
            .map(|v| v.date_published),
    };
    if current_date.is_some_and(|date| newest.date_published <= date) {
        return Ok(None);
    }
    Ok(Some(AvailableUpdate {
        version_id: newest.id.clone(),
        version_number: newest.version_number.clone(),
        name: newest.name.clone(),
        game_versions: newest.game_versions.clone(),
        date_published: newest.date_published.clone(),
        changelog: newest.changelog.clone(),
    }))
}

#[derive(Default, Serialize, Deserialize)]
struct Notified {
    version_id: String,
}

/// Checks one server, remembers what it found, and sends the event the first time a
/// version is found.
pub async fn check_instance(
    state: &AppState,
    instance: &MinecraftInstance,
) -> Result<Option<AvailableUpdate>, Error> {
    let uuid = instance.uuid().await;
    let Some(origin) = instance.modpack_origin().await else {
        state.mods.updates.set(&uuid, None);
        return Ok(None);
    };
    let (game_version, flavour) = instance.server_software().await;
    let found = newest_version(
        state.mods.modrinth.as_ref(),
        &origin,
        &flavour,
        &game_version,
    )
    .await?;
    state.mods.updates.set(&uuid, found.clone());
    let Some(update) = &found else {
        return Ok(None);
    };
    let path = instance.path().await.join(NOTIFIED_FILE);
    let notified: Notified = tokio::fs::read(&path)
        .await
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default();
    if notified.version_id != update.version_id {
        info!(
            "[{}] {} {} is out",
            instance.name().await,
            origin.name,
            update.version_number
        );
        state.event_broadcaster.send(Event {
            details: String::new(),
            snowflake: Default::default(),
            event_inner: EventInner::InstanceEvent(InstanceEvent {
                instance_uuid: uuid.clone(),
                instance_name: instance.name().await,
                instance_event_inner: InstanceEventInner::ModpackUpdateAvailable {
                    modpack: origin.name.clone(),
                    version_id: update.version_id.clone(),
                    version_number: update.version_number.clone(),
                },
            }),
            caused_by: CausedBy::System,
        });
        let json = serde_json::to_vec(&Notified {
            version_id: update.version_id.clone(),
        })
        .unwrap_or_default();
        if let Some(parent) = path.parent() {
            let _ = tokio::fs::create_dir_all(parent).await;
        }
        if let Err(e) = crate::persist::write_atomic(&path, json, None).await {
            warn!("Could not remember the modpack update notification: {e}");
        }
    }
    Ok(found)
}

/// Forgets what was found for a server, after it was updated.
pub fn forget(state: &AppState, uuid: &InstanceUuid) {
    state.mods.updates.set(uuid, None);
}

/// Checks every server made from a Modrinth modpack, soon after the core starts and
/// then every [`CHECK_EVERY`], unless the setting is off.
pub async fn watch(state: AppState) {
    tokio::time::sleep(FIRST_CHECK_AFTER).await;
    loop {
        if !state.global_settings.lock().await.check_modpack_updates() {
            tokio::time::sleep(CHECK_EVERY).await;
            continue;
        }
        let instances: Vec<GameInstance> = state
            .instances
            .iter()
            .map(|entry| entry.value().clone())
            .collect();
        for instance in instances {
            let GameInstance::MinecraftInstance(instance) = instance;
            if let Err(e) = check_instance(&state, &instance).await {
                warn!(
                    "Could not check {} for a modpack update: {e}",
                    instance.name().await
                );
            }
        }
        tokio::time::sleep(CHECK_EVERY).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::implementations::minecraft::{FabricLoaderVersion, NeoForgeBuildVersion};
    use crate::modrinth::fake::{FakeModrinth, FakeVersion};

    fn origin(version_id: &str) -> ModpackOrigin {
        ModpackOrigin {
            project_id: "pack".into(),
            version_id: version_id.into(),
            name: "Pack".into(),
            version_number: None,
            icon_url: None,
        }
    }

    /// `newest_version` for a server on Minecraft 1.20.1.
    async fn newest(
        fake: &FakeModrinth,
        origin: &ModpackOrigin,
        flavour: &Flavour,
    ) -> Option<AvailableUpdate> {
        newest_version(fake, origin, flavour, "1.20.1")
            .await
            .unwrap()
    }

    fn fabric() -> Flavour {
        Flavour::Fabric {
            loader_version: Some(FabricLoaderVersion::new("0.15.11".into())),
            installer_version: None,
        }
    }

    #[tokio::test]
    async fn finds_the_newest_version_for_the_loader() {
        let fake = FakeModrinth::default();
        let version = |id: &str, date: &str| FakeVersion::new("pack", id, "p.mrpack").date(date);
        fake.add(version("v1", "2024-01-01T00:00:00Z"));
        fake.add(
            version("v2", "2024-02-01T00:00:00Z")
                .beta()
                .changelog("- Sodium 0.6.9"),
        );
        let mut alpha = version("v3", "2024-03-01T00:00:00Z");
        alpha.version.version_type = ModrinthVersionType::Alpha;
        fake.add(alpha);
        fake.add(version("neo", "2024-04-01T00:00:00Z").loaders(&["neoforge"]));

        // the newest for Fabric that isn't an alpha
        let found = newest(&fake, &origin("v1"), &fabric()).await.unwrap();
        assert_eq!(found.version_id, "v2");
        assert_eq!(found.version_number, "v2-number");
        assert_eq!(found.changelog.as_deref(), Some("- Sodium 0.6.9"));

        // nothing newer
        assert_eq!(newest(&fake, &origin("v2"), &fabric()).await, None);
        // the server is on the alpha, newer than any release
        assert_eq!(newest(&fake, &origin("v3"), &fabric()).await, None);
        // a NeoForge server only gets NeoForge versions
        let neoforge = Flavour::NeoForge {
            build_version: Some(NeoForgeBuildVersion::new("21.1.1".into())),
        };
        assert_eq!(
            newest(&fake, &origin("v1"), &neoforge)
                .await
                .unwrap()
                .version_id,
            "neo"
        );
        // a newer release for an older Minecraft version isn't offered
        let mut older =
            FakeVersion::new("pack", "old-line", "p.mrpack").date("2024-05-01T00:00:00Z");
        older.version.game_versions = vec!["1.19.4".into()];
        fake.add(older);
        assert_eq!(
            newest(&fake, &origin("v2"), &fabric()).await,
            None,
            "an older line is never offered"
        );
        // uploaded packs and plain servers aren't checked
        assert_eq!(newest(&fake, &origin(""), &Flavour::Vanilla).await, None);
        let mut uploaded = origin("");
        uploaded.project_id.clear();
        assert_eq!(newest(&fake, &uploaded, &fabric()).await, None);
    }
}
