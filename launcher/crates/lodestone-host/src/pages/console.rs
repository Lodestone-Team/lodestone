//! An instance's Console tab: the history, the live stream, colours, the filter and the command input (`GameConsole.tsx`).
//!
//! Work package B: a stub until then (`launcher/docs/host-parity.md`, §4).

use gpui::{Context, Entity, IntoElement, Render, Window};

use crate::state::HostState;

pub struct ConsolePage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
}

impl ConsolePage {
    pub fn new(state: Entity<HostState>, uuid: String, _cx: &mut Context<Self>) -> Self {
        Self {
            state,
            uuid,
        }
    }
}

impl Render for ConsolePage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::not_built("B")
    }
}
