//! Settings › Updates: Lodestone Host's own version and updates, after the launcher's
//! Settings › General › Updates ([`crate::updater`]). The dashboard has no such page;
//! the Tauri Host asked in a system dialog (§8.7).
//!
//! Restarting to update stops the core, and so its servers. With servers running, the
//! page asks first.

use gpui::{
    AnyElement, Context, Entity, FocusHandle, IntoElement, ParentElement, Render, Styled,
    Subscription, Window, div,
};
use launcher_core::cores::admin::InstanceState;
use lodestone_ui::{
    components::{
        Button, ButtonVariant, Dialog, DialogSlot, Toggle,
        settings::{self, SettingRow},
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
    /// "Restart to update?", while servers run.
    confirm: DialogSlot<Self, FocusHandle>,
    _subscriptions: Vec<Subscription>,
}

/// How many of the core's servers are running (or starting or stopping): a restart
/// stops them.
pub fn running_servers(state: &HostState) -> usize {
    state
        .instances
        .iter()
        .flatten()
        .filter(|instance| {
            matches!(
                instance.state,
                InstanceState::Running | InstanceState::Starting | InstanceState::Stopping
            )
        })
        .count()
}

impl UpdatesPage {
    pub fn new(state: Entity<HostState>, cx: &mut Context<Self>) -> Self {
        let updater = Updater::global(cx);
        let subscriptions = updater
            .iter()
            .map(|updater| cx.observe(updater, |_, _, cx| cx.notify()))
            .collect();
        Self {
            state,
            updater,
            confirm: DialogSlot::new(|this: &mut Self| &mut this.confirm),
            _subscriptions: subscriptions,
        }
    }

    /// "Restart": asks first when servers run, which the restart stops.
    fn ask_restart(&mut self, cx: &mut Context<Self>) {
        if running_servers(self.state.read(cx)) == 0 {
            self.restart(cx);
        } else {
            let focus = cx.focus_handle();
            self.confirm.open(focus, cx);
        }
    }

    /// Quits into the downloaded build: the core stops (its servers first), the build
    /// is installed and started.
    fn restart(&mut self, cx: &mut Context<Self>) {
        self.confirm.close(cx);
        let Some(updater) = &self.updater else {
            return;
        };
        if updater.update(cx, |u, cx| u.restart(cx)) {
            // After this click is handled: `dispatch_action` goes through the active
            // window, which can't be updated while it handles the click.
            cx.defer(|cx| cx.dispatch_action(&crate::Quit));
        }
    }

    fn confirm_dialog(&self, focus: &FocusHandle, cx: &mut Context<Self>) -> AnyElement {
        let running = running_servers(self.state.read(cx));
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
                .child(trn!(CorePlural::ServersRunning, running))
                .child(t(CoreMsg::RestartUpdateBody)),
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
                .into_any_element()
        });
        let page = parts::page([settings::section(
            t(CoreMsg::UpdatesTitle),
            Some(t(CoreMsg::UpdatesLead).into()),
        )
        .child(settings::card(
            [
                SettingRow::new(t(CoreMsg::ThisVersion))
                    .hint(status)
                    .children_opt(action)
                    .below(
                        div()
                            .text_size(text::SMALL)
                            .text_color(ink::muted())
                            .child(version),
                    )
                    .into_any_element(),
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
                    )
                    .into_any_element(),
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
                    )
                    .into_any_element(),
            ]
            .into_iter()
            .chain(dev_row),
        ))
        .into_any_element()])
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
