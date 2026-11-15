use tokio::sync::broadcast::{Receiver, Sender};
use tracing::error;

use crate::events::Event;

#[derive(Debug, Clone)]
pub struct EventBroadcaster {
    event_tx: Sender<Event>,
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
