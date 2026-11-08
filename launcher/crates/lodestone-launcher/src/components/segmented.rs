use gpui::{App, SharedString, Window, div, prelude::*};

use super::{ArrowGroup, KeyboardFocus};
use crate::theme::{self, colors, radius, text, weights};

/// A row of choices with the selected one raised, as in "Add server". `selected` is
/// `None` when the value isn't one of the choices. The arrow keys move between them.
pub fn segmented<T: Clone + PartialEq + 'static>(
    id: impl Into<SharedString>,
    choices: Vec<(T, SharedString)>,
    selected: Option<&T>,
    disabled: bool,
    on_pick: impl Fn(T, &mut Window, &mut App) + Clone + 'static,
) -> impl IntoElement {
    let id: SharedString = id.into();
    ArrowGroup::new(id.clone())
        .flex()
        .flex_row()
        .flex_wrap()
        .gap_1()
        .p_1()
        .rounded(radius::LG)
        .bg(colors::GRAY_900)
        .border_1()
        .border_color(colors::FADE_700)
        .children(choices.into_iter().enumerate().map(|(i, (value, name))| {
            let is_selected = selected == Some(&value);
            let on_pick = on_pick.clone();
            div()
                .id(SharedString::from(format!("{id}-{i}")))
                .px_3()
                .py_1()
                .rounded(radius::MD)
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .map(|this| match (is_selected, disabled) {
                    (true, _) => this.bg(colors::GRAY_700).text_color(colors::GRAY_300),
                    (false, true) => this.text_color(colors::GRAY_600),
                    (false, false) => this
                        .text_color(colors::GRAY_500)
                        .hover(|s| s.text_color(colors::GRAY_300)),
                })
                .child(name.clone())
                .when(!disabled, |this| {
                    this.keyboard_focus().cursor_pointer().on_click({
                        let (id, name) = (id.clone(), name.clone());
                        move |_, window, cx| {
                            super::trace_ui(
                                "ui.select",
                                "segmented",
                                &gpui::ElementId::Name(id.clone()),
                                serde_json::json!({ "index": i, "label": name.as_ref() }),
                            );
                            on_pick(value.clone(), window, cx)
                        }
                    })
                })
        }))
}
