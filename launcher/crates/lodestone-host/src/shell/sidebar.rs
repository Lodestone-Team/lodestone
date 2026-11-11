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
    components::{
        Align, Button, ButtonSize, ButtonVariant, Dropdown, Icon, IconButton, ListRow,
        PowerButtons, power_name, settings::popover_panel, text_tooltip,
    },
    theme::{Color, colors, ink, px, radius, text, weights},
};

use super::HostShell;
use crate::{
    i18n::{ShellMsg, t, tr},
    routes::{InstanceTab, ModSupport, Route, TabContext, mod_support},
    state::{Connection, Creating, HostState},
};

/// An instance's state, as the dashboard's status dots colour it.
pub fn state_color(state: InstanceState) -> Color {
    match state {
        InstanceState::Running => colors::GREEN,
        InstanceState::Starting | InstanceState::Stopping => colors::YELLOW,
        InstanceState::Error => colors::RED,
        // also the colour of its name on the state pill, so it is readable there
        InstanceState::Stopped | InstanceState::Unknown => ink::muted(),
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

/// The empty selected-instance block's height before any instance was shown: about
/// what a stopped server's card with six tabs takes.
const EMPTY_BLOCK_HEIGHT: f32 = 368.;

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
        Some(instance) => {
            // remember how tall it is, for the empty block (read on the next render, so
            // no notify)
            let shell_entity = cx.entity().downgrade();
            div()
                .relative()
                .child(selected_card(shell, instance, selected_tab, state, cx))
                .child(
                    gpui::canvas(
                        move |bounds, _, cx| {
                            shell_entity
                                .update(cx, |shell, _| {
                                    shell.selected_block_height = Some(bounds.size.height)
                                })
                                .ok();
                        },
                        |_, _, _, _| {},
                    )
                    .absolute()
                    .size_full(),
                )
                .into_any_element()
        }
        None => div()
            .mx(px(8.))
            .p(px(16.))
            // the space a selected instance takes, so the list under it stays put: as
            // tall as the last one shown, or a stopped server with six tabs
            .h(shell
                .selected_block_height
                .unwrap_or_else(|| px(EMPTY_BLOCK_HEIGHT)))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
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
    let settings_selected = shell.settings_open().is_some();

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
                // a row like the others, as the footer's
                .child(
                    div().pt(px(4.)).child(
                        ListRow::new(
                            "new-server",
                            if may_create {
                                t(ShellMsg::NewServer)
                            } else {
                                t(ShellMsg::NoPermission)
                            },
                        )
                        .icon(Icon::SquarePlus)
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
                        .on_click(cx.listener(|this, _, _, cx| this.open_settings(None, cx))),
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

/// The power icon's colour, as the launcher's server rows and the dashboard's card
/// show the state on it: green running, yellow starting or stopping, red after a
/// crash, grey stopped.
pub fn power_color(state: InstanceState) -> Color {
    match state {
        InstanceState::Running => colors::GREEN_200,
        InstanceState::Starting | InstanceState::Stopping => colors::YELLOW,
        InstanceState::Error => colors::RED_200,
        InstanceState::Stopped | InstanceState::Unknown => colors::GRAY_500,
    }
}

/// The players pill's background and text, as the dashboard's `Label` colours it by
/// the state.
fn pill_colors(state: InstanceState) -> (Color, Color) {
    match state {
        InstanceState::Running => (colors::GREEN_FADED.opacity(25), colors::GREEN),
        InstanceState::Starting | InstanceState::Stopping => {
            (colors::YELLOW_FADED.opacity(25), colors::YELLOW)
        }
        InstanceState::Error => (colors::RED_FADED.opacity(25), colors::RED),
        InstanceState::Stopped | InstanceState::Unknown => {
            (colors::GRAY_FADED.opacity(30), colors::GRAY_300)
        }
    }
}

/// The power menu's id.
const POWER_MENU: &str = "power-menu";

/// The power menu's icon for `power`, after the dashboard's.
fn power_icon(power: Power) -> Icon {
    match power {
        Power::Start | Power::Stop => Icon::PowerOff,
        Power::Restart => Icon::ArrowsRotate,
        Power::Kill => Icon::Xmark,
    }
}

/// The selected instance, as the dashboard's card (`SelectedInstanceInfo.tsx`,
/// `InstanceCard.tsx`): its icon and name with the power button, which shows the
/// state and opens the power actions the user may do; Share (Copy IP for servers
/// that can't be shared) and the players pill; then the tabs.
fn selected_card(
    shell: &HostShell,
    instance: &InstanceInfo,
    selected_tab: Option<InstanceTab>,
    state: &Snapshot,
    cx: &mut Context<HostShell>,
) -> impl IntoElement {
    let me = &state.me;
    let uuid = instance.uuid.clone();
    let atom_blocked =
        instance.is_atom() && (!me.is_owner || !state.info.capabilities.generic_instances);
    // what the core lets this user do: a restart stops and starts the server
    let powers = {
        let (uuid, me) = (uuid.clone(), me.clone());
        PowerButtons::new("power", instance.state).allow(move |power| {
            let may = |permission| me.may(permission, &uuid);
            let allowed = match power {
                Power::Start => may(InstancePermission::Start),
                Power::Restart => may(InstancePermission::Start) && may(InstancePermission::Stop),
                Power::Stop | Power::Kill => may(InstancePermission::Stop),
            };
            allowed && !atom_blocked
        })
    }
    .actions();
    let menu_open = !powers.is_empty() && shell.power_menu.as_deref() == Some(uuid.as_str());

    let power_button = {
        let uuid = uuid.clone();
        IconButton::new("power-menu-button", Icon::PowerOff)
            .label(tr!(ShellMsg::PowerMenu, state = state_name(instance.state)))
            .color(power_color(instance.state))
            .disabled(powers.is_empty())
            .on_click(cx.listener(move |this, _, _, cx| {
                this.power_menu = match this.power_menu {
                    Some(ref open) if *open == uuid => None,
                    _ => Some(uuid.clone()),
                };
                cx.notify();
            }))
    };
    let panel = powers.iter().fold(
        popover_panel("power-menu-panel").min_w(px(128.)),
        |panel, &power| {
            let item = Button::new(
                gpui::SharedString::from(format!("power-menu-{power:?}")),
                power_name(power),
            )
            .size(ButtonSize::Slim)
            .variant(ButtonVariant::Ghost)
            .align(Align::Between)
            .icon_right(power_icon(power))
            .full_width();
            // a closing menu takes no presses
            let item = if menu_open {
                let uuid = uuid.clone();
                item.on_click(cx.listener(move |this, _, _, cx| {
                    this.power_menu = None;
                    this.state
                        .update(cx, |state, cx| state.power(&uuid, power, cx));
                    cx.notify();
                }))
            } else {
                item
            };
            panel.child(item)
        },
    );
    let close = cx.listener(|this, _: &(), _, cx| {
        this.power_menu = None;
        cx.notify();
    });
    let power = div()
        .flex_none()
        .child(Dropdown::new(power_button).align_end().animated_popover(
            POWER_MENU,
            menu_open,
            panel,
            move |window, cx| close(&(), window, cx),
        ));

    // Share opens the Overview's Share section, which Minecraft Java servers have, for
    // those who may change them
    let can_share = matches!(
        instance.game_type,
        launcher_core::cores::admin::models::Game::MinecraftJava { .. }
    ) && me.may(InstancePermission::Settings, &uuid)
        && !atom_blocked;
    let share: AnyElement = if can_share {
        let uuid = uuid.clone();
        div()
            .flex_1()
            .min_w_0()
            .child(
                Button::new("sidebar-share", t(crate::i18n::InstancesMsg::ShareShort))
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Secondary)
                    .icon(Icon::Link)
                    .full_width()
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
                    })),
            )
            .id("sidebar-share-hint")
            .tooltip(text_tooltip(t(crate::i18n::InstancesMsg::ShareShortHint)))
            .into_any_element()
    } else {
        let address = instance_address(state.settings.as_ref(), instance.port);
        address_chip(&address, cx).into_any_element()
    };
    // always shown, as the dashboard's: "/20 Online" while the count is unknown
    let players = {
        let count = |n: Option<u32>| n.map(|n| n.to_string()).unwrap_or_default();
        tr!(
            ShellMsg::PlayersOnline,
            online = count(instance.player_count),
            max = count(instance.max_player_count)
        )
    };
    let players = {
        let (bg, fg) = pill_colors(instance.state);
        div()
            .flex_1()
            .min_w_0()
            .flex()
            .justify_center()
            .px(px(8.))
            .py(px(2.))
            .rounded_full()
            .bg(bg)
            .text_size(text::SMALL)
            .font_weight(weights::BOLD.face())
            .text_color(fg)
            .child(div().truncate().child(players))
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
            // the dashboard's `my-2 border-b-2 pb-2`, at its paddings
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .mx(px(4.))
                .mt(px(4.))
                .mb(px(4.))
                .pb(px(8.))
                .border_b_2()
                .border_color(colors::GRAY_FADED.opacity(30))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(crate::instance_icons::game_icon(
                            "selected-instance-icon",
                            &instance.game_type,
                            instance.icon_url.as_deref(),
                            px(16.),
                            cx,
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_size(text::MEDIUM)
                                .font_weight(weights::BOLD.face())
                                .text_color(ink::text())
                                .truncate()
                                .child(instance.name.clone()),
                        )
                        .child(power),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .child(share)
                        .child(players),
                ),
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

/// The "Copy IP" chip (`ClipboardTextfield`), in place of Share for servers that
/// can't be shared: a click copies the address.
fn address_chip(address: &str, cx: &mut Context<HostShell>) -> impl IntoElement + use<> {
    let text = address.to_string();
    div()
        .id("copy-address")
        .flex()
        .items_center()
        .gap(px(6.))
        .flex_1()
        .min_w_0()
        .justify_center()
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
