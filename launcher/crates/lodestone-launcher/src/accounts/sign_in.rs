//! The "Sign in with Microsoft" dialog: the browser flow by default, a device code
//! as the fallback, and friendly errors with a retry.
//!
//! With Settings > Developer > "Sign in as the Minecraft Launcher" on, the dialog
//! signs in with the official launcher's client instead, with a device code. (Its
//! other way, pasting the address of login.live.com's final page, is left out: it
//! teaches people to copy sign-in addresses out of the browser.)

use crate::theme::px;
use std::time::Duration;

use super::{Accounts, auth_errors};
use crate::{
    components::{
        Button, ButtonVariant, Dialog, DialogSlot, Icon, IconButton, RichText,
        a11y::{Accessible, Role},
    },
    i18n::{CommonMsg, ShellMsg as M, t, tr},
    theme::{self, colors, fonts, leading, radius, text, weights},
};
use futures::FutureExt as _;
use gpui::{
    AsyncApp, ClipboardItem, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement,
    Render, SharedString, Task, WeakEntity, Window, div, prelude::*,
};
use launcher_core::auth::{AuthError, ClientIdentity, MsTokens, loopback::CancelHandle};

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

/// Where "Stop signing in" goes: back to the first step, or out of the dialog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StopTo {
    Back,
    Close,
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
    code_copied: bool,
    _code_copied_reset: Option<Task<()>>,
    /// "Stop signing in?", over the dialog once the code has been copied.
    stop_prompt: DialogSlot<SignInDialog, (FocusHandle, StopTo)>,
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
            code_copied: false,
            _code_copied_reset: None,
            stop_prompt: DialogSlot::new(|this: &mut SignInDialog| &mut this.stop_prompt),
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
        // The sign-in moved on (it finished, or failed): the question no longer applies.
        if self.stop_prompt.is_open() && !self.code_handed_out() {
            self.stop_prompt.close(cx);
        }
        cx.notify();
    }

    /// The code has been copied (with either button), so the user may be entering it
    /// on another device: leaving now would waste that, so it asks first.
    fn code_handed_out(&self) -> bool {
        matches!(self.stage, Stage::DeviceCode { opened: true, .. })
    }

    /// Back or closing (the × or Escape; the backdrop never closes this dialog): at
    /// once, or after "Stop signing in?" once the code has been copied.
    fn request_stop(&mut self, to: StopTo, cx: &mut Context<Self>) {
        if !self.code_handed_out() {
            return self.stop_to(to, cx);
        }
        if !self.stop_prompt.is_open() {
            self.stop_prompt.open((cx.focus_handle(), to), cx);
        }
    }

    /// Shows "Stop signing in?" as closing would, for the offscreen screenshots.
    #[cfg(feature = "screenshot")]
    pub fn ask_to_stop(&mut self, cx: &mut Context<Self>) {
        self.request_stop(StopTo::Close, cx);
    }

    /// "Keep waiting": back to the code, with the sign-in still waiting for it.
    fn keep_waiting(&mut self, cx: &mut Context<Self>) {
        self.stop_prompt.close(cx);
    }

    /// "Stop signing in", or Back and closing before the code was copied.
    fn stop_to(&mut self, to: StopTo, cx: &mut Context<Self>) {
        self.stop_prompt.close(cx);
        match to {
            StopTo::Back => self.back(cx),
            StopTo::Close => self.dismiss(cx),
        }
    }

    fn stop(&mut self) {
        if let Some(cancel) = self.cancel.take() {
            cancel.cancel();
        }
        self.task = None;
    }

    /// Back: stops the sign-in under way and goes back to the first step. (The × and
    /// Escape close the dialog; a click on the backdrop does nothing.)
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

    /// The first step: the browser for Lodestone's app, a code for the
    /// Minecraft Launcher's client.
    fn start_default(&mut self, cx: &mut Context<Self>) {
        if self.launcher_identity() {
            self.start_device_code(cx);
        } else {
            self.start_browser(cx);
        }
    }

    /// The button beside the code: copies it, and shows a check for a moment.
    fn copy_code(&mut self, cx: &mut Context<Self>) {
        let Stage::DeviceCode { user_code, .. } = &self.stage else {
            return;
        };
        let user_code = user_code.clone();
        cx.write_to_clipboard(ClipboardItem::new_string(user_code.to_string()));
        if let Stage::DeviceCode { opened, .. } = &mut self.stage {
            *opened = true;
        }
        self.code_copied = true;
        cx.notify();
        self._code_copied_reset = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(Duration::from_secs(2)).await;
            this.update(cx, |this, cx| {
                this.code_copied = false;
                cx.notify();
            })
            .ok();
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

/// "Stop signing in?": Stop signing in, or Keep waiting (which Escape and the × also
/// mean).
fn stop_prompt(focus: FocusHandle, to: StopTo, cx: &mut Context<SignInDialog>) -> Dialog {
    let entity = cx.entity();
    Dialog::new("sign-in-stop", t(M::StopSigningInTitle), focus)
        .keep_open_on_backdrop()
        .on_close(move |_, cx| entity.update(cx, |this, cx| this.keep_waiting(cx)))
        .child(
            div()
                .text_color(crate::theme::ink::secondary())
                .child(t(M::StopSigningInBody)),
        )
        .footer(
            Button::new("sign-in-stop-confirm", t(M::StopSigningIn))
                .variant(ButtonVariant::Danger)
                .on_click(cx.listener(move |this, _, _, cx| this.stop_to(to, cx))),
        )
        .footer(
            Button::new("sign-in-keep-waiting", t(M::KeepWaiting))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| this.keep_waiting(cx))),
        )
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
            this.update(cx, |this, cx| {
                // The browser's answer is in (or never will be): let go of the loopback
                // server, whose handle otherwise keeps its five threads and its port
                // until the dialog is closed or tried again.
                this.cancel = None;
                this.set_stage(Stage::Failed(err), cx)
            })
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
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let offline_enabled = self.accounts.read(cx).offline_enabled();
        let entity = cx.entity();
        let title = match &self.stage {
            Stage::Failed(err) => auth_errors::title(err),
            _ => t(M::SignInWithMicrosoft),
        };
        // Signing in can't be lost to a stray click: only the ×, Escape and Back leave,
        // and they ask first once the code has been copied.
        let mut dialog = Dialog::new("sign-in", title, self.focus_handle.clone())
            .keep_open_on_backdrop()
            .on_close(move |_, cx| {
                entity.update(cx, |this, cx| this.request_stop(StopTo::Close, cx))
            });

        let launcher_identity = self.launcher_identity();
        // The other way to sign in with Lodestone's app: a code. (The Minecraft
        // Launcher's client signs in with a code only.)
        let use_code = |cx: &mut Context<Self>| {
            Button::new("use-code", t(M::UseCodeInstead))
                .variant(ButtonVariant::Ghost)
                .on_click(cx.listener(|this, _, _, cx| this.start_device_code(cx)))
        };
        let back = |cx: &mut Context<Self>| {
            Button::new("sign-in-back", t(CommonMsg::Back))
                .variant(ButtonVariant::Secondary)
                .on_click(cx.listener(|this, _, _, cx| this.request_stop(StopTo::Back, cx)))
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
                    );
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
                                // The code, centred and selectable, with a copy button
                                // at the box's right edge.
                                div()
                                    .id("device-code")
                                    .accessible(Role::Label, "device-code", user_code.clone())
                                    .relative()
                                    .flex()
                                    .justify_center()
                                    .py_3()
                                    .px(px(56.))
                                    .rounded(radius::LG)
                                    .bg(colors::GRAY_900)
                                    .border_1()
                                    .border_color(crate::theme::ink::border())
                                    .font_family(fonts::mono())
                                    .text_size(theme::text_size(text::XLARGE_2))
                                    .line_height(theme::line_height(text::XLARGE_2, leading::TIGHT))
                                    .font_weight(weights::EXTRABOLD.into())
                                    .text_color(crate::theme::ink::text())
                                    .child(RichText::new(user_code.clone()).id("device-code-text"))
                                    .child(
                                        div()
                                            .absolute()
                                            .top_0()
                                            .bottom_0()
                                            .right(px(12.))
                                            .flex()
                                            .items_center()
                                            .child(
                                                IconButton::new(
                                                    "copy-device-code",
                                                    if self.code_copied {
                                                        Icon::Check
                                                    } else {
                                                        Icon::Copy
                                                    },
                                                )
                                                .label(if self.code_copied {
                                                    t(CommonMsg::Copied)
                                                } else {
                                                    t(M::CopySignInCode)
                                                })
                                                .on_click(
                                                    cx.listener(|this, _, _, cx| {
                                                        this.copy_code(cx)
                                                    }),
                                                ),
                                            ),
                                    ),
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
            Stage::Finishing => {
                dialog = dialog.child(waiting(t(M::SigningInToMinecraft)));
            }
            Stage::Failed(err) => {
                // Mojang approving the app is only pending for Lodestone's own client.
                let not_approved = *err == AuthError::AppNotApproved && !launcher_identity;
                dialog = dialog
                    .child(
                        div()
                            .id("sign-in-error")
                            .accessible(Role::Status, "sign-in-error", auth_errors::message(err))
                            .flex()
                            .flex_row()
                            .gap_3()
                            .p_4()
                            .rounded(radius::LG)
                            .bg(colors::YELLOW_FADED.opacity(10))
                            .border_1()
                            .border_color(colors::YELLOW_FADED.opacity(50))
                            .child(
                                div().pt(px(3.)).child(
                                    Icon::TriangleExclamation.render(px(16.), colors::YELLOW),
                                ),
                            )
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
                dialog = dialog.footer(
                    Button::new("retry", t(CommonMsg::TryAgain))
                        .variant(if not_approved && offline_enabled {
                            ButtonVariant::Secondary
                        } else {
                            ButtonVariant::Primary
                        })
                        .on_click(cx.listener(|this, _, _, cx| this.start_default(cx))),
                );
                if !launcher_identity {
                    dialog = dialog.footer(use_code(cx));
                }
            }
        }
        // The question is a second dialog over this one, so this view fills the window
        // for both of them.
        let stop_prompt = self.stop_prompt.render(window, |(focus, to)| {
            Some(stop_prompt(focus.clone(), *to, cx).into_any_element())
        });
        div()
            .absolute()
            .inset_0()
            .child(dialog)
            .children(stop_prompt)
    }
}

#[cfg(all(test, feature = "screenshot"))]
mod close_tests {
    use std::{cell::Cell, rc::Rc};

    use gpui::{Modifiers, TestAppContext, VisualTestContext, point};

    use super::*;
    use crate::components::EXIT;

    struct Opened<'a> {
        dialog: Entity<SignInDialog>,
        cx: &'a mut VisualTestContext,
        dismissed: Rc<Cell<usize>>,
    }

    impl Opened<'_> {
        fn asking(&mut self) -> bool {
            self.dialog
                .read_with(self.cx, |this, _| this.stop_prompt.is_open())
        }

        fn at_first_step(&mut self) -> bool {
            self.dialog
                .read_with(self.cx, |this, _| matches!(this.stage, Stage::Choose))
        }

        fn close(&mut self, how: &str) {
            match how {
                "escape" => self.cx.simulate_keystrokes("escape"),
                "×" => {
                    let bounds = self.cx.debug_bounds("dialog-close").expect("the ×");
                    self.cx.simulate_click(bounds.center(), Modifiers::none());
                }
                _ => self
                    .cx
                    .simulate_click(point(px(4.), px(4.)), Modifiers::none()),
            }
            self.cx.run_until_parked();
        }

        fn update(&mut self, f: impl FnOnce(&mut SignInDialog, &mut Context<SignInDialog>)) {
            self.dialog.update(self.cx, f);
            self.cx.run_until_parked();
        }

        /// Lets a closing prompt finish fading out, so it no longer takes clicks.
        fn settle(&mut self) {
            self.cx.executor().advance_clock(EXIT * 2);
            self.cx.run_until_parked();
        }
    }

    /// The dialog showing a code, copied (`copied`) or not.
    fn open(copied: bool, cx: &mut TestAppContext) -> Opened<'_> {
        cx.update(crate::components::init);
        let (dialog, cx) = cx.add_window_view(|_, cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            SignInDialog::new(accounts, cx).with_stage(Stage::DeviceCode {
                user_code: "F7K2-9QXM".into(),
                verification_uri: "https://www.microsoft.com/link".into(),
                opened: copied,
            })
        });
        let dismissed = Rc::new(Cell::new(0));
        let count = dismissed.clone();
        cx.update(|_, cx| {
            cx.subscribe(&dialog, move |_, event: &SignInEvent, _| {
                if matches!(event, SignInEvent::Dismissed) {
                    count.set(count.get() + 1)
                }
            })
            .detach()
        });
        cx.run_until_parked();
        Opened {
            dialog,
            cx,
            dismissed,
        }
    }

    #[test]
    fn the_backdrop_never_closes_it() {
        for copied in [false, true] {
            let mut cx = TestAppContext::single();
            let mut opened = open(copied, &mut cx);
            opened.close("backdrop");
            assert_eq!(opened.dismissed.get(), 0, "copied: {copied}");
            assert!(!opened.asking(), "copied: {copied}");
        }
    }

    #[test]
    fn before_the_code_is_copied_escape_and_the_x_close_at_once() {
        for how in ["escape", "×"] {
            let mut cx = TestAppContext::single();
            let mut opened = open(false, &mut cx);
            opened.close(how);
            assert!(!opened.asking(), "{how}: no question");
            assert_eq!(opened.dismissed.get(), 1, "{how} closes");
        }
    }

    #[test]
    fn once_the_code_is_copied_escape_and_the_x_ask_first() {
        for how in ["escape", "×"] {
            let mut cx = TestAppContext::single();
            let mut opened = open(true, &mut cx);
            opened.close(how);
            assert!(opened.asking(), "{how} asks");
            assert_eq!(opened.dismissed.get(), 0, "{how}: still open");
            // "Stop signing in" closes.
            opened.update(|this, cx| this.stop_to(StopTo::Close, cx));
            assert_eq!(opened.dismissed.get(), 1, "{how}, then Stop: closed");
        }
    }

    #[test]
    fn keep_waiting_and_escape_on_the_question_go_back_to_the_code() {
        let mut cx = TestAppContext::single();
        let mut opened = open(true, &mut cx);
        opened.close("×");
        // Escape on the question means Keep waiting.
        opened.close("escape");
        assert!(!opened.asking());
        assert_eq!(opened.dismissed.get(), 0);
        opened.settle();
        // So does the backdrop not close anything, nor ask.
        opened.close("backdrop");
        assert!(!opened.asking());
        assert_eq!(opened.dismissed.get(), 0);
    }

    #[test]
    fn back_asks_once_the_code_is_copied() {
        let mut cx = TestAppContext::single();
        let mut opened = open(true, &mut cx);
        opened.update(|this, cx| this.request_stop(StopTo::Back, cx));
        assert!(opened.asking());
        assert!(!opened.at_first_step());
        opened.update(|this, cx| this.stop_to(StopTo::Back, cx));
        assert!(!opened.asking());
        assert!(opened.at_first_step());
        assert_eq!(opened.dismissed.get(), 0, "Back doesn't close the dialog");
    }

    #[test]
    fn the_copy_button_counts_as_copying_the_code() {
        let mut cx = TestAppContext::single();
        let mut opened = open(false, &mut cx);
        opened.update(|this, cx| this.copy_code(cx));
        opened.update(|this, cx| this.request_stop(StopTo::Back, cx));
        assert!(opened.asking());
    }
}
