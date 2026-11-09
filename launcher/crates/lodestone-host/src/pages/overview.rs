//! An instance's Overview tab: the inline-edited name and description, the labels, the performance graphs (`/monitor/{uuid}`), the players and the details (`InstanceOverview.tsx`).
//!
//! Work package A: a stub until then (`launcher/docs/host-parity.md`, §3).

use gpui::{Context, Entity, IntoElement, Render, Window};

use crate::state::HostState;

pub struct OverviewPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
}

impl OverviewPage {
    pub fn new(state: Entity<HostState>, uuid: String, _cx: &mut Context<Self>) -> Self {
        Self { state, uuid }
    }
}

impl Render for OverviewPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::not_built("A")
    }
}
