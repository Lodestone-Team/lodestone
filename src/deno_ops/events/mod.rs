use std::{cell::RefCell, rc::Rc};

use deno_core::{
    anyhow::{self, Context},
    op, OpState,
};

use crate::{
    event_broadcaster::EventBroadcaster, events::Event, macro_executor::MacroPID,
    types::InstanceUuid,
};

#[op]
async fn next_event(state: Rc<RefCell<OpState>>) -> Result<Event, anyhow::Error> {
    let rx = state.borrow().borrow::<EventBroadcaster>().clone();
    let event = rx
        .subscribe()
        .recv()
        .await
        .context("Failed to receive event")?;
    Ok(event)
}

#[op]
fn emit_detach(
    state: Rc<RefCell<OpState>>,
    macro_pid: MacroPID,
    instance_uuid: Option<InstanceUuid>,
) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_macro_detach_event(instance_uuid, macro_pid));
}

#[op]
fn emit_console_out(
    state: Rc<RefCell<OpState>>,
    line: String,
    instance_name: String,
    instance_uuid: InstanceUuid,
) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_instance_output(
        instance_uuid,
        instance_name,
        line,
    ));
}

pub fn register_all_event_ops(
    worker_options: &mut deno_runtime::worker::WorkerOptions,
    event_broadcaster: EventBroadcaster,
) {
    worker_options.extensions.push(
        deno_core::Extension::builder("event_ops")
            .ops(vec![
                next_event::decl(),
                emit_console_out::decl(),
                emit_detach::decl(),
            ])
            .state(|state| {
                state.put(event_broadcaster);
            })
            .force_op_registration()
            .build(),
    );
}
