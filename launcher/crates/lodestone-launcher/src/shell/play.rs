//! The main area: the selected instance's header and tabs, the tab's content, and
//! the bottom bar with the large Play button.

use crate::skin::Skinned;
use crate::theme::px;
use gpui::{AnyElement, Context, IntoElement, ObjectFit, SharedString, div, img, prelude::*};
use launcher_core::instances::{BackgroundMode, Instance};
use launcher_core::ui_skin::SurfaceId;

use super::{InstanceTab, LauncherShell, loader_label};
use crate::{
    components::{
        Align, Button, ButtonSize, ButtonVariant, Icon, MagnetBar, MagnetLook, PlayButton,
        PlayButtonState, Tabs,
        a11y::{Accessible, Role},
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
                                .font_family(fonts::display())
                                .text_size(theme::text_size(text::H1))
                                .line_height(theme::line_height(text::H1, leading::TIGHT))
                                .font_weight(weights::BOLD.into())
                                .text_color(crate::theme::ink::text())
                                .child(title),
                        )
                        .child(
                            div()
                                .text_color(crate::theme::ink::muted())
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
                        let backdrop = match instance.as_ref().map(Instance::background_mode) {
                            Some(BackgroundMode::Slideshow(_)) => {
                                Backdrop::Slideshow(shell.slideshow.clone().into_any_element())
                            }
                            _ => instance
                                .as_ref()
                                .and_then(|i| shell.instances.read(cx).background(&i.id).cloned())
                                // the skin's hero for an instance without its own image
                                .or_else(|| crate::skin::look(cx).hero.clone())
                                .map_or(Backdrop::Artwork, Backdrop::Image),
                        };
                        hero(instance.as_ref(), backdrop, cx).into_any_element()
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

/// What the banner shows behind the name.
enum Backdrop {
    /// The launcher's artwork.
    Artwork,
    /// The instance's own image.
    Image(Background),
    /// The slideshow, which draws its own images and scrim.
    Slideshow(AnyElement),
}

/// The banner: our own artwork in the dashboard's colours, with the instance's name,
/// or an invitation to create one.
fn hero(
    instance: Option<&Instance>,
    backdrop: Backdrop,
    cx: &mut Context<LauncherShell>,
) -> impl IntoElement {
    let lit = !matches!(backdrop, Backdrop::Artwork);
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
        .border_color(crate::theme::ink::border())
        .bg(colors::GRAY_900)
        .map(|this| match backdrop {
            Backdrop::Slideshow(slideshow) => this.child(slideshow),
            // The instance's own image, with a scrim as dark as it needs under the text.
            Backdrop::Image(background) => this
                .child(
                    img(background.image.clone())
                        .absolute()
                        .inset_0()
                        .size_full()
                        .rounded(radius::XL)
                        .object_fit(ObjectFit::Cover),
                )
                .children(scrim_layers(background.scrim, radius::XL)),
            // the skin's colour or gradient, or the launcher's artwork
            Backdrop::Artwork => match crate::skin::painted(SurfaceId::Hero) {
                Some(painted) if painted.paint.is_some() => crate::skin::paint(this, &painted),
                _ => this.child(
                    img(crate::images::hero(cx))
                        .absolute()
                        .inset_0()
                        .size_full()
                        .rounded(radius::XL)
                        .object_fit(ObjectFit::Cover),
                ),
            },
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
                        .id("hero-title")
                        .accessible(Role::Heading, "hero-title", title.clone())
                        .aria_level(1)
                        .aria_description(body.clone())
                        .font_family(fonts::title())
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
                        .text_color(if lit {
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
                // Lodestone Host's green, as the instance's outline in the sidebar
                .bg(crate::components::host_gradient(100, false)),
        )
        .child(
            div()
                .truncate()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .text_color(crate::theme::accent::light())
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

/// How tall the Play bar and the sidebar's footer are at least (logical pixels at
/// 100% zoom), so the line along the top of one runs on into the other. Both fit
/// in it with room to spare: the footer's two links and version, and Play with
/// "Play without joining" under it.
pub const BOTTOM_BAR_HEIGHT: f32 = 108.0;

/// What Play shows and does for an instance: the Play bar's button, and the one over
/// the instance's icon in the sidebar.
pub(super) struct PlayTarget {
    pub state: PlayButtonState,
    /// "and join Survival SMP", or why Play waits.
    pub caption: Option<String>,
    /// Play joins the server the instance was made for.
    pub joins: bool,
}

impl LauncherShell {
    /// What Play shows and does for the instance `id`. An instance made for a server
    /// plays on it: the same mod check as the Servers page, then the game joins.
    pub(super) fn play_target(&self, id: &str, cx: &gpui::App) -> PlayTarget {
        let state = match self.instances.read(cx).play_state_of(id) {
            PlayState::NoInstance => PlayButtonState::Disabled,
            PlayState::Ready => PlayButtonState::Ready,
            PlayState::Playing => PlayButtonState::Playing,
            PlayState::Preparing {
                label,
                detail,
                fraction,
                step,
            } => PlayButtonState::Preparing {
                label,
                detail,
                fraction,
                step,
            },
        };
        let servers = self.servers.read(cx);
        let link = servers.instance_link(id);
        let busy = servers.instance_busy(id).map(|busy| busy.label());
        let joins = link.is_some() && matches!(state, PlayButtonState::Ready);
        let (state, caption) = match (&link, busy) {
            (Some(_), Some(busy)) if joins => (PlayButtonState::Disabled, Some(busy)),
            (Some(link), None) if joins => {
                (state, Some(tr!(M::AndJoin, server = link.server_name)))
            }
            _ => (state, None),
        };
        PlayTarget {
            state,
            caption,
            joins,
        }
    }

    /// Play: starts the instance `id`, joining its server when `joins`.
    pub(super) fn press_play(&mut self, id: &str, joins: bool, cx: &mut Context<Self>) {
        if joins {
            self.servers
                .update(cx, |servers, cx| servers.play_instance(id, cx));
        } else {
            self.instances
                .update(cx, |instances, cx| instances.play(id, cx));
        }
    }
}

/// How Play melts into filings, in the skin's colours: the button as the skin (or the
/// instance) colours and shapes it, and filings in the instance's Play colour, else
/// the skin's progress colours (all of a gradient's), else its Play colours, else the
/// gem's, made to read on the bar as the skin paints it.
fn magnet_look(
    skin: &crate::skin::Look,
    resolved: &crate::skin::Resolved,
    instance: Option<&Instance>,
) -> MagnetLook {
    use launcher_core::ui_skin::{Fill, Rgb, Shape, readable};
    let spec = &skin.spec;
    let play = &resolved.play;
    // an image or rich gradient on the button: melt from what it looks like on average
    let fill_rgb = play.color.or_else(|| {
        resolved
            .play_layer
            .then(|| skin.backdrops.get(&SurfaceId::PlayButton).map(|b| b.mean))
            .flatten()
    });
    let (fill, edge) = match fill_rgb {
        Some(c) => (
            crate::theme::Color::rgb(c.0),
            crate::theme::Color::rgb(c.mix(Rgb::WHITE, 0.2).0),
        ),
        None => (colors::GREEN_400, colors::GREEN_300),
    };
    let sample = |stops: &dyn Fn(f32) -> Rgb| (0..5).map(|i| stops(i as f32 / 4.0)).collect();
    let progress_fill = spec
        .surfaces
        .get(&SurfaceId::Progress)
        .and_then(|s| s.fill.as_ref());
    let palette = if let Some(color) = instance.and_then(|i| i.play_color) {
        MagnetLook::shades(color)
    } else if let Some(Fill::Gradient(gradient)) = progress_fill {
        sample(&|t| gradient.at(t))
    } else if let Some(color) = resolved.progress {
        MagnetLook::shades(color)
    } else if let (Some(from), Some(to)) = (play.color, play.gradient) {
        sample(&|t| from.mix(to, t))
    } else if let Some(color) = play.color {
        MagnetLook::shades(color)
    } else {
        crate::components::MAGNET_GEM.to_vec()
    };
    // the bar the filings are drawn on: the skin's paint, or the launcher's own
    let backdrop = match spec.surface(SurfaceId::PlayBar) {
        Some(_) => readable::surface_backdrop(spec, SurfaceId::PlayBar, &|surface| {
            skin.backdrops.get(&surface).copied()
        }),
        None => readable::Backdrop::solid(Rgb(crate::theme::palette::map(colors::GRAY_850).0 >> 8)),
    };
    MagnetLook {
        palette,
        backdrop,
        fill,
        gradient: play.gradient.map(|c| crate::theme::Color::rgb(c.0)),
        edge,
        corner: match play.shape {
            Shape::Rounded => Some(8.0),
            Shape::Pill => None,
            Shape::Pixel => Some(0.0),
        },
        glow: play.glow,
    }
}

/// The bar under the tabs: Play.
fn play_bar(
    shell: &LauncherShell,
    instance: Option<&Instance>,
    cx: &mut Context<LauncherShell>,
) -> impl IntoElement {
    let id = instance.map(|i| i.id.clone());
    let PlayTarget {
        state,
        caption,
        joins,
    } = match &id {
        Some(id) => shell.play_target(id, cx),
        None => PlayTarget {
            state: PlayButtonState::Disabled,
            caption: None,
            joins: false,
        },
    };
    let stop_id = id.clone();
    // "Play without joining" is the plain Play.
    let single_id = id.clone();
    let skin = crate::skin::look(cx);
    let resolved = crate::skin::resolve(&skin.spec, instance);
    let magnet = magnet_look(&skin, &resolved, instance);
    // the line under the step, in the filings' colour, made to read on the bar
    let progress = instance
        .and_then(|i| i.play_color)
        .or(resolved.progress)
        .or(resolved.play.color)
        .map(|c| launcher_core::ui_skin::readable::suggest(c, magnet.backdrop, 3.0));
    // a dimmed (disabled) button melts from its dimmed look
    let dimmed = matches!(state, PlayButtonState::Disabled);
    // how far a launch has come, for the filings around Play
    let launch = match &state {
        PlayButtonState::Preparing { step, fraction, .. } => Some(step.overall(*fraction)),
        _ => None,
    };
    // the button writes where it is, for the filings to melt from it alone
    let anchor = crate::components::MagnetAnchor::default();
    let magnet_id =
        SharedString::from(format!("play-magnet-{}", id.as_deref().unwrap_or_default()));
    div()
        .flex()
        .flex_row()
        .flex_none()
        .items_center()
        .justify_center()
        .gap_6()
        .w_full()
        .min_h(px(BOTTOM_BAR_HEIGHT))
        .overflow_hidden()
        .px_8()
        .py_1p5()
        .border_t_1()
        .border_color(crate::theme::ink::border())
        .bg(colors::GRAY_850)
        .surface(SurfaceId::PlayBar)
        .child(
            MagnetBar::new(
                magnet_id,
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .items_center()
                    .gap_1()
                    .child(
                        PlayButton::new("play")
                            .anchor(anchor.clone())
                            .state(state)
                            .look(resolved.play, progress)
                            .layer(resolved.play_layer)
                            .when_some(caption, |button, caption| button.caption(caption))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                if let Some(id) = &id {
                                    this.press_play(id, joins, cx);
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
                                        this.press_play(id, false, cx);
                                    }
                                })),
                        )
                    }),
            )
            .launch(launch)
            .look(magnet)
            .anchor(anchor)
            .dimmed(dimmed)
            // to the bar's edges, past its padding
            .bleed(40., 12.),
        )
}
