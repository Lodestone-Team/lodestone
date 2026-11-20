//! The window's root: starts the core, signs in to it, and then shows the shell.
//!
//! 1. The [`Supervisor`] starts the core and says when it answers.
//! 2. The Host claims owner access to it ([`session::claim_owner`]). A core without an
//!    owner gets one without a password, so the first run asks nothing (sharing the
//!    core asks for a password later); a core that refuses claims asks for a password
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
use launcher_core::{Secret, game::download::Sleep};
use lodestone_ui::{
    components::focus,
    theme::{self, colors, fonts},
};

use crate::{
    core_process::CoreConfig,
    onboarding::{self, PasswordForm, PasswordSubmitted},
    session::{self, Session, SignInError},
    shell::HostShell,
    state::{HostEvent, HostState},
    supervisor::{CoreStatus, Supervisor, SupervisorEvent},
};

enum Phase {
    /// Waiting for the core (or showing why it couldn't start).
    Core,
    SigningIn,
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
    /// The last run crashed: said once, on the first shell, once the window has been
    /// seen ([`launcher_core::logging::Started::previous_crash`]); until then the
    /// failure screens say it too, for a start that never gets to a shell.
    crash_notice: Option<launcher_core::logging::Crash>,
    /// The window has been active: a Host started hidden at sign-in hasn't been seen
    /// until the person opens it, and a toast would run out before then.
    window_seen: bool,
    /// The shell's state, whose toasts say the crash.
    crash_target: Option<gpui::WeakEntity<HostState>>,
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
            SupervisorEvent::Ready(config) => this.sign_in(config.clone(), cx),
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
                    crate::notifications::native::notify(crate::APP_NAME, reason);
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
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() && !this.window_seen {
                this.window_seen = true;
                this.say_crash(cx);
            }
        });
        Self {
            supervisor,
            phase: Phase::Core,
            root_focus: cx.focus_handle(),
            _focus_lost: None,
            _subscriptions: vec![subscription, appearance, activation],
            task: None,
            notice: None,
            crash_notice: crash_notice(),
            // the development screenshots' offscreen window is never active
            window_seen: window.is_window_active() || made_up_crash(),
            crash_target: None,
        }
    }

    /// Says on the shell that the last run crashed, once: when there is a shell and
    /// the window has been seen.
    fn say_crash(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.crash_target.as_ref().and_then(|s| s.upgrade()) else {
            return;
        };
        if let Some(toast) = crash_toast(&mut self.crash_notice, self.window_seen) {
            state.update(cx, |state, cx| state.toast(toast, cx));
        }
    }

    /// The running core's settings.
    fn config(&self, cx: &App) -> Option<CoreConfig> {
        self.supervisor.read(cx).config().cloned()
    }

    /// Claims owner access to the core (making its owner, on the first run).
    fn sign_in(&mut self, config: CoreConfig, cx: &mut Context<Self>) {
        self.phase = Phase::SigningIn;
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
            session::claim_owner(http, config.local_url(), &config.data_dir, None, sleep).await
        });
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(session) => this.show_shell(session, cx),
                    Err(SignInError::Refused) => {
                        let form = cx.new(PasswordForm::new);
                        let subscription = cx.subscribe(&form, Self::sign_in_with_password);
                        this._subscriptions.push(subscription);
                        this.phase = Phase::Password(form);
                    }
                    Err(SignInError::Failed(error)) => this.phase = Phase::Failed(error),
                }
                cx.notify();
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
        // what Restart to update and the Connect service count servers on: this
        // computer's core, which they restart, even while another is shown
        if on_local {
            crate::local_core::set(&state, cx);
        }
        // sharing with friends: this computer's core, as its owner (another core's
        // friends are its owner's, set up from their own Host)
        if on_local && state.read(cx).me.is_owner {
            state.update(cx, |state, cx| {
                let weak = cx.entity().downgrade();
                state.friends = Some(cx.new(|cx| crate::friends::FriendSharing::new(weak, cx)));
            });
            let weak = state.downgrade();
            let discord = crate::discord::create(weak, cx);
            state.update(cx, |state, _| state.discord = Some(discord));
        }
        self.crash_target = Some(state.downgrade());
        self.say_crash(cx);
        // after being signed out of another core
        if let Some(notice) = self.notice.take() {
            state.update(cx, |state, cx| {
                state.toast(
                    lodestone_ui::components::Toast::warning(crate::i18n::t(
                        crate::i18n::CoreMsg::RemoteSignedOut,
                    ))
                    .object(
                        lodestone_ui::components::ObjectChip::new(notice)
                            .icon(lodestone_ui::components::Icon::HardDrive),
                    ),
                    cx,
                )
            });
        }
        let subscription = cx.subscribe(&state, move |this, _, event, cx| match event {
            HostEvent::SignedOut => {
                if let Some(name) = &remote {
                    // the core's name; the toast says what happened
                    this.notice = Some(name.clone());
                }
                if let Some(config) = this.config(cx) {
                    this.sign_in(config, cx);
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
                        this.sign_in(config, cx);
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

    /// Restarts this computer's core with the settings as they are now (the Connect
    /// service changed): its servers stop first, then the Host signs in again.
    pub fn restart_core(&mut self, cx: &mut Context<Self>) {
        if matches!(self.phase, Phase::Quitting) {
            return;
        }
        self.phase = Phase::Core;
        cx.notify();
        self.supervisor
            .update(cx, |supervisor, cx| supervisor.restart(cx));
    }

    fn retry_core(&mut self, cx: &mut Context<Self>) {
        self.phase = Phase::Core;
        self.supervisor
            .update(cx, |supervisor, cx| supervisor.start(cx));
    }

    fn retry_sign_in(&mut self, cx: &mut Context<Self>) {
        match self.config(cx) {
            Some(config) => self.sign_in(config, cx),
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
        // the screens before the shell have no top bar of their own
        let bare = !matches!(self.phase, Phase::Shell(_));
        let content: AnyElement = match &self.phase {
            Phase::Core | Phase::SigningIn | Phase::Quitting => {
                let status = match self.phase {
                    Phase::Quitting => CoreStatus::Stopping,
                    _ => self.supervisor.read(cx).status.clone(),
                };
                let this = cx.entity().downgrade();
                onboarding::core_status(&status, self.crash_notice.is_some(), move |_, cx| {
                    this.update(cx, |this, cx| this.retry_core(cx)).ok();
                })
            }
            Phase::Failed(error) => {
                let this = cx.entity().downgrade();
                onboarding::sign_in_failed(error, self.crash_notice.is_some(), move |_, cx| {
                    this.update(cx, |this, cx| this.retry_sign_in(cx)).ok();
                })
            }
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
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(colors::GRAY_875)
            .text_color(theme::ink::text())
            .font_family(fonts::body())
            .map(crate::i18n::locale_fonts)
            // Windows: the title bar's place, on screens without the top bar
            .when(bare, |this| {
                this.children(crate::shell::top_bar::frame_bar())
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .flex()
                    .flex_col()
                    .child(content),
            )
            // Windows: the caption buttons, last, above everything
            .children(crate::shell::top_bar::window_controls(window))
    }
}

/// The toast that says the last run crashed, with "Open logs", once the window has
/// been `seen`; `None` before (the notice is kept) and for every shell after it (the
/// notice is taken).
fn crash_toast(
    notice: &mut Option<launcher_core::logging::Crash>,
    seen: bool,
) -> Option<lodestone_ui::components::Toast> {
    use crate::i18n::{ShellMsg, t};
    if !seen {
        return None;
    }
    notice.take()?;
    Some(
        lodestone_ui::components::Toast::info(t(ShellMsg::ClosedUnexpectedly))
            .action(t(ShellMsg::OpenLogs), |_, cx| crate::logs::open_folder(cx))
            .duration(std::time::Duration::from_secs(30)),
    )
}

/// The crash the last run ended with, if it did. `--crash-notice` (the `screenshot`
/// feature) makes one up, for the development screenshots.
fn crash_notice() -> Option<launcher_core::logging::Crash> {
    if made_up_crash() {
        return Some(launcher_core::logging::Crash {
            at: "2026-11-16T08:30:12Z".into(),
            summary: "a made-up crash".into(),
            file: std::env::temp_dir().join("last-crash.txt"),
        });
    }
    launcher_core::logging::started().and_then(|started| started.previous_crash)
}

/// `--crash-notice`, with the `screenshot` feature.
fn made_up_crash() -> bool {
    cfg!(feature = "screenshot") && std::env::args().any(|arg| arg == "--crash-notice")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The first shell after a crash says so; the next one (another core, signing in
    /// again) doesn't, and neither does any shell after a clean exit.
    #[test]
    fn the_crash_toast_is_shown_once() {
        let mut notice = Some(launcher_core::logging::Crash {
            at: "2026-11-16T08:30:12Z".into(),
            summary: "a made-up crash".into(),
            file: std::env::temp_dir().join("last-crash.txt"),
        });
        // started hidden at sign-in: nothing until the window is seen
        assert!(crash_toast(&mut notice, false).is_none());
        assert!(notice.is_some(), "kept for later");
        let toast = crash_toast(&mut notice, true).expect("a toast after a crash");
        let text: &str = toast.text.as_ref();
        assert_eq!(
            text,
            crate::i18n::t(crate::i18n::ShellMsg::ClosedUnexpectedly)
        );
        assert_eq!(toast.level, lodestone_ui::components::ToastLevel::Info);
        assert!(notice.is_none(), "taken");
        assert!(crash_toast(&mut notice, true).is_none(), "said once");
        assert!(
            crash_toast(&mut None, true).is_none(),
            "nothing after a clean exit"
        );
    }
}
