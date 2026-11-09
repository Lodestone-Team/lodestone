//! The Playit.gg tab (`PlayitggOverview.tsx`, `PlayitggSignupModal.tsx`): signing up
//! for playit.gg, its runner on this computer, and the account's tunnels. They are the
//! core's, not the instance's; the tab shows only while the core's "Enable playit.gg"
//! setting is on, and every route is the owner's.
//!
//! The account is checked and the tunnels fetched each time the tab opens. Without an
//! account the sign-up dialog opens by itself: it makes a link (the core waits for the
//! claim), the user makes the account in the browser, then presses Verify.

use gpui::{
    AnyElement, AppContext as _, ClipboardItem, Context, Entity, FocusHandle, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, Task, Window, div, prelude::*,
};
use launcher_core::cores::{
    CoreError,
    admin::{
        EventInner,
        events::PlayitggRunnerEventInner,
        playit::{PlayitSignupData, PlayitTunnelInfo},
    },
};
use lodestone_ui::{
    components::{
        Button, ButtonSize, ButtonVariant, Dialog, DialogSlot, Icon, IconButton, LoadingDots,
        Toast,
        a11y::Accessible as _,
        settings::{SettingRow, card, content_width, link, section, tag},
        window_layer,
    },
    theme::{Color, colors, ink, leading, line_height, px, text},
};

use crate::{
    i18n::{CommonMsg, InstanceMsg as M, t, tr},
    state::{HostEvent, HostState},
};

/// playit.gg's website, where tunnels are made.
const PLAYIT_URL: &str = "https://playit.gg/";
/// How the integration works.
const HELP_URL: &str = "https://github.com/Lodestone-Team/lodestone/wiki/Playit.gg-Integration";

/// The runner, as the dashboard shows it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runner {
    Stopped,
    Loading,
    Started,
}

impl Runner {
    fn label(self) -> &'static str {
        t(match self {
            Runner::Stopped => M::PlayitStopped,
            Runner::Loading => M::PlayitLoading,
            Runner::Started => M::PlayitStarted,
        })
    }

    fn color(self) -> Color {
        match self {
            Runner::Stopped => colors::GRAY_500,
            Runner::Loading => colors::YELLOW,
            Runner::Started => colors::GREEN,
        }
    }

    /// The runner after a `PlayitggRunnerEvent`.
    pub fn after(event: &PlayitggRunnerEventInner) -> Option<Runner> {
        match event {
            PlayitggRunnerEventInner::RunnerStarted => Some(Runner::Started),
            PlayitggRunnerEventInner::RunnerLoading => Some(Runner::Loading),
            PlayitggRunnerEventInner::RunnerStopped => Some(Runner::Stopped),
            PlayitggRunnerEventInner::Unknown => None,
        }
    }
}

/// The sign-up dialog.
struct Signup {
    focus: FocusHandle,
}

pub struct PlayitPage {
    pub state: Entity<HostState>,
    /// The instance (the tab is listed under it; playit.gg is the core's).
    pub uuid: String,
    /// Whether the core has a playit.gg account: `None` while checking.
    verified: Option<Result<bool, String>>,
    /// `None` until known.
    runner: Option<Runner>,
    /// `None` until first fetched.
    tunnels: Option<Vec<PlayitTunnelInfo>>,
    refreshing: bool,
    signup: DialogSlot<Self, Signup>,
    signup_data: Option<PlayitSignupData>,
    signup_busy: bool,
    runner_busy: bool,
    _tasks: Vec<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

impl PlayitPage {
    pub fn new(state: Entity<HostState>, uuid: String, cx: &mut Context<Self>) -> Self {
        let subscriptions = vec![cx.subscribe(&state, |this, _, event, cx| {
            if let HostEvent::Core(event) = event
                && let EventInner::PlayitggRunnerEvent(runner) = &event.event_inner
                && let Some(runner) = Runner::after(&runner.playitgg_runner_event_inner)
            {
                this.runner = Some(runner);
                cx.notify();
            }
        })];
        let mut this = Self {
            state,
            uuid,
            verified: None,
            runner: None,
            tunnels: None,
            refreshing: false,
            signup: DialogSlot::new(|this: &mut Self| &mut this.signup),
            signup_data: None,
            signup_busy: false,
            runner_busy: false,
            _tasks: Vec::new(),
            _subscriptions: subscriptions,
        };
        this.load(cx);
        this
    }

    fn push(&mut self, task: Task<()>) {
        self._tasks.retain(|task| !task.is_ready());
        self._tasks.push(task);
    }

    fn toast(&self, toast: Toast, cx: &mut Context<Self>) {
        self.state.update(cx, |state, cx| state.toast(toast, cx));
    }

    /// Checks the account, the runner and the tunnels.
    fn load(&mut self, cx: &mut Context<Self>) {
        let admin = self.state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let work = cx.background_spawn(async move {
                let verified = admin.playit_verify().await;
                let running = admin.playit_running().await.ok();
                // none when they can't be fetched (not signed up yet)
                let tunnels = admin.playit_tunnels().await.unwrap_or_default();
                (verified, running, tunnels)
            });
            let (verified, running, tunnels) = work.await;
            this.update(cx, |this, cx| {
                if matches!(verified, Err(CoreError::SignedOut)) {
                    this.state.update(cx, |state, cx| state.reload(cx));
                }
                this.verified = Some(verified.map_err(|err| err.to_string()));
                if let Some(running) = running {
                    this.runner = Some(if running {
                        Runner::Started
                    } else {
                        Runner::Stopped
                    });
                }
                this.tunnels = Some(tunnels);
                if this.verified == Some(Ok(false)) {
                    this.open_signup(cx);
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn refresh_tunnels(&mut self, cx: &mut Context<Self>) {
        if self.refreshing {
            return;
        }
        self.refreshing = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let tunnels = cx
                .background_spawn(async move { admin.playit_tunnels().await.unwrap_or_default() })
                .await;
            this.update(cx, |this, cx| {
                this.tunnels = Some(tunnels);
                this.refreshing = false;
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn open_signup(&mut self, cx: &mut Context<Self>) {
        if !self.signup.is_open() {
            let focus = cx.focus_handle();
            self.signup.open(Signup { focus }, cx);
        }
    }

    fn generate_link(&mut self, cx: &mut Context<Self>) {
        self.signup_busy = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let link = cx
                .background_spawn(async move { admin.playit_signup_link().await })
                .await;
            this.update(cx, |this, cx| {
                this.signup_busy = false;
                match link {
                    Ok(link) => this.signup_data = Some(link),
                    Err(err) => {
                        this.toast(Toast::error(tr!(M::PlayitSignupFailed, error = err)), cx)
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// Verify, after signing up: checks the account again and fetches its tunnels.
    fn verify(&mut self, cx: &mut Context<Self>) {
        self.signup.close(cx);
        let admin = self.state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let verified = cx
                .background_spawn(async move { admin.playit_verify().await })
                .await;
            this.update(cx, |this, cx| {
                match verified {
                    Ok(verified) => {
                        this.verified = Some(Ok(verified));
                        this.refresh_tunnels(cx);
                    }
                    Err(err) => {
                        this.toast(Toast::error(tr!(M::PlayitVerifyFailed, error = err)), cx)
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// The power button: starts a stopped runner, else stops it.
    fn toggle_runner(&mut self, cx: &mut Context<Self>) {
        if self.verified != Some(Ok(true)) {
            return self.open_signup(cx);
        }
        let start = self.runner.unwrap_or(Runner::Stopped) == Runner::Stopped;
        self.runner_busy = true;
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    if start {
                        admin.playit_start().await
                    } else {
                        admin.playit_stop().await
                    }
                })
                .await;
            this.update(cx, |this, cx| {
                this.runner_busy = false;
                match result {
                    Ok(()) => {
                        this.runner = Some(if start {
                            Runner::Loading
                        } else {
                            Runner::Stopped
                        })
                    }
                    Err(err) => {
                        let message = if start {
                            tr!(M::PlayitStartFailed, error = err)
                        } else {
                            tr!(M::PlayitStopFailed, error = err)
                        };
                        this.toast(Toast::error(message), cx);
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn runner_section(&self, verified: bool, cx: &mut Context<Self>) -> AnyElement {
        let runner = self.runner;
        let row = SettingRow::new(t(M::PlayitRunner))
            .hint(t(if verified {
                M::PlayitTrouble
            } else {
                M::PlayitSignupRequired
            }))
            .when_some(runner, |row, runner| {
                row.child(tag(runner.label(), runner.color()))
            })
            .when(!verified, |row| {
                row.child(
                    link("playit-sign-up", t(M::PlayitSignUp))
                        .on_press(cx.listener(|this, _, _, cx| this.open_signup(cx))),
                )
            })
            .child({
                let stopped = runner.unwrap_or(Runner::Stopped) == Runner::Stopped;
                Button::new(
                    "playit-power",
                    t(if stopped {
                        CommonMsg::Start
                    } else {
                        CommonMsg::Stop
                    }),
                )
                .variant(if stopped {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Secondary
                })
                .size(ButtonSize::Slim)
                .loading(self.runner_busy)
                .disabled(!verified || self.runner_busy || runner.is_none())
                .on_click(cx.listener(|this, _, _, cx| this.toggle_runner(cx)))
            });
        section(t(M::PlayitRunner), Some(t(M::PlayitRunnerLead).into()))
            .child(card([row.into_any_element()]))
            .into_any_element()
    }

    fn tunnels_section(&self, verified: bool, cx: &mut Context<Self>) -> AnyElement {
        let runner_started = self.runner == Some(Runner::Started);
        let rows: Vec<AnyElement> = match &self.tunnels {
            None => vec![loading_row()],
            Some(tunnels) if tunnels.is_empty() => vec![
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(8.))
                    .px(px(16.))
                    .py(px(14.))
                    .text_color(ink::muted())
                    .child(t(if verified {
                        M::PlayitNoTunnels
                    } else {
                        M::PlayitNotSignedUp
                    }))
                    .into_any_element(),
            ],
            Some(tunnels) => tunnels
                .iter()
                .enumerate()
                .map(|(index, tunnel)| {
                    let active = tunnel.active && runner_started;
                    let address = tunnel.server_address.clone();
                    SettingRow::new(tunnel.name.clone())
                        .key(SharedString::from(format!("tunnel-{index}")))
                        .selectable_label()
                        .leading(
                            div()
                                .id(SharedString::from(format!("tunnel-dot-{index}")))
                                .flex_none()
                                .size(px(8.))
                                .rounded_full()
                                .bg(if active {
                                    colors::GREEN
                                } else {
                                    colors::GRAY_600
                                })
                                .aria_label(t(if active {
                                    M::PlayitActive
                                } else {
                                    M::PlayitInactive
                                })),
                        )
                        .hint(tr!(
                            M::PlayitLocalServer,
                            address = format!("{}:{}", tunnel.local_ip, tunnel.local_port)
                        ))
                        .child(
                            div()
                                .font_family(lodestone_ui::theme::fonts::mono())
                                .text_color(ink::secondary())
                                .child(SharedString::from(address.clone())),
                        )
                        .child(
                            IconButton::new(
                                SharedString::from(format!("tunnel-copy-{index}")),
                                Icon::Copy,
                            )
                            .label(t(M::PlayitCopyAddress))
                            .on_click(cx.listener(
                                move |this, _, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        address.clone(),
                                    ));
                                    this.toast(
                                        Toast::success(tr!(M::PlayitCopied, address = address))
                                            .key("playit-copied"),
                                        cx,
                                    );
                                },
                            )),
                        )
                        .into_any_element()
                })
                .collect(),
        };
        section(t(M::PlayitTunnels), Some(t(M::PlayitTunnelsLead).into()))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(16.))
                    .child(
                        link("playit-website", t(M::PlayitWebsite)).on_press(|_, _, cx| {
                            cx.open_url(PLAYIT_URL);
                        }),
                    )
                    .child(div().flex_1())
                    .child(
                        Button::new("playit-refresh", t(CommonMsg::Refresh))
                            .size(ButtonSize::Slim)
                            .loading(self.refreshing)
                            .disabled(self.refreshing || self.tunnels.is_none())
                            .on_click(cx.listener(|this, _, _, cx| this.refresh_tunnels(cx))),
                    ),
            )
            .child(card(rows))
            .into_any_element()
    }

    fn signup_dialog(&self, signup: &Signup, cx: &mut Context<Self>) -> AnyElement {
        let paragraph = |text_: &'static str| {
            div()
                .text_size(text::MEDIUM)
                .line_height(line_height(text::MEDIUM, leading::NORMAL))
                .text_color(ink::secondary())
                .child(text_)
        };
        let dialog = Dialog::new(
            "playit-signup",
            t(M::PlayitSetupTitle),
            signup.focus.clone(),
        )
        .on_close({
            let this = cx.entity().downgrade();
            move |_, cx| {
                this.update(cx, |this, cx| {
                    this.signup.close(cx);
                })
                .ok();
            }
        });
        match &self.signup_data {
            None => dialog.child(paragraph(t(M::PlayitSetupIntro))).footer(
                Button::new("playit-generate", t(M::PlayitGenerateLink))
                    .variant(ButtonVariant::Primary)
                    .loading(self.signup_busy)
                    .disabled(self.signup_busy)
                    .on_click(cx.listener(|this, _, _, cx| this.generate_link(cx))),
            ),
            Some(data) => {
                let url = data.url.clone();
                dialog
                    .child(paragraph(t(M::PlayitFollowLink)))
                    .child(div().flex().flex_col().gap(px(6.)).child(
                        link("playit-open-link", t(M::PlayitOpenLink)).on_press(move |_, _, cx| {
                            cx.open_url(&url);
                        }),
                    ))
                    .footer(
                        Button::new("playit-verify", t(M::PlayitVerify))
                            .variant(ButtonVariant::Primary)
                            .on_click(cx.listener(|this, _, _, cx| this.verify(cx))),
                    )
            }
        }
        .into_any_element()
    }
}

fn loading_row() -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .h(px(64.))
        .child(LoadingDots)
        .into_any_element()
}

impl Render for PlayitPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body: Vec<AnyElement> = match self.verified.clone() {
            None => vec![card([loading_row()]).into_any_element()],
            Some(Err(error)) => vec![
                div()
                    .text_color(colors::RED_200)
                    .child(tr!(M::PlayitLoadFailed, error = error))
                    .into_any_element(),
            ],
            Some(Ok(verified)) => vec![
                self.runner_section(verified, cx),
                self.tunnels_section(verified, cx),
                div()
                    .flex()
                    .child(link("playit-help", t(M::PlayitHelp)).on_press(|_, _, cx| {
                        cx.open_url(HELP_URL);
                    }))
                    .into_any_element(),
            ],
        };
        let dialogs = self
            .signup
            .render(window, |signup| Some(self.signup_dialog(signup, cx)));
        div()
            .id("playit")
            .size_full()
            .overflow_y_scroll()
            .text_size(text::MEDIUM)
            .line_height(line_height(text::MEDIUM, leading::NORMAL))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(32.))
                    .pb(px(64.))
                    .w_full()
                    .max_w(content_width())
                    .children(body),
            )
            .children(window_layer(window, dialogs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runner_follows_its_events() {
        assert_eq!(
            Runner::after(&PlayitggRunnerEventInner::RunnerLoading),
            Some(Runner::Loading)
        );
        assert_eq!(
            Runner::after(&PlayitggRunnerEventInner::RunnerStarted),
            Some(Runner::Started)
        );
        assert_eq!(
            Runner::after(&PlayitggRunnerEventInner::RunnerStopped),
            Some(Runner::Stopped)
        );
        assert_eq!(Runner::after(&PlayitggRunnerEventInner::Unknown), None);
    }
}
