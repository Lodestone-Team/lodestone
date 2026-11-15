//! The steps of the create dialogs, down a narrow rail (`WizardSteps.tsx`): a check
//! for each step that's done, the current step's number filled, the steps to come
//! muted. A step that's done can be clicked to go back to it.

use gpui::{
    AnyElement, App, IntoElement, ParentElement, SharedString, Styled, Window, div, prelude::*,
};
use lodestone_ui::{
    components::{Accessible as _, Icon, KeyboardFocus as _},
    theme::{colors, ink, px, radius, text, weights},
};

/// Going back to a step that's done.
pub type StepHandler = std::rc::Rc<dyn Fn(usize, &mut Window, &mut App)>;

/// The rail. `on_select(i)` is offered for the steps before `current` that `can_select`.
pub fn rail(
    steps: &[&'static str],
    current: usize,
    can_select: impl Fn(usize) -> bool,
    on_select: Option<StepHandler>,
) -> AnyElement {
    let count = steps.len();
    div()
        .flex_none()
        .w(px(144.))
        .h_full()
        .flex()
        .flex_col()
        .gap(px(4.))
        .px(px(8.))
        .pt(px(32.))
        .border_r_1()
        .border_color(colors::GRAY_700)
        .children(steps.iter().enumerate().map(|(i, label)| {
            let done = i < current;
            let active = i == current;
            let clickable = done && on_select.is_some() && can_select(i);
            let marker = div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(20.))
                .rounded_full()
                .text_size(text::CAPTION)
                .font_weight(weights::BOLD.face())
                .map(|this| {
                    if done {
                        this.bg(colors::GREEN_300)
                            .child(Icon::Check.render(px(10.), colors::GRAY_900))
                    } else if active {
                        this.bg(lodestone_ui::theme::accent::base())
                            .text_color(colors::WHITE)
                            .child((i + 1).to_string())
                    } else {
                        this.border_1()
                            .border_color(colors::GRAY_FADED.opacity(30))
                            .bg(colors::GRAY_850)
                            .text_color(lodestone_ui::theme::ink::muted())
                            .child((i + 1).to_string())
                    }
                });
            let on_select = on_select.clone();
            div()
                .id(SharedString::from(format!("wizard-step-{i}")))
                .relative()
                .flex()
                .flex_row()
                .items_start()
                .gap(px(8.))
                .p(px(8.))
                .rounded(radius::LG)
                .text_size(text::SMALL)
                .font_weight(weights::MEDIUMBOLD.face())
                .text_color(if active {
                    ink::text()
                } else if done {
                    colors::GRAY_400
                } else {
                    lodestone_ui::theme::ink::muted()
                })
                .when(active, |this| this.bg(colors::GRAY_800))
                .when(i + 1 < count, |this| {
                    // the line down to the next step's circle
                    this.child(
                        div()
                            .absolute()
                            .top(px(30.))
                            .bottom(px(-6.))
                            .left(px(17.5))
                            .w(px(1.))
                            .bg(if done {
                                colors::GREEN_300.opacity(60)
                            } else {
                                colors::GRAY_FADED.opacity(30)
                            }),
                    )
                })
                .child(marker)
                .child(div().min_w_0().pt(px(2.)).child(*label))
                .when(clickable, |this| {
                    this.accessible(gpui::Role::Button, format!("wizard-step-{i}"), *label)
                        .keyboard_focus()
                        .cursor_pointer()
                        .hover(|style| style.text_color(ink::text()))
                        .when_some(on_select, |this, on_select| {
                            this.on_press(move |_, window, cx| on_select(i, window, cx))
                        })
                })
                .into_any_element()
        }))
        .into_any_element()
}

/// A step's title and the line under it (`StepTitle`).
pub fn title(title: &'static str, description: impl Into<SharedString>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .mb(px(16.))
        .child(
            div()
                .text_size(text::H2)
                .font_weight(weights::EXTRABOLD.face())
                .text_color(ink::text())
                .child(title),
        )
        .child(
            div()
                .text_size(text::MEDIUM)
                .italic()
                .text_color(ink::muted())
                .child(description.into()),
        )
        .into_any_element()
}
