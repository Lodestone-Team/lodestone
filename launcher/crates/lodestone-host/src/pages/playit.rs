//! The Playit.gg tab: sign-up, the runner and the tunnels (`PlayitggOverview.tsx`).
//!
//! Work package C: a stub until then (`launcher/docs/host-parity.md`, §6.7).

use gpui::{Context, Entity, IntoElement, Render, Window};

use crate::state::HostState;

pub struct PlayitPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
}

impl PlayitPage {
    pub fn new(state: Entity<HostState>, uuid: String, _cx: &mut Context<Self>) -> Self {
        Self {
            state,
            uuid,
        }
    }
}

impl Render for PlayitPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::not_built("C")
    }
}
