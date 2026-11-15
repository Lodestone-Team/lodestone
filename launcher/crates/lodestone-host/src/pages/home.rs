//! The page without a selected instance (`pages/home.tsx`): a welcome, and "Create a
//! new server" for a user who may.

use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div, img};
use launcher_core::cores::admin::models::CorePermission;
use lodestone_ui::{
    components::{Button, ButtonVariant, Icon},
    theme::{fonts, ink, px, text, weights},
};

use crate::{
    i18n::{ShellMsg, t},
    state::HostState,
};

pub struct HomePage {
    pub state: Entity<HostState>,
}

impl HomePage {
    pub fn new(state: Entity<HostState>, _cx: &mut Context<Self>) -> Self {
        Self { state }
    }
}

impl Render for HomePage {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let may_create = self
            .state
            .read(cx)
            .me
            .may_core(CorePermission::CreateInstance);
        let state = self.state.downgrade();
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            // the green Lodestone mark, as the Launcher's
            .child(img("images/lodestone_mark.svg").size(px(64.)).flex_none())
            .child(
                div()
                    .font_family(fonts::heading())
                    .text_size(text::H1)
                    .font_weight(weights::BOLD.face())
                    .text_color(ink::text())
                    .child(t(ShellMsg::HomeTitle)),
            )
            .child(
                div()
                    .text_size(text::MEDIUM)
                    .text_color(ink::muted())
                    .child(t(ShellMsg::HomeSubtitle)),
            )
            .child(
                Button::new("home-create-server", t(ShellMsg::CreateServer))
                    .variant(ButtonVariant::Primary)
                    .icon(Icon::Plus)
                    .disabled(!may_create)
                    .on_click(move |_, _, cx| {
                        state
                            .update(cx, |state, cx| state.open_create_instance(cx))
                            .ok();
                    }),
            )
    }
}
