//! An instance's Event logs tab (a placeholder in the dashboard too).
//!
//! Work package C: a stub until then (`launcher/docs/host-parity.md`, §6.4).

use gpui::{Context, Entity, IntoElement, Render, Window};

use crate::state::HostState;

pub struct EventLogsPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
}

impl EventLogsPage {
    pub fn new(state: Entity<HostState>, uuid: String, _cx: &mut Context<Self>) -> Self {
        Self {
            state,
            uuid,
        }
    }
}

impl Render for EventLogsPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::not_built("C")
    }
}
