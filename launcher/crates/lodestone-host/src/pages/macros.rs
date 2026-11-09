//! An instance's Macros tab. Macros and tasks run on Deno, which Lodestone Host's core
//! is built without, so this is out of scope: the tab is hidden unless the core has
//! the `macros` capability, and then says macros aren't available, as the dashboard
//! does for a core without scripting.

use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use lodestone_ui::theme::{ink, px, text};

use crate::{
    i18n::{ShellMsg, t},
    state::HostState,
};

pub struct MacrosPage {
    pub state: Entity<HostState>,
    pub uuid: String,
}

impl MacrosPage {
    pub fn new(state: Entity<HostState>, uuid: String, _cx: &mut Context<Self>) -> Self {
        Self { state, uuid }
    }
}

impl Render for MacrosPage {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .p(px(16.))
            .text_size(text::MEDIUM)
            .text_color(ink::muted())
            .child(t(ShellMsg::NeedsScripting))
    }
}
