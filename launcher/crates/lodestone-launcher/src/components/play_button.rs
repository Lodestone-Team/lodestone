use gpui::{App, ClickEvent, ElementId, IntoElement, RenderOnce, Window, div, prelude::*, px};

use super::ClickHandler;
use crate::theme::{self, colors, fonts, leading, radius, text, weights};

/// The large Play button in the bottom bar, in the dashboard's green.
#[derive(IntoElement)]
pub struct PlayButton {
    id: ElementId,
    disabled: bool,
    on_click: Option<ClickHandler>,
}

impl PlayButton {
    pub fn new(id: impl Into<ElementId>) -> Self {
        Self {
            id: id.into(),
            disabled: false,
            on_click: None,
        }
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn on_click(
        mut self,
        handler: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        self.on_click = Some(Box::new(handler));
        self
    }
}

impl RenderOnce for PlayButton {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        div()
            .id(self.id)
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .w(px(260.))
            .h(px(60.))
            .rounded(radius::LG)
            .border_1()
            .font_family(fonts::TITLE)
            .text_size(theme::text_size(text::H1))
            .line_height(theme::line_height(text::H1, leading::TIGHT))
            .font_weight(weights::EXTRABOLD.into())
            .map(|this| {
                if self.disabled {
                    this.bg(colors::GREEN_400.opacity(25))
                        .border_color(colors::GREEN_400.opacity(30))
                        .text_color(colors::WHITE.opacity(40))
                } else {
                    this.bg(colors::GREEN_400)
                        .border_color(colors::GREEN_300)
                        .text_color(colors::WHITE)
                        .cursor_pointer()
                        .hover(|style| style.bg(colors::GREEN_300))
                        .when_some(self.on_click, |this, on_click| this.on_click(on_click))
                }
            })
            .child("PLAY")
    }
}
