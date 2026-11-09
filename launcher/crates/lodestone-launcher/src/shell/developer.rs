//! Settings > Developer: "Sign in as the Minecraft Launcher", and the dialogs that
//! explain it before it is turned on and offer to sign accounts out when it is
//! turned off.

use super::{LauncherShell, Modal};
use crate::theme::px;
use crate::{
    components::{
        Button, ButtonVariant, Dialog, Toggle,
        settings::{SettingRow, card},
    },
    i18n::{CommonMsg, ShellMsg as M, ShellPlural as P, format, t, tr, trn},
    theme::{self, leading, text, weights},
};
use gpui::{AnyElement, Context, FocusHandle, IntoElement, SharedString, Window, div, prelude::*};

/// The disclosure shown before the option is turned on, as (heading, paragraph).
pub fn disclosure() -> Vec<(&'static str, SharedString)> {
    vec![
        (t(M::WhatThisDoes), t(M::WhatThisDoesBody).into()),
        (t(M::Risks), t(M::RisksBody).into()),
        (t(M::WhyHere), t(M::WhyHereBody).into()),
        (
            t(M::WhenSignIn),
            tr!(M::WhenSignInBody, add = t(M::AddMicrosoftAccount)).into(),
        ),
        (
            t(M::WhatLauncherGets),
            tr!(
                M::WhatLauncherGetsBody,
                keychain = t(CommonMsg::KeychainInFull),
                badge = t(M::LauncherIdentity),
            )
            .into(),
        ),
        (t(M::HowToUndo), t(M::HowToUndoBody).into()),
    ]
}

fn paragraph(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::MEDIUM))
        .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
        .text_color(crate::theme::ink::secondary())
        .child(text.into())
}

/// Settings > Developer.
pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let enabled = shell.accounts.read(cx).launcher_identity_enabled();
    card([SettingRow::new(t(M::SignInAsLauncher))
        .hint(t(M::SignInAsLauncherHint))
        .child(
            Toggle::new("launcher-identity", enabled).on_change(cx.listener(
                |this, value: &bool, window, cx| this.request_launcher_identity(*value, window, cx),
            )),
        )
        .into_any_element()])
}

impl LauncherShell {
    /// Turning the option on asks first; turning it off asks what to do with the
    /// accounts that used it, if there are any.
    fn request_launcher_identity(
        &mut self,
        enabled: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.account_menu_open = false;
        let focus = cx.focus_handle();
        if enabled {
            window.focus(&focus, cx);
            self.modal.open(Modal::EnableLauncherIdentity { focus }, cx);
        } else {
            let usernames: Vec<String> = self
                .accounts
                .read(cx)
                .launcher_identity_accounts()
                .into_iter()
                .map(|account| account.username)
                .collect();
            if usernames.is_empty() {
                self.accounts
                    .update(cx, |accounts, cx| accounts.set_launcher_identity(false, cx));
            } else {
                window.focus(&focus, cx);
                self.modal
                    .open(Modal::DisableLauncherIdentity { focus, usernames }, cx);
            }
        }
        cx.notify();
    }

    /// Opens the disclosure, for the offscreen screenshots.
    pub fn preview_enable_launcher_identity(&mut self, cx: &mut Context<Self>) {
        let focus = cx.focus_handle();
        self.modal.open(Modal::EnableLauncherIdentity { focus }, cx);
    }

    pub(super) fn render_enable_launcher_identity(
        &self,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        Dialog::new(
            "enable-launcher-identity",
            t(M::SignInAsLauncherTitle),
            focus,
        )
        .on_close(move |_, cx| entity.update(cx, |this, cx| this.close_modal(cx)))
        .child(
            div()
                .id("launcher-identity-disclosure")
                .flex()
                .flex_col()
                .gap_3()
                .max_h(px(340.))
                .overflow_y_scroll()
                .pr_2()
                .children(disclosure().into_iter().map(|(heading, body)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_size(theme::text_size(text::MEDIUM))
                                .font_weight(weights::BOLD.into())
                                .text_color(crate::theme::ink::text())
                                .child(heading),
                        )
                        .child(paragraph(body))
                })),
        )
        .footer(
            Button::new("confirm-launcher-identity", t(M::TurnItOn))
                .variant(ButtonVariant::Primary)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.accounts
                        .update(cx, |accounts, cx| accounts.set_launcher_identity(true, cx));
                    this.close_modal(cx);
                })),
        )
        .footer(
            Button::new("cancel-launcher-identity", t(CommonMsg::Cancel))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
        )
        .into_any_element()
    }

    pub(super) fn render_disable_launcher_identity(
        &self,
        focus: FocusHandle,
        usernames: &[String],
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        let accounts = match usernames {
            [one] => tr!(M::OneLauncherAccount, name = one),
            many => trn!(P::LauncherAccounts, many.len(), names = format::list(many)),
        };
        Dialog::new(
            "disable-launcher-identity",
            t(M::TurnOffLauncherTitle),
            focus,
        )
        .on_close(move |_, cx| entity.update(cx, |this, cx| this.close_modal(cx)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .child(paragraph(t(M::TurnOffLauncherBody)))
                .child(paragraph(tr!(
                    M::TurnOffLauncherAccounts,
                    accounts = accounts,
                    keychain = t(CommonMsg::Keychain),
                ))),
        )
        .footer(
            Button::new("disable-and-sign-out", t(M::TurnOffAndSignOut))
                .variant(ButtonVariant::Danger)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.accounts.update(cx, |accounts, cx| {
                        accounts.set_launcher_identity(false, cx);
                        accounts.sign_out_launcher_identity_accounts(cx);
                    });
                    this.close_modal(cx);
                })),
        )
        .footer(
            Button::new("disable-keep-accounts", t(M::TurnOffOnly))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.accounts
                        .update(cx, |accounts, cx| accounts.set_launcher_identity(false, cx));
                    this.close_modal(cx);
                })),
        )
        .footer(
            Button::new("cancel-disable-launcher-identity", t(CommonMsg::Cancel))
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
        )
        .into_any_element()
    }
}
