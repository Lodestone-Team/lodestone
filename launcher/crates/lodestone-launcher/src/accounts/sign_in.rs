//! The "Sign in with Microsoft" dialog: the browser flow by default, a device code
//! as the fallback, and friendly errors with a retry.

use std::time::Duration;

use futures::FutureExt as _;
use gpui::{
    AsyncApp, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Task, WeakEntity, Window, div, prelude::*, px,
};
use launcher_core::auth::{AuthError, MsTokens, loopback::CancelHandle};

use super::Accounts;
use crate::{
    components::{Button, ButtonVariant, Dialog, Icon},
    theme::{self, colors, fonts, leading, radius, text, weights},
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
}

impl EventEmitter<SignInEvent> for SignInDialog {}

impl Focusable for SignInDialog {
    fn focus_handle(&self, _: &gpui::App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl SignInDialog {
    pub fn new(accounts: Entity<Accounts>, cx: &mut Context<Self>) -> Self {
        Self {
            accounts,
            focus_handle: cx.focus_handle(),
            stage: Stage::Choose,
            task: None,
            cancel: None,
        }
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
    }

    fn cancel(&mut self, cx: &mut Context<Self>) {
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
        self.task = Some(cx.spawn(async move |this, cx| {
            let code = match auth.start_device_code().await {
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

fn muted(text: impl Into<SharedString>) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::MEDIUM))
        .line_height(theme::line_height(text::MEDIUM, leading::NORMAL))
        .text_color(colors::GRAY_400)
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
        .border_color(colors::FADE_700)
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
            Stage::Failed(err) => err.title(),
            _ => "Sign in with Microsoft",
        };
        let mut dialog = Dialog::new("sign-in", title, self.focus_handle.clone())
            .on_close(move |_, cx| entity.update(cx, |this, cx| this.dismiss(cx)));

        let use_code = |cx: &mut Context<Self>| {
            Button::new("use-code", "Use a code instead")
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(|this, _, _, cx| this.start_device_code(cx)))
        };
        let cancel = |cx: &mut Context<Self>| {
            Button::new("cancel-sign-in", "Cancel")
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| this.cancel(cx)))
        };

        match &self.stage {
            Stage::Choose => {
                dialog = dialog
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(muted(
                                "Sign in with the Microsoft account you play Minecraft: Java Edition with. Your browser opens to finish signing in.",
                            ))
                            .child(muted(
                                "Lodestone Launcher keeps your sign-in in the macOS Keychain.",
                            )),
                    )
                    .footer(
                        Button::new("sign-in-browser", "Sign in with Microsoft")
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
                            .child(waiting("Waiting for your browser…"))
                            .child(muted(
                                "Finish signing in in the browser window that opened, then come back here.",
                            )),
                    )
                    .footer(cancel(cx))
                    .footer(
                        Button::new("reopen-browser", "Open the browser again")
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(|this, _, _, cx| this.start_browser(cx))),
                    )
                    .footer(use_code(cx));
            }
            Stage::RequestingCode => {
                dialog = dialog.child(waiting("Getting a code…")).footer(cancel(cx));
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
                            .child(muted(format!(
                                "Go to {host} on any device and enter this code:"
                            )))
                            .child(
                                div()
                                    .flex()
                                    .justify_center()
                                    .py_3()
                                    .rounded(radius::LG)
                                    .bg(colors::GRAY_900)
                                    .border_1()
                                    .border_color(colors::FADE_700)
                                    .font_family(fonts::MONO)
                                    .text_size(theme::text_size(text::XLARGE_2))
                                    .line_height(theme::line_height(text::XLARGE_2, leading::TIGHT))
                                    .font_weight(weights::EXTRABOLD.into())
                                    .text_color(colors::GRAY_300)
                                    .child(user_code.clone()),
                            )
                            .child(waiting(if *opened {
                                "Code copied. Waiting for you to enter it…"
                            } else {
                                "Waiting for you to enter the code…"
                            })),
                    )
                    .footer(
                        Button::new("copy-and-open", format!("Copy code and open {host}"))
                            .variant(ButtonVariant::Primary)
                            .icon(Icon::Copy)
                            .on_click(cx.listener(|this, _, _, cx| this.copy_code_and_open(cx))),
                    )
                    .footer(cancel(cx));
            }
            Stage::Finishing => {
                dialog = dialog.child(waiting("Signing in to Minecraft…"));
            }
            Stage::Failed(err) => {
                let not_approved = *err == AuthError::AppNotApproved;
                dialog = dialog
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
                            .child(
                                div()
                                    .pt(px(3.))
                                    .child(Icon::TriangleExclamation.render(px(16.), colors::YELLOW)),
                            )
                            .child(div().flex_1().min_w_0().child(muted(err.message()))),
                    )
                    .when(not_approved, |this| {
                        this.child(div().mt_2().child(muted(
                            "This is expected until Mojang approves the app. Nothing is wrong with your account.",
                        )))
                    });
                if not_approved && offline_enabled {
                    dialog = dialog.footer(
                        Button::new("use-offline", "Add offline account")
                            .variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|_, _, _, cx| cx.emit(SignInEvent::UseOffline))),
                    );
                }
                dialog = dialog
                    .footer(
                        Button::new("retry", "Try again")
                            .variant(if not_approved && offline_enabled {
                                ButtonVariant::Secondary
                            } else {
                                ButtonVariant::Primary
                            })
                            .on_click(cx.listener(|this, _, _, cx| this.start_browser(cx))),
                    )
                    .footer(use_code(cx));
            }
        }
        dialog
    }
}
