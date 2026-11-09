//! Settings › General: the core's name, address, Playit.gg, public listing, information and the danger zone (`CoreSettings.tsx`).
//!
//! Work package D: a stub until then (`launcher/docs/host-parity.md`, §7.1).

use gpui::{Context, Entity, IntoElement, Render, Window};

use crate::state::HostState;

pub struct CoreSettingsPage {
    pub state: Entity<HostState>,
}

impl CoreSettingsPage {
    pub fn new(state: Entity<HostState>, _cx: &mut Context<Self>) -> Self {
        Self { state }
    }
}

impl Render for CoreSettingsPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::not_built("D")
    }
}
