//! The "New server" dialog: every game the core offers (`GET /games`, the setup
//! manifest's form) and the Modrinth modpack flow, after `InstanceCreateForm.tsx` and
//! `ModpackCreateForm.tsx`.
//!
//! Work package A: a stub until then (`launcher/docs/host-parity.md`, §2.3 and §2.4).
//! The shell opens it from the sidebar's "New server" and closes it on
//! [`CreateInstanceEvent::Close`]; it renders its own `Dialog`.

use gpui::{
    Context, Entity, EventEmitter, FocusHandle, IntoElement, ParentElement, Render, Styled,
    Window, div,
};
use lodestone_ui::components::Dialog;

use crate::{
    i18n::{ShellMsg, t},
    state::HostState,
};

/// What the dialog tells the shell.
pub enum CreateInstanceEvent {
    Close,
}

pub struct CreateInstance {
    pub state: Entity<HostState>,
    focus: FocusHandle,
}

impl EventEmitter<CreateInstanceEvent> for CreateInstance {}

impl CreateInstance {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        Self {
            state,
            focus: cx.focus_handle(),
        }
    }
}

impl Render for CreateInstance {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let this = cx.entity().downgrade();
        div().absolute().inset_0().child(
            Dialog::new("create-instance", t(ShellMsg::NewServer), self.focus.clone())
                .on_close(move |_, cx| {
                    this.update(cx, |_, cx| cx.emit(CreateInstanceEvent::Close))
                        .ok();
                })
                .child(super::not_built("A")),
        )
    }
}
