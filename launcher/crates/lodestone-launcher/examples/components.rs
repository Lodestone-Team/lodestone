//! A gallery of the launcher's components, one section per component.
//!
//! `cargo run --example components`, with `--dialog` to start with the dialog open.

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, IntoElement, Render,
    SharedString, Window, div, prelude::*,
};
use lodestone_launcher::{
    assets::Assets,
    components::{
        Align, Button, ButtonSize, ButtonVariant, Dialog, Icon, IconButton, ListRow, Tabs,
        TextInput, Toggle,
    },
    theme::{self, colors, fonts, leading, text, weights},
};

struct Gallery {
    name: Entity<TextInput>,
    filled: Entity<TextInput>,
    disabled_input: Entity<TextInput>,
    notifications: bool,
    auto_update: bool,
    tab: usize,
    selected_row: usize,
    dialog_open: bool,
    dialog_focus: FocusHandle,
}

impl Gallery {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            name: cx.new(|cx| TextInput::new(cx).with_placeholder("Instance name")),
            filled: cx.new(|cx| TextInput::new(cx).with_text("Survival with friends")),
            disabled_input: cx.new(|cx| {
                TextInput::new(cx)
                    .with_text("Can't edit this")
                    .with_disabled(true)
            }),
            notifications: true,
            auto_update: false,
            tab: 0,
            selected_row: 0,
            dialog_open: std::env::args().any(|arg| arg == "--dialog"),
            dialog_focus: cx.focus_handle(),
        }
    }
}

/// A titled section of the gallery.
fn section(title: &'static str, content: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .font_family(fonts::HEADING)
                .text_size(theme::text_size(text::H2))
                .line_height(theme::line_height(text::H2, leading::TIGHT))
                .font_weight(weights::BOLD.into())
                .child(title),
        )
        .child(content)
}

fn row(children: impl IntoIterator<Item = AnyElement>) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_3()
        .children(children)
}

fn caption(label: impl Into<SharedString>) -> impl IntoElement {
    div()
        .w_24()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .text_color(colors::GRAY_FADED.opacity(30))
        .child(label.into())
}

impl Gallery {
    fn close_dialog(&mut self, cx: &mut Context<Self>) {
        self.dialog_open = false;
        cx.notify();
    }

    fn buttons(&self) -> impl IntoElement {
        let variants = [
            ("Primary", ButtonVariant::Primary),
            ("Secondary", ButtonVariant::Secondary),
            ("Danger", ButtonVariant::Danger),
            ("Ghost", ButtonVariant::Ghost),
        ];
        let rows = variants.into_iter().map(|(name, variant)| {
            row([
                caption(name).into_any_element(),
                Button::new((name, 0usize), name)
                    .variant(variant)
                    .into_any_element(),
                Button::new((name, 1usize), "With icon")
                    .variant(variant)
                    .icon(Icon::SquarePlus)
                    .into_any_element(),
                Button::new((name, 2usize), "Disabled")
                    .variant(variant)
                    .disabled(true)
                    .into_any_element(),
                Button::new((name, 3usize), "Loading")
                    .variant(variant)
                    .loading(true)
                    .into_any_element(),
            ])
            .into_any_element()
        });
        let sizes = row([
            caption("Sizes").into_any_element(),
            Button::new("slim", "Slim")
                .size(ButtonSize::Slim)
                .into_any_element(),
            Button::new("medium", "Medium").into_any_element(),
            Button::new("large", "Large")
                .size(ButtonSize::Large)
                .into_any_element(),
            div()
                .w_48()
                .child(
                    Button::new("between", "Account")
                        .icon(Icon::User)
                        .icon_right(Icon::CaretDown)
                        .align(Align::Between)
                        .full_width(),
                )
                .into_any_element(),
            IconButton::new("gear", Icon::Gear).into_any_element(),
            IconButton::new("gear-disabled", Icon::Gear)
                .disabled(true)
                .into_any_element(),
        ]);
        section(
            "Button",
            div().flex().flex_col().gap_3().children(rows).child(sizes),
        )
    }

    fn inputs(&self) -> impl IntoElement {
        section(
            "TextInput",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .w_80()
                .child(self.name.clone())
                .child(self.filled.clone())
                .child(self.disabled_input.clone()),
        )
    }

    fn toggles(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let labelled = |label: &'static str, toggle: Toggle| {
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(toggle)
                .child(label)
        };
        section(
            "Toggle",
            row([
                labelled(
                    "Notifications",
                    Toggle::new("notifications", self.notifications).on_change(cx.listener(
                        |this, value, _, cx| {
                            this.notifications = *value;
                            cx.notify();
                        },
                    )),
                )
                .into_any_element(),
                labelled(
                    "Auto-update",
                    Toggle::new("auto-update", self.auto_update).on_change(cx.listener(
                        |this, value, _, cx| {
                            this.auto_update = *value;
                            cx.notify();
                        },
                    )),
                )
                .into_any_element(),
                labelled("Disabled", Toggle::new("disabled", true).disabled(true))
                    .into_any_element(),
            ]),
        )
    }

    fn tabs(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let labels = ["General", "Java", "Mods", "Advanced"];
        section(
            "Tabs",
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    Tabs::new("tabs", labels)
                        .selected(self.tab)
                        .on_select(cx.listener(|this, index, _, cx| {
                            this.tab = *index;
                            cx.notify();
                        })),
                )
                .child(
                    div()
                        .text_color(colors::GRAY_FADED.opacity(30))
                        .child(format!("{} settings go here.", labels[self.tab])),
                ),
        )
    }

    fn list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let rows = [
            ("Vanilla 1.21.4", "Ready"),
            ("Fabric 1.20.1", "Updating"),
            ("A very long instance name that does not fit", ""),
        ];
        section(
            "ListRow",
            div()
                .flex()
                .flex_col()
                .gap_y_1()
                .w_64()
                .p_2()
                .bg(colors::GRAY_850)
                .children(
                    rows.into_iter()
                        .enumerate()
                        .map(|(index, (label, detail))| {
                            ListRow::new(("row", index), label)
                                .icon(Icon::Expand)
                                .when(!detail.is_empty(), |row| row.detail(detail))
                                .selected(index == self.selected_row)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.selected_row = index;
                                    cx.notify();
                                }))
                        }),
                ),
        )
    }

    fn dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        section(
            "Dialog",
            row([Button::new("open-dialog", "Open dialog")
                .on_click(cx.listener(|this, _, window, cx| {
                    this.dialog_open = true;
                    window.focus(&this.dialog_focus, cx);
                    cx.notify();
                }))
                .into_any_element()]),
        )
    }
}

impl Render for Gallery {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        div()
            .relative()
            .size_full()
            .bg(colors::GRAY_875)
            .font_family(fonts::SANS)
            .text_color(colors::GRAY_300)
            .text_size(theme::text_size(text::MEDIUM))
            .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
            .child(
                div()
                    .id("gallery")
                    .size_full()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap_10()
                    .px_12()
                    // Clear the macOS traffic-light buttons.
                    .pt_12()
                    .pb_12()
                    .child(self.buttons())
                    .child(self.inputs())
                    .child(self.toggles(cx))
                    .child(self.tabs(cx))
                    .child(self.list(cx))
                    .child(self.dialog(cx)),
            )
            .when(self.dialog_open, |this| {
                this.child(
                    Dialog::new("dialog", "Delete instance?", self.dialog_focus.clone())
                        .on_close(move |_, cx| entity.update(cx, Self::close_dialog))
                        .child("This removes the instance and its worlds. It cannot be undone.")
                        .footer(
                            Button::new("cancel", "Cancel")
                                .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx))),
                        )
                        .footer(
                            Button::new("confirm", "Delete")
                                .variant(ButtonVariant::Danger)
                                .on_click(cx.listener(|this, _, _, cx| this.close_dialog(cx))),
                        ),
                )
            })
    }
}

fn main() {
    gpui_platform::application()
        .with_assets(Assets)
        .run(|cx: &mut App| {
            lodestone_launcher::init(cx);
            let options = lodestone_launcher::window_options(
                "Lodestone Launcher components",
                900.,
                1000.,
                cx,
            );
            lodestone_launcher::open_window(options, Gallery::new, cx);
        });
}
