//! Settings › Users: the users, their permissions, API keys and Minecraft accounts (`UserSettings.tsx`).
//!
//! Work package D: a stub until then (`launcher/docs/host-parity.md`, §7.2).

use gpui::{Context, Entity, IntoElement, Render, Window};

use crate::state::HostState;

pub struct UsersPage {
    pub state: Entity<HostState>,
}

impl UsersPage {
    pub fn new(state: Entity<HostState>, _cx: &mut Context<Self>) -> Self {
        Self { state }
    }
}

impl Render for UsersPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::not_built("D")
    }
}
