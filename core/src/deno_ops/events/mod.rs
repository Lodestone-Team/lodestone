use std::{borrow::Cow, cell::RefCell, collections::HashMap, rc::Rc};

use deno_core::{anyhow::anyhow, op2, Extension, OpState};
use tokio::sync::{broadcast::Receiver, Mutex};

use super::{OpError, Strict};

use crate::{
    event_broadcaster::{
        instance_event, instance_output, instance_player_change, instance_player_message,
        instance_state_change, instance_system_message, EventBroadcaster, PlayerChange,
        PlayerMessage,
    },
    events::{
        CausedBy, Event, InstanceEvent, ProgressionEndValue, ProgressionEventID,
        ProgressionStartValue,
    },
    macro_executor::MacroPID,
    traits::t_server::State,
    types::InstanceUuid,
};

/// Which events a waiting op is looking for. Each kind (and instance) gets its own receiver,
/// so concurrent waits of different kinds do not take each other's events.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum StreamKey {
    All,
    Instance(InstanceUuid),
    StateChange(InstanceUuid),
    Output(InstanceUuid),
    PlayerMessage(InstanceUuid),
    SystemMessage(InstanceUuid),
    PlayerChange(InstanceUuid),
}

/// Event receivers of one macro, kept for its whole run so events that arrive while the
/// macro is busy between two waits are buffered instead of lost.
#[derive(Default)]
struct EventStreams(HashMap<StreamKey, Rc<Mutex<Receiver<Event>>>>);

/// Waits for the next event on the macro's receiver for `key` that `select` maps to a value.
async fn next_from_stream<T>(
    state: &Rc<RefCell<OpState>>,
    key: StreamKey,
    select: impl FnMut(Event) -> Option<T>,
) -> Result<T, OpError> {
    let rx = {
        let mut state = state.borrow_mut();
        let broadcaster = state.borrow::<EventBroadcaster>().clone();
        state
            .borrow_mut::<EventStreams>()
            .0
            .entry(key)
            .or_insert_with(|| Rc::new(Mutex::new(broadcaster.subscribe())))
            .clone()
    };
    let mut rx = rx.lock().await;
    EventBroadcaster::next_matching(&mut rx, select)
        .await
        .ok_or_else(|| anyhow!("Event channel closed").into())
}

#[op2]
#[serde]
async fn next_event(state: Rc<RefCell<OpState>>) -> Result<Event, OpError> {
    next_from_stream(&state, StreamKey::All, Some).await
}

#[op2]
#[serde]
async fn next_instance_event(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<InstanceEvent, OpError> {
    let key = StreamKey::Instance(instance_uuid.clone());
    next_from_stream(&state, key, |e| instance_event(e, &instance_uuid)).await
}

#[op2]
#[serde]
async fn next_instance_state_change(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<State, OpError> {
    let key = StreamKey::StateChange(instance_uuid.clone());
    next_from_stream(&state, key, |e| instance_state_change(e, &instance_uuid)).await
}

#[op2]
#[string]
async fn next_instance_output(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<String, OpError> {
    let key = StreamKey::Output(instance_uuid.clone());
    next_from_stream(&state, key, |e| instance_output(e, &instance_uuid)).await
}

#[op2]
#[serde]
async fn next_instance_player_message(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<PlayerMessage, OpError> {
    let key = StreamKey::PlayerMessage(instance_uuid.clone());
    next_from_stream(&state, key, |e| instance_player_message(e, &instance_uuid)).await
}

#[op2]
#[string]
async fn next_instance_system_message(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<String, OpError> {
    let key = StreamKey::SystemMessage(instance_uuid.clone());
    next_from_stream(&state, key, |e| instance_system_message(e, &instance_uuid)).await
}

#[op2]
#[serde]
async fn next_instance_player_change(
    state: Rc<RefCell<OpState>>,
    #[serde] instance_uuid: InstanceUuid,
) -> Result<PlayerChange, OpError> {
    let key = StreamKey::PlayerChange(instance_uuid.clone());
    next_from_stream(&state, key, |e| instance_player_change(e, &instance_uuid)).await
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
            state.put(EventStreams::default());
        })),
        ..Default::default()
    });
}
