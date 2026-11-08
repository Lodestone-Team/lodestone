//! Remembers which instances had mods changed while running, until they next start.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

use tokio::sync::broadcast::{error::RecvError, Receiver};

use crate::events::{Event, EventInner, InstanceEvent, InstanceEventInner};
use crate::traits::t_server::State;
use crate::types::InstanceUuid;

#[derive(Clone, Default)]
pub struct RestartTracker {
    flagged: Arc<Mutex<HashSet<InstanceUuid>>>,
}

impl RestartTracker {
    /// Records a change to the mods of an instance in `state`. Changes while the server
    /// starts or runs only apply after a restart.
    pub fn note_change(&self, uuid: &InstanceUuid, state: State) {
        if matches!(state, State::Starting | State::Running) {
            self.flagged
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(uuid.clone());
        }
    }

    pub fn is_required(&self, uuid: &InstanceUuid) -> bool {
        self.flagged
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(uuid)
    }

    pub fn clear(&self, uuid: &InstanceUuid) {
        self.flagged
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(uuid);
    }

    /// Handles one event: an instance that starts loads its mods again.
    fn on_event(&self, event: &Event) {
        if let EventInner::InstanceEvent(InstanceEvent {
            instance_uuid,
            instance_event_inner:
                InstanceEventInner::StateTransition {
                    to: State::Starting,
                },
            ..
        }) = &event.event_inner
        {
            self.clear(instance_uuid);
        }
    }

    /// Clears the flag of each instance when it starts. Runs until the event channel
    /// closes.
    pub async fn watch(self, mut events: Receiver<Event>) {
        loop {
            match events.recv().await {
                Ok(event) => self.on_event(&event),
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flag_is_set_while_running_and_cleared_on_start() {
        let tracker = RestartTracker::default();
        let uuid = InstanceUuid::default();
        tracker.note_change(&uuid, State::Stopped);
        assert!(!tracker.is_required(&uuid));
        tracker.note_change(&uuid, State::Running);
        assert!(tracker.is_required(&uuid));

        // stopping does not clear it, starting does
        tracker.on_event(&Event::new_instance_state_transition(
            uuid.clone(),
            "x".into(),
            State::Stopped,
        ));
        assert!(tracker.is_required(&uuid));
        tracker.on_event(&Event::new_instance_state_transition(
            uuid.clone(),
            "x".into(),
            State::Starting,
        ));
        assert!(!tracker.is_required(&uuid));

        tracker.note_change(&uuid, State::Starting);
        assert!(tracker.is_required(&uuid));
    }
}
