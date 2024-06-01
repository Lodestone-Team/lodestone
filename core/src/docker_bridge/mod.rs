mod virtual_fs;

use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use bollard::container::{AttachContainerOptions, AttachContainerResults, ListContainersOptions};
use bollard::{secret::EventMessage, system::EventsOptions, Docker};
use color_eyre::eyre::{Context, ContextCompat};
use futures::AsyncWrite;
use tokio::fs::create_dir_all;
use tokio::sync::broadcast::Sender;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use tracing::{error, event};
use virtual_fs::{get_virtual_path, to_virtual_path};

use crate::handlers::global_fs::FileEntry;
use crate::traits::t_configurable::Game::Generic;
use crate::util::{list_dir, scoped_join_win_safe};
use crate::{
    error::Error,
    event_broadcaster::{self, EventBroadcaster},
    events::Event,
    traits::{t_configurable::GameType, t_server::State, InstanceInfo},
    types::InstanceUuid,
};

#[derive(Debug, Clone)]
pub struct DockerBridge {
    docker: Docker,
    event_broadcaster: EventBroadcaster,
    watch_list: Arc<RwLock<HashSet<InstanceUuid>>>,
    working_dir: PathBuf,
}

fn is_lodestone_managed(labels: &HashMap<String, String>) -> bool {
    labels
        .get("lodestone_managed")
        .map(|v| v == "basic" || v == "full")
        .unwrap_or(false)
}

fn event_filter(
    event: &std::result::Result<bollard::secret::EventMessage, bollard::errors::Error>,
) -> bool {
    if event.is_err() {
        return false;
    }
    let event = event.as_ref().unwrap();
    event
        .actor
        .as_ref()
        .and_then(|actor| {
            actor
                .attributes
                .as_ref()
                .map(|attr| is_lodestone_managed(attr))
        })
        .unwrap_or(false)
}

fn docker_id_to_uuid(name: &str) -> InstanceUuid {
    format!("DOCKER-{}", name).into()
}

fn extract_container_id(event: &EventMessage) -> Option<String> {
    event.actor.as_ref().and_then(|actor| actor.id.clone())
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
    let instance_uuid = docker_id_to_uuid(&extract_container_name(&event)?);

    let instance_name = extract_container_name(&event)?;
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

impl DockerBridge {
    pub async fn new(
        event_broadcaster: EventBroadcaster,
        working_dir: PathBuf,
    ) -> Result<Self, Error> {
        let docker =
            Docker::connect_with_local_defaults().context("Failed to connect to docker")?;
        create_dir_all(&working_dir)
            .await
            .context("Failed to create working directory")?;
        // read or create watch_list.json
        let watch_list_file = working_dir.join("watch_list.json");
        let watch_list = if watch_list_file.exists() {
            let watch_list = tokio::fs::read_to_string(&watch_list_file)
                .await
                .context("Failed to read watch list")?;
            serde_json::from_str(&watch_list).context("Failed to parse watch list")?
        } else {
            tokio::fs::write(&watch_list_file, "[]")
                .await
                .context("Failed to create watch list file")?;
            HashSet::new()
        };

        dbg!(&watch_list);

        let watch_list = Arc::new(RwLock::new(watch_list));
        tokio::spawn({
            let docker = docker.clone();
            let event_broadcaster = event_broadcaster.clone();
            let watch_list = watch_list.clone();
            async move {
                let mut stream = docker.events(None::<EventsOptions<String>>);
                while let Some(event) = stream.next().await {
                    match event {
                        Ok(event) => {
                            if !watch_list.read().await.contains(&docker_id_to_uuid(
                                &extract_container_name(&event).unwrap(),
                            )) {
                                continue;
                            }
                            if let Some(lodestone_event) = docker_event_to_lodestone_event(event) {
                                event_broadcaster.send(lodestone_event);
                            }
                        }
                        Err(e) => {
                            error!("Error while listening to docker events: {:?}", e);
                        }
                    }
                }
            }
        });
        Ok(Self {
            docker,
            working_dir,
            event_broadcaster,
            watch_list,
        })
    }

    pub async fn list_containers(&self) -> Result<Vec<InstanceInfo>, Error> {
        let mut ret = Vec::new();
        let containers = self
            .docker
            .list_containers(Some(ListContainersOptions::<String> {
                all: true,
                ..Default::default()
            }))
            .await
            .context("Failed to list containers")?;
        for container in containers {
            let name = container.names.unwrap()[0].clone().replace('/', "");
            let uuid = docker_id_to_uuid(&name);
            if !self.watch_list.read().await.contains(&uuid) {
                continue;
            }
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
                state: State::from_docker_state(&container.state.unwrap()),
                player_count: None,
                max_player_count: None,
                player_list: None,
            };
            ret.push(instance);
        }
        Ok(ret)
    }

    pub async fn stop_container(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let name = uuid.to_string().replace("DOCKER-", "");
        self.docker
            .stop_container(&name, None::<bollard::container::StopContainerOptions>)
            .await
            .context("Failed to stop container")?;
        Ok(())
    }

    pub async fn start_container(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let name = uuid.to_string().replace("DOCKER-", "");
        self.docker
            .start_container(
                &name,
                None::<bollard::container::StartContainerOptions<String>>,
            )
            .await
            .context("Failed to start container")?;
        let options = Some(AttachContainerOptions::<String> {
            stdin: Some(true),
            stdout: Some(true),
            stderr: Some(true),
            stream: Some(true),
            logs: Some(false),
            detach_keys: None,
        });
        let AttachContainerResults {
            mut output,
            mut input,
        } = self.docker.attach_container(&name, options).await.unwrap();

        let event_broadcaster = self.event_broadcaster.clone();
        tokio::spawn({
            let name = name.clone();
            let uuid = uuid.clone();
            async move {
                while let Some(Ok(output)) = output.next().await {
                    dbg!(output.to_string());
                    let name = name.clone();
                    let uuid = uuid.clone();
                    event_broadcaster.send(Event::new_instance_output(
                        uuid.clone(),
                        name,
                        output.to_string(),
                    ));
                    output.to_string();
                }
            }
        });

        Ok(())
    }

    pub async fn restart_container(&self, uuid: &InstanceUuid) -> Result<(), Error> {
        let name = uuid.to_string().replace("DOCKER-", "");
        self.docker
            .restart_container(&name, None::<bollard::container::RestartContainerOptions>)
            .await
            .context("Failed to restart container")?;
        Ok(())
    }

    pub async fn add_to_watch_list(&self, name: String) -> Result<(), Error> {
        let mut watch_list = self.watch_list.write().await;
        watch_list.insert(docker_id_to_uuid(&name));
        Ok(())
    }

    pub async fn list_files(
        &self,
        uuid: &InstanceUuid,
        relative_path: PathBuf,
    ) -> Result<Vec<FileEntry>, Error> {
        let name = uuid.to_string().replace("DOCKER-", "");
        let mounts = &self
            .docker
            .inspect_container(&name, None)
            .await
            .context("Failed to inspect container")?
            .mounts;
        if mounts.is_none() {
            return Ok(Vec::new());
        }
        let mounts = mounts.as_ref().unwrap();
        if mounts.is_empty() {
            return Ok(Vec::new());
        }
        let safe_relative_path = scoped_join_win_safe("/", &relative_path)?;
        if safe_relative_path == PathBuf::from("/") {
            return Ok(mounts
                .iter()
                .filter_map(|m| {
                    let path = PathBuf::from(m.clone().source?);
                    let mut r: FileEntry = path.as_path().into();
                    r.path = path.components().last()?.as_os_str().to_str()?.to_owned();
                    Some(r)
                })
                .collect());
        }
        let virtual_roots: Vec<PathBuf> = mounts
            .iter()
            .filter_map(|m| m.source.as_ref())
            .map(PathBuf::from)
            .collect();
        let (path, v_root, mount_point) = get_virtual_path(&virtual_roots, &relative_path).unwrap();
        dbg!(&path, &mount_point);
        let ret: Vec<FileEntry> = list_dir(&path, None)
            .await?
            .iter()
            .filter_map(move |p| -> Option<FileEntry> {
                // remove the root path from the file path
                let mut r: FileEntry = p.as_path().into();
                r.path = to_virtual_path(p, &v_root, &mount_point)
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .to_string();
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
        let name = uuid.to_string().replace("DOCKER-", "");
        let mounts = &self
            .docker
            .inspect_container(&name, None)
            .await
            .context("Failed to inspect container")?
            .mounts;
        if mounts.is_none() {
            return Ok("".to_string());
        }
        let mounts = mounts.as_ref().unwrap();
        if mounts.is_empty() {
            return Ok("".to_string());
        }
        let virtual_roots: Vec<PathBuf> = mounts
            .iter()
            .filter_map(|m| m.source.as_ref())
            .map(PathBuf::from)
            .collect();
        let (path, ..) = get_virtual_path(&virtual_roots, &relative_path).unwrap();


        let content = tokio::fs::read_to_string(&path)
            .await
            .context("Failed to read file")?;
        Ok(content)
    }

    pub async fn write_container_file(
        &self,
        uuid: &InstanceUuid,
        relative_path: PathBuf,
        content: &[u8],
    ) -> Result<(), Error> {
        let name = uuid.to_string().replace("DOCKER-", "");
        let mounts = &self
            .docker
            .inspect_container(&name, None)
            .await
            .context("Failed to inspect container")?
            .mounts;
        if mounts.is_none() {
            return Ok(());
        }
        let mounts = mounts.as_ref().unwrap();
        if mounts.is_empty() {
            return Ok(());
        }
        let virtual_roots: Vec<PathBuf> = mounts
        .iter()
        .filter_map(|m| m.source.as_ref())
        .map(PathBuf::from)
        .collect();
    let (path, ..) = get_virtual_path(&virtual_roots, &relative_path).unwrap();

        tokio::fs::write(&path, content)
            .await
            .context("Failed to write file")?;
        Ok(())
    }
}
