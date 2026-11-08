//! A small label shown while the pointer rests on an element.

use gpui::{
    AnyView, App, AppContext as _, Context, IntoElement, Render, SharedString, Window, div,
    prelude::*,
};

use crate::theme::{self, colors, px, radius, text, weights};

/// The tooltip's view: the text, wrapping at a readable width.
pub struct TextTooltip(SharedString);

impl Render for TextTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .max_w(px(320.))
            .px_2()
            .py_1()
            .rounded(radius::MD)
            .border_1()
            .border_color(colors::FADE_700)
            .bg(colors::GRAY_850)
            .text_size(theme::text_size(text::SMALL))
            .font_weight(weights::MEDIUM.into())
            .text_color(colors::GRAY_300)
            .child(self.0.clone())
    }
}

/// For `.tooltip(…)` on an element with an id: shows `text`.
pub fn text_tooltip(
    text: impl Into<SharedString>,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let text = text.into();
    move |_, cx| cx.new(|_| TextTooltip(text.clone())).into()
}
