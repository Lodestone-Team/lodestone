//! The account dialogs of Settings › Users and Profile: creating a user and changing
//! a password (`CreateUserForm.tsx`, `ChangeSelfPasswordForm.tsx`,
//! `ChangeUserPasswordForm.tsx`), and setting up the owner account before the core is
//! shared.
//!
//! Lodestone Host makes its core's owner without a password, and the core then serves
//! only this computer (`core/src/remote_access.rs`). Whatever only makes sense from other
//! devices (allowing access from them, other users, API keys, the public listing) first
//! asks for the owner's username and password with [`AccountDialog::owner_account`]
//! ([`needs_owner_account`]); setting them up lets other devices in.

use gpui::{
    AppContext as _, Context, Entity, EventEmitter, FocusHandle, IntoElement, KeyDownEvent,
    ParentElement, Render, Styled, Window, div, prelude::*,
};
use launcher_core::cores::admin::PublicUser;
use lodestone_ui::components::{Button, ButtonVariant, Dialog, TextInput};

use super::parts;
use crate::{
    i18n::{CommonMsg, CoreMsg, t},
    state::HostState,
};

/// The shortest owner password the core takes, in characters.
pub const OWNER_PASSWORD_MIN: usize = 12;

/// Whether the signed-in owner has no password yet: sharing the core asks for one first
/// ([`AccountDialog::owner_account`]).
pub fn needs_owner_account(state: &HostState) -> bool {
    state.me.is_owner && !state.me.has_password
}

/// What the user typed wrong, as the dashboard's forms check it; the core checks again.
pub fn password_problem(
    username: Option<&str>,
    old: Option<&str>,
    password: &str,
    confirm: &str,
) -> Option<CoreMsg> {
    if username.is_some_and(|u| u.trim().is_empty()) {
        Some(CoreMsg::UsernameRequired)
    } else if old.is_some_and(str::is_empty) || password.is_empty() {
        Some(CoreMsg::PasswordRequired)
    } else if confirm.is_empty() {
        Some(CoreMsg::ConfirmRequired)
    } else if password != confirm {
        Some(CoreMsg::PasswordsMustMatch)
    } else {
        None
    }
}

pub enum AccountDialogEvent {
    Close,
    /// A user was made.
    Created(Box<PublicUser>),
    /// The password changed.
    Changed,
    /// The owner account was set up (the owner as the core now has them), and other
    /// devices may use the core. [`HostState::owner_account_set_up`] is done.
    OwnerSetUp(Box<PublicUser>),
}

enum Kind {
    CreateUser,
    /// Changing `uid`'s password; the user's own needs the old one.
    Password {
        uid: String,
        own: bool,
    },
    /// The owner's username and password, before the core is shared.
    OwnerAccount,
}

pub struct AccountDialog {
    state: Entity<HostState>,
    kind: Kind,
    focus: FocusHandle,
    username: Option<Entity<TextInput>>,
    old: Option<Entity<TextInput>>,
    password: Entity<TextInput>,
    confirm: Entity<TextInput>,
    error: Option<String>,
    busy: bool,
}

impl EventEmitter<AccountDialogEvent> for AccountDialog {}

impl AccountDialog {
    pub fn create_user(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        Self::new(state, Kind::CreateUser, cx)
    }

    /// Sets up the owner account: a username (the current one to start with) and a
    /// password, which lets other devices use the core.
    pub fn owner_account(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        let username = state.read(cx).me.username.clone();
        let this = Self::new(state, Kind::OwnerAccount, cx);
        if let Some(input) = &this.username {
            input.update(cx, |input, cx| input.set_text(username, cx));
        }
        this
    }

    /// Changes `uid`'s password: with the old one if it is the signed-in user's own.
    pub fn change_password(state: Entity<HostState>, uid: String, cx: &mut Context<Self>) -> Self {
        let own = state.read(cx).me.uid == uid;
        Self::new(state, Kind::Password { uid, own }, cx)
    }

    fn new(state: Entity<HostState>, kind: Kind, cx: &mut Context<Self>) -> Self {
        let username = matches!(kind, Kind::CreateUser | Kind::OwnerAccount)
            .then(|| parts::input(t(CoreMsg::Username), false, cx));
        let old = matches!(kind, Kind::Password { own: true, .. })
            .then(|| parts::input(t(CoreMsg::OldPassword), true, cx));
        Self {
            state,
            kind,
            focus: cx.focus_handle(),
            username,
            old,
            password: parts::input(t(CoreMsg::Password), true, cx),
            confirm: parts::input(t(CoreMsg::ConfirmPassword), true, cx),
            error: None,
            busy: false,
        }
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let read =
            |input: &Entity<TextInput>, cx: &Context<Self>| input.read(cx).text().to_string();
        let username = self
            .username
            .as_ref()
            .map(|i| read(i, cx).trim().to_string());
        let old = self.old.as_ref().map(|i| read(i, cx));
        let password = read(&self.password, cx);
        let confirm = read(&self.confirm, cx);
        if let Some(problem) =
            password_problem(username.as_deref(), old.as_deref(), &password, &confirm)
        {
            self.error = Some(t(problem).to_string());
            cx.notify();
            return;
        }
        if matches!(self.kind, Kind::OwnerAccount) && password.chars().count() < OWNER_PASSWORD_MIN
        {
            self.error = Some(t(CoreMsg::PasswordTooShort).to_string());
            cx.notify();
            return;
        }
        self.error = None;
        self.busy = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        match &self.kind {
            Kind::CreateUser => {
                let username = username.unwrap_or_default();
                parts::spawn(
                    cx,
                    async move { admin.create_user(&username, &password).await },
                    |this, result, cx| {
                        this.busy = false;
                        match result {
                            Ok(user) => cx.emit(AccountDialogEvent::Created(Box::new(user))),
                            Err(err) => this.error = Some(err.to_string()),
                        }
                    },
                );
            }
            Kind::OwnerAccount => {
                let username = username.unwrap_or_default();
                parts::spawn(
                    cx,
                    async move { admin.set_up_owner_account(&username, &password).await },
                    |this, result, cx| {
                        this.busy = false;
                        match result {
                            Ok(owner) => {
                                this.state.update(cx, |state, cx| {
                                    state.owner_account_set_up(owner.clone(), cx)
                                });
                                cx.emit(AccountDialogEvent::OwnerSetUp(Box::new(owner)));
                            }
                            Err(err) => this.error = Some(err.to_string()),
                        }
                    },
                );
            }
            Kind::Password { uid, .. } => {
                let uid = uid.clone();
                parts::spawn(
                    cx,
                    async move { admin.change_password(&uid, old.as_deref(), &password).await },
                    |this, result, cx| {
                        this.busy = false;
                        match result {
                            Ok(()) => cx.emit(AccountDialogEvent::Changed),
                            Err(err) => this.error = Some(err.to_string()),
                        }
                    },
                );
            }
        }
    }
}

impl AccountDialog {
    /// Types `username` and `password` (twice) and presses the button, as a user would:
    /// for the development screenshots' end-to-end run (`--owner`).
    #[cfg(feature = "screenshot")]
    pub fn fill_and_submit(&mut self, username: &str, password: &str, cx: &mut Context<Self>) {
        if let Some(input) = &self.username {
            input.update(cx, |input, cx| input.set_text(username.to_string(), cx));
        }
        for input in [&self.password, &self.confirm] {
            input.update(cx, |input, cx| input.set_text(password.to_string(), cx));
        }
        self.submit(cx);
    }
}

impl Render for AccountDialog {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (title, action) = match self.kind {
            Kind::CreateUser => (CoreMsg::CreateUserTitle, CoreMsg::CreateUserButton),
            Kind::Password { .. } => (CoreMsg::ChangePassword, CoreMsg::ChangePasswordButton),
            Kind::OwnerAccount => (CoreMsg::OwnerAccountTitle, CoreMsg::OwnerAccountButton),
        };
        let close = cx.entity().downgrade();
        Dialog::new("account-dialog", t(title), self.focus.clone())
            .on_close(move |_, cx| {
                close
                    .update(cx, |_, cx| cx.emit(AccountDialogEvent::Close))
                    .ok();
            })
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                        if event.keystroke.key == "enter" {
                            this.submit(cx);
                        }
                    }))
                    .when(matches!(self.kind, Kind::CreateUser), |this| {
                        this.child(parts::paragraph(t(CoreMsg::CreateUserLead)))
                    })
                    .when(matches!(self.kind, Kind::OwnerAccount), |this| {
                        this.child(parts::paragraph(t(CoreMsg::OwnerAccountLead)))
                    })
                    .children(
                        self.username
                            .as_ref()
                            .map(|input| parts::field(t(CoreMsg::Username), input)),
                    )
                    .children(
                        self.old
                            .as_ref()
                            .map(|input| parts::field(t(CoreMsg::OldPassword), input)),
                    )
                    .child(parts::field(t(CoreMsg::Password), &self.password))
                    .child(parts::field(t(CoreMsg::ConfirmPassword), &self.confirm))
                    .children(self.error.clone().map(parts::error)),
            )
            .footer(
                Button::new("account-dialog-submit", t(action))
                    .variant(ButtonVariant::Primary)
                    .loading(self.busy)
                    .on_click(cx.listener(|this, _, _, cx| this.submit(cx))),
            )
            .footer(
                Button::new("account-dialog-cancel", t(CommonMsg::Cancel))
                    .on_click(cx.listener(|_, _, _, cx| cx.emit(AccountDialogEvent::Close))),
            )
    }
}

/// Makes the dialog and calls `on_event` with the dialog and what it says. The owner
/// keeps it in a `DialogSlot`, so it fades out when closed; `on_event` should check
/// that the dialog is still the slot's open one.
pub fn open<V: 'static>(
    make: impl FnOnce(&mut Context<AccountDialog>) -> AccountDialog,
    on_event: impl Fn(&mut V, &Entity<AccountDialog>, &AccountDialogEvent, &mut Context<V>) + 'static,
    cx: &mut Context<V>,
) -> (Entity<AccountDialog>, gpui::Subscription) {
    let dialog = cx.new(make);
    let subscription = cx.subscribe(&dialog, move |this, dialog, event, cx| {
        on_event(this, &dialog, event, cx)
    });
    (dialog, subscription)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forms_are_checked_as_the_dashboards() {
        assert_eq!(
            password_problem(Some(" "), None, "a", "a"),
            Some(CoreMsg::UsernameRequired)
        );
        assert_eq!(
            password_problem(None, Some(""), "a", "a"),
            Some(CoreMsg::PasswordRequired)
        );
        assert_eq!(
            password_problem(None, None, "", ""),
            Some(CoreMsg::PasswordRequired)
        );
        assert_eq!(
            password_problem(None, None, "a", ""),
            Some(CoreMsg::ConfirmRequired)
        );
        assert_eq!(
            password_problem(None, None, "a", "b"),
            Some(CoreMsg::PasswordsMustMatch)
        );
        assert_eq!(password_problem(Some("x"), Some("o"), "a", "a"), None);
    }
}
