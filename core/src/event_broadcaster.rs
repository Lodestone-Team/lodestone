use std::collections::HashSet;

use tokio::sync::broadcast::{error::RecvError, Receiver, Sender};
use tracing::{error, warn};

use crate::{
    events::{Event, EventInner, InstanceEvent, InstanceEventInner},
    traits::{t_player::Player, t_server::State},
    types::InstanceUuid,
};

#[derive(Debug, Clone)]
pub struct EventBroadcaster {
    event_tx: Sender<Event>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayerMessage {
    pub player: String,
    pub message: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PlayerChange {
    player_list: HashSet<Player>,
    players_joined: HashSet<Player>,
    players_left: HashSet<Player>,
}

impl EventBroadcaster {
    pub fn new(capacity: usize) -> (Self, Receiver<Event>) {
        let (event_tx, rx) = tokio::sync::broadcast::channel(capacity);
        (Self { event_tx }, rx)
    }

    pub fn send(&self, event: Event) {
        if let Err(e) = self.event_tx.send(event) {
            error!("Failed to send event: {e}");
        }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<Event> {
        self.event_tx.subscribe()
    }

    /// Waits on `rx` for the next event that `select` maps to a value.
    ///
    /// Reuse the same receiver across calls so events that arrive in between are not
    /// lost. If the receiver falls behind, the skipped events are logged and waiting goes
    /// on. Returns `None` once every sender is gone.
    pub async fn next_matching<T>(
        rx: &mut Receiver<Event>,
        mut select: impl FnMut(Event) -> Option<T>,
    ) -> Option<T> {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Some(value) = select(event) {
                        return Some(value);
                    }
                }
                Err(RecvError::Lagged(skipped)) => {
                    warn!("Event receiver fell behind, skipped {skipped} events");
                }
                Err(RecvError::Closed) => return None,
            }
        }
    }
}

/// The instance event inside `event`, if it is one for `instance_uuid`.
pub fn instance_event(event: Event, instance_uuid: &InstanceUuid) -> Option<InstanceEvent> {
    match event.event_inner {
        EventInner::InstanceEvent(inner) if inner.instance_uuid == instance_uuid => Some(inner),
        _ => None,
    }
}

/// The console line in `event`, if it is one for `instance_uuid`.
pub fn instance_output(event: Event, instance_uuid: &InstanceUuid) -> Option<String> {
    match instance_event(event, instance_uuid)?.instance_event_inner {
        InstanceEventInner::InstanceOutput { message } => Some(message),
        _ => None,
    }
}

/// The new state in `event`, if it is a state change of `instance_uuid`.
pub fn instance_state_change(event: Event, instance_uuid: &InstanceUuid) -> Option<State> {
    match instance_event(event, instance_uuid)?.instance_event_inner {
        InstanceEventInner::StateTransition { to } => Some(to),
        _ => None,
    }
}

/// The system message in `event`, if it is one for `instance_uuid`.
pub fn instance_system_message(event: Event, instance_uuid: &InstanceUuid) -> Option<String> {
    match instance_event(event, instance_uuid)?.instance_event_inner {
        InstanceEventInner::SystemMessage { message } => Some(message),
        _ => None,
    }
}

/// The player chat message in `event`, if it is one for `instance_uuid`.
pub fn instance_player_message(
    event: Event,
    instance_uuid: &InstanceUuid,
) -> Option<PlayerMessage> {
    match instance_event(event, instance_uuid)?.instance_event_inner {
        InstanceEventInner::PlayerMessage {
            player,
            player_message,
        } => Some(PlayerMessage {
            player,
            message: player_message,
        }),
        _ => None,
    }
}

/// The player list change in `event`, if it is one for `instance_uuid`.
pub fn instance_player_change(event: Event, instance_uuid: &InstanceUuid) -> Option<PlayerChange> {
    match instance_event(event, instance_uuid)?.instance_event_inner {
        InstanceEventInner::PlayerChange {
            player_list,
            players_joined,
            players_left,
        } => Some(PlayerChange {
            player_list,
            players_joined,
            players_left,
        }),
        _ => None,
    }
}

impl From<EventBroadcaster> for Sender<Event> {
    fn from(event_broadcaster: EventBroadcaster) -> Self {
        event_broadcaster.event_tx
    }
}

impl AsRef<Sender<Event>> for EventBroadcaster {
    fn as_ref(&self) -> &Sender<Event> {
        &self.event_tx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn output(uuid: &InstanceUuid, line: &str) -> Event {
        Event::new_instance_output(uuid.clone(), "test".to_string(), line.to_string())
    }

    #[tokio::test]
    async fn events_between_waits_are_kept() {
        let (broadcaster, _rx) = EventBroadcaster::new(16);
        let uuid = InstanceUuid::default();
        let other = InstanceUuid::default();
        let mut rx = broadcaster.subscribe();
        broadcaster.send(output(&other, "other instance"));
        broadcaster.send(output(&uuid, "first"));
        broadcaster.send(output(&uuid, "second"));
        let first = EventBroadcaster::next_matching(&mut rx, |e| instance_output(e, &uuid)).await;
        let second = EventBroadcaster::next_matching(&mut rx, |e| instance_output(e, &uuid)).await;
        assert_eq!(first.as_deref(), Some("first"));
        assert_eq!(second.as_deref(), Some("second"));
    }

    #[tokio::test]
    async fn lagging_receiver_skips_and_continues() {
        let (broadcaster, _rx) = EventBroadcaster::new(2);
        let uuid = InstanceUuid::default();
        let mut rx = broadcaster.subscribe();
        for i in 0..10 {
            broadcaster.send(output(&uuid, &format!("line {i}")));
        }
        let line = EventBroadcaster::next_matching(&mut rx, |e| instance_output(e, &uuid)).await;
        assert_eq!(line.as_deref(), Some("line 8"));
    }

    #[tokio::test]
    async fn closed_channel_ends_the_wait() {
        let (broadcaster, _rx) = EventBroadcaster::new(2);
        let mut rx = broadcaster.subscribe();
        drop(broadcaster);
        let uuid = InstanceUuid::default();
        let next = EventBroadcaster::next_matching(&mut rx, |e| instance_output(e, &uuid)).await;
        assert!(next.is_none());
    }
}
