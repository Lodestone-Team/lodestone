//! An instance's Mods (or Plugins) tab: installed mods, Modrinth and the client pack (`components/Mods`).
//!
//! Work package C: a stub until then (`launcher/docs/host-parity.md`, §6.2).

use gpui::{Context, Entity, IntoElement, Render, Window};

use crate::state::HostState;

pub struct ModsPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
}

impl ModsPage {
    pub fn new(state: Entity<HostState>, uuid: String, _cx: &mut Context<Self>) -> Self {
        Self { state, uuid }
    }
}

impl Render for ModsPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::not_built("C")
    }
}
