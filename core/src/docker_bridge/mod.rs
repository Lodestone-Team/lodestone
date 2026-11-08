mod virtual_fs;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bollard::container::AttachContainerResults;
use bollard::models::EventMessage;
use bollard::query_parameters::{
    AttachContainerOptions, EventsOptions, KillContainerOptions, ListContainersOptions,
    RestartContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::Docker;
use color_eyre::eyre::{eyre, Context, ContextCompat};

use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tracing::{error, info, warn};
use virtual_fs::{get_virtual_path, to_virtual_path};

use crate::error::ErrorKind;
use crate::handlers::global_fs::{FileEntry, MAX_READ_BYTES};
use crate::persist::write_atomic;
use crate::traits::t_configurable::Game::Generic;
use crate::util::{list_dir, scoped_join_win_safe};
use crate::{
    error::Error,
    event_broadcaster::EventBroadcaster,
    events::Event,
    traits::{t_configurable::GameType, t_server::State, InstanceInfo},
    types::InstanceUuid,
};

const DOCKER_UUID_PREFIX: &str = "DOCKER-";

#[derive(Debug, Clone)]
pub struct DockerBridge {
    /// `None` when no Docker daemon is reachable at startup. bollard >= 0.16 checks for the
    /// socket eagerly, so a missing daemon must not stop Lodestone from starting.
    docker: Option<Docker>,
    event_broadcaster: EventBroadcaster,
    watch_list: Arc<RwLock<HashSet<InstanceUuid>>>,
    db_file_path: PathBuf,
}

fn docker_id_to_uuid(name: &str) -> InstanceUuid {
    format!("{DOCKER_UUID_PREFIX}{name}").into()
}

/// The container name for a Docker instance UUID (`DOCKER-<name>`).
fn container_name(uuid: &InstanceUuid) -> Result<&str, Error> {
    uuid.as_ref()
        .strip_prefix(DOCKER_UUID_PREFIX)
        .ok_or_else(|| Error {
            kind: ErrorKind::BadRequest,
            source: eyre!("{uuid} is not a Docker instance"),
        })
}

fn extract_container_name(event: &EventMessage) -> Option<String> {
    event.actor.as_ref().and_then(|actor| {
        actor
            .attributes
            .as_ref()
            .and_then(|attr| attr.get("name").map(|name| name.to_string()))
    })
}

fn docker_event_to_lodestone_event(event: EventMessage) -> Option<Event> {
    let instance_name = extract_container_name(&event)?;
    let instance_uuid = docker_id_to_uuid(&instance_name);
    Some(match event.action?.as_str() {
        "start" => {
            Event::new_instance_state_transition(instance_uuid, instance_name, State::Running)
        }
        "stop" => {
            Event::new_instance_state_transition(instance_uuid, instance_name, State::Stopping)
        }
        "die" => Event::new_instance_state_transition(instance_uuid, instance_name, State::Stopped),
        _ => return None,
    })
}

/// Reads the watch list. A missing or empty file is an empty list. A file that cannot be
/// parsed is moved aside (to `<file>.corrupt`) instead of being silently overwritten.
async fn load_watch_list(db_file_path: &Path) -> HashSet<InstanceUuid> {
    let contents = match tokio::fs::read(db_file_path).await {
        Ok(contents) => contents,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => {
            error!(
                "Failed to read Docker watch list at {}: {e}. Starting with an empty list",
                db_file_path.display()
            );
            return HashSet::new();
        }
    };
    if contents.iter().all(u8::is_ascii_whitespace) {
        info!("Creating new docker bridge db file");
        return HashSet::new();
    }
    match serde_json::from_slice(&contents) {
        Ok(watch_list) => watch_list,
        Err(e) => {
            let mut backup = db_file_path.as_os_str().to_owned();
            backup.push(".corrupt");
            error!(
                "Docker watch list at {} is corrupt ({e}); moving it to {} and starting with an empty list",
                db_file_path.display(),
                PathBuf::from(&backup).display()
            );
            if let Err(e) = tokio::fs::rename(db_file_path, &backup).await {
                error!("Failed to move the corrupt Docker watch list: {e}");
            }
            HashSet::new()
        }
    }
}

/// Forwards Docker events for watched containers. Reconnects when the event stream ends
/// (for example when the daemon restarts).
async fn forward_docker_events(
    docker: Docker,
    event_broadcaster: EventBroadcaster,
    watch_list: Arc<RwLock<HashSet<InstanceUuid>>>,
) {
    let options = EventsOptions {
        filters: Some(HashMap::from([(
            "type".to_string(),
            vec!["container".to_string()],
        )])),
        ..Default::default()
    };
    let mut retry_delay = Duration::from_secs(1);
    loop {
        let mut stream = docker.events(Some(options.clone()));
        while let Some(event) = stream.next().await {
            match event {
                Ok(event) => {
                    retry_delay = Duration::from_secs(1);
                    let Some(name) = extract_container_name(&event) else {
                        continue;
                    };
                    if !watch_list.read().await.contains(&docker_id_to_uuid(&name)) {
                        continue;
                    }
                    if let Some(lodestone_event) = docker_event_to_lodestone_event(event) {
                        event_broadcaster.send(lodestone_event);
                    }
                }
                Err(e) => {
                    error!("Error while listening to docker events: {:?}", e);
                    break;
                }
            }
        }
        warn!(
            "Docker event stream ended, reconnecting in {}s",
            retry_delay.as_secs()
        );
        tokio::time::sleep(retry_delay).await;
        retry_delay = (retry_delay * 2).min(Duration::from_secs(60));
    }
}

impl DockerBridge {
    pub async fn new(
        event_broadcaster: EventBroadcaster,
        db_file_path: PathBuf,
    ) -> Result<Self, Error> {
        let docker = match Docker::connect_with_local_defaults() {
            Ok(docker) => Some(docker),
            Err(e) => {
                warn!("Docker is not available, Docker instances are disabled: {e}");
                None
            }
        };
        let watch_list = Arc::new(RwLock::new(load_watch_list(&db_file_path).await));
        if let Some(docker) = docker.clone() {
            tokio::spawn(forward_docker_events(
                docker,
                event_broadcaster.clone(),
                watch_list.clone(),
            ));
        }
        let bridge = Self {
            docker,
            db_file_path,
            event_broadcaster,
            watch_list,
        };
        if !bridge.db_file_path.exists() {
            bridge.sync_watch_list().await?;
        }
        Ok(bridge)
    }

    /// A bridge that never talks to a Docker daemon, with the watch list in
    /// `db_file_path`. For tests.
    #[cfg(test)]
    pub async fn without_docker(
        event_broadcaster: EventBroadcaster,
        db_file_path: PathBuf,
    ) -> Self {
        let watch_list = Arc::new(RwLock::new(load_watch_list(&db_file_path).await));
        Self {
            docker: None,
            db_file_path,
            event_broadcaster,
            watch_list,
        }
    }

    fn docker(&self) -> Result<&Docker, Error> {
        Ok(self.docker.as_ref().context("Docker is not available")?)
    }

    /// Whether the container behind `uuid` is on the watch list.
    pub async fn is_watched(&self, uuid: &InstanceUuid) -> bool {
        self.watch_list.read().await.contains(uuid)
    }

    pub async fn list_containers(&self) -> Result<Vec<InstanceInfo>, Error> {
        let mut ret = Vec::new();
        let containers = self
            .docker()?
            .list_containers(Some(ListContainersOptions {
                all: true,
                ..Default::default()
            }))
            .await
            .context("Failed to list containers")?;
        for container in containers {
            let Some(name) = container
                .names
                .as_ref()
                .and_then(|names| names.first())
                .map(|name| name.replace('/', ""))
            else {
                continue;
            };
            let uuid = docker_id_to_uuid(&name);
            if !self.watch_list.read().await.contains(&uuid) {
                continue;
            }
            let state = container.state.as_ref().map_or(State::Error, |state| {
                State::from_docker_state_string(state.as_ref())
            });
            let instance = InstanceInfo {
                uuid,
                name,
                game_type: Generic {
                    game_name: GameType::Generic,
                    game_display_name: "Docker".to_string(),
                },
                description: "Docker container test".to_string(),
                version: "".to_string(),
                port: 0,
                creation_time: container.created.unwrap_or(0),
                path: "/no_peek/Volume".to_string(),
                auto_start: false,
                restart_on_crash: false,
                state,
                player_count: None,
                max_player_count: None,
                player_list: None,
                icon_url: None,
                modpack: None,
                loader_version: None,
                min_ram: None,
                max_ram: None,
            };
            ret.push(instance);
        }
        Ok(ret)
    }

    pub async fn get_container_state(&self, uuid: &InstanceUuid) -> Result<State, Error> {
        let name = container_name(uuid)?;
        let state = self
            .docker()?
            .inspect_container(name, None)
            .await
            .context("Failed to inspect container")?
            .state
            .context("Failed to get container state")?;
        Ok(State::from_container_state(&state))
    }

    pub async fn stop_container(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let name = container_name(uuid)?;
        self.docker()?
            .stop_container(name, None::<StopContainerOptions>)
            .await
            .context("Failed to stop container")?;
        Ok(())
    }

    pub async fn start_container(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let name = container_name(uuid)?.to_string();
        self.docker()?
            .start_container(&name, None::<StartContainerOptions>)
            .await
            .context("Failed to start container")?;
        let options = Some(AttachContainerOptions {
            stdin: true,
            stdout: true,
            stderr: true,
            stream: true,
            logs: false,
            detach_keys: None,
        });
        // The container is running at this point, so failing to attach (for example
        // because it exited right away) only means there is no console output.
        let AttachContainerResults { mut output, .. } =
            match self.docker()?.attach_container(&name, options).await {
                Ok(results) => results,
                Err(e) => {
                    warn!("Started container {name} but could not attach to its output: {e}");
                    return Ok(());
                }
            };

        let event_broadcaster = self.event_broadcaster.clone();
        let uuid = uuid.clone();
        tokio::spawn(async move {
            while let Some(Ok(output)) = output.next().await {
                event_broadcaster.send(Event::new_instance_output(
                    uuid.clone(),
                    name.clone(),
                    output.to_string(),
                ));
            }
        });

        Ok(())
    }

    pub async fn restart_container(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let name = container_name(uuid)?;
        self.docker()?
            .restart_container(name, None::<RestartContainerOptions>)
            .await
            .context("Failed to restart container")?;
        Ok(())
    }

    pub async fn kill_container(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let name = container_name(uuid)?;
        self.docker()?
            .kill_container(name, None::<KillContainerOptions>)
            .await
            .context("Failed to kill container")?;
        Ok(())
    }

    /// Saves the watch list with a crash-safe write (temporary file, then rename).
    async fn sync_watch_list(&self) -> Result<(), Error> {
        let contents = {
            let watch_list = self.watch_list.read().await;
            serde_json::to_vec(&*watch_list).context("Failed to serialize watch list")?
        };
        write_atomic(&self.db_file_path, contents, None)
            .await
            .context("Failed to write db file")?;
        Ok(())
    }

    pub async fn add_to_watch_list(&self, name: String) {
        let mut watch_list = self.watch_list.write().await;
        watch_list.insert(docker_id_to_uuid(&name));
        drop(watch_list);
        let _ = self.sync_watch_list().await.map_err(|e| {
            error!("Failed to add to watch list: {:?}", e);
        });
    }

    pub async fn remove_from_watch_list(&self, name: String) {
        let mut watch_list = self.watch_list.write().await;
        watch_list.remove(&docker_id_to_uuid(&name));
        drop(watch_list);
        let _ = self.sync_watch_list().await.map_err(|e| {
            error!("Failed to remove from watch list: {:?}", e);
        });
    }

    /// The host paths of the container's mounts. Empty if it has none.
    async fn mount_roots(&self, uuid: &InstanceUuid) -> Result<Vec<PathBuf>, Error> {
        let name = container_name(uuid)?;
        let mounts = self
            .docker()?
            .inspect_container(name, None)
            .await
            .context("Failed to inspect container")?
            .mounts
            .unwrap_or_default();
        Ok(mounts
            .into_iter()
            .filter_map(|m| m.source)
            .map(PathBuf::from)
            .collect())
    }

    /// Resolves a path of the form `<mount name>/<path inside the mount>` to a host path.
    /// Returns `(host path, mount root, mount name)`.
    fn resolve_virtual_path(
        roots: &[PathBuf],
        relative_path: &Path,
    ) -> Result<(PathBuf, PathBuf, String), Error> {
        get_virtual_path(roots, relative_path).ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: eyre!(
                "{} does not start with the name of one of the container's mounts",
                relative_path.display()
            ),
        })
    }

    pub async fn list_files(
        &self,
        uuid: &InstanceUuid,
        relative_path: PathBuf,
    ) -> Result<Vec<FileEntry>, Error> {
        let roots = self.mount_roots(uuid).await?;
        if roots.is_empty() {
            return Ok(Vec::new());
        }
        let safe_relative_path = scoped_join_win_safe("/", &relative_path)?;
        if safe_relative_path == Path::new("/") {
            return Ok(roots
                .iter()
                .filter_map(|path| {
                    let mut r: FileEntry = path.as_path().into();
                    r.path = path
                        .components()
                        .next_back()?
                        .as_os_str()
                        .to_str()?
                        .to_owned();
                    Some(r)
                })
                .collect());
        }
        let (path, v_root, mount_point) = Self::resolve_virtual_path(&roots, &relative_path)?;
        let ret: Vec<FileEntry> = list_dir(&path, None)
            .await?
            .iter()
            .filter_map(|p| {
                // remove the root path from the file path
                let mut r: FileEntry = p.as_path().into();
                r.path = to_virtual_path(p, &v_root, &mount_point)?
                    .to_string_lossy()
                    .into_owned();
                Some(r)
            })
            .collect();

        Ok(ret)
    }

    pub async fn read_container_file(
        &self,
        uuid: &InstanceUuid,
        relative_path: PathBuf,
    ) -> Result<String, Error> {
        let roots = self.mount_roots(uuid).await?;
        if roots.is_empty() {
            return Ok("".to_string());
        }
        let (path, ..) = Self::resolve_virtual_path(&roots, &relative_path)?;

        crate::util::fs::read_to_string_capped(&path, MAX_READ_BYTES).await
    }

    pub async fn write_container_file(
        &self,
        uuid: &InstanceUuid,
        relative_path: PathBuf,
        content: &[u8],
    ) -> Result<(), Error> {
        let roots = self.mount_roots(uuid).await?;
        if roots.is_empty() {
            return Ok(());
        }
        let (path, ..) = Self::resolve_virtual_path(&roots, &relative_path)?;

        tokio::fs::write(&path, content)
            .await
            .context("Failed to write file")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn container_name_strips_only_the_prefix() {
        let uuid: InstanceUuid = "DOCKER-my-DOCKER-box".to_string().into();
        assert_eq!(container_name(&uuid).unwrap(), "my-DOCKER-box");
        let uuid: InstanceUuid = "INSTANCE_abc".to_string().into();
        assert!(container_name(&uuid).is_err());
    }

    #[test]
    fn unknown_mount_is_an_error_not_a_panic() {
        let roots = [PathBuf::from("/data/world")];
        assert!(DockerBridge::resolve_virtual_path(&roots, Path::new("other/file")).is_err());
        assert!(DockerBridge::resolve_virtual_path(&roots, Path::new("/")).is_err());
        let (path, ..) =
            DockerBridge::resolve_virtual_path(&roots, Path::new("world/level.dat")).unwrap();
        assert_eq!(path, PathBuf::from("/data/world/level.dat"));
    }

    #[test]
    fn event_without_name_is_ignored() {
        let event = EventMessage {
            action: Some("start".to_string()),
            ..Default::default()
        };
        assert!(docker_event_to_lodestone_event(event).is_none());
    }

    #[tokio::test]
    async fn corrupt_watch_list_is_kept_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("docker_bridge.json");
        std::fs::write(&path, "{not json").unwrap();
        assert!(load_watch_list(&path).await.is_empty());
        assert!(!path.exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("docker_bridge.json.corrupt")).unwrap(),
            "{not json"
        );

        std::fs::write(&path, r#"["DOCKER-a"]"#).unwrap();
        let list = load_watch_list(&path).await;
        assert!(list.contains(&docker_id_to_uuid("a")));
    }
}
