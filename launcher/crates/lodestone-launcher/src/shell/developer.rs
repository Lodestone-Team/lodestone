//! Settings > Developer: "Sign in as the Minecraft Launcher", and the dialogs that
//! explain it before it is turned on and offer to sign accounts out when it is
//! turned off.

use gpui::{
    AnyElement, Context, FocusHandle, IntoElement, SharedString, Window, div, prelude::*, px,
};

use super::{LauncherShell, Modal};
use crate::{
    components::{Button, ButtonVariant, Dialog, Toggle},
    theme::{self, colors, fonts, leading, radius, text, weights},
};

/// The badge on accounts signed in as the Minecraft Launcher.
pub const BADGE: &str = "Launcher identity";

/// The disclosure shown before the option is turned on, as (heading, paragraph).
pub const DISCLOSURE: &[(&str, &str)] = &[
    (
        "What this does",
        "Lodestone Launcher will tell Microsoft that it is Mojang's official Minecraft Launcher, using that launcher's public client ID, instead of Lodestone's own registered app. Microsoft and Mojang haven't approved this, Lodestone isn't affiliated with them, and it may be against Microsoft's terms for their apps.",
    ),
    (
        "Why it's here",
        "Lodestone's own app is still waiting for Mojang's approval, so normal Microsoft sign-in can't finish yet.",
    ),
    (
        "When you sign in",
        "Add Microsoft account gives you a code. Enter it at microsoft.com/link and sign in to your Microsoft account there. Microsoft's pages may show Minecraft's name instead of Lodestone's. If the code doesn't work, you can sign in with a link instead: your browser opens login.live.com, and you paste back the address of the blank page it ends on.",
    ),
    (
        "What the launcher gets",
        "The same as a normal sign-in: tokens that let it play Minecraft as that account. The long-lived one is kept in your macOS Keychain and the rest only in memory. These accounts are marked \"Launcher identity\" in the account menu.",
    ),
    (
        "Risks",
        "Microsoft can block this at any time. If it does, accounts signed in this way stop working. Use it only for personal testing until Lodestone's app is approved.",
    ),
    (
        "How to undo it",
        "Turn this setting off and sign those accounts out. Lodestone Launcher offers to sign them out when you turn it off.",
    ),
];

fn paragraph(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::MEDIUM))
        .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
        .text_color(colors::GRAY_400)
        .child(text.into())
}

/// The Developer section of the Settings page.
pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let enabled = shell.accounts.read(cx).launcher_identity_enabled();
    div()
        .flex()
        .flex_col()
        .gap_3()
        .w_full()
        .px_8()
        .pt_8()
        .child(
            div()
                .font_family(fonts::HEADING)
                .text_size(theme::text_size(text::H3))
                .font_weight(weights::BOLD.into())
                .text_color(colors::GRAY_300)
                .child("Developer"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_4()
                .max_w(px(640.))
                .px_4()
                .py_3()
                .rounded(radius::LG)
                .bg(colors::GRAY_850)
                .border_1()
                .border_color(colors::FADE_700)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .gap_1()
                        .child(
                            div()
                                .font_weight(weights::BOLD.into())
                                .text_color(colors::GRAY_300)
                                .child("Sign in as the Minecraft Launcher"),
                        )
                        .child(
                            div()
                                .text_size(theme::text_size(text::SMALL))
                                .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                                .text_color(colors::GRAY_500)
                                .child("New Microsoft sign-ins use the official Minecraft Launcher's app instead of Lodestone's, while Lodestone's waits for Mojang's approval. For personal testing."),
                        ),
                )
                .child(
                    Toggle::new("launcher-identity", enabled).on_change(cx.listener(
                        |this, value: &bool, window, cx| {
                            this.request_launcher_identity(*value, window, cx)
                        },
                    )),
                ),
        )
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
            self.modal = Some(Modal::EnableLauncherIdentity { focus });
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
                self.modal = Some(Modal::DisableLauncherIdentity { focus, usernames });
            }
        }
        cx.notify();
    }

    /// Opens the disclosure, for the offscreen screenshots.
    pub fn preview_enable_launcher_identity(&mut self, cx: &mut Context<Self>) {
        self.modal = Some(Modal::EnableLauncherIdentity {
            focus: cx.focus_handle(),
        });
        cx.notify();
    }

    pub(super) fn render_enable_launcher_identity(
        &self,
        focus: FocusHandle,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let entity = cx.entity();
        Dialog::new(
            "enable-launcher-identity",
            "Sign in as the Minecraft Launcher?",
            focus,
        )
        .on_close(move |_, cx| entity.update(cx, |this, cx| this.close_modal(cx)))
        .child(
            div()
                .id("launcher-identity-disclosure")
                .flex()
                .flex_col()
                .gap_3()
                .max_h(px(280.))
                .overflow_y_scroll()
                .pr_2()
                .children(DISCLOSURE.iter().map(|(heading, body)| {
                    div()
                        .flex()
                        .flex_col()
                        .gap_0p5()
                        .child(
                            div()
                                .text_size(theme::text_size(text::MEDIUM))
                                .font_weight(weights::BOLD.into())
                                .text_color(colors::GRAY_300)
                                .child(*heading),
                        )
                        .child(paragraph(*body))
                })),
        )
        .footer(
            Button::new("confirm-launcher-identity", "I understand, turn it on")
                .variant(ButtonVariant::Primary)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.accounts
                        .update(cx, |accounts, cx| accounts.set_launcher_identity(true, cx));
                    this.close_modal(cx);
                })),
        )
        .footer(
            Button::new("cancel-launcher-identity", "Cancel")
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
            [one] => format!("{one} is signed in as the Minecraft Launcher."),
            many => format!(
                "{} accounts are signed in as the Minecraft Launcher: {}.",
                many.len(),
                many.join(", ")
            ),
        };
        Dialog::new("disable-launcher-identity", "Turn off the Minecraft Launcher sign-in?", focus)
            .on_close(move |_, cx| entity.update(cx, |this, cx| this.close_modal(cx)))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(paragraph(
                        "New Microsoft sign-ins will use Lodestone's own app again.",
                    ))
                    .child(paragraph(format!(
                        "{accounts} Kept accounts go on working until Microsoft blocks this. Signing out removes them from Lodestone Launcher and deletes their saved sign-ins from your Keychain."
                    ))),
            )
            .footer(
                Button::new("disable-and-sign-out", "Turn off and sign out")
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
                Button::new("disable-keep-accounts", "Turn off only")
                    .variant(ButtonVariant::Secondary)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.accounts
                            .update(cx, |accounts, cx| accounts.set_launcher_identity(false, cx));
                        this.close_modal(cx);
                    })),
            )
            .footer(
                Button::new("cancel-disable-launcher-identity", "Cancel")
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| this.close_modal(cx))),
            )
            .into_any_element()
    }
}
