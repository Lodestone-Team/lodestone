use gpui::{App, SharedString, Window, div, prelude::*};

use super::{
    ArrowGroup, KeyboardFocus,
    a11y::{Accessible, Role, Toggled},
};
use crate::theme::{self, colors, radius, text, weights};
use launcher_core::ui_skin::SurfaceId;

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
        .group_role(Role::RadioGroup)
        .flex()
        .flex_row()
        .flex_wrap()
        .gap_1()
        .p_1()
        .rounded(radius::LG)
        .bg(colors::GRAY_900)
        .border_1()
        .border_color(crate::theme::ink::border())
        .children(choices.into_iter().enumerate().map(|(i, (value, name))| {
            let is_selected = selected == Some(&value);
            let on_pick = on_pick.clone();
            let item_id = SharedString::from(format!("{id}-{i}"));
            let item = div()
                .id(item_id.clone())
                .accessible(Role::RadioButton, item_id, name.clone())
                .aria_toggled(if is_selected {
                    Toggled::True
                } else {
                    Toggled::False
                })
                .px_3()
                .py_1()
                .rounded(radius::MD)
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .when(!is_selected, |this| {
                    this.hover(move |s| {
                        if disabled {
                            s
                        } else {
                            s.text_color(crate::theme::ink::text())
                        }
                    })
                })
                .child(name.clone())
                .when(!disabled, |this| {
                    this.keyboard_focus().cursor_pointer().on_press({
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
                });
            // The raised background fades in under the chosen one, and out of the
            // one left.
            // the skin's pill, when it paints one in a colour
            let skin = crate::skin::painted(SurfaceId::Pill);
            let chosen = match skin.as_ref().and_then(|s| s.paint.clone()) {
                Some(crate::skin::Paint::Solid(color)) => color,
                _ => colors::GRAY_700,
            };
            let chosen_text = skin
                .as_ref()
                .and_then(|s| s.text)
                .unwrap_or(colors::GRAY_300);
            super::motion::Animated::new(
                ("segmented-choice", i),
                is_selected,
                item,
                move |this, t| {
                    let rest = if disabled {
                        colors::GRAY_600
                    } else {
                        crate::theme::ink::muted()
                    };
                    this.bg(chosen.opacity(0).mix(chosen, t))
                        .text_color(rest.mix(chosen_text, t))
                },
            )
        }))
}
