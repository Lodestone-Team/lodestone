//! The screens before the shell, after the dashboard's login pages
//! (`dashboard/src/pages/login/*`): the core starting (or failing), and signing in with
//! a password. The first run asks nothing: the Host makes the core's owner without a
//! password (`crate::session`), and asks for one only when the core is shared
//! (`pages::account_dialogs`).
//!
//! The form is a view that emits its values when submitted; [`crate::app`] does the
//! work and puts errors back with `set_error`.

use gpui::{
    AnyElement, App, Context, Entity, EventEmitter, IntoElement, KeyDownEvent, ParentElement,
    Render, SharedString, Styled, Window, div, img, prelude::*,
};
use lodestone_ui::{
    components::{Button, ButtonVariant, TextInput},
    theme::{colors, fonts, ink, px, radius, text, weights},
};

use crate::{
    i18n::{CommonMsg, ShellMsg, t},
    supervisor::CoreStatus,
};

/// A card in the middle of the window, as the dashboard's `LoginLayout`.
pub fn card(children: impl IntoIterator<Item = AnyElement>) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .bg(colors::GRAY_900)
        .child(
            div()
                .id("login-card")
                .w(px(500.))
                .max_h_full()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(16.))
                .p(px(32.))
                .rounded(radius::XL)
                .bg(colors::GRAY_850)
                .border_1()
                .border_color(ink::border())
                .child(img(crate::shell::logo()).h(px(28.)).w(px(134.)))
                .children(children),
        )
}

fn title(text_: impl Into<SharedString>) -> AnyElement {
    div()
        .font_family(fonts::heading())
        .text_size(text::H1)
        .font_weight(weights::BOLD.face())
        .text_color(ink::text())
        .child(text_.into())
        .into_any_element()
}

fn paragraph(text_: impl Into<SharedString>) -> AnyElement {
    div()
        .text_size(text::MEDIUM)
        .text_color(ink::muted())
        .child(text_.into())
        .into_any_element()
}

fn error_line(error: &Option<String>) -> Option<AnyElement> {
    error.as_ref().map(|error| {
        div()
            .text_size(text::MEDIUM)
            .text_color(colors::RED_200)
            .child(error.clone())
            .into_any_element()
    })
}

fn field(label: &'static str, input: &Entity<TextInput>) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .child(
            div()
                .text_size(text::MEDIUM)
                .font_weight(weights::MEDIUM.face())
                .text_color(ink::secondary())
                .child(label),
        )
        .child(input.clone())
        .into_any_element()
}

fn input(label: ShellMsg, masked: bool, text_: &str, cx: &mut App) -> Entity<TextInput> {
    let text_ = SharedString::from(text_.to_string());
    cx.new(|cx| {
        let input = TextInput::new(cx).with_label(label).with_text(text_);
        if masked { input.with_masked() } else { input }
    })
}

/// While the core starts, restarts or stops, and when it couldn't start.
pub fn core_status(
    status: &CoreStatus,
    log_folder: std::path::PathBuf,
    retry: impl Fn(&mut Window, &mut App) + 'static,
) -> AnyElement {
    let waiting = |message: SharedString| {
        card([div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(lodestone_ui::components::LoadingDots)
            .child(
                div()
                    .text_size(text::H3)
                    .text_color(ink::text())
                    .child(message),
            )
            .into_any_element()])
        .into_any_element()
    };
    match status {
        CoreStatus::Starting => waiting(t(ShellMsg::StartingCore).into()),
        CoreStatus::Restarting { .. } => waiting(t(ShellMsg::RestartingCore).into()),
        CoreStatus::Running { .. } => waiting(t(ShellMsg::SigningIn).into()),
        CoreStatus::Stopping | CoreStatus::Stopped => waiting(t(ShellMsg::StoppingCore).into()),
        CoreStatus::Failed { reason, log } => card([
            title(t(ShellMsg::CoreFailed)),
            paragraph(reason.clone()),
            div()
                .when(!log.is_empty(), |this| {
                    this.child(paragraph(t(ShellMsg::CoreLog)))
                })
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .p(px(8.))
                        .rounded(radius::MD)
                        .bg(colors::GRAY_900)
                        .font_family(fonts::mono())
                        .text_size(text::SMALL)
                        .text_color(ink::secondary())
                        .children(log.iter().map(|line| div().child(line.clone()))),
                )
                .into_any_element(),
            div()
                .flex()
                .gap(px(8.))
                .child(
                    Button::new("retry-core", t(CommonMsg::TryAgain))
                        .variant(ButtonVariant::Primary)
                        .on_click(move |_, window, cx| retry(window, cx)),
                )
                .child(
                    Button::new("open-log", t(ShellMsg::OpenLogFolder)).on_click(
                        move |_, _, cx| {
                            let _ = std::fs::create_dir_all(&log_folder);
                            cx.open_with_system(&log_folder);
                        },
                    ),
                )
                .into_any_element(),
        ])
        .into_any_element(),
    }
}

/// A sign-in or setup error, with a button to try again.
pub fn sign_in_failed(error: &str, retry: impl Fn(&mut Window, &mut App) + 'static) -> AnyElement {
    card([
        title(t(ShellMsg::SignInTitle)),
        paragraph(error.to_string()),
        Button::new("retry-sign-in", t(CommonMsg::TryAgain))
            .variant(ButtonVariant::Primary)
            .on_click(move |_, window, cx| retry(window, cx))
            .into_any_element(),
    ])
    .into_any_element()
}

/// A username and password (`UserLogin.tsx`), for a core that refuses the claim.
pub struct PasswordSubmitted {
    pub username: String,
    pub password: String,
}

pub struct PasswordForm {
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    error: Option<String>,
    busy: bool,
}

impl EventEmitter<PasswordSubmitted> for PasswordForm {}

impl PasswordForm {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            username: input(ShellMsg::Username, false, "", cx),
            password: input(ShellMsg::Password, true, "", cx),
            error: None,
            busy: false,
        }
    }

    pub fn set_error(&mut self, error: Option<String>, cx: &mut Context<Self>) {
        self.error = error;
        self.busy = false;
        cx.notify();
    }

    fn submit(&mut self, cx: &mut Context<Self>) {
        if self.busy {
            return;
        }
        let username = self.username.read(cx).text().trim().to_string();
        let password = self.password.read(cx).text().to_string();
        if username.is_empty() || password.is_empty() {
            self.error = Some(
                t(if username.is_empty() {
                    ShellMsg::NoUsername
                } else {
                    ShellMsg::NoPassword
                })
                .to_string(),
            );
            cx.notify();
            return;
        }
        self.error = None;
        self.busy = true;
        cx.emit(PasswordSubmitted { username, password });
        cx.notify();
    }
}

impl Render for PasswordForm {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let submit = cx.listener(|this, _, _, cx| this.submit(cx));
        div()
            .size_full()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                if event.keystroke.key == "enter" {
                    this.submit(cx);
                }
            }))
            .child(card(
                [
                    title(t(ShellMsg::SignInTitle)),
                    paragraph(t(ShellMsg::SignInIntro)),
                    field(t(ShellMsg::Username), &self.username),
                    field(t(ShellMsg::Password), &self.password),
                ]
                .into_iter()
                .chain(error_line(&self.error))
                .chain([Button::new("sign-in", t(CommonMsg::SignIn))
                    .variant(ButtonVariant::Primary)
                    .loading(self.busy)
                    .on_click(submit)
                    .into_any_element()]),
            ))
    }
}
