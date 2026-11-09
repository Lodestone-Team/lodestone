//! Settings › Profile: the username, password, appearance, API keys and Minecraft accounts (`profile.tsx`).
//!
//! Work package D: a stub until then (`launcher/docs/host-parity.md`, §7.3).

use gpui::{Context, Entity, IntoElement, Render, Window};

use crate::state::HostState;

pub struct ProfilePage {
    pub state: Entity<HostState>,
}

impl ProfilePage {
    pub fn new(state: Entity<HostState>, _cx: &mut Context<Self>) -> Self {
        Self {
            state,
        }
    }
}

impl Render for ProfilePage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::not_built("D")
    }
}
