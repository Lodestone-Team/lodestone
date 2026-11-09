//! "Add server": a core's address, and optionally a username and password to sign
//! in with. The password is sent once, to mint this device's key, and never kept.

use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, Render, SharedString,
    Window, div, prelude::*,
};
use launcher_core::{Secret, cores::live::LauncherScope};

use crate::components::{
    KeyboardFocus,
    a11y::{Accessible, Role, Toggled},
};
use crate::{
    components::{Button, ButtonVariant, Dialog, TextInput, Toggle},
    i18n::{CommonMsg, ServersMsg as M, t, tr},
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
        t(match self {
            KeyExpiry::Days90 => M::KeyDays90,
            KeyExpiry::Year => M::KeyYear,
            KeyExpiry::Never => M::KeyNever,
        })
    }

    /// The choice's element id, the same in every language.
    fn id(self) -> &'static str {
        match self {
            KeyExpiry::Days90 => "key-expiry-90-days",
            KeyExpiry::Year => "key-expiry-year",
            KeyExpiry::Never => "key-expiry-never",
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
                let input = TextInput::new(cx)
                    .with_placeholder(M::AddressPlaceholder)
                    .with_automation_id("add-server-address");
                match &address {
                    Some(address) => input.with_text(address.clone()).with_disabled(true),
                    None => input,
                }
            }),
            username: cx.new(|cx| {
                TextInput::new(cx)
                    .with_placeholder(M::UsernameOptional)
                    .with_automation_id("add-server-username")
            }),
            password: cx.new(|cx| {
                TextInput::new(cx)
                    .with_placeholder(M::Password)
                    .with_automation_id("add-server-password")
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
            return self.fail(t(M::EnterAddress).into(), cx);
        }
        if self.signing_in && username.is_empty() {
            return self.fail(t(M::EnterUsernamePassword).into(), cx);
        }
        if !username.is_empty() && password.is_empty() {
            return self.fail(t(M::EnterPassword).into(), cx);
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
                        .flex_1()
                        .min_w_0()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(crate::theme::ink::secondary())
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
            .child(label(t(M::ConsoleAccess)))
            .child(row(
                t(M::ReadConsole),
                Toggle::new("console-read", self.console.console_read)
                    .label(t(M::ReadConsole))
                    .on_change(move |value, _, cx| {
                        let value = *value;
                        read.update(cx, |this, cx| {
                            this.console.console_read = value;
                            cx.notify();
                        })
                    }),
            ))
            .child(row(
                t(M::SendChat),
                Toggle::new("console-write", self.console.console_write)
                    .label(t(M::SendChat))
                    .on_change(move |value, _, cx| {
                        let value = *value;
                        write.update(cx, |this, cx| {
                            this.console.console_write = value;
                            cx.notify();
                        })
                    }),
            ))
            .child(
                div()
                    .text_size(theme::text_size(text::SMALL))
                    .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                    .text_color(crate::theme::ink::muted())
                    .child(tr!(
                        M::ConsoleAccessHint,
                        computer = t(CommonMsg::ThisComputer)
                    )),
            )
    }
}

fn label(text_value: &'static str) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .text_color(crate::theme::ink::secondary())
        .child(text_value)
}

impl Render for AddServerDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity();
        let title = t(if self.signing_in {
            M::SignInTitle
        } else {
            M::AddServer
        });
        let expiry = self.expiry;
        let pills = crate::components::ArrowGroup::new("key-expiry")
            .group_role(Role::RadioGroup)
            .flex()
            .flex_row()
            .gap_1()
            .p_1()
            .rounded(radius::LG)
            .bg(colors::GRAY_900)
            .border_1()
            .border_color(crate::theme::ink::border())
            .children(KeyExpiry::ALL.into_iter().map(|choice| {
                let selected = choice == expiry;
                div()
                    .id(choice.id())
                    .accessible(Role::RadioButton, choice.id(), choice.label())
                    .aria_toggled(if selected {
                        Toggled::True
                    } else {
                        Toggled::False
                    })
                    .keyboard_focus()
                    .px_3()
                    .py_1()
                    .rounded(radius::MD)
                    .cursor_pointer()
                    .text_size(theme::text_size(text::SMALL))
                    .font_weight(weights::BOLD.into())
                    .map(|this| {
                        if selected {
                            this.bg(colors::GRAY_700)
                                .text_color(crate::theme::ink::text())
                        } else {
                            this.text_color(crate::theme::ink::muted())
                                .hover(|s| s.text_color(crate::theme::ink::text()))
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
                            .text_color(crate::theme::ink::secondary())
                            .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
                            .child(if self.signing_in {
                                tr!(M::SignInIntro, computer = t(CommonMsg::ThisComputer))
                            } else {
                                t(M::AddIntro).to_string()
                            }),
                    )
                    .child(label(t(M::Address)))
                    .child(self.address.clone())
                    .child(label(t(M::SignInOptional)))
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
                                    .flex_1()
                                    .min_w_0()
                                    .text_size(theme::text_size(text::SMALL))
                                    .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                                    .text_color(crate::theme::ink::muted())
                                    .child(tr!(
                                        M::KeyExpiresAfter,
                                        computer = t(CommonMsg::ThisComputer)
                                    )),
                            )
                            .child(pills.flex_none()),
                    )
                    .child(self.console_choices(cx))
                    .when(expiry == KeyExpiry::Never, |this| {
                        this.child(
                            div()
                                .text_size(theme::text_size(text::SMALL))
                                .text_color(colors::YELLOW)
                                .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                                .child(t(M::NeverExpiresWarning)),
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
                Button::new(
                    "add-server-submit",
                    if self.signing_in {
                        t(CommonMsg::SignIn)
                    } else {
                        t(M::AddServer)
                    },
                )
                .variant(ButtonVariant::Primary)
                .loading(self.busy)
                .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            )
            .footer(
                Button::new("add-server-cancel", t(CommonMsg::Cancel))
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
