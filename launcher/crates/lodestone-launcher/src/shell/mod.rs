//! The main window: a top bar, the sidebar and the main area, as in the dashboard app.

mod home;
mod sidebar;
mod top_bar;

pub use top_bar::TOP_BAR_HEIGHT;

use gpui::{Context, FocusHandle, IntoElement, Render, Window, div, prelude::*};

use crate::{
    components::{Button, ButtonVariant, Dialog},
    theme::{self, colors, fonts, leading, text},
};

/// What the main area shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Home,
    Settings,
}

/// The root view of the launcher window.
pub struct LauncherShell {
    page: Page,
    new_instance_open: bool,
    dialog_focus: FocusHandle,
    /// Set by a mouse down on the top bar; the next mouse move starts a window drag.
    top_bar_drag_pending: bool,
}

impl LauncherShell {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            page: Page::default(),
            new_instance_open: false,
            dialog_focus: cx.focus_handle(),
            top_bar_drag_pending: false,
        }
    }

    fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    fn open_new_instance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.new_instance_open = true;
        window.focus(&self.dialog_focus, cx);
        cx.notify();
    }

    fn close_new_instance(&mut self, cx: &mut Context<Self>) {
        self.new_instance_open = false;
        cx.notify();
    }

    fn new_instance_dialog(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let close = cx.listener(|this, _, _, cx| this.close_new_instance(cx));
        Dialog::new("new-instance", "New instance", self.dialog_focus.clone())
            .on_close({
                let entity = cx.entity();
                move |_, cx| entity.update(cx, |this, cx| this.close_new_instance(cx))
            })
            .child("Creating instances comes in a later milestone of Lodestone Launcher.")
            .footer(
                Button::new("close-new-instance", "Close")
                    .variant(ButtonVariant::Secondary)
                    .full_width()
                    .on_click(close),
            )
    }
}

impl Render for LauncherShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main = match self.page {
            Page::Home => home::render(cx).into_any_element(),
            Page::Settings => home::render_settings().into_any_element(),
        };

        div()
            .id("launcher-shell")
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(colors::GRAY_875)
            .font_family(fonts::SANS)
            .text_color(colors::GRAY_300)
            .text_size(theme::text_size(text::MEDIUM))
            .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
            .child(top_bar::render(cx))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .child(sidebar::render(self, cx))
                    .child(div().flex().flex_1().min_w_0().h_full().mx_8().child(main)),
            )
            .when(self.new_instance_open, |this| {
                this.child(self.new_instance_dialog(cx))
            })
    }
}
