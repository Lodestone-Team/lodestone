//! "Add offline account": a username for single-player and offline-mode servers.
//! Only offered in debug builds or with `LODESTONE_LAUNCHER_OFFLINE=1`.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Window, div, prelude::*,
};
use launcher_core::accounts::Account;

use super::Accounts;
use crate::{
    components::{Button, ButtonVariant, Dialog, TextInput},
    theme::{self, colors, leading, text, weights},
};

/// Shown wherever an offline account appears.
pub const OFFLINE_NOTE: &str = "Offline – single-player and offline-mode servers only";

pub struct OfflineDialog {
    accounts: Entity<Accounts>,
    input: Entity<TextInput>,
    focus_handle: FocusHandle,
    error: Option<SharedString>,
}

pub struct OfflineDialogDismissed;

impl EventEmitter<OfflineDialogDismissed> for OfflineDialog {}

impl Focusable for OfflineDialog {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        // Typing goes straight into the username field.
        self.input.focus_handle(cx)
    }
}

impl OfflineDialog {
    pub fn new(accounts: Entity<Accounts>, cx: &mut Context<Self>) -> Self {
        Self {
            accounts,
            input: cx.new(|cx| TextInput::new(cx).with_placeholder("Username")),
            focus_handle: cx.focus_handle(),
            error: None,
        }
    }

    /// Fills in the username, for the offscreen screenshots.
    pub fn with_username(self, name: &str, cx: &mut Context<Self>) -> Self {
        self.input
            .update(cx, |input, cx| input.set_text(name.to_string(), cx));
        self
    }

    fn add(&mut self, cx: &mut Context<Self>) {
        let name = self.input.read(cx).text().trim().to_string();
        match Account::offline(&name) {
            Ok(account) => {
                self.accounts
                    .update(cx, |accounts, cx| accounts.add_offline(account, cx));
                cx.emit(OfflineDialogDismissed);
            }
            Err(err) => {
                self.error = Some(err.to_string().into());
                cx.notify();
            }
        }
    }
}

impl Render for OfflineDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        Dialog::new("offline-account", "Add offline account", self.focus_handle.clone())
            .on_close(move |_, cx| entity.update(cx, |_, cx| cx.emit(OfflineDialogDismissed)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_size(theme::text_size(text::MEDIUM))
                            .text_color(colors::GRAY_400)
                            .child(
                                "For development and testing. The account has no Microsoft sign-in, and its UUID is the one offline-mode servers give this name.",
                            ),
                    )
                    .child(self.input.clone())
                    .child(
                        div()
                            .text_size(theme::text_size(text::SMALL))
                            .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                            .font_weight(weights::BOLD.into())
                            .map(|this| match &self.error {
                                Some(error) => this.text_color(colors::RED_200).child(error.clone()),
                                None => this
                                    .text_color(colors::GRAY_500)
                                    .child("3 to 16 letters, numbers or underscores."),
                            }),
                    )
                    .child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded(theme::radius::LG)
                            .bg(colors::GRAY_800)
                            .text_size(theme::text_size(text::SMALL))
                            .font_weight(weights::BOLD.into())
                            .text_color(colors::YELLOW)
                            .child(OFFLINE_NOTE),
                    ),
            )
            .footer(
                Button::new("add-offline", "Add account")
                    .variant(ButtonVariant::Primary)
                    .on_click(cx.listener(|this, _, _, cx| this.add(cx))),
            )
            .footer(
                Button::new("cancel-offline", "Cancel")
                    .variant(ButtonVariant::Secondary)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(OfflineDialogDismissed))),
            )
    }
}
