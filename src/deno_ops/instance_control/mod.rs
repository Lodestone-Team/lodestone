use std::collections::HashSet;

use deno_core::{
    anyhow::{self, bail, Context},
    op,
};

use crate::{
    events::CausedBy,
    macro_executor::MacroPID,
    prelude::app_state,
    traits::{
        t_configurable::{Game, TConfigurable},
        t_player::{Player, TPlayerManagement},
        t_server::{MonitorReport, State, TServer},
    },
    types::InstanceUuid,
};

#[op]
async fn start_instance(
    instance_uuid: InstanceUuid,
    task_pid: MacroPID,
    block: bool,
) -> Result<(), anyhow::Error> {
    let mut instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    instance
        .start(
            CausedBy::Macro {
                macro_pid: task_pid,
            },
            block,
        )
        .await
        .context("Failed to start instance")
}

#[op]
async fn stop_instance(
    instance_uuid: InstanceUuid,
    task_pid: MacroPID,
    block: bool,
) -> Result<(), anyhow::Error> {
    let mut instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    instance
        .stop(
            CausedBy::Macro {
                macro_pid: task_pid,
            },
            block,
        )
        .await
        .context("Failed to start instance")
}

#[op]
async fn restart_instance(
    instance_uuid: InstanceUuid,
    task_pid: MacroPID,
    block: bool,
) -> Result<(), anyhow::Error> {
    let mut instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    instance
        .restart(
            CausedBy::Macro {
                macro_pid: task_pid,
            },
            block,
        )
        .await
        .context("Failed to start instance")
}

#[op]
async fn kill_instance(
    instance_uuid: InstanceUuid,
    task_pid: MacroPID,
) -> Result<(), anyhow::Error> {
    let mut instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    instance
        .kill(CausedBy::Macro {
            macro_pid: task_pid,
        })
        .await
        .context("Failed to start instance")
}

#[op]
async fn get_instance_state(instance_uuid: InstanceUuid) -> Result<State, anyhow::Error> {
    let instance = app_state()
        .instances
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;

    Ok(instance.state().await)
}

#[op]
async fn send_command(
    instance_uuid: InstanceUuid,
    command: String,
    task_pid: MacroPID,
) -> Result<(), anyhow::Error> {
    let instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    instance
        .send_command(
            &command,
            CausedBy::Macro {
                macro_pid: task_pid,
            },
        )
        .await
        .context("Failed to start instance")
}

#[op]
async fn monitor_instance(instance_uuid: InstanceUuid) -> Result<MonitorReport, anyhow::Error> {
    let instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.monitor().await)
}

#[op]
async fn get_instance_player_count(instance_uuid: InstanceUuid) -> Result<u32, anyhow::Error> {
    let instance = app_state()
        .instances
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.get_player_count().await?)
}

#[op]
async fn get_instance_max_players(instance_uuid: InstanceUuid) -> Result<u32, anyhow::Error> {
    let instance = app_state()
        .instances
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.get_max_player_count().await?)
}

#[op]
async fn get_instance_player_list(
    instance_uuid: InstanceUuid,
) -> Result<HashSet<Player>, anyhow::Error> {
    let instance = app_state()
        .instances
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.get_player_list().await?)
}

#[op]
async fn get_instance_name(instance_uuid: InstanceUuid) -> Result<String, anyhow::Error> {
    let instance = app_state()
        .instances
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.name().await)
}

#[op]
async fn get_instance_game(instance_uuid: InstanceUuid) -> Result<Game, anyhow::Error> {
    let instance = app_state()
        .instances
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.game_type().await)
}

#[op]
async fn get_instance_game_version(instance_uuid: InstanceUuid) -> Result<String, anyhow::Error> {
    let instance = app_state()
        .instances
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.version().await)
}

#[op]
async fn get_instance_description(instance_uuid: InstanceUuid) -> Result<String, anyhow::Error> {
    let instance = app_state()
        .instances
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.description().await)
}

#[op]
async fn get_instance_port(instance_uuid: InstanceUuid) -> Result<u32, anyhow::Error> {
    let instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.port().await)
}

#[op]
async fn get_instance_path(instance_uuid: InstanceUuid) -> Result<String, anyhow::Error> {
    let instance = app_state()
        .instances
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.path().await.to_string_lossy().to_string())
}

#[op]
async fn set_instance_name(instance_uuid: InstanceUuid, name: String) -> Result<(), anyhow::Error> {
    let mut instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;

    instance
        .set_name(name)
        .await
        .context("Failed to set instance name")
}

#[op]
async fn set_instance_description(
    instance_uuid: InstanceUuid,
    description: String,
) -> Result<(), anyhow::Error> {
    let mut instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;

    instance
        .set_description(description)
        .await
        .context("Failed to set instance description")
}

#[op]
async fn set_instance_port(instance_uuid: InstanceUuid, port: u32) -> Result<(), anyhow::Error> {
    let mut instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;

    instance
        .set_port(port)
        .await
        .context("Failed to set instance port")
}

#[op]
async fn set_instance_auto_start(
    instance_uuid: InstanceUuid,
    auto_start: bool,
) -> Result<(), anyhow::Error> {
    let mut instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;

    instance
        .set_auto_start(auto_start)
        .await
        .context("Failed to set instance auto start")
}

#[op]
async fn is_rcon_available(instance_uuid: InstanceUuid) -> Result<bool, anyhow::Error> {
    let instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    match instance.value() {
        crate::prelude::GameInstance::MinecraftInstance(v) => Ok(v.is_rcon_available().await),
        crate::prelude::GameInstance::GenericInstance(_) => {
            bail!("RCON not available for atom instances")
        }
    }
}

#[op]
async fn send_rcon_command(
    instance_uuid: InstanceUuid,
    command: String,
) -> Result<String, anyhow::Error> {
    let instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    match instance.value() {
        crate::prelude::GameInstance::MinecraftInstance(v) => Ok(v.send_rcon(&command).await?),
        crate::prelude::GameInstance::GenericInstance(_) => {
            bail!("RCON not available for atom instances")
        }
    }
}

#[op]
async fn wait_till_rcon_available(instance_uuid: InstanceUuid) -> Result<(), anyhow::Error> {
    let instance = app_state()
        .instances
        .get_mut(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    match instance.value() {
        crate::prelude::GameInstance::MinecraftInstance(v) => loop {
            if v.is_rcon_available().await {
                break Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        },
        crate::prelude::GameInstance::GenericInstance(_) => {
            bail!("RCON not available for atom instances")
        }
    }
}

pub fn register_instance_control_ops(worker_options: &mut deno_runtime::worker::WorkerOptions) {
    worker_options.extensions.push(
        deno_core::Extension::builder("prelude_ops")
            .ops(vec![
                get_instance_state::decl(),
                get_instance_path::decl(),
                get_instance_name::decl(),
                get_instance_player_count::decl(),
                get_instance_max_players::decl(),
                get_instance_player_list::decl(),
                get_instance_game::decl(),
                get_instance_game_version::decl(),
                get_instance_description::decl(),
                get_instance_port::decl(),
                set_instance_name::decl(),
                set_instance_description::decl(),
                set_instance_port::decl(),
                set_instance_auto_start::decl(),
                start_instance::decl(),
                stop_instance::decl(),
                restart_instance::decl(),
                monitor_instance::decl(),
                send_command::decl(),
                kill_instance::decl(),
                is_rcon_available::decl(),
                send_rcon_command::decl(),
                wait_till_rcon_available::decl(),
            ])
            .build(),
    );
}
