use deno_core::{anyhow, op};

use crate::{
    prelude::app_state,
    traits::t_server::{State, TServer},
    types::InstanceUuid,
};

#[op]
async fn get_instance_state(instance_uuid: InstanceUuid) -> Result<State, anyhow::Error> {
    let instance = app_state()
        .instances
        .lock()
        .await
        .get(&instance_uuid)
        .cloned()
        .ok_or(anyhow::anyhow!("Instance not found"))?;
    Ok(instance.state().await)
}

pub fn register_prelude_ops(worker_options: &mut deno_runtime::worker::WorkerOptions) {
    worker_options.extensions.push(
        deno_core::Extension::builder("prelude_ops")
            .ops(vec![get_instance_state::decl()])
            .build(),
    );
}
