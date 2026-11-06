use std::{borrow::Cow, cell::RefCell, rc::Rc};

use deno_core::{anyhow::Context, op2, Extension, OpState};

use super::{op_try, OpError, Strict};

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

#[op2]
#[serde]
async fn next_event(state: Rc<RefCell<OpState>>) -> Result<Event, OpError> {
    let rx = state.borrow().borrow::<EventBroadcaster>().clone();
    op_try(async move {
        let event = rx
            .subscribe()
            .recv()
            .await
            .context("Failed to receive event")?;
        Ok(event)
    })
    .await
}

#[op2]
#[serde]
async fn next_instance_event(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> InstanceEvent {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster.next_instance_event(&instance_uuid).await
}

#[op2]
#[serde]
async fn next_instance_state_change(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> State {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster
        .next_instance_state_change(&instance_uuid)
        .await
}

#[op2]
#[string]
async fn next_instance_output(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> String {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster.next_instance_output(&instance_uuid).await
}

#[op2]
#[serde]
async fn next_instance_player_message(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> PlayerMessage {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster
        .next_instance_player_message(&instance_uuid)
        .await
}

#[op2]
#[string]
async fn next_instance_system_message(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> String {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster
        .next_instance_system_message(&instance_uuid)
        .await
}

#[op2]
#[serde]
async fn next_instance_player_change(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> PlayerChange {
    let event_broadcaster = state.borrow().borrow::<EventBroadcaster>().clone();
    event_broadcaster
        .next_instance_player_change(&instance_uuid)
        .await
}

#[op2]
fn emit_detach(state: Rc<RefCell<OpState>>, #[serde] macro_pid: MacroPID) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_macro_detach_event(macro_pid));
}

#[op2]
fn emit_console_out(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] instance_name: Strict<String>,
    #[serde] line: Strict<String>,
) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_instance_output(
        instance_uuid,
        instance_name.0,
        line.0,
    ));
}

#[op2]
fn emit_state_change(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
    #[serde] instance_name: Strict<String>,
    #[serde] new_state: State,
) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_instance_state_transition(
        instance_uuid,
        instance_name.0,
        new_state,
    ))
}

#[op2]
#[serde]
fn emit_progression_event_start(
    state: Rc<RefCell<OpState>>,
    #[serde] progression_name: Strict<String>,
    total: Option<f64>,
    #[serde] inner: Option<ProgressionStartValue>,
) -> ProgressionEventID {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    let (event, id) =
        Event::new_progression_event_start(progression_name.0, total, inner, CausedBy::System);
    tx.send(event);
    id
}

#[op2]
fn emit_progression_event_update(
    state: Rc<RefCell<OpState>>,
    #[serde] event_id: ProgressionEventID,
    #[serde] progress_msg: Strict<String>,
    progress: f64,
) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_progression_event_update(
        &event_id,
        progress_msg.0,
        progress,
    ));
}

#[op2]
fn emit_progression_event_end(
    state: Rc<RefCell<OpState>>,
    #[serde] event_id: ProgressionEventID,
    success: bool,
    #[serde] message: Option<Strict<String>>,
    #[serde] inner: Option<ProgressionEndValue>,
) {
    let tx = state.borrow().borrow::<EventBroadcaster>().clone();
    tx.send(Event::new_progression_event_end(
        event_id,
        success,
        message.map(|m| m.0),
        inner,
    ));
}

pub fn register_all_event_ops(
    worker_options: &mut deno_runtime::worker::WorkerOptions,
    event_broadcaster: EventBroadcaster,
) {
    worker_options.extensions.push(Extension {
        name: "event_ops",
        ops: Cow::Owned(vec![
            next_event(),
            emit_console_out(),
            emit_detach(),
            emit_state_change(),
            next_instance_event(),
            next_instance_state_change(),
            next_instance_output(),
            next_instance_player_message(),
            next_instance_system_message(),
            next_instance_player_change(),
            emit_progression_event_start(),
            emit_progression_event_update(),
            emit_progression_event_end(),
        ]),
        op_state_fn: Some(Box::new(move |state| {
            state.put(event_broadcaster);
        })),
        ..Default::default()
    });
}
