use std::{borrow::Cow, cell::RefCell, collections::HashSet, rc::Rc};

use deno_core::{
    anyhow::{self, bail, Context},
    op2, Extension, OpState,
};

use super::{op_on_main, op_on_main_abortable, MainRuntime, OpError, Strict};

use crate::{
    events::CausedBy,
    macro_executor::MacroPID,
    prelude::{app_state, GameInstance},
    traits::{
        t_configurable::{Game, TConfigurable},
        t_player::{Player, TPlayerManagement},
        t_server::{MonitorReport, State, TServer},
    },
    types::InstanceUuid,
};

/// Looks up an instance, cloned out so the map entry is not locked while the op awaits.
fn get_instance(instance_uuid: &InstanceUuid) -> anyhow::Result<GameInstance> {
    app_state()
        .instances
        .get(instance_uuid)
        .map(|entry| entry.value().clone())
        .ok_or_else(|| anyhow::anyhow!("Instance not found"))
}

#[op2]
fn instance_exists(#[serde] instance_uuid: InstanceUuid) -> bool {
    app_state().instances.contains_key(&instance_uuid)
}

#[op2]
#[serde]
fn all_instances() -> Vec<InstanceUuid> {
    app_state()
        .instances
        .iter()
        .map(|entry| entry.key().clone())
        .collect()
}

#[op2]
async fn start_instance(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] task_pid: MacroPID,
    block: bool,
) -> Result<(), OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        instance
            .start(
                CausedBy::Macro {
                    macro_pid: task_pid,
                },
                block,
            )
            .await
            .context("Failed to start instance")
    })
    .await
}

#[op2]
async fn stop_instance(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] task_pid: MacroPID,
    block: bool,
) -> Result<(), OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        instance
            .stop(
                CausedBy::Macro {
                    macro_pid: task_pid,
                },
                block,
            )
            .await
            .context("Failed to stop instance")
    })
    .await
}

#[op2]
async fn restart_instance(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] task_pid: MacroPID,
    block: bool,
) -> Result<(), OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        instance
            .restart(
                CausedBy::Macro {
                    macro_pid: task_pid,
                },
                block,
            )
            .await
            .context("Failed to restart instance")
    })
    .await
}

#[op2]
async fn kill_instance(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] task_pid: MacroPID,
) -> Result<(), OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        instance
            .kill(CausedBy::Macro {
                macro_pid: task_pid,
            })
            .await
            .context("Failed to kill instance")
    })
    .await
}

#[op2]
#[serde]
async fn get_instance_state(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<State, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;

        Ok(instance.state().await)
    })
    .await
}

#[op2]
async fn send_command(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] command: Strict<String>,
    #[serde] task_pid: Option<MacroPID>,
) -> Result<(), OpError> {
    let command = command.0;
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        instance
            .send_command(
                &command,
                // Older macro-lib wrappers did not pass the task pid for this op.
                task_pid
                    .map(|macro_pid| CausedBy::Macro { macro_pid })
                    .unwrap_or(CausedBy::Unknown),
            )
            .await
            .context("Failed to send command to instance")
    })
    .await
}

#[op2]
#[serde]
async fn monitor_instance(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<MonitorReport, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        Ok(instance.monitor().await)
    })
    .await
}

#[op2]
async fn get_instance_player_count(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<u32, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        Ok(instance.get_player_count().await?)
    })
    .await
}

#[op2]
async fn get_instance_max_players(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<u32, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        Ok(instance.get_max_player_count().await?)
    })
    .await
}

#[op2]
#[serde]
async fn get_instance_player_list(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<HashSet<Player>, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        Ok(instance.get_player_list().await?)
    })
    .await
}

#[op2]
#[string]
async fn get_instance_name(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<String, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        Ok(instance.name().await)
    })
    .await
}

#[op2]
#[serde]
async fn get_instance_game(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<Game, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        Ok(instance.game_type().await)
    })
    .await
}

#[op2]
#[string]
async fn get_instance_game_version(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<String, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        Ok(instance.version().await)
    })
    .await
}

#[op2]
#[string]
async fn get_instance_description(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<String, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        Ok(instance.description().await)
    })
    .await
}

#[op2]
async fn get_instance_port(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<u32, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        Ok(instance.port().await)
    })
    .await
}

#[op2]
#[string]
async fn get_instance_path(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<String, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        Ok(instance.path().await.to_string_lossy().to_string())
    })
    .await
}

#[op2]
async fn set_instance_name(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] name: Strict<String>,
) -> Result<(), OpError> {
    let name = name.0;
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;

        instance
            .set_name(name)
            .await
            .context("Failed to set instance name")
    })
    .await
}

#[op2]
async fn set_instance_description(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] description: Strict<String>,
) -> Result<(), OpError> {
    let description = description.0;
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;

        instance
            .set_description(description)
            .await
            .context("Failed to set instance description")
    })
    .await
}

#[op2]
async fn set_instance_port(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] port: Strict<u32>,
) -> Result<(), OpError> {
    let port = port.0;
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;

        instance
            .set_port(port)
            .await
            .context("Failed to set instance port")
    })
    .await
}

#[op2]
async fn set_instance_auto_start(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    auto_start: bool,
) -> Result<(), OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;

        instance
            .set_auto_start(auto_start)
            .await
            .context("Failed to set instance auto start")
    })
    .await
}

#[op2]
async fn is_rcon_available(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<bool, OpError> {
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        match &instance {
            GameInstance::MinecraftInstance(v) => Ok(v.get_rcon().lock().await.is_some()),
            GameInstance::GenericInstance(_) => {
                bail!("RCON not available for atom instances")
            }
        }
    })
    .await
}

#[op2]
#[string]
async fn try_send_rcon_command(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] command: Strict<String>,
) -> Result<Option<String>, OpError> {
    let command = command.0;
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        match &instance {
            GameInstance::MinecraftInstance(v) => Ok(v.send_rcon(&command).await.ok()),
            GameInstance::GenericInstance(_) => {
                bail!("RCON not available for atom instances")
            }
        }
    })
    .await
}

#[op2]
#[string]
async fn send_rcon_command(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] command: Strict<String>,
) -> Result<String, OpError> {
    let command = command.0;
    // Waiting for RCON may never finish, so that part is cancelled if the macro is killed.
    // The command exchange itself is not, to keep the shared RCON connection in sync.
    op_on_main_abortable(&state, wait_till_rcon_available_impl(instance_uuid.clone())).await?;
    op_on_main(&state, async move {
        let instance = get_instance(&instance_uuid)?;
        match &instance {
            GameInstance::MinecraftInstance(v) => {
                // This stage is not abortable, so it must not wait: if RCON went away since
                // the wait above (e.g. the server stopped), fail instead of polling forever.
                match v.get_rcon().lock().await.as_mut() {
                    Some(rcon) => Ok(rcon.cmd(&command).await?),
                    None => bail!("RCON is no longer available"),
                }
            }
            GameInstance::GenericInstance(_) => {
                bail!("RCON not available for atom instances")
            }
        }
    })
    .await
}

async fn wait_till_rcon_available_impl(instance_uuid: InstanceUuid) -> anyhow::Result<()> {
    let instance = get_instance(&instance_uuid)?;
    match &instance {
        GameInstance::MinecraftInstance(v) => {
            // There is no notification for RCON becoming available, so poll.
            let rcon = v.get_rcon();
            loop {
                if rcon.lock().await.is_some() {
                    break Ok(());
                }
                if !app_state().instances.contains_key(&instance_uuid) {
                    bail!("Instance was removed while waiting for RCON");
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }
        GameInstance::GenericInstance(_) => {
            bail!("RCON not available for atom instances")
        }
    }
}

#[op2]
async fn wait_till_rcon_available(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<(), OpError> {
    // may never finish, so it is cancelled if the macro is killed
    op_on_main_abortable(&state, wait_till_rcon_available_impl(instance_uuid)).await
}

pub fn register_instance_control_ops(
    worker_options: &mut deno_runtime::worker::WorkerOptions,
    main_runtime: tokio::runtime::Handle,
) {
    worker_options.extensions.push(Extension {
        name: "instance_control_ops",
        ops: Cow::Owned(vec![
            instance_exists(),
            all_instances(),
            get_instance_state(),
            get_instance_path(),
            get_instance_name(),
            get_instance_player_count(),
            get_instance_max_players(),
            get_instance_player_list(),
            get_instance_game(),
            get_instance_game_version(),
            get_instance_description(),
            get_instance_port(),
            set_instance_name(),
            set_instance_description(),
            set_instance_port(),
            set_instance_auto_start(),
            start_instance(),
            stop_instance(),
            restart_instance(),
            monitor_instance(),
            send_command(),
            kill_instance(),
            is_rcon_available(),
            try_send_rcon_command(),
            send_rcon_command(),
            wait_till_rcon_available(),
        ]),
        op_state_fn: Some(Box::new(move |state| {
            state.put(MainRuntime(main_runtime));
        })),
        ..Default::default()
    });
}
