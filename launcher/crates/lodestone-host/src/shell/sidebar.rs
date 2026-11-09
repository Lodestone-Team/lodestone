//! The sidebar, after the dashboard's `LeftNav`: the selected instance with its power
//! button and tabs, every instance with its live state, "New server", and the footer
//! with the connection, the signed-in user and the settings.

use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, prelude::*};
use launcher_core::cores::admin::{
    FullCoreInfo, InstanceInfo, InstanceState, PublicUser,
    instances::Power,
    models::{CorePermission, GlobalSettingsData, InstancePermission},
};
use lodestone_ui::{
    components::{Button, ButtonSize, ButtonVariant, Icon, ListRow, text_tooltip},
    theme::{Color, colors, ink, px, radius, text, weights},
};

use super::HostShell;
use crate::{
    i18n::{CommonMsg, ShellMsg, t, tr},
    routes::{InstanceTab, ModSupport, Route, SettingsPage, TabContext, mod_support},
    state::{Connection, Creating, HostState, power_actions},
};

/// An instance's state, as the dashboard's status dots colour it.
pub fn state_color(state: InstanceState) -> Color {
    match state {
        InstanceState::Running => colors::GREEN,
        InstanceState::Starting | InstanceState::Stopping => colors::YELLOW,
        InstanceState::Error => colors::RED,
        InstanceState::Stopped | InstanceState::Unknown => colors::GRAY_500,
    }
}

pub fn state_name(state: InstanceState) -> &'static str {
    t(match state {
        InstanceState::Starting => ShellMsg::StateStarting,
        InstanceState::Running => ShellMsg::StateRunning,
        InstanceState::Stopping => ShellMsg::StateStopping,
        InstanceState::Stopped => ShellMsg::StateStopped,
        InstanceState::Error => ShellMsg::StateError,
        InstanceState::Unknown => ShellMsg::StateUnknown,
    })
}

pub fn power_name(power: Power) -> &'static str {
    match power {
        Power::Start => t(CommonMsg::Start),
        Power::Stop => t(CommonMsg::Stop),
        Power::Restart => t(ShellMsg::Restart),
        Power::Kill => t(ShellMsg::Kill),
    }
}

/// A tab's name for `instance`: "Mods" or "Plugins" by what it runs.
pub fn tab_name(tab: InstanceTab, instance: &InstanceInfo) -> &'static str {
    t(match tab {
        InstanceTab::Overview => ShellMsg::TabOverview,
        InstanceTab::Settings => ShellMsg::TabSettings,
        InstanceTab::Console => ShellMsg::TabConsole,
        InstanceTab::Files => ShellMsg::TabFiles,
        InstanceTab::Mods => match mod_support(instance) {
            Some(ModSupport::Plugins) => ShellMsg::TabPlugins,
            _ => ShellMsg::TabMods,
        },
        InstanceTab::Macros => ShellMsg::TabMacros,
        InstanceTab::EventLogs => ShellMsg::TabEventLogs,
        InstanceTab::Playit => ShellMsg::TabPlayit,
    })
}

/// A tab's icon, as the dashboard's tab list (`pages/InstanceTabs/tabList.tsx`) shows
/// it beside the name.
pub fn tab_icon(tab: InstanceTab) -> Icon {
    match tab {
        InstanceTab::Overview => Icon::ChartLine,
        InstanceTab::Settings => Icon::Gear,
        InstanceTab::Console => Icon::Server,
        InstanceTab::Files => Icon::Folder,
        InstanceTab::Mods => Icon::PuzzlePiece,
        InstanceTab::Macros => Icon::CodeCompare,
        InstanceTab::EventLogs => Icon::Inbox,
        // the dashboard's is a car in a tunnel
        InstanceTab::Playit => Icon::Link,
    }
}

fn dot(color: Color) -> impl IntoElement {
    div().flex_none().size(px(8.)).rounded_full().bg(color)
}

fn section(label: &'static str) -> impl IntoElement {
    div()
        .px(px(8.))
        .pt(px(12.))
        .pb(px(4.))
        .text_size(text::SMALL)
        .font_weight(weights::BOLD.face())
        .text_color(ink::muted())
        .child(label.to_uppercase())
}

/// What the sidebar shows of the state, copied so the sidebar can make listeners.
struct Snapshot {
    me: PublicUser,
    info: FullCoreInfo,
    settings: Option<GlobalSettingsData>,
    instances: Option<Vec<InstanceInfo>>,
    creating: Vec<Creating>,
    connection: Connection,
    /// The core's address, as the footer's tooltip shows it.
    address: String,
}

impl Snapshot {
    fn of(state: &HostState) -> Self {
        Self {
            me: state.me.clone(),
            info: state.info.clone(),
            settings: state.settings.clone(),
            instances: state.instances.clone(),
            creating: state.creating.clone(),
            connection: state.connection.clone(),
            address: launcher_core::cores::client::display_address(state.admin.core().base()),
        }
    }

    fn instance(&self, uuid: &str) -> Option<&InstanceInfo> {
        self.instances.as_ref()?.iter().find(|i| i.uuid == uuid)
    }
}

pub fn render(shell: &HostShell, cx: &mut Context<HostShell>) -> impl IntoElement {
    let state = Snapshot::of(shell.state.read(cx));
    let state = &state;
    let selected_uuid = match &shell.route {
        Route::Instance { uuid, .. } => Some(uuid.clone()),
        _ => None,
    };
    let selected = selected_uuid
        .as_deref()
        .and_then(|uuid| state.instance(uuid))
        .cloned();
    let selected_tab = match &shell.route {
        Route::Instance { tab, .. } => Some(*tab),
        _ => None,
    };

    let selected_block: AnyElement = match &selected {
        Some(instance) => selected_card(instance, selected_tab, state, cx).into_any_element(),
        None => div()
            .mx(px(8.))
            .p(px(16.))
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .rounded(radius::LG)
            .border_1()
            .border_dashed()
            .border_color(ink::border())
            .text_size(text::MEDIUM)
            .text_color(ink::muted())
            .child(Icon::Expand.render(px(20.), ink::faint()))
            .child(t(ShellMsg::NoInstanceSelected))
            .into_any_element(),
    };

    let list: Vec<AnyElement> = match &state.instances {
        None => (0..3)
            .map(|i| {
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(8.))
                    .py(px(8.))
                    .child(
                        div()
                            .size(px(16.))
                            .rounded(radius::DEFAULT)
                            .bg(colors::GRAY_700),
                    )
                    .child(
                        div()
                            .h(px(10.))
                            .w(gpui::relative([0.6, 0.45, 0.7][i]))
                            .rounded(radius::DEFAULT)
                            .bg(colors::GRAY_700),
                    )
                    .into_any_element()
            })
            .collect(),
        Some(list) if list.is_empty() && state.creating.is_empty() => vec![
            div()
                .px(px(8.))
                .py(px(4.))
                .text_size(text::MEDIUM)
                .text_color(ink::muted())
                .italic()
                .child(t(ShellMsg::NoInstances))
                .into_any_element(),
        ],
        Some(list) => list
            .iter()
            .map(|instance| {
                let uuid = instance.uuid.clone();
                div()
                    .id(gpui::SharedString::from(format!("instance-row-{uuid}")))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .px(px(8.))
                    .py(px(6.))
                    .rounded(radius::MD)
                    .cursor_pointer()
                    .text_size(text::MEDIUM)
                    .text_color(ink::text())
                    .hover(|style| style.bg(colors::GRAY_700))
                    .when(selected_uuid.as_deref() == Some(uuid.as_str()), |this| {
                        this.bg(colors::GRAY_750)
                    })
                    .child(crate::instance_icons::game_icon(
                        format!("instance-icon-{uuid}"),
                        &instance.game_type,
                        instance.icon_url.as_deref(),
                        px(16.),
                        cx,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .child(instance.name.clone()),
                    )
                    .child(
                        div()
                            .id(gpui::SharedString::from(format!("instance-state-{uuid}")))
                            .p(px(2.))
                            .child(dot(state_color(instance.state)))
                            .tooltip(text_tooltip(state_name(instance.state))),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| this.select(&uuid, cx)))
                    .into_any_element()
            })
            .chain(state.creating.iter().map(|creating| {
                let percent = creating
                    .total
                    .filter(|total| *total > 0.0)
                    .map(|total| (creating.progress / total * 100.0).clamp(0.0, 99.0) as u32);
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .px(px(8.))
                    .py(px(6.))
                    .text_size(text::MEDIUM)
                    .text_color(ink::muted())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(lodestone_ui::components::LoadingDots)
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .child(tr!(ShellMsg::SettingUp, name = creating.name)),
                            )
                            .when_some(percent, |this, percent| {
                                this.child(lodestone_ui::i18n::format::percent(percent.into()))
                            }),
                    )
                    .child(
                        div()
                            .h(px(3.))
                            .w_full()
                            .rounded_full()
                            .bg(colors::GRAY_700)
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(lodestone_ui::theme::accent::base())
                                    .w(gpui::relative(percent.unwrap_or(0) as f32 / 100.0)),
                            ),
                    )
                    .into_any_element()
            }))
            .collect(),
    };

    let may_create = state.me.may_core(CorePermission::CreateInstance);
    let core_name = state
        .settings
        .as_ref()
        .map(|s| s.core_name.clone())
        .unwrap_or_else(|| state.info.core_name.clone());
    let username = state.me.username.clone();
    let settings_selected = matches!(shell.route, Route::Settings(_));

    div()
        .id("sidebar")
        .flex_none()
        .w(px(shell.sidebar_width))
        .h_full()
        .flex()
        .flex_col()
        .bg(colors::GRAY_850)
        .border_r_1()
        .border_color(ink::border())
        .child(
            div()
                .id("sidebar-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .px(px(8.))
                .pb(px(12.))
                .child(section(t(ShellMsg::SelectedInstance)))
                .child(selected_block)
                .child(section(t(ShellMsg::AllInstances)))
                .children(list)
                .child(
                    div().pt(px(8.)).child(
                        Button::new(
                            "new-server",
                            if may_create {
                                t(ShellMsg::NewServer)
                            } else {
                                t(ShellMsg::NoPermission)
                            },
                        )
                        .variant(ButtonVariant::Ghost)
                        .icon(Icon::SquarePlus)
                        .full_width()
                        .disabled(!may_create)
                        .on_click(cx.listener(|this, _, _, cx| this.open_create(cx))),
                    ),
                ),
        )
        .child(
            div()
                .flex_none()
                .flex()
                .flex_col()
                .gap(px(2.))
                .p(px(8.))
                .border_t_1()
                .border_color(ink::border())
                .child(connection_row(state, core_name))
                .child(
                    div()
                        .px(px(8.))
                        .text_size(text::SMALL)
                        .text_color(ink::muted())
                        .truncate()
                        .child(tr!(ShellMsg::SignedInAs, name = username)),
                )
                .child(
                    ListRow::new("settings", t(ShellMsg::Settings))
                        .icon(Icon::Gear)
                        .selected(settings_selected)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.navigate(Route::Settings(SettingsPage::General), cx)
                        })),
                )
                // other cores (package D)
                .child(
                    ListRow::new("change-core", t(crate::i18n::CoreMsg::ChangeCore))
                        .icon(Icon::HardDrive)
                        .on_click(cx.listener(|this, _, _, cx| this.open_cores(cx))),
                ),
        )
}

/// The footer's connection to the core (`CoreConnectionStatus.tsx`).
fn connection_row(state: &Snapshot, core_name: String) -> AnyElement {
    let connection = &state.connection;
    let (color, label) = match connection {
        Connection::Live => (colors::GREEN, t(ShellMsg::Connected)),
        Connection::Connecting => (colors::GRAY_500, t(ShellMsg::Connecting)),
        Connection::Degraded(_) => (colors::YELLOW, t(ShellMsg::Degraded)),
        Connection::Reconnecting(_) => (colors::RED, t(ShellMsg::Reconnecting)),
        Connection::SignedOut => (colors::RED, t(ShellMsg::SignedOut)),
    };
    let info = &state.info;
    let mut lines = vec![match connection {
        Connection::Reconnecting(reason) | Connection::Degraded(reason) => {
            format!("{label}: {reason}")
        }
        _ => label.to_string(),
    }];
    lines.push(tr!(ShellMsg::CoreAddress, address = state.address.clone()));
    lines.push(tr!(ShellMsg::CoreVersion, version = info.version.clone()));
    if !info.host_name.is_empty() {
        lines.push(tr!(ShellMsg::CoreHost, host = info.host_name.clone()));
    }
    lines.push(tr!(
        ShellMsg::CoreSystem,
        system = system_name(&info.os, &info.arch)
    ));
    let now = chrono::Utc::now().timestamp();
    if info.up_since > 0 && now > info.up_since {
        let uptime = lodestone_ui::i18n::format::play_time((now - info.up_since) as u64);
        lines.push(tr!(ShellMsg::CoreUptime, uptime = uptime));
    }
    let detail = lines.join("\n");
    div()
        .id("connection")
        .flex()
        .items_center()
        .gap(px(8.))
        .px(px(8.))
        .py(px(4.))
        .text_size(text::MEDIUM)
        .text_color(ink::text())
        .child(dot(color))
        .child(div().flex_1().min_w_0().truncate().child(core_name))
        .tooltip(text_tooltip(detail))
        .into_any_element()
}

/// The core's system as the dashboard names it: "macOS arm64", "Windows x86_64".
pub fn system_name(os: &str, arch: &str) -> String {
    let os = match os {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    };
    let arch = match arch {
        "aarch64" => "arm64",
        other => other,
    };
    format!("{os} {arch}")
}

/// The selected instance: its name, game, state, players, power button and tabs
/// (`SelectedInstanceInfo.tsx`, `InstanceCard.tsx`).
fn selected_card(
    instance: &InstanceInfo,
    selected_tab: Option<InstanceTab>,
    state: &Snapshot,
    cx: &mut Context<HostShell>,
) -> impl IntoElement {
    let me = &state.me;
    let uuid = instance.uuid.clone();
    let atom_blocked =
        instance.is_atom() && (!me.is_owner || !state.info.capabilities.generic_instances);
    let powers = power_actions(instance.state)
        .iter()
        .copied()
        .filter(|power| {
            let permission = match power {
                Power::Start => InstancePermission::Start,
                Power::Restart => InstancePermission::Start,
                Power::Stop | Power::Kill => InstancePermission::Stop,
            };
            me.may(permission, &uuid) && !atom_blocked
        })
        .map(|power| {
            let uuid = uuid.clone();
            Button::new(
                gpui::SharedString::from(format!("power-{power:?}")),
                power_name(power),
            )
            .size(ButtonSize::Slim)
            .variant(match power {
                Power::Start => ButtonVariant::Primary,
                Power::Kill => ButtonVariant::Danger,
                _ => ButtonVariant::Secondary,
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                let uuid = uuid.clone();
                this.state
                    .update(cx, |state, cx| state.power(&uuid, power, cx));
            }))
        })
        .collect::<Vec<_>>();
    // "Share" next to Start: opens the Overview, which starts sharing
    let share = (matches!(
        instance.game_type,
        launcher_core::cores::admin::models::Game::MinecraftJava { .. }
    ) && me.may(InstancePermission::Settings, &uuid)
        && !atom_blocked)
        .then(|| {
            let uuid = uuid.clone();
            Button::new("sidebar-share", t(crate::i18n::InstancesMsg::ShareShort))
                .size(ButtonSize::Slim)
                .variant(ButtonVariant::Secondary)
                .icon(Icon::Link)
                .on_click(cx.listener(move |this, _, _, cx| {
                    let uuid = uuid.clone();
                    // an Overview already open hears it through the state
                    this.state.update(cx, |state, cx| {
                        state.share_requested = Some(uuid.clone());
                        cx.notify();
                    });
                    this.navigate(
                        Route::Instance {
                            uuid,
                            tab: InstanceTab::Overview,
                        },
                        cx,
                    );
                }))
        });
    let address = instance_address(state.settings.as_ref(), instance.port);
    let players = match (instance.player_count, instance.max_player_count) {
        (Some(online), Some(max)) => Some(tr!(ShellMsg::PlayersOnline, online = online, max = max)),
        _ => None,
    };
    let tab_cx = TabContext {
        me,
        info: &state.info,
        settings: state.settings.as_ref(),
        instance,
    };
    let tabs: Vec<AnyElement> = InstanceTab::ALL
        .into_iter()
        .filter(|tab| tab.visible(&tab_cx))
        .map(|tab| {
            let uuid = uuid.clone();
            ListRow::new(
                gpui::SharedString::from(format!("tab-{tab:?}")),
                tab_name(tab, instance),
            )
            .icon(tab_icon(tab))
            .selected(selected_tab == Some(tab))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.navigate(
                    Route::Instance {
                        uuid: uuid.clone(),
                        tab,
                    },
                    cx,
                )
            }))
            .into_any_element()
        })
        .collect();

    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(6.))
                .p(px(10.))
                .rounded(radius::LG)
                .bg(colors::GRAY_800)
                .border_1()
                .border_color(ink::border())
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(crate::instance_icons::game_icon(
                            "selected-instance-icon",
                            &instance.game_type,
                            instance.icon_url.as_deref(),
                            px(20.),
                            cx,
                        ))
                        .child(
                            div()
                                .min_w_0()
                                .text_size(text::H3)
                                .font_weight(weights::BOLD.face())
                                .text_color(ink::text())
                                .truncate()
                                .child(instance.name.clone()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(text::SMALL)
                        .text_color(ink::muted())
                        .child(dot(state_color(instance.state)))
                        .child(state_name(instance.state))
                        .child("·")
                        .child(div().truncate().child(instance.game_type.title())),
                )
                .when_some(players, |this, players| {
                    this.child(
                        div()
                            .text_size(text::SMALL)
                            .text_color(state_color(instance.state))
                            .child(players),
                    )
                })
                .child(address_chip(&address, cx))
                .when(!powers.is_empty() || share.is_some(), |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .children(powers)
                            .children(share),
                    )
                }),
        )
        .children(tabs)
}

/// The address players join an instance at: the core's public domain or address, else
/// `localhost` (the dashboard's `domain || address`), and the instance's port.
pub fn instance_address(settings: Option<&GlobalSettingsData>, port: u32) -> String {
    let domain = settings
        .and_then(|s| s.domain.clone())
        .map(|d| d.trim().to_string())
        .filter(|d| !d.is_empty())
        .unwrap_or_else(|| "localhost".to_string());
    format!("{domain}:{port}")
}

/// The "Copy IP" chip (`ClipboardTextfield`): a click copies the address.
fn address_chip(address: &str, cx: &mut Context<HostShell>) -> impl IntoElement + use<> {
    let text = address.to_string();
    div()
        .id("copy-address")
        .flex()
        .items_center()
        .gap(px(6.))
        .self_start()
        .max_w_full()
        .px(px(6.))
        .py(px(2.))
        .rounded(radius::MD)
        .bg(colors::BLUE_FADED.opacity(20))
        .text_size(text::SMALL)
        .font_family(lodestone_ui::theme::fonts::mono())
        .text_color(colors::BLUE_200)
        .cursor_pointer()
        .hover(|style| style.bg(colors::BLUE_FADED.opacity(35)))
        .child(div().truncate().child(text.clone()))
        .child(Icon::Copy.render(px(10.), colors::BLUE_200))
        .tooltip(text_tooltip(t(crate::i18n::InstancesMsg::CopyAddress)))
        .on_click(cx.listener(move |this, _, _, cx| {
            cx.write_to_clipboard(gpui::ClipboardItem::new_string(text.clone()));
            let message = crate::instance_icons::copied(&text);
            this.state.update(cx, |state, cx| {
                state.toast(
                    lodestone_ui::components::Toast::success(message).key("copied"),
                    cx,
                )
            });
        }))
}
