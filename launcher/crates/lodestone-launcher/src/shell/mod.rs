//! The main window, laid out like a game launcher in the dashboard's style: the
//! top bar, a sidebar with the account box, the instances and Settings, and the
//! selected instance with its tabs and a large Play button.

mod account_box;
mod play;
mod sidebar;
mod top_bar;

pub use top_bar::TOP_BAR_HEIGHT;

use gpui::{
    AnyElement, Context, Entity, FocusHandle, Focusable as _, IntoElement, Render, Subscription,
    Window, div, prelude::*,
};

use crate::{
    accounts::{
        Accounts,
        offline_dialog::{OfflineDialog, OfflineDialogDismissed},
        sign_in::{SignInDialog, SignInEvent, Stage},
    },
    components::{Button, ButtonVariant, Dialog},
    theme::{self, colors, fonts, leading, text},
};

/// What the main area shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Page {
    #[default]
    Instance,
    Settings,
}

/// The tabs of an instance.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum InstanceTab {
    #[default]
    Play,
    Mods,
    Worlds,
    Settings,
    Logs,
}

impl InstanceTab {
    pub const ALL: [InstanceTab; 5] = [
        InstanceTab::Play,
        InstanceTab::Mods,
        InstanceTab::Worlds,
        InstanceTab::Settings,
        InstanceTab::Logs,
    ];

    pub fn label(self) -> &'static str {
        match self {
            InstanceTab::Play => "Play",
            InstanceTab::Mods => "Mods",
            InstanceTab::Worlds => "Worlds",
            InstanceTab::Settings => "Settings",
            InstanceTab::Logs => "Logs",
        }
    }
}

/// The dialog over the window, if any.
enum Modal {
    NewInstance(FocusHandle),
    SignIn {
        dialog: Entity<SignInDialog>,
        _events: Subscription,
    },
    Offline {
        dialog: Entity<OfflineDialog>,
        _events: Subscription,
    },
    RemoveAccount {
        id: String,
        username: String,
        focus: FocusHandle,
    },
}

/// The root view of the launcher window.
pub struct LauncherShell {
    accounts: Entity<Accounts>,
    page: Page,
    tab: InstanceTab,
    modal: Option<Modal>,
    account_menu_open: bool,
    /// Set by a mouse down on the top bar; the next mouse move starts a window drag.
    top_bar_drag_pending: bool,
    _observe_accounts: Subscription,
}

impl LauncherShell {
    pub fn new(accounts: Entity<Accounts>, cx: &mut Context<Self>) -> Self {
        let observe = cx.observe(&accounts, |_, _, cx| cx.notify());
        Self {
            accounts,
            page: Page::default(),
            tab: InstanceTab::default(),
            modal: None,
            account_menu_open: false,
            top_bar_drag_pending: false,
            _observe_accounts: observe,
        }
    }

    fn show(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    fn select_tab(&mut self, tab: InstanceTab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    fn set_account_menu(&mut self, open: bool, cx: &mut Context<Self>) {
        self.account_menu_open = open;
        cx.notify();
    }

    fn close_modal(&mut self, cx: &mut Context<Self>) {
        self.modal = None;
        cx.notify();
    }

    fn open_new_instance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        self.modal = Some(Modal::NewInstance(focus));
        cx.notify();
    }

    fn open_sign_in(&mut self, window: Option<&mut Window>, cx: &mut Context<Self>) {
        self.open_sign_in_at(Stage::Choose, window, cx);
    }

    /// Opens the sign-in dialog at `stage`. Only the screenshots start past
    /// [`Stage::Choose`].
    pub fn open_sign_in_at(
        &mut self,
        stage: Stage,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.account_menu_open = false;
        let accounts = self.accounts.clone();
        let dialog = cx.new(|cx| SignInDialog::new(accounts, cx).with_stage(stage));
        let subscription = cx.subscribe(&dialog, |this, _, event, cx| match event {
            SignInEvent::Dismissed => this.close_modal(cx),
            SignInEvent::UseOffline => this.open_offline(None, None, cx),
        });
        if let Some(window) = window {
            window.focus(&dialog.focus_handle(cx), cx);
        }
        self.modal = Some(Modal::SignIn {
            dialog,
            _events: subscription,
        });
        cx.notify();
    }

    /// Opens "Add offline account", with `username` filled in for screenshots.
    pub fn open_offline(
        &mut self,
        username: Option<&str>,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.account_menu_open = false;
        let accounts = self.accounts.clone();
        let dialog = cx.new(|cx| {
            let dialog = OfflineDialog::new(accounts, cx);
            match username {
                Some(name) => dialog.with_username(name, cx),
                None => dialog,
            }
        });
        let subscription = cx.subscribe(&dialog, |this, _, _: &OfflineDialogDismissed, cx| {
            this.close_modal(cx)
        });
        if let Some(window) = window {
            window.focus(&dialog.focus_handle(cx), cx);
        }
        self.modal = Some(Modal::Offline {
            dialog,
            _events: subscription,
        });
        cx.notify();
    }

    /// Asks before removing the account.
    pub fn confirm_remove(
        &mut self,
        id: &str,
        window: Option<&mut Window>,
        cx: &mut Context<Self>,
    ) {
        self.account_menu_open = false;
        let Some(account) = self
            .accounts
            .read(cx)
            .accounts()
            .iter()
            .find(|a| a.id == id)
            .cloned()
        else {
            return;
        };
        let focus = cx.focus_handle();
        if let Some(window) = window {
            window.focus(&focus, cx);
        }
        self.modal = Some(Modal::RemoveAccount {
            id: account.id,
            username: account.username,
            focus,
        });
        cx.notify();
    }

    /// Opens the account menu, for the screenshots.
    pub fn open_account_menu(&mut self, cx: &mut Context<Self>) {
        self.set_account_menu(true, cx);
    }

    fn render_modal(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let close = |cx: &mut Context<Self>| {
            let entity = cx.entity();
            move |_: &mut Window, cx: &mut gpui::App| {
                entity.update(cx, |this, cx| this.close_modal(cx))
            }
        };
        Some(match self.modal.as_ref()? {
            Modal::NewInstance(focus) => Dialog::new("new-instance", "New instance", focus.clone())
                .on_close(close(cx))
                .child("Creating instances comes in a later milestone of Lodestone Launcher.")
                .footer(
                    Button::new("close-new-instance", "Close")
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                )
                .into_any_element(),
            Modal::SignIn { dialog, .. } => dialog.clone().into_any_element(),
            Modal::Offline { dialog, .. } => dialog.clone().into_any_element(),
            Modal::RemoveAccount {
                id,
                username,
                focus,
            } => {
                let id = id.clone();
                Dialog::new("remove-account", format!("Remove {username}?"), focus.clone())
                    .on_close(close(cx))
                    .child(
                        div()
                            .text_color(colors::GRAY_400)
                            .child("Lodestone Launcher forgets this account and deletes its saved sign-in from your keychain. You can add it again later."),
                    )
                    .footer(
                        Button::new("confirm-remove", "Remove account")
                            .variant(ButtonVariant::Danger)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.accounts
                                    .update(cx, |accounts, cx| accounts.remove(&id, cx));
                                this.close_modal(cx);
                            })),
                    )
                    .footer(
                        Button::new("cancel-remove", "Cancel")
                            .variant(ButtonVariant::Secondary)
                            .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
                    )
                    .into_any_element()
            }
        })
    }
}

impl Render for LauncherShell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let main = match self.page {
            Page::Instance => play::render(self, cx).into_any_element(),
            Page::Settings => play::render_settings().into_any_element(),
        };
        let modal = self.render_modal(cx);

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
                    .child(div().flex().flex_1().min_w_0().h_full().child(main)),
            )
            .children(modal)
    }
}
