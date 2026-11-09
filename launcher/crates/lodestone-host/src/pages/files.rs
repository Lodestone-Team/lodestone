//! An instance's Files tab: the file manager and the editor (`components/FileViewer`).
//!
//! Work package B: a stub until then (`launcher/docs/host-parity.md`, §5).

use gpui::{Context, Entity, IntoElement, Render, Window};

use crate::state::HostState;

pub struct FilesPage {
    pub state: Entity<HostState>,
    /// The instance.
    pub uuid: String,
}

impl FilesPage {
    pub fn new(state: Entity<HostState>, uuid: String, _cx: &mut Context<Self>) -> Self {
        Self { state, uuid }
    }
}

impl Render for FilesPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        super::not_built("B")
    }
}
