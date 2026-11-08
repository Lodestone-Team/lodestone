//! The main area: the selected instance's header and tabs, the tab's content, and
//! the bottom bar with the large Play button.

use gpui::{Context, IntoElement, ObjectFit, SharedString, div, img, prelude::*, px};
use launcher_core::instances::Instance;

use super::{InstanceTab, LauncherShell, loader_label};
use crate::{
    components::{
        Align, Button, ButtonSize, ButtonVariant, Icon, PlayButton, PlayButtonState, Tabs,
    },
    i18n::{ShellMsg as M, t, tr},
    instances::appearance::{Background, scrim_layers},
    instances::{Ago, PlayState, play_time},
    servers::InstanceLink,
    theme::{self, colors, fonts, leading, radius, text, weights},
};

pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let tab = shell.tab;
    let selected = InstanceTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
    let instance = shell.instances.read(cx).selected().cloned();
    let link = instance
        .as_ref()
        .and_then(|i| shell.servers.read(cx).instance_link(&i.id));
    let (title, subtitle): (SharedString, SharedString) = match &instance {
        Some(instance) => (
            instance.name.clone().into(),
            {
                let mut parts = vec![
                    tr!(M::MinecraftVersion, version = instance.game_version),
                    loader_label(instance),
                ];
                // A game that is running for the first time has been played, but has
                // no play time yet.
                if instance.play_time > 0 || instance.last_played.is_none() {
                    parts.push(play_time(instance.play_time));
                }
                parts.extend(instance.last_played.map(|when| Ago::since(when).played()));
                parts.join(" · ")
            }
            .into(),
        ),
        None => (
            t(M::NoInstanceSelected).into(),
            t(M::NoInstanceSelectedHint).into(),
        ),
    };
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            // The header: the instance's name and its tabs.
            div()
                .flex()
                .flex_col()
                .flex_none()
                .gap_3()
                .px_8()
                .pt_6()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .font_family(fonts::TITLE)
                                .text_size(theme::text_size(text::H1))
                                .line_height(theme::line_height(text::H1, leading::TIGHT))
                                .font_weight(weights::BOLD.into())
                                .text_color(colors::GRAY_300)
                                .child(title),
                        )
                        .child(
                            div()
                                .text_color(colors::GRAY_500)
                                .font_weight(weights::BOLD.into())
                                .child(subtitle),
                        )
                        .children(link.as_ref().map(linked_to)),
                )
                .child(
                    Tabs::new("instance-tabs", InstanceTab::ALL.map(InstanceTab::label))
                        .selected(selected)
                        .on_select({
                            let entity = cx.entity();
                            move |index, _, cx| {
                                let tab = InstanceTab::ALL[*index];
                                entity.update(cx, |this, cx| this.select_tab(tab, cx));
                            }
                        }),
                ),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .px_8()
                .py_5()
                .child(match (tab, &instance) {
                    (InstanceTab::Play, _) => {
                        let background = instance
                            .as_ref()
                            .and_then(|i| shell.instances.read(cx).background(&i.id).cloned());
                        hero(instance.as_ref(), background, cx).into_any_element()
                    }
                    (InstanceTab::Settings, Some(_)) => match shell.settings_view() {
                        Some(settings) => settings.clone().into_any_element(),
                        None => placeholder(t(M::SettingsPlaceholder)).into_any_element(),
                    },
                    (InstanceTab::Logs, Some(_)) => shell.console.clone().into_any_element(),
                    (InstanceTab::Content, Some(_)) => shell.content.clone().into_any_element(),
                    (InstanceTab::Worlds, Some(_)) => shell.worlds.clone().into_any_element(),
                    (other, None) => placeholder(t(match other {
                        InstanceTab::Content => M::ContentPlaceholder,
                        InstanceTab::Worlds => M::WorldsPlaceholder,
                        InstanceTab::Logs => M::LogsPlaceholder,
                        InstanceTab::Play | InstanceTab::Settings => M::SettingsPlaceholder,
                    }))
                    .into_any_element(),
                }),
        )
        .child(play_bar(shell, instance.as_ref(), cx))
}

/// The banner: our own artwork in the dashboard's colours, with the instance's name,
/// or an invitation to create one.
fn hero(
    instance: Option<&Instance>,
    background: Option<Background>,
    cx: &mut Context<LauncherShell>,
) -> impl IntoElement {
    let (title, body): (SharedString, SharedString) = match instance {
        Some(instance) => (
            instance.name.clone().into(),
            match instance.last_played.filter(|_| instance.play_time > 0) {
                Some(_) => tr!(
                    M::HeroPlayed,
                    game = described(instance),
                    time = play_time(instance.play_time).to_lowercase()
                )
                .into(),
                None => tr!(M::HeroNotPlayed, game = described(instance)).into(),
            },
        ),
        None => (
            t(M::CreateFirstInstance).into(),
            t(M::InstanceExplainer).into(),
        ),
    };
    div()
        .relative()
        .flex()
        .flex_col()
        .justify_end()
        .size_full()
        .rounded(radius::XL)
        .overflow_hidden()
        .border_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_900)
        .map(|this| match &background {
            // The instance's own image, with a scrim as dark as it needs under the text.
            Some(background) => this
                .child(
                    img(background.image.clone())
                        .absolute()
                        .inset_0()
                        .size_full()
                        .object_fit(ObjectFit::Cover),
                )
                .children(scrim_layers(background.scrim)),
            None => this.child(
                img(crate::images::hero(cx))
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover),
            ),
        })
        .child(
            div()
                .relative()
                .flex()
                .flex_col()
                .gap_2()
                .p_8()
                .max_w(px(460.))
                .child(
                    div()
                        .font_family(fonts::TITLE)
                        .text_size(theme::text_size(text::XLARGE_2))
                        .line_height(theme::line_height(text::XLARGE_2, leading::TIGHT))
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::WHITE)
                        .child(title),
                )
                .child(
                    div()
                        .text_size(theme::text_size(text::H3))
                        .line_height(theme::line_height(text::H3, leading::NORMAL))
                        .text_color(if background.is_some() {
                            colors::GRAY_300
                        } else {
                            colors::GRAY_400
                        })
                        .child(body),
                )
                .when(instance.is_none(), |this| {
                    this.child(
                        div().flex().mt_2().child(
                            Button::new("hero-new-instance", t(M::NewInstance))
                                .variant(ButtonVariant::Primary)
                                .icon(Icon::SquarePlus)
                                .align(Align::Start)
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.open_new_instance(window, cx)
                                })),
                        ),
                    )
                }),
        )
}

/// "Linked to Survival SMP on Peter's core", under the instance's name.
fn linked_to(link: &InstanceLink) -> impl IntoElement + use<> {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .mt_1()
        .child(
            div()
                .flex_none()
                .size(px(6.))
                .rounded_full()
                .bg(colors::BLUE_200),
        )
        .child(
            div()
                .truncate()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .text_color(colors::BLUE_200)
                .child(tr!(
                    if link.core_known {
                        M::LinkedTo
                    } else {
                        M::LinkedToRemoved
                    },
                    server = link.server_name,
                    core = link.core_name
                )),
        )
}

/// "Minecraft 1.20.1", or "Minecraft 1.20.1 with Fabric 0.16.14".
fn described(instance: &Instance) -> String {
    match instance.loader.kind() {
        None => tr!(M::MinecraftVersion, version = instance.game_version),
        Some(_) => tr!(
            M::MinecraftWith,
            version = instance.game_version,
            loader = instance.loader_label()
        ),
    }
}

fn placeholder(message: impl Into<SharedString>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .size_full()
        .rounded(radius::XL)
        .border_2()
        .border_dashed()
        .border_color(colors::GRAY_FADED.opacity(10))
        .text_color(colors::GRAY_FADED.opacity(30))
        .font_weight(weights::BOLD.into())
        .child(message.into())
}

/// A picker in the bottom bar: a caption over a value.
fn picker(
    id: &'static str,
    caption: &'static str,
    value: impl Into<SharedString>,
    enabled: bool,
) -> impl IntoElement {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .w(px(200.))
        .px_3()
        .py_2()
        .rounded(radius::LG)
        .border_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_800)
        .when(!enabled, |this| this.opacity(0.6))
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .text_size(theme::text_size(text::CAPTION))
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::GRAY_500)
                        .truncate()
                        .child(caption.to_uppercase()),
                )
                .child(
                    div()
                        .truncate()
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::GRAY_300)
                        .child(value.into()),
                ),
        )
}

/// The bar under the tabs: the instance, Play, and the version.
fn play_bar(
    shell: &LauncherShell,
    instance: Option<&Instance>,
    cx: &mut Context<LauncherShell>,
) -> impl IntoElement {
    let state = match shell.instances.read(cx).play_state() {
        PlayState::NoInstance => PlayButtonState::Disabled,
        PlayState::Ready => PlayButtonState::Ready,
        PlayState::Playing => PlayButtonState::Playing,
        PlayState::Preparing {
            label,
            detail,
            fraction,
        } => PlayButtonState::Preparing {
            label,
            detail,
            fraction,
        },
    };
    let id = instance.map(|i| i.id.clone());
    let stop_id = id.clone();
    // An instance made for a server plays on it: the same mod check as the Servers
    // page, then the game joins. "Play without joining" is the plain Play.
    let servers = shell.servers.read(cx);
    let link = id.as_ref().and_then(|id| servers.instance_link(id));
    let busy = id
        .as_ref()
        .and_then(|id| servers.instance_busy(id))
        .map(|busy| busy.label());
    let joins = link.is_some() && matches!(state, PlayButtonState::Ready);
    let (state, caption) = match (&link, busy) {
        (Some(_), Some(busy)) if joins => (PlayButtonState::Disabled, Some(busy)),
        (Some(link), None) if joins => (state, Some(tr!(M::AndJoin, server = link.server_name))),
        _ => (state, None),
    };
    let single_id = id.clone();
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .justify_center()
        .gap_6()
        .w_full()
        .px_8()
        .py_4()
        .border_t_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_850)
        .child(picker(
            "instance-picker",
            t(M::PickerInstance),
            instance.map_or(t(M::NoInstances).to_string(), |i| i.name.clone()),
            instance.is_some(),
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap_1()
                .child(
                    PlayButton::new("play")
                        .state(state)
                        .when_some(caption, |button, caption| button.caption(caption))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            let Some(id) = &id else {
                                return;
                            };
                            if joins {
                                this.servers
                                    .update(cx, |servers, cx| servers.play_instance(id, cx));
                            } else {
                                this.instances
                                    .update(cx, |instances, cx| instances.play(id, cx));
                            }
                        }))
                        .on_stop(cx.listener(move |this, _, _, cx| {
                            if let Some(id) = &stop_id {
                                this.instances
                                    .update(cx, |instances, cx| instances.stop(id, cx));
                            }
                        })),
                )
                .when(joins, |this| {
                    this.child(
                        Button::new("play-without-joining", t(M::PlayWithoutJoining))
                            .size(ButtonSize::Slim)
                            .variant(ButtonVariant::Ghost)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(id) = &single_id {
                                    this.instances
                                        .update(cx, |instances, cx| instances.play(id, cx));
                                }
                            })),
                    )
                }),
        )
        .child(picker(
            "version-picker",
            t(M::PickerVersion),
            instance.map_or("—".to_string(), |i| {
                format!("{} · {}", i.game_version, loader_label(i))
            }),
            instance.is_some(),
        ))
}
