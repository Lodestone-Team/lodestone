//! "Add server": a core's address, and optionally a username and password to sign
//! in with. The password is sent once, to mint this device's key, and never kept.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Window, div, prelude::*,
};
use launcher_core::wording;
use launcher_core::{Secret, cores::live::LauncherScope};

use crate::components::KeyboardFocus;
use crate::{
    components::{Button, ButtonVariant, Dialog, TextInput, Toggle},
    theme::{self, colors, leading, radius, text, weights},
};

/// How long the device key lasts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeyExpiry {
    Days90,
    #[default]
    Year,
    Never,
}

impl KeyExpiry {
    pub const ALL: [KeyExpiry; 3] = [KeyExpiry::Days90, KeyExpiry::Year, KeyExpiry::Never];

    pub fn label(self) -> &'static str {
        match self {
            KeyExpiry::Days90 => "90 days",
            KeyExpiry::Year => "1 year",
            KeyExpiry::Never => "Never",
        }
    }

    /// The expiry as unix seconds, from `now`.
    pub fn expires_at(self, now: i64) -> Option<i64> {
        match self {
            KeyExpiry::Days90 => Some(now + 90 * 24 * 3600),
            KeyExpiry::Year => Some(now + 365 * 24 * 3600),
            KeyExpiry::Never => None,
        }
    }
}

/// What "Add" asks for. No `Debug`: it holds the password.
#[derive(Clone)]
pub struct AddRequest {
    pub address: String,
    pub username: Option<String>,
    pub password: Option<Secret>,
    pub expires_at: Option<i64>,
    /// The console access asked for; the core still limits it to the user's own.
    pub console: LauncherScope,
}

pub enum AddServerEvent {
    Submit(AddRequest),
    Dismissed,
}

pub struct AddServerDialog {
    address: Entity<TextInput>,
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    expiry: KeyExpiry,
    console: LauncherScope,
    /// Signing in to a core that is already added.
    signing_in: bool,
    busy: bool,
    error: Option<SharedString>,
    focus_handle: FocusHandle,
}

impl EventEmitter<AddServerEvent> for AddServerDialog {}

impl Focusable for AddServerDialog {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        if self.signing_in {
            self.username.focus_handle(cx)
        } else {
            self.address.focus_handle(cx)
        }
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

impl AddServerDialog {
    /// `address` is set when signing in to a core that is already added.
    pub fn new(address: Option<String>, cx: &mut Context<Self>) -> Self {
        let signing_in = address.is_some();
        Self {
            address: cx.new(|cx| {
                let input = TextInput::new(cx).with_placeholder("192.168.1.20 or localhost:16662");
                match &address {
                    Some(address) => input.with_text(address.clone()).with_disabled(true),
                    None => input,
                }
            }),
            username: cx.new(|cx| TextInput::new(cx).with_placeholder("Username (optional)")),
            password: cx.new(|cx| {
                TextInput::new(cx)
                    .with_placeholder("Password")
                    .with_masked()
            }),
            expiry: KeyExpiry::default(),
            console: LauncherScope::default(),
            signing_in,
            busy: false,
            error: None,
            focus_handle: cx.focus_handle(),
        }
    }

    /// Fills in the fields, for the offscreen screenshots.
    pub fn preview(
        &mut self,
        address: &str,
        username: &str,
        password: &str,
        cx: &mut Context<Self>,
    ) {
        self.address
            .update(cx, |input, cx| input.set_text(address.to_string(), cx));
        self.username
            .update(cx, |input, cx| input.set_text(username.to_string(), cx));
        self.password
            .update(cx, |input, cx| input.set_text(password.to_string(), cx));
    }

    pub fn fail(&mut self, error: String, cx: &mut Context<Self>) {
        self.busy = false;
        self.error = Some(error.into());
        cx.notify();
    }

    /// Waiting again, after the certificate prompt was answered.
    pub fn set_busy(&mut self, cx: &mut Context<Self>) {
        self.busy = true;
        self.error = None;
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let address = self.address.read(cx).text().trim().to_string();
        let username = self.username.read(cx).text().trim().to_string();
        let password = self.password.read(cx).text().to_string();
        if address.is_empty() {
            return self.fail("Enter the core's address.".into(), cx);
        }
        if self.signing_in && username.is_empty() {
            return self.fail("Enter your username and password.".into(), cx);
        }
        if !username.is_empty() && password.is_empty() {
            return self.fail(
                "Enter your password, or leave the username empty.".into(),
                cx,
            );
        }
        let signing_in = !username.is_empty();
        self.busy = true;
        self.error = None;
        // the password leaves the field at once
        self.password
            .update(cx, |input, cx| input.set_text(String::new(), cx));
        cx.emit(AddServerEvent::Submit(AddRequest {
            address,
            username: signing_in.then_some(username),
            password: signing_in.then(|| Secret::new(password)),
            expires_at: self.expiry.expires_at(now()),
            console: self.console,
        }));
        cx.notify();
    }
}

impl AddServerDialog {
    /// Turns the console opt-ins on, for the offscreen screenshots.
    pub fn preview_console(&mut self, console: LauncherScope) {
        self.console = console;
    }

    /// The console opt-ins: off unless chosen, and never more than the user may do.
    fn console_choices(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let row = |title: &'static str, toggle: Toggle| {
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(colors::GRAY_400)
                        .child(title),
                )
                .child(toggle)
        };
        let read = cx.entity();
        let write = cx.entity();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(label("Console access (optional)"))
            .child(row(
                "Read the console",
                Toggle::new("console-read", self.console.console_read).on_change(
                    move |value, _, cx| {
                        let value = *value;
                        read.update(cx, |this, cx| {
                            this.console.console_read = value;
                            cx.notify();
                        })
                    },
                ),
            ))
            .child(row(
                "Send chat and commands",
                Toggle::new("console-write", self.console.console_write).on_change(
                    move |value, _, cx| {
                        let value = *value;
                        write.update(cx, |this, cx| {
                            this.console.console_write = value;
                            cx.notify();
                        })
                    },
                ),
            ))
            .child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                    .text_color(colors::GRAY_500)
                    .child(format!("Only where your user may use the console. Commands can do anything an operator can, so allow them only on a {} you trust.", wording::COMPUTER)),
            )
    }
}

fn label(text_value: &'static str) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .text_color(colors::GRAY_400)
        .child(text_value)
}

impl Render for AddServerDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let title = if self.signing_in {
            "Sign in to server"
        } else {
            "Add server"
        };
        let expiry = self.expiry;
        let pills = crate::components::ArrowGroup::new("key-expiry")
            .flex()
            .flex_row()
            .gap_1()
            .p_1()
            .rounded(radius::LG)
            .bg(colors::GRAY_900)
            .border_1()
            .border_color(colors::FADE_700)
            .children(KeyExpiry::ALL.into_iter().map(|choice| {
                let selected = choice == expiry;
                div()
                    .id(choice.label())
                    .keyboard_focus()
                    .px_3()
                    .py_1()
                    .rounded(radius::MD)
                    .cursor_pointer()
                    .text_size(theme::text_size(text::SMALL))
                    .font_weight(weights::BOLD.into())
                    .map(|this| {
                        if selected {
                            this.bg(colors::GRAY_700).text_color(colors::GRAY_300)
                        } else {
                            this.text_color(colors::GRAY_500)
                                .hover(|s| s.text_color(colors::GRAY_300))
                        }
                    })
                    .child(choice.label())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.expiry = choice;
                        cx.notify();
                    }))
            }));
        Dialog::new("add-server", title, self.focus_handle.clone())
            .on_close(move |_, cx| entity.update(cx, |_, cx| cx.emit(AddServerEvent::Dismissed)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(
                        div()
                            .text_size(theme::text_size(text::MEDIUM))
                            .text_color(colors::GRAY_400)
                            .child(if self.signing_in {
                                format!("Sign in to start and stop servers and see the ones that aren't listed publicly. Lodestone Launcher makes its own key for this {}; your password isn't kept.", wording::COMPUTER)
                            } else {
                                "The address of a Lodestone core. Without signing in you see the servers its owner lists publicly.".to_string()
                            }),
                    )
                    .child(label("Address"))
                    .child(self.address.clone())
                    .child(label("Sign in (optional)"))
                    .child(self.username.clone())
                    .child(self.password.clone())
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .text_color(colors::GRAY_500)
                                    .child(format!("This {}'s key expires after", wording::COMPUTER)),
                            )
                            .child(pills),
                    )
                    .child(self.console_choices(cx))
                    .when(expiry == KeyExpiry::Never, |this| {
                        this.child(
                            div()
                                .text_size(theme::text_size(text::SMALL))
                                .text_color(colors::YELLOW)
                                .child("A key that never expires works until you revoke it in the dashboard."),
                        )
                    })
                    .children(self.error.clone().map(|error| {
                        div()
                            .text_size(theme::text_size(text::SMALL))
                            .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                            .font_weight(weights::BOLD.into())
                            .text_color(colors::RED_200)
                            .child(error)
                    })),
            )
            .footer(
                Button::new("add-server-submit", if self.signing_in { "Sign in" } else { "Add server" })
                    .variant(ButtonVariant::Primary)
                    .loading(self.busy)
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            )
            .footer(
                Button::new("add-server-cancel", "Cancel")
                    .variant(ButtonVariant::Secondary)
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(AddServerEvent::Dismissed))),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_choices() {
        assert_eq!(KeyExpiry::Days90.expires_at(0), Some(90 * 86_400));
        assert_eq!(KeyExpiry::Year.expires_at(10), Some(10 + 365 * 86_400));
        assert_eq!(KeyExpiry::Never.expires_at(10), None);
    }
}
