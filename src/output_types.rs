use serde::Serialize;
use ts_rs::TS;

use crate::events::{
    CausedBy, Event, EventInner, EventLevel, InstanceEventInner, MacroEventInner,
    ProgressionEventInner,
};

#[derive(Serialize, Clone, Debug, TS)]
#[ts(export)]
pub struct ClientEvent {
    pub event_inner: EventInner,
    pub details: String,
    pub snowflake: i64,
    pub snowflake_str: String,
    pub level: EventLevel,
    pub caused_by: CausedBy,
}

impl From<&Event> for ClientEvent {
    fn from(event: &Event) -> Self {
        let level = match &event.event_inner {
            EventInner::InstanceEvent(i) => match i.instance_event_inner {
                InstanceEventInner::InstanceError => EventLevel::Error,
                InstanceEventInner::InstanceWarning => EventLevel::Warning,
                _ => EventLevel::Info,
            },
            EventInner::UserEvent(_) => EventLevel::Info,
            EventInner::MacroEvent(m) => match m.macro_event_inner {
                MacroEventInner::MacroStarted => EventLevel::Info,
                MacroEventInner::MacroStopped => EventLevel::Info,
                MacroEventInner::MacroErrored { .. } => EventLevel::Error,
            },
            EventInner::ProgressionEvent(p) => match p.progression_event_inner {
                ProgressionEventInner::ProgressionStart { .. } => EventLevel::Info,
                ProgressionEventInner::ProgressionUpdate { .. } => EventLevel::Info,
                ProgressionEventInner::ProgressionEnd { success, .. } => {
                    if success {
                        EventLevel::Info
                    } else {
                        EventLevel::Error
                    }
                }
            },
            EventInner::FSEvent(_) => EventLevel::Info,
        };
        ClientEvent {
            event_inner: event.event_inner.clone(),
            details: event.details.clone(),
            snowflake: event.snowflake,
            snowflake_str: event.snowflake.to_string(),
            level,
            caused_by: event.caused_by.clone(),
        }
    }
}

impl From<Event> for ClientEvent {
    fn from(event: Event) -> Self {
        ClientEvent::from(&event)
    }
}

impl AsRef<ClientEvent> for ClientEvent {
    fn as_ref(&self) -> &ClientEvent {
        self
    }
}

impl AsRef<Event> for ClientEvent {
    fn as_ref(&self) -> &Event {
        unsafe { std::mem::transmute(self) }
    }
}
