use std::{cell::RefCell, rc::Rc};

use deno_core::{
    anyhow::{self, Context},
    op, OpState,
};

use crate::{
    event_broadcaster::{EventBroadcaster, PlayerMessage},
    events::{Event, InstanceEvent},
    macro_executor::MacroPID,
    traits::t_server::State,
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
async fn next_instance_event(
    state: Rc<RefCell<OpState>>,
    instance_uuid: InstanceUuid,
) -> InstanceEvent {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster.next_instance_event(&instance_uuid).await
}

#[op]
async fn next_instance_state_change(
    state: Rc<RefCell<OpState>>,
    instance_uuid: InstanceUuid,
) -> State {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster
        .next_instance_state_change(&instance_uuid)
        .await
}

#[op]
async fn next_instance_output(state: Rc<RefCell<OpState>>, instance_uuid: InstanceUuid) -> String {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster.next_instance_output(&instance_uuid).await
}

#[op]
async fn next_instance_player_message(
    state: Rc<RefCell<OpState>>,
    instance_uuid: InstanceUuid,
) -> PlayerMessage {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster
        .next_instance_player_message(&instance_uuid)
        .await
}

#[op]
async fn next_instance_system_message(
    state: Rc<RefCell<OpState>>,
    instance_uuid: InstanceUuid,
) -> String {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster
        .next_instance_system_message(&instance_uuid)
        .await
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
    instance_uuid: InstanceUuid,
    instance_name: String,
    line: String,
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
                next_instance_event::decl(),
                next_instance_state_change::decl(),
                next_instance_output::decl(),
                next_instance_player_message::decl(),
                next_instance_system_message::decl(),
            ])
            .state(|state| {
                state.put(event_broadcaster);
            })
            .build(),
    );
}
