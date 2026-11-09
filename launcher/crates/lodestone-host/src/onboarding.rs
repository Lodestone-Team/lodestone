//! The screens before the shell, after the dashboard's login pages
//! (`dashboard/src/pages/login/*`): the core starting (or failing), the first run's
//! owner account and core name, and signing in with a password.
//!
//! Each form is a view that emits its values when submitted; [`crate::app`] does the
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
    session::{OwnerProblem, owner_problem},
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
                .child(
                    img(lodestone_ui::theme::palette::logo())
                        .h(px(28.))
                        .w(px(134.)),
                )
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

/// The owner account typed on the first run.
pub struct OwnerSubmitted {
    pub username: String,
    pub password: String,
}

/// The first run: the owner account (`CoreSetupNew.tsx`). The setup key the dashboard
/// asks for isn't needed: the claim proves the Host may set up its own core.
pub struct OwnerForm {
    username: Entity<TextInput>,
    password: Entity<TextInput>,
    confirm: Entity<TextInput>,
    error: Option<String>,
    busy: bool,
}

impl EventEmitter<OwnerSubmitted> for OwnerForm {}

impl OwnerForm {
    pub fn new(cx: &mut Context<Self>) -> Self {
        Self {
            username: input(ShellMsg::Username, false, "", cx),
            password: input(ShellMsg::Password, true, "", cx),
            confirm: input(ShellMsg::ConfirmPassword, true, "", cx),
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
        let confirm = self.confirm.read(cx).text().to_string();
        if let Some(problem) = owner_problem(&username, &password, &confirm) {
            self.error = Some(
                t(match problem {
                    OwnerProblem::NoUsername => ShellMsg::NoUsername,
                    OwnerProblem::NoPassword => ShellMsg::NoPassword,
                    OwnerProblem::Mismatch => ShellMsg::PasswordsDiffer,
                })
                .to_string(),
            );
            cx.notify();
            return;
        }
        self.error = None;
        self.busy = true;
        cx.emit(OwnerSubmitted { username, password });
        cx.notify();
    }

    /// Types the owner account and presses Create, for the development screenshots'
    /// end-to-end run (`--owner`).
    #[cfg(feature = "screenshot")]
    pub fn fill_and_submit(&mut self, username: &str, password: &str, cx: &mut Context<Self>) {
        self.username
            .update(cx, |input, cx| input.set_text(username.to_string(), cx));
        for input in [&self.password, &self.confirm] {
            input.update(cx, |input, cx| input.set_text(password.to_string(), cx));
        }
        self.submit(cx);
    }
}

impl Render for OwnerForm {
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
                    title(t(ShellMsg::SetupTitle)),
                    paragraph(t(ShellMsg::SetupIntro)),
                    field(t(ShellMsg::Username), &self.username),
                    field(t(ShellMsg::Password), &self.password),
                    field(t(ShellMsg::ConfirmPassword), &self.confirm),
                ]
                .into_iter()
                .chain(error_line(&self.error))
                .chain([Button::new("create-owner", t(ShellMsg::CreateOwner))
                    .variant(ButtonVariant::Primary)
                    .loading(self.busy)
                    .on_click(submit)
                    .into_any_element()]),
            ))
    }
}

/// The core's name and public address (`CoreConfigNew.tsx`).
pub struct CoreConfigSubmitted {
    pub name: String,
    pub domain: String,
}

pub struct CoreConfigForm {
    name: Entity<TextInput>,
    domain: Entity<TextInput>,
    error: Option<String>,
    busy: bool,
}

impl EventEmitter<CoreConfigSubmitted> for CoreConfigForm {}

impl CoreConfigForm {
    pub fn new(name: &str, domain: &str, cx: &mut Context<Self>) -> Self {
        Self {
            name: input(ShellMsg::CoreName, false, name, cx),
            domain: input(ShellMsg::PublicAddress, false, domain, cx),
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
        let name = self.name.read(cx).text().trim().to_string();
        let domain = self.domain.read(cx).text().trim().to_string();
        self.busy = true;
        cx.emit(CoreConfigSubmitted { name, domain });
        cx.notify();
    }

    /// Presses Continue with the values shown, for the development screenshots.
    #[cfg(feature = "screenshot")]
    pub fn submit_for_screenshot(&mut self, cx: &mut Context<Self>) {
        self.submit(cx);
    }
}

impl Render for CoreConfigForm {
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
                    title(t(ShellMsg::FirstConfigTitle)),
                    field(t(ShellMsg::CoreName), &self.name),
                    field(t(ShellMsg::PublicAddress), &self.domain),
                    paragraph(t(ShellMsg::PublicAddressHint)),
                ]
                .into_iter()
                .chain(error_line(&self.error))
                .chain([Button::new("save-core-config", t(CommonMsg::Continue))
                    .variant(ButtonVariant::Primary)
                    .loading(self.busy)
                    .on_click(submit)
                    .into_any_element()]),
            ))
    }
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
