//! The window's root: starts the core, signs in to it, and then shows the shell.
//!
//! 1. The [`Supervisor`] starts the core and says when it answers.
//! 2. The Host claims owner access to it ([`session::claim_owner`]). A core without an
//!    owner asks for one first ([`OwnerForm`]) and then for its name
//!    ([`CoreConfigForm`]); a core that refuses claims asks for a password
//!    ([`PasswordForm`]).
//! 3. The [`HostShell`] shows the dashboard. If the core restarts, or the session
//!    ends, the Host signs in again.
//!
//! Closing the window keeps the Host, the core and its servers running in the
//! background (the menu bar icon and the Dock bring it back); Quit stops the core,
//! whose servers stop first, then quits.

use std::sync::Arc;

use gpui::{
    AnyElement, App, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement,
    Render, Styled, Subscription, Task, Window, div, prelude::*,
};
use launcher_core::{Secret, cores::local_claim::OwnerCredentials, game::download::Sleep};
use lodestone_ui::{
    components::focus,
    theme::{self, colors, fonts},
};

use crate::{
    core_process::CoreConfig,
    onboarding::{
        self, CoreConfigForm, CoreConfigSubmitted, OwnerForm, OwnerSubmitted, PasswordForm,
        PasswordSubmitted,
    },
    session::{self, Session, SignInError},
    shell::HostShell,
    state::{HostEvent, HostState},
    supervisor::{CoreStatus, Supervisor, SupervisorEvent},
};

enum Phase {
    /// Waiting for the core (or showing why it couldn't start).
    Core,
    SigningIn,
    Owner(Entity<OwnerForm>),
    FirstConfig {
        form: Entity<CoreConfigForm>,
        session: Box<Session>,
    },
    Password(Entity<PasswordForm>),
    Failed(String),
    Shell(Entity<HostShell>),
    Quitting,
}

pub struct HostApp {
    pub supervisor: Entity<Supervisor>,
    phase: Phase,
    root_focus: FocusHandle,
    _focus_lost: Option<Subscription>,
    _subscriptions: Vec<Subscription>,
    task: Option<Task<()>>,
    /// A warning for the next shell: why it is back on this computer's core.
    notice: Option<String>,
}

/// GPUI's timer, for the claim's wait on the core's answer file.
fn gpui_sleep(cx: &App) -> Sleep {
    let executor = cx.background_executor().clone();
    Arc::new(move |duration| Box::pin(executor.timer(duration)))
}

impl HostApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let supervisor = cx.new(Supervisor::new);
        let subscription = cx.subscribe(&supervisor, |this, _, event, cx| match event {
            SupervisorEvent::Ready(config) => this.sign_in(config.clone(), None, cx),
            SupervisorEvent::Status => {
                // the core went away: back to its status, until it answers again
                if !matches!(this.phase, Phase::Quitting)
                    && !matches!(this.supervisor.read(cx).status, CoreStatus::Running { .. })
                {
                    this.phase = Phase::Core;
                }
                // a core that can't start, while the window is in the background: say
                // so with the system's notification, as the Tauri Host did
                if let CoreStatus::Failed { reason, .. } = &this.supervisor.read(cx).status
                    && crate::notifications::in_background(cx)
                {
                    crate::notifications::native::notify(
                        crate::APP_NAME,
                        &crate::i18n::tr!(crate::i18n::CoreMsg::CoreFailedNotice, error = reason),
                    );
                }
                cx.notify();
            }
        });
        // Closing the window keeps the Host and its servers running in the background.
        window.on_window_should_close(cx, |window, cx| {
            hide(window, cx);
            false
        });
        // the theme follows the computer's light or dark setting, when set to
        let appearance = window.observe_window_appearance(|window, cx| {
            if crate::prefs::Prefs::apply_theme(window.appearance(), cx) {
                cx.refresh_windows();
            }
        });
        crate::prefs::Prefs::apply_theme(window.appearance(), cx);
        Self {
            supervisor,
            phase: Phase::Core,
            root_focus: cx.focus_handle(),
            _focus_lost: None,
            _subscriptions: vec![subscription, appearance],
            task: None,
            notice: None,
        }
    }

    /// The running core's settings.
    fn config(&self, cx: &App) -> Option<CoreConfig> {
        self.supervisor.read(cx).config().cloned()
    }

    /// Claims owner access to the core, with `owner` to make on a core without one.
    fn sign_in(
        &mut self,
        config: CoreConfig,
        owner: Option<OwnerCredentials>,
        cx: &mut Context<Self>,
    ) {
        let first_run = owner.is_some();
        if !first_run {
            self.phase = Phase::SigningIn;
        }
        cx.notify();
        let http = match launcher_core::http::default_client() {
            Ok(http) => http,
            Err(err) => {
                self.phase = Phase::Failed(err.to_string());
                return;
            }
        };
        let sleep = gpui_sleep(cx);
        let work = cx.background_spawn(async move {
            session::claim_owner(http, config.local_url(), &config.data_dir, owner, sleep).await
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(session) if first_run => {
                        let settings = session.info.core_name.clone();
                        let form = cx.new(|cx| CoreConfigForm::new(&settings, "", cx));
                        let subscription = cx.subscribe(&form, Self::save_core_config);
                        this._subscriptions.push(subscription);
                        this.phase = Phase::FirstConfig {
                            form,
                            session: Box::new(session),
                        };
                    }
                    Ok(session) => this.show_shell(session, cx),
                    Err(SignInError::NeedsOwner) => {
                        let form = cx.new(OwnerForm::new);
                        let subscription = cx.subscribe(&form, Self::create_owner);
                        this._subscriptions.push(subscription);
                        this.phase = Phase::Owner(form);
                    }
                    Err(SignInError::Refused) if first_run => {
                        // the core refused the account itself (a weak password)
                        if let Phase::Owner(form) = &this.phase {
                            form.update(cx, |form, cx| {
                                form.set_error(Some(SignInError::Refused.message()), cx)
                            });
                        }
                    }
                    Err(SignInError::Refused) => {
                        let form = cx.new(PasswordForm::new);
                        let subscription = cx.subscribe(&form, Self::sign_in_with_password);
                        this._subscriptions.push(subscription);
                        this.phase = Phase::Password(form);
                    }
                    Err(SignInError::Failed(error)) => match &this.phase {
                        Phase::Owner(form) => {
                            form.update(cx, |form, cx| form.set_error(Some(error), cx))
                        }
                        _ => this.phase = Phase::Failed(error),
                    },
                }
                cx.notify();
            })
            .ok();
        }));
    }

    fn create_owner(
        &mut self,
        _: Entity<OwnerForm>,
        event: &OwnerSubmitted,
        cx: &mut Context<Self>,
    ) {
        let Some(config) = self.config(cx) else {
            return;
        };
        let owner = OwnerCredentials {
            username: event.username.clone(),
            password: Secret::new(event.password.clone()),
        };
        self.sign_in(config, Some(owner), cx);
    }

    fn save_core_config(
        &mut self,
        form: Entity<CoreConfigForm>,
        event: &CoreConfigSubmitted,
        cx: &mut Context<Self>,
    ) {
        let Phase::FirstConfig { session, .. } = &self.phase else {
            return;
        };
        let session = (**session).clone();
        let admin = session.admin.clone();
        let (name, domain) = (event.name.clone(), event.domain.clone());
        let work = cx.background_spawn(async move {
            if !name.is_empty() {
                admin.set_core_name(&name).await?;
            }
            admin.set_domain(&domain).await
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| match result {
                Ok(()) => this.show_shell(session, cx),
                Err(err) => form.update(cx, |form, cx| form.set_error(Some(err.to_string()), cx)),
            })
            .ok();
        }));
    }

    fn sign_in_with_password(
        &mut self,
        form: Entity<PasswordForm>,
        event: &PasswordSubmitted,
        cx: &mut Context<Self>,
    ) {
        let Some(config) = self.config(cx) else {
            return;
        };
        let http = match launcher_core::http::default_client() {
            Ok(http) => http,
            Err(err) => {
                form.update(cx, |form, cx| form.set_error(Some(err.to_string()), cx));
                return;
            }
        };
        let (username, password) = (event.username.clone(), Secret::new(event.password.clone()));
        let work = cx.background_spawn(async move {
            session::sign_in_with_password(http, config.local_url(), &username, &password).await
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| match result {
                Ok(session) => this.show_shell(session, cx),
                Err(err) => form.update(cx, |form, cx| form.set_error(Some(err.message()), cx)),
            })
            .ok();
        }));
    }

    fn show_shell(&mut self, session: Session, cx: &mut Context<Self>) {
        self.show_shell_on(session, None, cx);
    }

    /// The shell for `session`; `remote` names another core than this computer's.
    fn show_shell_on(&mut self, session: Session, remote: Option<String>, cx: &mut Context<Self>) {
        let on_local = remote.is_none();
        let state = cx.new(|cx| HostState::new(session, cx));
        // after being signed out of another core
        if let Some(notice) = self.notice.take() {
            state.update(cx, |state, cx| {
                state.toast(lodestone_ui::components::Toast::warning(notice), cx)
            });
        }
        let subscription = cx.subscribe(&state, move |this, _, event, cx| match event {
            HostEvent::SignedOut => {
                if let Some(name) = &remote {
                    this.notice = Some(crate::i18n::tr!(
                        crate::i18n::CoreMsg::RemoteSignedOut,
                        name = name
                    ));
                }
                if let Some(config) = this.config(cx) {
                    this.sign_in(config, None, cx);
                }
            }
            // the Cores dialog: this computer's core, or another
            HostEvent::SwitchCore(target) => match target {
                Some(session) => {
                    let name = session.info.core_name.clone();
                    this.show_shell_on((**session).clone(), Some(name), cx);
                }
                None => {
                    if let Some(config) = this.config(cx) {
                        this.sign_in(config, None, cx);
                    }
                }
            },
            _ => {}
        });
        self._subscriptions.push(subscription);
        let shell = cx.new(|cx| {
            let mut shell = HostShell::new(state, cx);
            shell.on_local = on_local;
            shell
        });
        self.phase = Phase::Shell(shell);
        cx.notify();
    }

    /// Stops the core (its servers stop first), then quits.
    pub fn quit(&mut self, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Quitting) {
            return;
        }
        self.phase = Phase::Quitting;
        cx.notify();
        let stopped = self
            .supervisor
            .update(cx, |supervisor, cx| supervisor.shutdown(cx));
        cx.spawn(async move |_, cx| {
            stopped.await;
            cx.update(|cx| {
                // with the core stopped, a downloaded update can replace it (and
                // restart the Host, if that is why it quits)
                crate::updater::Updater::apply(cx);
                cx.quit()
            });
        })
        .detach();
    }

    /// The shell, once signed in: for the development screenshots.
    pub fn shell(&self) -> Option<Entity<HostShell>> {
        match &self.phase {
            Phase::Shell(shell) => Some(shell.clone()),
            _ => None,
        }
    }

    /// The first run's forms, filled in and sent as a user would (`--owner`, development
    /// screenshots): the owner account, then the core's name as suggested. Returns the
    /// step it is on: "owner", "config", "shell" or "waiting".
    #[cfg(feature = "screenshot")]
    pub fn onboard_for_screenshot(
        &mut self,
        username: &str,
        password: &str,
        cx: &mut Context<Self>,
    ) -> &'static str {
        match &self.phase {
            Phase::Owner(form) => {
                form.update(cx, |form, cx| form.fill_and_submit(username, password, cx));
                "owner"
            }
            Phase::FirstConfig { form, .. } => {
                form.update(cx, |form, cx| form.submit_for_screenshot(cx));
                "config"
            }
            Phase::Shell(_) => "shell",
            _ => "waiting",
        }
    }

    /// Selects the first instance, for the development screenshots.
    pub fn select_first_instance(&mut self, cx: &mut Context<Self>) {
        if let Phase::Shell(shell) = &self.phase {
            shell.update(cx, |shell, cx| {
                let first = shell
                    .state
                    .read(cx)
                    .instances
                    .as_ref()
                    .and_then(|list| list.first())
                    .map(|i| i.uuid.clone());
                if let Some(uuid) = first {
                    shell.select(&uuid, cx);
                }
            });
        }
    }

    fn retry_core(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Core;
        self.supervisor
            .update(cx, |supervisor, cx| supervisor.start(cx));
    }

    fn retry_sign_in(&mut self, cx: &mut Context<Self>) {
        match self.config(cx) {
            Some(config) => self.sign_in(config, None, cx),
            None => self.retry_core(cx),
        }
    }

    /// Zooms in or out a step, and remembers it (`lodestone-zoom`).
    fn zoom_by(&mut self, up: bool, cx: &mut Context<Self>) {
        let zoom = theme::zoom::step(theme::zoom::get(), up);
        crate::prefs::Prefs::update(cx, |prefs| prefs.zoom = zoom);
    }
}

impl SignInError {
    /// The error, for people.
    pub fn message(&self) -> String {
        match self {
            SignInError::NeedsOwner => "This core has no owner yet.".into(),
            SignInError::Refused => "The core refused.".into(),
            SignInError::Failed(error) => error.clone(),
        }
    }
}

/// Hides the window, keeping the Host running: on macOS the app hides (the Dock and the
/// menu bar icon bring it back), elsewhere the window minimizes.
pub fn hide(window: &mut Window, cx: &mut App) {
    if cfg!(target_os = "macos") {
        cx.hide();
    } else {
        window.minimize_window();
    }
}

impl Render for HostApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.set_rem_size(theme::zoom::rem_size());
        if self._focus_lost.is_none() {
            self._focus_lost = Some(focus::install(&self.root_focus, window, cx));
        }
        let content: AnyElement = match &self.phase {
            Phase::Core | Phase::SigningIn | Phase::Quitting => {
                let status = match self.phase {
                    Phase::Quitting => CoreStatus::Stopping,
                    _ => self.supervisor.read(cx).status.clone(),
                };
                let log_folder = self
                    .supervisor
                    .read(cx)
                    .log_file
                    .parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_default();
                let this = cx.entity().downgrade();
                onboarding::core_status(&status, log_folder, move |_, cx| {
                    this.update(cx, |this, cx| this.retry_core(cx)).ok();
                })
            }
            Phase::Failed(error) => {
                let this = cx.entity().downgrade();
                onboarding::sign_in_failed(error, move |_, cx| {
                    this.update(cx, |this, cx| this.retry_sign_in(cx)).ok();
                })
            }
            Phase::Owner(form) => form.clone().into_any_element(),
            Phase::FirstConfig { form, .. } => form.clone().into_any_element(),
            Phase::Password(form) => form.clone().into_any_element(),
            Phase::Shell(shell) => shell.clone().into_any_element(),
        };
        div()
            .id("host-app")
            .track_focus(&self.root_focus)
            .on_action(|_: &focus::FocusNext, window, cx| focus::step(true, window, cx))
            .on_action(|_: &focus::FocusPrev, window, cx| focus::step(false, window, cx))
            .on_action(cx.listener(|this, _: &crate::ZoomIn, _, cx| this.zoom_by(true, cx)))
            .on_action(cx.listener(|this, _: &crate::ZoomOut, _, cx| this.zoom_by(false, cx)))
            // the Settings dialog, once signed in (the app menu's Settings…)
            .on_action(cx.listener(|this, _: &crate::OpenSettings, _, cx| {
                if let Some(shell) = this.shell() {
                    shell.update(cx, |shell, cx| shell.open_settings(None, cx));
                }
            }))
            .on_action(cx.listener(|_, _: &crate::ResetZoom, _, cx| {
                crate::prefs::Prefs::update(cx, |prefs| prefs.zoom = 1.0);
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(colors::GRAY_875)
            .text_color(theme::ink::text())
            .font_family(fonts::body())
            .map(crate::i18n::locale_fonts)
            .child(content)
    }
}
