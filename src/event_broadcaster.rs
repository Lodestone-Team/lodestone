use tokio::sync::broadcast::{Receiver, Sender};
use tracing::error;

use crate::{
    events::{Event, EventInner, InstanceEvent, InstanceEventInner},
    traits::t_server::State,
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

    /// Returns the next event that matches the given instance uuid.
    ///
    /// Will block forever if instance_uuid is not found.
    pub async fn next_instance_event(&self, instance_uuid: &InstanceUuid) -> InstanceEvent {
        let mut rx = self.subscribe();
        loop {
            let event = rx.recv().await.expect("Infallible");
            if let EventInner::InstanceEvent(inner) = &event.event_inner {
                if inner.instance_uuid == instance_uuid {
                    return inner.to_owned();
                }
            }
        }
    }

    /// Returns the next instance output event that matches the given instance uuid.
    ///
    /// Will block forever if instance_uuid is not found.
    pub async fn next_instance_output(&self, instance_uuid: &InstanceUuid) -> String {
        loop {
            let instance_event = self.next_instance_event(instance_uuid).await;
            if let InstanceEventInner::InstanceOutput { message } =
                instance_event.instance_event_inner
            {
                return message;
            }
        }
    }

    /// Returns the next instance state change event that matches the given instance uuid.
    ///
    /// Will block forever if instance_uuid is not found.
    pub async fn next_instance_state_change(&self, instance_uuid: &InstanceUuid) -> State {
        loop {
            let instance_event = self.next_instance_event(instance_uuid).await;
            if let InstanceEventInner::StateTransition { to } = instance_event.instance_event_inner
            {
                return to;
            }
        }
    }

    pub async fn next_instance_system_message(&self, instance_uuid: &InstanceUuid) -> String {
        loop {
            let instance_event = self.next_instance_event(instance_uuid).await;
            if let InstanceEventInner::SystemMessage { message } =
                instance_event.instance_event_inner
            {
                return message;
            }
        }
    }

    pub async fn next_instance_player_message(
        &self,
        instance_uuid: &InstanceUuid,
    ) -> PlayerMessage {
        loop {
            let event = self.next_instance_event(instance_uuid).await;
            if let InstanceEventInner::PlayerMessage {
                player,
                player_message,
            } = event.instance_event_inner
            {
                return PlayerMessage {
                    player,
                    message: player_message,
                };
            }
        }
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
