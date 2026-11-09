//! The parts both Settings pages are built from: sections with a heading, cards of
//! rows (the name and a hint on the left, the control on the right), small tags,
//! the list of sections beside the page, a select button with its popover, and the
//! short notice that says a change was saved.

use crate::skin::Skinned;
use crate::theme::px;
use launcher_core::ui_skin::SurfaceId;
use std::{rc::Rc, time::Duration};

use gpui::{
    Anchor, AnimationExt, AnyElement, App, Context, Div, ElementId, IntoElement, Pixels,
    RenderOnce, ScrollHandle, SharedString, Stateful, Task, Window, anchored, deferred, div,
    prelude::*,
};

use crate::{
    components::{
        ArrowGroup, FocusScope, Icon,
        a11y::{Accessible, Role, automation_id},
        focus::KeyboardFocus,
    },
    i18n::{CommonMsg, t},
    theme::{self, Color, colors, fonts, leading, radius, shadow, text, weights},
};

/// The width of the column the settings sit in.
pub fn content_width() -> Pixels {
    px(680.)
}
/// The width of the list of sections beside it.
pub fn nav_width() -> Pixels {
    px(176.)
}

/// A section: its heading and an optional line under it, then its cards.
pub fn section(title: impl Into<SharedString>, lead: Option<SharedString>) -> Div {
    let title: SharedString = title.into();
    div().flex().flex_col().gap_2().w_full().child(
        div()
            .flex()
            .flex_col()
            .gap_0p5()
            .child(
                div()
                    .id(title.clone())
                    .role(Role::Heading)
                    .aria_level(2)
                    .aria_label(title.clone())
                    .font_family(fonts::heading())
                    .text_size(theme::text_size(text::H3))
                    .font_weight(weights::BOLD.into())
                    .text_color(crate::theme::ink::text())
                    .child(title),
            )
            .when_some(lead, |this, lead| {
                this.child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                        .text_color(crate::theme::ink::muted())
                        .child(lead),
                )
            }),
    )
}

/// Rows in a rounded box, with a line between each.
pub fn card(rows: impl IntoIterator<Item = AnyElement>) -> Div {
    let mut card = div()
        .flex()
        .flex_col()
        .w_full()
        .overflow_hidden()
        .rounded(radius::XL)
        .border_1()
        .border_color(crate::theme::ink::border())
        .bg(colors::GRAY_850)
        .surface(SurfaceId::SettingCard);
    for (index, row) in rows.into_iter().enumerate() {
        card = card.child(
            div()
                .when(index > 0, |this| {
                    this.border_t_1().border_color(crate::theme::ink::border())
                })
                .child(row),
        );
    }
    card
}

/// One setting: its name and hint on the left and its control on the right, with an
/// optional note (an error or a warning) and content under them.
#[derive(IntoElement)]
pub struct SettingRow {
    /// Its element id, when its name may not be unique (rows named after instances or
    /// accounts); the name otherwise.
    key: Option<ElementId>,
    label: SharedString,
    label_color: Color,
    hint: Option<SharedString>,
    control: Vec<AnyElement>,
    note: Option<(SharedString, Color)>,
    below: Option<AnyElement>,
    leading: Option<AnyElement>,
    indent: bool,
    /// The name can be selected, for a name worth copying such as an account's.
    selectable_label: bool,
}

impl SettingRow {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            key: None,
            label: label.into(),
            label_color: colors::GRAY_300,
            hint: None,
            control: Vec::new(),
            note: None,
            below: None,
            leading: None,
            indent: false,
            selectable_label: false,
        }
    }

    /// Lets the name be selected and copied. Not for names that label a control.
    pub fn selectable_label(mut self) -> Self {
        self.selectable_label = true;
        self
    }

    /// An element id for a row whose name another row may share, such as one named
    /// after an instance: assistive technology needs each row to be unique.
    pub fn key(mut self, key: impl Into<ElementId>) -> Self {
        self.key = Some(key.into());
        self
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn hint_opt(mut self, hint: Option<impl Into<SharedString>>) -> Self {
        self.hint = hint.map(Into::into);
        self
    }

    /// A red name, for deleting.
    pub fn danger(mut self) -> Self {
        self.label_color = colors::RED_200;
        self
    }

    /// A row inside an opened row, for the values of a category.
    pub fn indent(mut self) -> Self {
        self.indent = true;
        self
    }

    /// Something before the name, such as an avatar.
    pub fn leading(mut self, element: impl IntoElement) -> Self {
        self.leading = Some(element.into_any_element());
        self
    }

    /// Adds a control on the right.
    pub fn child(mut self, control: impl IntoElement) -> Self {
        self.control.push(control.into_any_element());
        self
    }

    pub fn note(mut self, text_: impl Into<SharedString>, color: Color) -> Self {
        self.note = Some((text_.into(), color));
        self
    }

    pub fn note_opt(mut self, note: Option<(SharedString, Color)>) -> Self {
        self.note = note;
        self
    }

    /// Content under the row, across its whole width.
    pub fn below(mut self, content: impl IntoElement) -> Self {
        self.below = Some(content.into_any_element());
        self
    }
}

impl RenderOnce for SettingRow {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        // A group named after the row, so its controls (a switch, a slider, a value)
        // are found under the setting they change.
        let id = self
            .key
            .clone()
            .unwrap_or_else(|| self.label.clone().into());
        div()
            .id(id.clone())
            .role(Role::Group)
            .accessibility_id(automation_id(&id))
            .aria_label(self.label.clone())
            .when_some(self.hint.clone(), |this, hint| this.aria_description(hint))
            .flex()
            .flex_col()
            .gap_2()
            .when(self.indent, |this| {
                this.pl_8().pr_4().py_2().bg(colors::GRAY_875)
            })
            .when(!self.indent, |this| this.px_4().py_3())
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap_x_4()
                    .gap_y_2()
                    .min_h(px(32.))
                    .children(self.leading)
                    .child(
                        // The name keeps a readable width; in a narrow window the
                        // controls wrap under it instead.
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(160.))
                            .child(
                                div()
                                    .font_weight(weights::BOLD.into())
                                    .text_color(self.label_color)
                                    .child(
                                        super::RichText::new(self.label)
                                            .selectable(self.selectable_label),
                                    ),
                            )
                            .when_some(self.hint, |this, hint| {
                                this.child(
                                    div()
                                        .text_size(theme::text_size(text::SMALL))
                                        .line_height(theme::line_height(
                                            text::SMALL,
                                            leading::NORMAL,
                                        ))
                                        .text_color(crate::theme::ink::muted())
                                        .child(super::RichText::new(hint)),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .flex_none()
                            .ml_auto()
                            .items_center()
                            .justify_end()
                            .gap_2()
                            .children(self.control),
                    ),
            )
            .when_some(self.note, |this, (note, color)| {
                this.child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                        .font_weight(weights::BOLD.into())
                        .text_color(color)
                        .child(super::RichText::new(note)),
                )
            })
            .children(self.below)
    }
}

/// A value shown in a row, which can be copied. Inherited values are dimmer.
#[track_caller]
pub fn value(text_: impl Into<SharedString>, inherited: bool) -> Div {
    div()
        .max_w(px(280.))
        .truncate()
        .text_color(if inherited {
            colors::GRAY_400
        } else {
            colors::GRAY_300
        })
        .child(super::RichText::new(text_))
}

/// A small outlined label, such as "Default" or "Recommended".
pub fn tag(text_: impl Into<SharedString>, color: Color) -> Div {
    div()
        .flex_none()
        .px_1p5()
        .rounded(radius::MD)
        .border_1()
        .border_color(color.opacity(35))
        .text_size(theme::text_size(text::CAPTION))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .font_weight(weights::BOLD.into())
        .text_color(color)
        .child(text_.into())
}

/// A blue text action, such as "Change".
pub fn link(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    link_colored(id, label, crate::theme::accent::light())
}

/// A grey text action, such as "Reset".
pub fn muted_link(id: impl Into<ElementId>, label: impl Into<SharedString>) -> Stateful<Div> {
    link_colored(id, label, colors::GRAY_500)
}

fn link_colored(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    color: Color,
) -> Stateful<Div> {
    let id = id.into();
    let label: SharedString = label.into();
    div()
        .id(id.clone())
        .accessible_id(Role::Button, &id, label.clone())
        .keyboard_focus()
        // Room for the focus ring around the text, without moving it.
        .px_1()
        .py_0p5()
        .mx(px(-4.))
        .my(px(-2.))
        .rounded(radius::DEFAULT)
        .flex_none()
        .cursor_pointer()
        .font_weight(weights::BOLD.into())
        .text_size(theme::text_size(text::SMALL))
        .text_color(color)
        .hover(|style| style.text_color(crate::theme::ink::text()))
        .child(label)
}

/// A button that opens a list, styled like a text field. Name it for assistive
/// technology with `.aria_label(…)` and give the choice shown as `.aria_value(…)`.
pub fn select_button(id: impl Into<ElementId>, label: impl IntoElement) -> Stateful<Div> {
    let id = id.into();
    div()
        .id(id.clone())
        .role(Role::ComboBox)
        .accessibility_id(automation_id(&id))
        .keyboard_focus_outer()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .h(px(32.))
        .px_3()
        .rounded(radius::LG)
        .border_1()
        .border_color(colors::GRAY_700)
        .bg(colors::GRAY_900)
        .cursor_pointer()
        .hover(|style| style.border_color(colors::GRAY_600))
        .child(div().flex().flex_row().items_center().gap_2().child(label))
        .child(Icon::CaretDown.render(px(10.), colors::GRAY_500))
}

/// A list drawn over the page under the control it is placed after. Place it in a
/// column with `items_end` after the control: the anchored element takes the
/// panel's width in that column, so its left edge is where the panel starts, and
/// the panel lines up with the control's right edge.
///
/// It takes focus when it opens and keeps Tab inside it; Escape gives focus back to
/// the control and calls `on_dismiss`, which should close it.
pub fn popover(
    id: impl Into<ElementId>,
    panel: impl IntoElement,
    on_dismiss: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    deferred(
        anchored()
            .anchor(Anchor::TopLeft)
            .snap_to_window_with_margin(px(8.))
            .child(
                FocusScope::new(id)
                    .mt_1()
                    .arrow_keys()
                    .on_dismiss(on_dismiss)
                    .child(panel),
            ),
    )
    .with_priority(1)
}

/// The box a popover's list sits in.
pub fn popover_panel(id: impl Into<ElementId>) -> Stateful<Div> {
    let id = id.into();
    div()
        .id(id.clone())
        .role(Role::List)
        .accessibility_id(automation_id(&id))
        .occlude()
        .flex()
        .flex_col()
        .p_1()
        .rounded(radius::LG)
        .border_1()
        .border_color(colors::GRAY_FADED.opacity(30))
        .bg(colors::GRAY_800)
        .surface(SurfaceId::Popover)
        .shadow(vec![gpui::BoxShadow {
            color: shadow::COLOR.into(),
            offset: gpui::point(px(0.), px(4.)),
            blur_radius: px(12.),
            spread_radius: px(0.),
            inset: false,
        }])
}

/// Where the list of sections points: the section at the top of the page, or the
/// one last picked until the page is scrolled by hand.
#[derive(Default)]
pub struct SectionNav {
    pub scroll: ScrollHandle,
    picked: Option<usize>,
}

impl SectionNav {
    pub fn active(&self, count: usize) -> usize {
        let last = count.saturating_sub(1);
        if let Some(picked) = self.picked {
            return picked.min(last);
        }
        let scrolled = -self.scroll.offset().y;
        let max = self.scroll.max_offset().y;
        if max > px(0.) && scrolled >= max - px(4.) {
            return last;
        }
        self.scroll.top_item().min(last)
    }

    pub fn pick(&mut self, index: usize) {
        self.picked = Some(index);
        self.scroll.scroll_to_top_of_item(index);
    }

    /// The page was scrolled by hand.
    pub fn scrolled(&mut self) {
        self.picked = None;
    }
}

/// The list of sections beside a Settings page. Up and Down move between them.
pub fn section_list(
    id: &'static str,
    titles: &[SharedString],
    active: usize,
    on_pick: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_pick = Rc::new(on_pick);
    let count = titles.len();
    ArrowGroup::new((id, usize::MAX))
        .group_role(Role::TabList)
        .flex()
        .flex_col()
        .flex_none()
        .gap_0p5()
        .w(nav_width())
        .children(titles.iter().enumerate().map(|(index, title)| {
            let on_pick = on_pick.clone();
            let selected = index == active;
            div()
                .id((id, index))
                .accessible(Role::Tab, format!("{id}-{index}"), title.clone())
                .aria_selected(selected)
                .aria_position_in_set(index + 1)
                .aria_size_of_set(count)
                .keyboard_focus()
                .px_2p5()
                .py_1p5()
                .rounded(radius::LG)
                .cursor_pointer()
                .font_weight(weights::BOLD.into())
                .map(|this| {
                    if selected {
                        this.bg(colors::GRAY_850)
                            .text_color(crate::theme::ink::text())
                    } else {
                        this.text_color(crate::theme::ink::muted())
                            .hover(|style| style.text_color(crate::theme::ink::text()))
                    }
                })
                .child(title.clone())
                .on_click(move |_, window, cx| on_pick(index, window, cx))
        }))
}

/// The short notice that says a change was saved, or why it wasn't.
#[derive(Default)]
pub struct Toast {
    message: Option<(SharedString, bool)>,
    /// A link after the message, such as Undo, and what it does.
    action: Option<(SharedString, ToastAction)>,
    /// Counts the messages shown, so each one animates in.
    serial: u64,
    /// The message is animating out.
    leaving: bool,
    _hide: Option<Task<()>>,
}

/// How long the toast takes to come in and to go.
const TOAST_ENTER: Duration = Duration::from_millis(200);
const TOAST_LEAVE: Duration = Duration::from_millis(150);
/// How far it rises as it comes in (and drops as it goes).
const TOAST_RISE: f32 = 10.;

/// What a toast's link does.
pub type ToastAction = Rc<dyn Fn(&mut Window, &mut App)>;

impl Toast {
    /// Shows `message` for a few seconds. `get` finds this toast in the view.
    pub fn show<V: 'static>(
        &mut self,
        message: impl Into<SharedString>,
        error: bool,
        get: fn(&mut V) -> &mut Toast,
        cx: &mut Context<V>,
    ) {
        self.show_with(message, error, None, get, cx);
    }

    /// Shows `message` with a link after it, such as Redo after Undo.
    pub fn show_with<V: 'static>(
        &mut self,
        message: impl Into<SharedString>,
        error: bool,
        action: Option<(SharedString, ToastAction)>,
        get: fn(&mut V) -> &mut Toast,
        cx: &mut Context<V>,
    ) {
        self.action = action;
        self.message = Some((message.into(), error));
        self.serial += 1;
        self.leaving = false;
        let shown = Duration::from_millis(if error { 6000 } else { 2500 });
        self._hide = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(shown).await;
            let still = this.update(cx, |view, cx| {
                get(view).leaving = true;
                cx.notify();
            });
            if still.is_err() {
                return;
            }
            cx.background_executor().timer(TOAST_LEAVE).await;
            this.update(cx, |view, cx| {
                get(view).message = None;
                get(view).leaving = false;
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Drawn at the bottom of the page, centred. The page must be `relative`. It fades
    /// in as it rises into place, and fades out as it drops (only fades, with reduced
    /// motion).
    pub fn render(&self) -> Option<impl IntoElement + use<>> {
        let (message, error) = self.message.clone()?;
        let action = self.action.clone();
        let rise = if super::dialog::prefers_reduced_motion() {
            0.
        } else {
            TOAST_RISE
        };
        let leaving = self.leaving;
        let animation = if leaving {
            gpui::Animation::new(TOAST_LEAVE).with_easing(|t| t * t)
        } else {
            gpui::Animation::new(TOAST_ENTER).with_easing(|t| 1. - (1. - t).powi(3))
        };
        let pill = div()
            .id("toast")
            .accessible(Role::Status, "toast", message.clone())
            .relative()
            .occlude()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .max_w(px(560.))
            .px_3()
            .py_1p5()
            .rounded(radius::LG)
            .border_1()
            .border_color(crate::theme::ink::border())
            .bg(colors::GRAY_750)
            .surface(SurfaceId::Toast)
            .shadow(vec![gpui::BoxShadow {
                color: shadow::COLOR.into(),
                offset: gpui::point(px(0.), px(4.)),
                blur_radius: px(12.),
                spread_radius: px(0.),
                inset: false,
            }])
            .text_size(theme::text_size(text::SMALL))
            .font_weight(weights::BOLD.into())
            .child(if error {
                Icon::TriangleExclamation.render(px(12.), colors::RED_200)
            } else {
                Icon::Check.render(px(12.), colors::GREEN_300)
            })
            .child(
                div()
                    .text_color(
                        crate::skin::painted(SurfaceId::Toast)
                            .and_then(|p| p.text)
                            .unwrap_or_else(crate::theme::ink::text),
                    )
                    // Messages name packs and worlds, which may have `§` codes.
                    .child(super::RichText::minecraft(message)),
            )
            .when_some(action, |pill, (label, action)| {
                pill.child(
                    link("toast-action", label)
                        .ml_1()
                        .on_click(move |_, window, cx| action(window, cx)),
                )
            })
            .with_animation(
                ElementId::from(("toast", (self.serial * 2 + leaving as u64) as usize)),
                animation,
                move |pill, delta| {
                    // in: 0 → 1 rises from below; out: 1 → 0 drops back
                    let shown = if leaving { 1. - delta } else { delta };
                    pill.opacity(shown).top(px(rise * (1. - shown)))
                },
            );
        Some(
            div()
                .absolute()
                .bottom_5()
                .left_0()
                .right_0()
                .flex()
                .flex_row()
                .justify_center()
                .child(pill),
        )
    }
}

/// The button that opens the list of sections in a window too narrow for it: the
/// current section's name after a menu icon.
pub fn section_menu_button(
    id: impl Into<ElementId>,
    title: impl Into<SharedString>,
) -> Stateful<Div> {
    let id = id.into();
    let title: SharedString = title.into();
    div()
        .id(id.clone())
        .accessible_id(Role::Button, &id, title.clone())
        .keyboard_focus()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .gap_2()
        .h(px(32.))
        .px_2p5()
        .rounded(radius::LG)
        .cursor_pointer()
        .font_weight(weights::BOLD.into())
        .text_color(crate::theme::ink::text())
        .hover(|style| style.bg(colors::GRAY_800))
        .child(Icon::Bars.render(px(13.), colors::GRAY_500))
        .child(title)
}

/// A panel over the left of an area, with the rest dimmed; a click on the dimmed
/// part, or Escape, calls `on_close`. Place it as the last child of a `relative`
/// element. It takes focus when it opens and keeps Tab inside it.
pub fn left_overlay(
    id: &'static str,
    panel: impl IntoElement,
    on_close: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let on_close = Rc::new(on_close);
    let on_escape = on_close.clone();
    FocusScope::new(id)
        .arrow_keys()
        .on_dismiss(move |window, cx| on_escape(&gpui::ClickEvent::default(), window, cx))
        .absolute()
        .inset_0()
        .occlude()
        .child(
            div()
                .id((id, 0usize))
                .accessible(Role::Button, format!("{id}-close"), t(CommonMsg::Close))
                .absolute()
                .inset_0()
                .bg(colors::GRAY_900.opacity(60))
                .on_click(move |event, window, cx| on_close(event, window, cx)),
        )
        .child(
            div()
                .id((id, 1usize))
                .occlude()
                .absolute()
                .top_0()
                .bottom_0()
                .left_0()
                .w(px(232.))
                .p_3()
                .overflow_y_scroll()
                .bg(colors::GRAY_875)
                .border_r_1()
                .border_color(crate::theme::ink::border())
                .surface(SurfaceId::SettingsNav)
                .shadow(vec![gpui::BoxShadow {
                    color: shadow::COLOR.into(),
                    offset: gpui::point(px(4.), px(0.)),
                    blur_radius: px(16.),
                    spread_radius: px(0.),
                    inset: false,
                }])
                .child(panel),
        )
}

/// The bar that says a page has unsaved changes, with Discard and Save, drawn at
/// the bottom of the page like the toast. The page must be `relative`.
pub fn unsaved_bar(
    id: &'static str,
    message: impl Into<SharedString>,
    can_save: bool,
    on_discard: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
    on_save: impl Fn(&gpui::ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let reduce_motion = super::dialog::prefers_reduced_motion();
    let message: SharedString = message.into();
    div()
        .absolute()
        .bottom_5()
        .left_0()
        .right_0()
        .flex()
        .flex_row()
        .justify_center()
        .px_4()
        .child(
            div()
                .id(id)
                .accessible(Role::Status, id, message.clone())
                .occlude()
                .relative()
                .flex()
                .flex_row()
                .items_center()
                .gap_4()
                .w_full()
                .max_w(px(600.))
                .pl_4()
                .pr_2()
                .py_2()
                .rounded(radius::XL)
                .border_1()
                .border_color(crate::theme::ink::border())
                .bg(colors::GRAY_750)
                // A soft shadow, just enough to lift it off the page.
                .shadow(vec![gpui::BoxShadow {
                    color: shadow::COLOR.opacity(35).into(),
                    offset: gpui::point(px(0.), px(2.)),
                    blur_radius: px(10.),
                    spread_radius: px(0.),
                    inset: false,
                }])
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .font_weight(weights::BOLD.into())
                        .text_color(crate::theme::ink::text())
                        .child(message),
                )
                .child(muted_link((id, 0usize), t(CommonMsg::Discard)).on_click(on_discard))
                .child(
                    crate::components::Button::new((id, 1usize), t(CommonMsg::Save))
                        .variant(crate::components::ButtonVariant::Primary)
                        .size(crate::components::ButtonSize::Slim)
                        .disabled(!can_save)
                        .on_click(on_save),
                )
                .with_animation(
                    (id, 2usize),
                    gpui::Animation::new(TOAST_ENTER).with_easing(|t| 1. - (1. - t).powi(3)),
                    move |bar, delta| {
                        let rise = if reduce_motion { 0. } else { TOAST_RISE };
                        bar.opacity(delta).top(px(rise * (1. - delta)))
                    },
                ),
        )
}
