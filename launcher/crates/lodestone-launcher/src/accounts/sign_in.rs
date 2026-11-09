//! The "Sign in with Microsoft" dialog: the browser flow by default, a device code
//! as the fallback, and friendly errors with a retry.
//!
//! With Settings > Developer > "Sign in as the Minecraft Launcher" on, the dialog
//! signs in with the official launcher's client instead: a device code by default,
//! or a pasted link from login.live.com's final page.

use crate::theme::px;
use std::time::Duration;

use super::{Accounts, auth_errors};
use crate::{
    components::{Button, ButtonVariant, Dialog, Icon, TextInput},
    i18n::{CommonMsg, ShellMsg as M, t, tr},
    theme::{self, colors, fonts, leading, radius, text, weights},
};
use futures::FutureExt as _;
use gpui::{
    AsyncApp, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Task, WeakEntity, Window, div, prelude::*,
};
use launcher_core::auth::{
    AuthError, ClientIdentity, LinkSignIn, MsTokens, loopback::CancelHandle,
};

/// How long the browser sign-in waits for the redirect.
const BROWSER_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// What the dialog shows.
#[derive(Debug)]
pub enum Stage {
    Choose,
    /// Waiting for the browser to come back.
    Browser,
    /// Asking Microsoft for a device code.
    RequestingCode,
    DeviceCode {
        user_code: SharedString,
        verification_uri: SharedString,
        opened: bool,
    },
    /// Waiting for the address of login.live.com's final page, pasted by the user.
    /// `error` is why the last paste was refused.
    Link {
        error: Option<AuthError>,
    },
    /// Signing in to Xbox Live and Minecraft.
    Finishing,
    Failed(AuthError),
}

pub enum SignInEvent {
    /// Signed in, or closed.
    Dismissed,
    /// The user chose to add an offline account instead.
    UseOffline,
}

pub struct SignInDialog {
    accounts: Entity<Accounts>,
    focus_handle: FocusHandle,
    stage: Stage,
    /// The sign-in in progress. Dropping it cancels it.
    task: Option<Task<()>>,
    cancel: Option<CancelHandle>,
    /// The client to sign in with, from the developer setting when the dialog opened.
    identity: ClientIdentity,
    /// The sign-in by pasted link in progress, and the field it is pasted into.
    link: Option<LinkSignIn>,
    link_input: Entity<TextInput>,
}

impl EventEmitter<SignInEvent> for SignInDialog {}

impl Focusable for SignInDialog {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SignInDialog {
    pub fn new(accounts: Entity<Accounts>, cx: &mut Context<Self>) -> Self {
        let identity = accounts.read(cx).sign_in_identity();
        Self {
            accounts,
            focus_handle: cx.focus_handle(),
            stage: Stage::Choose,
            task: None,
            cancel: None,
            identity,
            link: None,
            link_input: cx.new(|cx| {
                TextInput::new(cx).with_placeholder(format!("{}?code=…", auth_errors::LINK_PAGE))
            }),
        }
    }

    fn launcher_identity(&self) -> bool {
        self.identity == ClientIdentity::MinecraftLauncher
    }

    /// Shows `stage` without running a sign-in, for the offscreen screenshots.
    pub fn with_stage(mut self, stage: Stage) -> Self {
        self.stage = stage;
        self
    }

    fn set_stage(&mut self, stage: Stage, cx: &mut Context<Self>) {
        self.stage = stage;
        cx.notify();
    }

    fn stop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
        self.task = None;
        self.link = None;
    }

    /// Back: stops the sign-in under way and goes back to the first step. (The ×,
    /// Escape and a click on the backdrop close the dialog.)
    fn back(&mut self, cx: &mut Context<Self>) {
        self.stop();
        self.set_stage(Stage::Choose, cx);
    }

    fn dismiss(&mut self, cx: &mut Context<Self>) {
        self.stop();
        cx.emit(SignInEvent::Dismissed);
    }

    fn start_browser(&mut self, cx: &mut Context<Self>) {
        self.stop();
        let auth = self.accounts.read(cx).authenticator().clone();
        let sign_in = match auth.start_browser_sign_in(BROWSER_TIMEOUT) {
            Ok(sign_in) => sign_in,
            Err(err) => return self.set_stage(Stage::Failed(err), cx),
        };
        cx.open_url(sign_in.url().as_str());
        self.cancel = Some(sign_in.cancel_handle());
        self.set_stage(Stage::Browser, cx);
        self.task = Some(cx.spawn(async move |this, cx| {
            let tokens = auth.complete_browser_sign_in(sign_in).await;
            finish(this, tokens, cx).await;
        }));
    }

    fn start_device_code(&mut self, cx: &mut Context<Self>) {
        self.stop();
        self.set_stage(Stage::RequestingCode, cx);
        let auth = self.accounts.read(cx).authenticator().clone();
        let executor = cx.background_executor().clone();
        let identity = self.identity;
        self.task = Some(cx.spawn(async move |this, cx| {
            let code = match auth.start_device_code(identity).await {
                Ok(code) => code,
                Err(err) => {
                    this.update(cx, |this, cx| this.set_stage(Stage::Failed(err), cx))
                        .ok();
                    return;
                }
            };
            let shown = this.update(cx, |this, cx| {
                this.set_stage(
                    Stage::DeviceCode {
                        user_code: code.user_code.clone().into(),
                        verification_uri: code.verification_uri.clone().into(),
                        opened: false,
                    },
                    cx,
                )
            });
            if shown.is_err() {
                return;
            }
            let sleep = move |duration| executor.timer(duration).boxed_local();
            let tokens = auth.complete_device_code(&code, &sleep).await;
            finish(this, tokens, cx).await;
        }));
    }

    /// Opens login.live.com's sign-in page and waits for the user to paste the
    /// address of the page it ends on.
    fn start_link(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.stop();
        let sign_in = self.accounts.read(cx).authenticator().start_link_sign_in();
        cx.open_url(sign_in.url().as_str());
        self.link = Some(sign_in);
        self.link_input
            .update(cx, |input, cx| input.set_text("", cx));
        window.focus(&self.link_input.focus_handle(cx), cx);
        self.set_stage(Stage::Link { error: None }, cx);
    }

    fn reopen_link(&mut self, cx: &mut Context<Self>) {
        if let Some(sign_in) = &self.link {
            cx.open_url(sign_in.url().as_str());
        }
    }

    /// Checks the pasted address. A bad one keeps the dialog on the paste step with
    /// the reason; a good one is exchanged for tokens.
    fn submit_link(&mut self, cx: &mut Context<Self>) {
        let Some(sign_in) = &self.link else {
            return;
        };
        // The address holds a sign-in code: it is checked here and never logged.
        let pasted = self.link_input.read(cx).text().to_string();
        let code = match sign_in.check(&pasted) {
            Ok(code) => code,
            Err(err) => return self.set_stage(Stage::Link { error: Some(err) }, cx),
        };
        // A code works once; clear it from the field.
        self.link_input
            .update(cx, |input, cx| input.set_text("", cx));
        self.link = None;
        self.set_stage(Stage::Finishing, cx);
        let auth = self.accounts.read(cx).authenticator().clone();
        self.task = Some(cx.spawn(async move |this, cx| {
            let tokens = auth.exchange_link_code(&code).await;
            finish(this, tokens, cx).await;
        }));
    }

    /// The first step: the browser for Lodestone's app, a code for the
    /// Minecraft Launcher's client.
    fn start_default(&mut self, cx: &mut Context<Self>) {
        if self.launcher_identity() {
            self.start_device_code(cx);
        } else {
            self.start_browser(cx);
        }
    }

    fn copy_code_and_open(&mut self, cx: &mut Context<Self>) {
        if let Stage::DeviceCode {
            user_code,
            verification_uri,
            opened,
        } = &mut self.stage
        {
            cx.write_to_clipboard(ClipboardItem::new_string(user_code.to_string()));
            cx.open_url(verification_uri);
            *opened = true;
            cx.notify();
        }
    }
}

/// Runs the rest of the chain with the Microsoft tokens and adds the account.
async fn finish(
    this: WeakEntity<SignInDialog>,
    tokens: Result<MsTokens, AuthError>,
    cx: &mut AsyncApp,
) {
    let tokens = match tokens {
        Ok(tokens) => tokens,
        Err(AuthError::Cancelled) => return,
        Err(err) => {
            this.update(cx, |this, cx| this.set_stage(Stage::Failed(err), cx))
                .ok();
            return;
        }
    };
    let Ok(accounts) = this.update(cx, |this, cx| {
        this.cancel = None;
        this.set_stage(Stage::Finishing, cx);
        this.accounts.clone()
    }) else {
        return;
    };
    let (auth, secrets) = accounts.read_with(cx, |accounts, _| {
        (accounts.authenticator().clone(), accounts.secrets().clone())
    });
    let store = secrets.store(cx);
    match auth.finish_sign_in(tokens, &*store).await {
        Ok(signed_in) => {
            accounts.update(cx, |accounts, cx| accounts.add_signed_in(signed_in, cx));
            this.update(cx, |this, cx| {
                this.task = None;
                cx.emit(SignInEvent::Dismissed);
            })
            .ok();
        }
        Err(err) => {
            this.update(cx, |this, cx| this.set_stage(Stage::Failed(err), cx))
                .ok();
        }
    }
}

/// Says that this sign-in uses the developer option.
fn launcher_identity_note() -> impl IntoElement {
    div()
        .px_3()
        .py_2()
        .rounded(radius::LG)
        .bg(colors::GRAY_800)
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .font_weight(weights::BOLD.into())
        .text_color(colors::YELLOW)
        .child(t(M::LauncherIdentityNote))
}

fn muted(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::MEDIUM))
        .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
        .text_color(crate::theme::ink::secondary())
        .child(text.into())
}

/// A row with a pulsing indicator and a status line.
fn waiting(label: &'static str) -> impl IntoElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_3()
        .rounded(radius::LG)
        .bg(colors::GRAY_800)
        .border_1()
        .border_color(crate::theme::ink::border())
        .child(
            Button::new("waiting-indicator", "")
                .variant(ButtonVariant::Ghost)
                .loading(true),
        )
        .child(
            div()
                .text_size(theme::text_size(text::MEDIUM))
                .font_weight(weights::BOLD.into())
                .child(label),
        )
}

impl Render for SignInDialog {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let offline_enabled = self.accounts.read(cx).offline_enabled();
        let entity = cx.entity();
        let title = match &self.stage {
            Stage::Failed(err) => auth_errors::title(err),
            _ => t(M::SignInWithMicrosoft),
        };
        let mut dialog = Dialog::new("sign-in", title, self.focus_handle.clone())
            .on_close(move |_, cx| entity.update(cx, |this, cx| this.dismiss(cx)));

        let launcher_identity = self.launcher_identity();
        // The other way to sign in: a code for Lodestone's app, a pasted link for
        // the Minecraft Launcher's client (whose default is the code).
        let use_code = |cx: &mut Context<Self>| {
            if launcher_identity {
                Button::new("use-link", t(M::UseLinkInstead))
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|this, _, window, cx| this.start_link(window, cx)))
            } else {
                Button::new("use-code", t(M::UseCodeInstead))
                    .variant(ButtonVariant::Ghost)
                    .on_click(cx.listener(|this, _, _, cx| this.start_device_code(cx)))
            }
        };
        let back = |cx: &mut Context<Self>| {
            Button::new("sign-in-back", t(CommonMsg::Back))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| this.back(cx)))
        };

        match &self.stage {
            Stage::Choose if launcher_identity => {
                dialog = dialog
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(launcher_identity_note())
                            .child(muted(t(M::SignInCodeBody)))
                            .child(muted(tr!(
                                M::KeepsSignIn,
                                keychain = t(CommonMsg::KeychainInFull)
                            ))),
                    )
                    .footer(
                        Button::new("get-code", t(M::GetCode))
                            .variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|this, _, _, cx| this.start_device_code(cx))),
                    )
                    .footer(use_code(cx));
            }
            Stage::Choose => {
                dialog = dialog
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(muted(t(M::SignInBrowserBody)))
                            .child(muted(tr!(
                                M::KeepsSignIn,
                                keychain = t(CommonMsg::KeychainInFull)
                            ))),
                    )
                    .footer(
                        Button::new("sign-in-browser", t(M::SignInWithMicrosoft))
                            .variant(ButtonVariant::Primary)
                            .icon(Icon::ArrowUpRightFromSquare)
                            .on_click(cx.listener(|this, _, _, cx| this.start_browser(cx))),
                    )
                    .footer(use_code(cx));
            }
            Stage::Browser => {
                dialog = dialog
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(waiting(t(M::WaitingForBrowser)))
                            .child(muted(t(M::FinishInBrowser))),
                    )
                    .footer(
                        Button::new("reopen-browser", t(M::OpenBrowserAgain))
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| this.start_browser(cx))),
                    )
                    .footer(use_code(cx))
                    .footer(back(cx));
            }
            Stage::RequestingCode => {
                dialog = dialog.child(waiting(t(M::GettingCode))).footer(back(cx));
            }
            Stage::DeviceCode {
                user_code,
                verification_uri,
                opened,
            } => {
                let host = url::Url::parse(verification_uri)
                    .ok()
                    .and_then(|url| {
                        url.host_str().map(|host| {
                            format!("{}{}", host.trim_start_matches("www."), url.path())
                        })
                    })
                    .unwrap_or_else(|| verification_uri.to_string());
                dialog = dialog
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(muted(tr!(M::GoToHost, host = host)))
                            .child(
                                div()
                                    .flex()
                                    .justify_center()
                                    .py_3()
                                    .rounded(radius::LG)
                                    .bg(colors::GRAY_900)
                                    .border_1()
                                    .border_color(crate::theme::ink::border())
                                    .font_family(fonts::mono())
                                    .text_size(theme::text_size(text::XLARGE_2))
                                    .line_height(theme::line_height(text::XLARGE_2, leading::TIGHT))
                                    .font_weight(weights::EXTRABOLD.into())
                                    .text_color(crate::theme::ink::text())
                                    .child(user_code.clone()),
                            )
                            .child(waiting(if *opened {
                                t(M::CodeCopiedWaiting)
                            } else {
                                t(M::WaitingForCode)
                            })),
                    )
                    .footer(
                        Button::new("copy-and-open", tr!(M::CopyCodeAndOpen, host = host))
                            .variant(ButtonVariant::Primary)
                            .icon(Icon::Copy)
                            .on_click(cx.listener(|this, _, _, cx| this.copy_code_and_open(cx))),
                    )
                    .footer(back(cx));
            }
            Stage::Link { error } => {
                dialog = dialog
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_3()
                            .child(launcher_identity_note())
                            .child(muted(t(M::LinkInstructions)))
                            .child(self.link_input.clone())
                            .child(
                                div()
                                    .text_size(theme::text_size(text::SMALL))
                                    .line_height(theme::line_height(text::SMALL, leading::NORMAL))
                                    .font_weight(weights::BOLD.into())
                                    .map(|this| match error {
                                        Some(err) => this
                                            .text_color(colors::RED_200)
                                            .child(auth_errors::message(err)),
                                        None => this.text_color(crate::theme::ink::muted()).child(
                                            tr!(M::LinkStartsWith, url = auth_errors::LINK_PAGE),
                                        ),
                                    }),
                            ),
                    )
                    .footer(
                        Button::new("submit-link", t(CommonMsg::SignIn))
                            .variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|this, _, _, cx| this.submit_link(cx))),
                    )
                    .footer(
                        Button::new("reopen-link", t(M::OpenBrowserAgain))
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| this.reopen_link(cx))),
                    )
                    .footer(back(cx));
            }
            Stage::Finishing => {
                dialog = dialog.child(waiting(t(M::SigningInToMinecraft)));
            }
            Stage::Failed(err) => {
                // Mojang approving the app is only pending for Lodestone's own client.
                let not_approved = *err == AuthError::AppNotApproved && !launcher_identity;
                dialog =
                    dialog
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .gap_3()
                                .p_4()
                                .rounded(radius::LG)
                                .bg(colors::YELLOW_FADED.opacity(10))
                                .border_1()
                                .border_color(colors::YELLOW_FADED.opacity(50))
                                .child(div().pt(px(3.)).child(
                                    Icon::TriangleExclamation.render(px(16.), colors::YELLOW),
                                ))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(muted(auth_errors::message(err))),
                                ),
                        )
                        .when(not_approved, |this| {
                            this.child(div().mt_2().child(muted(t(M::ExpectedUntilApproved))))
                        });
                if not_approved && offline_enabled {
                    dialog = dialog.footer(
                        Button::new("use-offline", t(M::AddOfflineAccount))
                            .variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SignInEvent::UseOffline))),
                    );
                }
                dialog = dialog
                    .footer(
                        Button::new("retry", t(CommonMsg::TryAgain))
                            .variant(if not_approved && offline_enabled {
                                ButtonVariant::Secondary
                            } else {
                                ButtonVariant::Primary
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.start_default(cx))),
                    )
                    .footer(use_code(cx));
            }
        }
        dialog
    }
}
