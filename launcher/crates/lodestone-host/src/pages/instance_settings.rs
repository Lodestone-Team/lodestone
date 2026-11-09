//! An instance's Settings tab: the core's settings manifest, automation, public listing, the resource pack and the danger zone (`InstanceSettings.tsx`).
//!
//! Work package C: a stub until then (`launcher/docs/host-parity.md`, §6.1).

use gpui::{Context, Entity, IntoElement, Render, Window};

use crate::state::HostState;

pub struct InstanceSettingsPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
}

impl InstanceSettingsPage {
    pub fn new(state: Entity<HostState>, uuid: String, _cx: &mut Context<Self>) -> Self {
        Self {
            state,
            uuid,
        }
    }
}

impl Render for InstanceSettingsPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::not_built("C")
    }
}
