use deno_core::{anyhow, op};

use crate::{
    prelude::app_state,
    traits::{
        t_configurable::TConfigurable,
        t_server::{State, TServer},
    },
    types::InstanceUuid,
};

#[op]
async fn get_instance_state(instance_uuid: InstanceUuid) -> Result<State, anyhow::Error> {
    let instances = app_state().instances.lock().await;
    let instance = instances
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;

    Ok(instance.state().await)
}

#[op]
async fn get_instance_path(instance_uuid: InstanceUuid) -> Result<String, anyhow::Error> {
    let app_state = app_state().instances.lock().await;
    let instance = app_state
        .get(&instance_uuid)
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.path().await.to_string_lossy().to_string())
}

pub fn register_prelude_ops(worker_options: &mut deno_runtime::worker::WorkerOptions) {
    worker_options.extensions.push(
        deno_core::Extension::builder("prelude_ops")
            .ops(vec![get_instance_state::decl(), get_instance_path::decl()])
            .force_op_registration()
            .build(),
    );
}
