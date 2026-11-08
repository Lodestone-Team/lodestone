//! The main area: the selected instance's header and tabs, the tab's content, and
//! the bottom bar with the large Play button.

use gpui::{Context, IntoElement, ObjectFit, SharedString, div, img, prelude::*, px};
use launcher_core::instances::Instance;

use super::{InstanceTab, LauncherShell};
use crate::{
    components::{Align, Button, ButtonVariant, Icon, PlayButton, PlayButtonState, Tabs},
    instances::{PlayState, play_time},
    theme::{self, colors, fonts, leading, radius, text, weights},
};

pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let tab = shell.tab;
    let selected = InstanceTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
    let instance = shell.instances.read(cx).selected().cloned();
    let (title, subtitle): (SharedString, SharedString) = match &instance {
        Some(instance) => (
            instance.name.clone().into(),
            format!(
                "Minecraft {} · {} · {}",
                instance.game_version,
                instance.loader_label(),
                play_time(instance.play_time)
            )
            .into(),
        ),
        None => (
            "No instance selected".into(),
            "Create an instance to choose a Minecraft version and loader.".into(),
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
                        ),
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
                    (InstanceTab::Play, _) => hero(instance.as_ref(), cx).into_any_element(),
                    (InstanceTab::Settings, Some(_)) => match shell.settings_view() {
                        Some(settings) => settings.clone().into_any_element(),
                        None => placeholder("Settings appear here once you have an instance.")
                            .into_any_element(),
                    },
                    (InstanceTab::Logs, Some(_)) => shell.console.clone().into_any_element(),
                    (InstanceTab::Content, Some(_)) => shell.content.clone().into_any_element(),
                    (InstanceTab::Worlds, Some(_)) => shell.worlds.clone().into_any_element(),
                    (other, None) => placeholder(format!(
                        "{} appear here once you have an instance.",
                        other.label()
                    ))
                    .into_any_element(),
                }),
        )
        .child(play_bar(shell, instance.as_ref(), cx))
}

/// The banner: our own artwork in the dashboard's colours, with the instance's name,
/// or an invitation to create one.
fn hero(instance: Option<&Instance>, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let (title, body): (SharedString, SharedString) = match instance {
        Some(instance) => (
            instance.name.clone().into(),
            match instance.last_played {
                Some(_) => format!(
                    "{}. Played for {}.",
                    described(instance),
                    play_time(instance.play_time).to_lowercase()
                )
                .into(),
                None => format!(
                    "{}. Press Play to download it and start the game.",
                    described(instance)
                )
                .into(),
            },
        ),
        None => (
            "Create your first instance".into(),
            "An instance is a Minecraft version with its own mods, worlds and settings.".into(),
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
        .child(
            img(crate::images::hero(cx))
                .absolute()
                .inset_0()
                .size_full()
                .object_fit(ObjectFit::Cover),
        )
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
                        .text_color(colors::GRAY_400)
                        .child(body),
                )
                .when(instance.is_none(), |this| {
                    this.child(
                        div().flex().mt_2().child(
                            Button::new("hero-new-instance", "New instance...")
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

/// "Minecraft 1.20.1", or "Minecraft 1.20.1 with Fabric 0.16.14".
fn described(instance: &Instance) -> String {
    match instance.loader.kind() {
        None => format!("Minecraft {}", instance.game_version),
        Some(_) => format!(
            "Minecraft {} with {}",
            instance.game_version,
            instance.loader_label()
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
                        .child(caption),
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
            "INSTANCE",
            instance.map_or("No instances".to_string(), |i| i.name.clone()),
            instance.is_some(),
        ))
        .child(
            PlayButton::new("play")
                .state(state)
                .on_click(cx.listener(move |this, _, _, cx| {
                    if let Some(id) = &id {
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
        .child(picker(
            "version-picker",
            "VERSION",
            instance.map_or("—".to_string(), |i| {
                format!("{} · {}", i.game_version, i.loader_label())
            }),
            instance.is_some(),
        ))
}
