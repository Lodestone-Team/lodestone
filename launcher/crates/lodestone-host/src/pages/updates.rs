//! Settings › Updates: Lodestone Host's own version and updates, after the launcher's
//! Settings › General › Updates ([`crate::updater`]). The dashboard has no such page;
//! the Tauri Host asked in a system dialog (§8.7).
//!
//! Restarting to update stops this computer's core, and so its servers. With servers
//! running on it, the page asks first ([`crate::local_core`]: whichever core the shell
//! shows).

use gpui::{
    AnyElement, AppContext as _, Context, Entity, FocusHandle, IntoElement, ParentElement, Render,
    Styled, Subscription, Task, Window, div, prelude::FluentBuilder as _,
};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, Dialog, DialogSlot, Toggle,
        settings::{SearchRow, SettingRow, SettingsSearch},
        window_layer,
    },
    theme::{colors, ink, leading, text},
};

use super::parts;
use crate::{
    i18n::{CoreMsg, CorePlural, t, tr, trn},
    prefs::Prefs,
    state::HostState,
    updater::{UpdateState, Updater},
};

pub struct UpdatesPage {
    pub state: Entity<HostState>,
    updater: Option<Entity<Updater>>,
    /// "Restart to update?", while servers run on this computer's core: with their
    /// count when it was asked for, `None` when it isn't known.
    confirm: DialogSlot<Self, (FocusHandle, Option<usize>)>,
    /// Counting this computer's core's servers, after Restart (replaced by the next).
    asking: Option<Task<()>>,
    /// Developer › "Connect service (relay)".
    connect_service: Entity<super::connect_service::ConnectServiceSetting>,
    /// The Settings' search field: the page shows the settings that match it.
    search: Entity<SettingsSearch>,
    _subscriptions: Vec<Subscription>,
}

impl UpdatesPage {
    pub fn new(
        state: Entity<HostState>,
        search: Entity<SettingsSearch>,
        cx: &mut Context<Self>,
    ) -> Self {
        let updater = Updater::global(cx);
        let mut subscriptions: Vec<Subscription> = updater
            .iter()
            .map(|updater| cx.observe(updater, |_, _, cx| cx.notify()))
            .collect();
        subscriptions.push(cx.observe(&search, |_, _, cx| cx.notify()));
        let connect_service = cx.new(super::connect_service::ConnectServiceSetting::new);
        Self {
            state,
            updater,
            confirm: DialogSlot::new(|this: &mut Self| &mut this.confirm),
            asking: None,
            connect_service,
            search,
            _subscriptions: subscriptions,
        }
    }

    /// Developer › "Connect service (relay)", for the development screenshots.
    pub fn connect_service(&self) -> &Entity<super::connect_service::ConnectServiceSetting> {
        &self.connect_service
    }

    /// "Restart": asks first when servers run on this computer's core, which the
    /// restart stops (or when that isn't known). The shell may show another core.
    fn ask_restart(&mut self, cx: &mut Context<Self>) {
        let running = crate::local_core::running_servers(cx);
        self.asking = Some(cx.spawn(async move |this, cx| {
            let running = running.await;
            this.update(cx, |this, cx| {
                if crate::local_core::asks(running) {
                    let focus = cx.focus_handle();
                    this.confirm.open((focus, running), cx);
                } else {
                    this.restart(cx);
                }
            })
            .ok();
        }));
    }

    /// Quits into the downloaded build: the core stops (its servers first), the build
    /// is installed and started.
    fn restart(&mut self, cx: &mut Context<Self>) {
        self.confirm.close(cx);
        let Some(updater) = &self.updater else {
            return;
        };
        if updater.update(cx, |u, cx| u.restart(cx)) {
            // after this click is handled
            crate::dispatch_later(crate::Quit, cx);
        }
    }

    fn confirm_dialog(
        &self,
        (focus, asked): &(FocusHandle, Option<usize>),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // live while the shell shows this computer's core
        let running = crate::local_core::running_now(cx).or(*asked);
        Dialog::new(
            "restart-update-dialog",
            t(CoreMsg::RestartUpdateTitle),
            focus.clone(),
        )
        .on_close({
            let close = cx.listener(|this, _: &(), _, cx| {
                this.confirm.close(cx);
            });
            move |window, cx| close(&(), window, cx)
        })
        .child(
            div()
                .text_size(text::MEDIUM)
                .line_height(lodestone_ui::theme::line_height(
                    text::MEDIUM,
                    leading::NORMAL,
                ))
                .text_color(ink::secondary())
                // without the count (this computer's core didn't answer), the title and
                // "Stop servers and restart" say it
                .when_some(running, |this, running| {
                    this.child(trn!(CorePlural::ServersRunning, running))
                        .child(t(CoreMsg::RestartUpdateBody))
                }),
        )
        .footer(
            Button::new("restart-update-confirm", t(CoreMsg::RestartUpdateConfirm))
                .variant(ButtonVariant::Primary)
                .on_click(cx.listener(|this, _, _, cx| this.restart(cx))),
        )
        .into_any_element()
    }

    fn status(&self, cx: &mut Context<Self>) -> (String, Option<AnyElement>) {
        let Some(updater) = &self.updater else {
            return (t(CoreMsg::UpdatesUnavailable).to_string(), None);
        };
        let this = updater.clone();
        let button = |id: &'static str, label: CoreMsg, primary: bool| {
            Button::new(id, t(label)).variant(if primary {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Secondary
            })
        };
        match updater.read(cx).state().clone() {
            UpdateState::Unavailable => (t(CoreMsg::UpdatesUnavailable).to_string(), None),
            UpdateState::Idle => (
                t(CoreMsg::UpdateNotChecked).to_string(),
                Some(
                    button("check-updates", CoreMsg::CheckForUpdates, false)
                        .on_click(move |_, _, cx| this.update(cx, |u, cx| u.check(cx)))
                        .into_any_element(),
                ),
            ),
            UpdateState::Checking => (t(CoreMsg::UpdateChecking).to_string(), None),
            UpdateState::UpToDate => (
                t(CoreMsg::UpToDate).to_string(),
                Some(
                    button("check-updates", CoreMsg::CheckForUpdates, false)
                        .on_click(move |_, _, cx| this.update(cx, |u, cx| u.check(cx)))
                        .into_any_element(),
                ),
            ),
            UpdateState::Available(found) => (
                tr!(CoreMsg::UpdateAvailable, build = found.build),
                Some(
                    button("download-update", CoreMsg::DownloadUpdate, true)
                        .on_click(move |_, _, cx| this.update(cx, |u, cx| u.download_available(cx)))
                        .into_any_element(),
                ),
            ),
            UpdateState::Downloading { build, done, total } => (
                tr!(
                    CoreMsg::UpdateDownloading,
                    build = build,
                    percent =
                        lodestone_ui::i18n::format::percent(UpdateState::percent(&done, total))
                ),
                None,
            ),
            UpdateState::Ready { build } => (
                tr!(CoreMsg::UpdateReady, build = build),
                Some(
                    button("restart-update", CoreMsg::RestartToUpdate, true)
                        .on_click(cx.listener(|this, _, _, cx| this.ask_restart(cx)))
                        .into_any_element(),
                ),
            ),
            UpdateState::Failed(error) => (
                tr!(CoreMsg::UpdateFailed, error = error),
                Some(
                    button("check-updates", CoreMsg::CheckForUpdates, false)
                        .on_click(move |_, _, cx| this.update(cx, |u, cx| u.check(cx)))
                        .into_any_element(),
                ),
            ),
        }
    }
}

impl Render for UpdatesPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = Prefs::get(cx).clone();
        let (build, commit) = self
            .updater
            .as_ref()
            .map(|u| (u.read(cx).build, u.read(cx).commit))
            .unwrap_or_default();
        let version = format!(
            "{} {} · {}",
            crate::APP_NAME,
            env!("CARGO_PKG_VERSION"),
            match (build, commit) {
                (Some(build), Some(commit)) => tr!(
                    CoreMsg::BuildNumberCommit,
                    build = build,
                    commit = launcher_core::update::short_commit(commit)
                ),
                (Some(build), None) => tr!(CoreMsg::BuildNumber, build = build),
                (None, _) => t(CoreMsg::DevelopmentBuild).to_string(),
            }
        );
        let available = self
            .updater
            .as_ref()
            .is_some_and(|u| !matches!(u.read(cx).state(), UpdateState::Unavailable));
        let (status, action) = self.status(cx);
        let changed = |updater: &Option<Entity<Updater>>| {
            let updater = updater.clone();
            move |cx: &mut gpui::App| {
                if let Some(updater) = &updater {
                    updater.update(cx, |u, cx| u.settings_changed(cx));
                }
            }
        };
        let auto_changed = changed(&self.updater);
        let beta_changed = changed(&self.updater);
        let dev_changed = changed(&self.updater);
        // Only developers see "Developer builds": in a developer build, or once on.
        let offer_dev =
            prefs.dev_builds || crate::updater::this_build().is_some_and(|build| build.dev);
        let dev_row = offer_dev.then(|| {
            SettingRow::new(t(CoreMsg::DeveloperBuilds))
                .hint(t(CoreMsg::DeveloperBuildsHint))
                .child(
                    Toggle::new("developer-builds", prefs.dev_builds)
                        .label(t(CoreMsg::DeveloperBuilds))
                        .disabled(!available)
                        .on_change(move |value, _, cx| {
                            let value = *value;
                            Prefs::update(cx, |p| p.dev_builds = value);
                            dev_changed(cx);
                        }),
                )
        });
        let query = self.search.read(cx).query();
        // Developer: the Connect service, for developers and testers only
        let developer = super::connect_service::offered(cx)
            .then(|| {
                query
                    .section(t(CoreMsg::DeveloperSection), None)
                    .row(SearchRow::new(
                        self.connect_service.clone(),
                        [t(CoreMsg::ConnectService), t(CoreMsg::ConnectServiceHint)],
                    ))
                    .finish()
            })
            .flatten()
            .map(IntoElement::into_any_element);
        let updates = query
            .section(
                t(CoreMsg::UpdatesTitle),
                Some(t(CoreMsg::UpdatesLead).into()),
            )
            .card(
                [
                    SettingRow::new(t(CoreMsg::ThisVersion))
                        .hint(status)
                        .children_opt(action)
                        .search_text(version.clone())
                        .below(
                            div()
                                .text_size(text::SMALL)
                                .text_color(ink::muted())
                                .child(version),
                        ),
                    SettingRow::new(t(CoreMsg::UpdateAutomatically))
                        .hint(t(CoreMsg::UpdateAutomaticallyHint))
                        .child(
                            Toggle::new("auto-update", prefs.auto_update)
                                .label(t(CoreMsg::UpdateAutomatically))
                                .disabled(!available)
                                .on_change(move |value, _, cx| {
                                    let value = *value;
                                    Prefs::update(cx, |p| p.auto_update = value);
                                    auto_changed(cx);
                                }),
                        ),
                    SettingRow::new(t(CoreMsg::BetaBuilds))
                        .hint(t(CoreMsg::BetaBuildsHint))
                        .child(
                            // Developer builds include betas.
                            Toggle::new("beta-builds", prefs.beta_builds || prefs.dev_builds)
                                .label(t(CoreMsg::BetaBuilds))
                                .disabled(!available || prefs.dev_builds)
                                .on_change(move |value, _, cx| {
                                    let value = *value;
                                    Prefs::update(cx, |p| p.beta_builds = value);
                                    beta_changed(cx);
                                }),
                        ),
                ]
                .into_iter()
                .chain(dev_row),
            )
            .finish()
            .map(IntoElement::into_any_element);
        let page = parts::page(
            // the sections are built: the status counts what they show
            query.status().into_iter().chain(updates).chain(developer),
        )
        .text_color(colors::GRAY_300);
        let dialogs = self
            .confirm
            .render(window, |focus| Some(self.confirm_dialog(focus, cx)));
        div()
            .relative()
            .size_full()
            .child(page)
            .children(window_layer(window, dialogs))
    }
}

/// `SettingRow::child` for an optional control.
trait ChildrenOpt {
    fn children_opt(self, child: Option<AnyElement>) -> Self;
}

impl ChildrenOpt for SettingRow {
    fn children_opt(self, child: Option<AnyElement>) -> Self {
        match child {
            Some(child) => self.child(child),
            None => self,
        }
    }
}

/// With GPUI's test platform, which the `screenshot` feature turns on.
#[cfg(all(test, feature = "screenshot"))]
mod tests {
    use gpui::TestAppContext;
    use lodestone_ui::components::settings::SettingsSearch;

    use super::*;
    use crate::local_core::{self, gpui_tests::state, instance};

    /// Settings › Updates while the shell shows another core (`shown`), with `local` as
    /// this computer's core; whether pressing Restart asks. Without an [`Updater`],
    /// restarting does nothing, so nothing quits.
    fn asks_on_restart(local: &str, shown: &str) -> bool {
        let mut cx = TestAppContext::single();
        cx.update(|cx| {
            lodestone_ui::components::init(cx);
            crate::prefs::Prefs::init_for_tests(cx);
        });
        let local = state(vec![instance("a", local)], &mut cx);
        let remote = state(vec![instance("b", shown)], &mut cx);
        cx.update(|cx| local_core::set(&local, cx));
        let page = cx.new(|cx| {
            let search = cx.new(|cx| SettingsSearch::new("settings-search", cx));
            UpdatesPage::new(remote, search, cx)
        });
        page.update(&mut cx, |page, cx| page.ask_restart(cx));
        cx.run_until_parked();
        page.read_with(&cx, |page, _| page.confirm.is_open())
    }

    /// LS-11: a server running on this computer's core asks, though the core shown
    /// runs none: the restart stops this computer's.
    #[test]
    fn restart_asks_about_this_computers_servers_while_another_core_is_shown() {
        assert!(asks_on_restart("Running", "Stopped"));
    }

    /// LS-11: the shown core's running servers don't ask: the restart doesn't touch them.
    #[test]
    fn restart_doesnt_ask_about_the_shown_cores_servers() {
        assert!(!asks_on_restart("Stopped", "Running"));
    }
}
