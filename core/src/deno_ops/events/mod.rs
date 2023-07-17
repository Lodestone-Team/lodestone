use std::{cell::RefCell, rc::Rc};

use deno_core::{
    anyhow::{self, Context},
    op, OpState,
};

use crate::{
    event_broadcaster::{EventBroadcaster, PlayerChange, PlayerMessage},
    events::{
        CausedBy, Event, InstanceEvent, ProgressionEndValue, ProgressionEventID,
        ProgressionStartValue,
    },
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
async fn next_instance_player_change(
    state: Rc<RefCell<OpState>>,
    instance_uuid: InstanceUuid,
) -> PlayerChange {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster
        .next_instance_player_change(&instance_uuid)
        .await
}

#[op]
fn emit_detach(state: Rc<RefCell<OpState>>, macro_pid: MacroPID) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_macro_detach_event(macro_pid));
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

#[op]
fn emit_state_change(
    state: Rc<RefCell<OpState>>,
    instance_uuid: InstanceUuid,
    instance_name: String,
    new_state: State,
) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_instance_state_transition(
        instance_uuid,
        instance_name,
        new_state,
    ))
}

#[op]
fn emit_progression_event_start(
    state: Rc<RefCell<OpState>>,
    progression_name: String,
    total: Option<f64>,
    inner: Option<ProgressionStartValue>,
) -> ProgressionEventID {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    let (event, id) =
        Event::new_progression_event_start(progression_name, total, inner, CausedBy::System);
    tx.send(event);
    id
}

#[op]
fn emit_progression_event_update(
    state: Rc<RefCell<OpState>>,
    event_id: ProgressionEventID,
    progress_msg: String,
    progress: f64,
) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_progression_event_update(
        &event_id,
        progress_msg,
        progress,
    ));
}

#[op]
fn emit_progression_event_end(
    state: Rc<RefCell<OpState>>,
    event_id: ProgressionEventID,
    success: bool,
    message: Option<String>,
    inner: Option<ProgressionEndValue>,
) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_progression_event_end(
        event_id, success, message, inner,
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
                emit_state_change::decl(),
                next_instance_event::decl(),
                next_instance_state_change::decl(),
                next_instance_output::decl(),
                next_instance_player_message::decl(),
                next_instance_system_message::decl(),
                next_instance_player_change::decl(),
                emit_progression_event_start::decl(),
                emit_progression_event_update::decl(),
                emit_progression_event_end::decl(),
            ])
            .state(|state| {
                state.put(event_broadcaster);
            })
            .build(),
    );
}
