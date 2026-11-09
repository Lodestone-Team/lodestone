//! Settings › Updates: Lodestone Host's own version and updates, after the launcher's
//! Settings › General › Updates ([`crate::updater`]). The dashboard has no such page;
//! the Tauri Host asked in a system dialog (§8.7).

use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div,
};
use lodestone_ui::{
    components::{
        Button, ButtonVariant, Toggle,
        settings::{self, SettingRow},
    },
    theme::{colors, ink, text},
};

use super::parts;
use crate::{
    i18n::{CoreMsg, t, tr},
    prefs::Prefs,
    state::HostState,
    updater::{UpdateState, Updater},
};

pub struct UpdatesPage {
    pub state: Entity<HostState>,
    updater: Option<Entity<Updater>>,
    _subscriptions: Vec<Subscription>,
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
            _subscriptions: subscriptions,
        }
    }

    fn status(&self, cx: &Context<Self>) -> (String, Option<AnyElement>) {
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
                        .on_click(move |_, _, cx| {
                            if this.update(cx, |u, cx| u.restart(cx)) {
                                cx.dispatch_action(&crate::Quit);
                            }
                        })
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
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = Prefs::get(cx).clone();
        let build = self.updater.as_ref().and_then(|u| u.read(cx).build);
        let version = format!(
            "{} {} · {}",
            crate::APP_NAME,
            env!("CARGO_PKG_VERSION"),
            match build {
                Some(build) => tr!(CoreMsg::BuildNumber, build = build),
                None => t(CoreMsg::DevelopmentBuild).to_string(),
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
        parts::page([settings::section(
            t(CoreMsg::UpdatesTitle),
            Some(t(CoreMsg::UpdatesLead).into()),
        )
        .child(settings::card([
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
                    Toggle::new("beta-builds", prefs.beta_builds)
                        .label(t(CoreMsg::BetaBuilds))
                        .disabled(!available)
                        .on_change(move |value, _, cx| {
                            let value = *value;
                            Prefs::update(cx, |p| p.beta_builds = value);
                            beta_changed(cx);
                        }),
                )
                .into_any_element(),
        ]))
        .into_any_element()])
        .text_color(colors::GRAY_300)
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
