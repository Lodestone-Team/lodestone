//! The main area: the selected instance's header and tabs, the tab's content, and
//! the bottom bar with the large Play button.

use crate::skin::Skinned;
use crate::theme::px;
use gpui::{AnyElement, Context, IntoElement, ObjectFit, SharedString, div, img, prelude::*};
use launcher_core::instances::{BackgroundMode, Instance};
use launcher_core::ui_skin::SurfaceId;

use lodestone_ui::{theme::Tone, values::truncate::sanitize};

use super::{InstanceTab, LauncherShell, Page};
use crate::{
    components::{
        Align, Button, ButtonSize, ButtonVariant, Fact, FactLine, FactValue, Icon, Kind,
        MagnetLook, ObjectChip, PlayButton, PlayButtonState, StatusBadge, Tabs,
        a11y::{Accessible, Role},
    },
    i18n::{FactMsg, ServersMsg, ShellMsg as M, format, t, tr},
    instances::appearance::{Background, scrim_layers},
    instances::{PlayState, facts, play_time},
    servers::{InstanceLink, friends::Liveness},
    theme::{self, colors, fonts, leading, radius, text, weights},
};

pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let tab = shell.tab;
    let selected = InstanceTab::ALL.iter().position(|t| *t == tab).unwrap_or(0);
    let instance = shell.instances.read(cx).selected().cloned();
    let link = instance
        .as_ref()
        .and_then(|i| shell.servers.read(cx).instance_link(&i.id));
    let rosetta = instance
        .as_ref()
        .and_then(|i| shell.instances.read(cx).rosetta_note(i));
    let (title, subtitle): (SharedString, AnyElement) = match &instance {
        Some(instance) => (instance.name.clone().into(), {
            // the game, its loader, play time and last played as keys over values;
            // a linked server is a cell of its own
            let mut facts = facts::version_facts(instance);
            facts.extend(facts::played_facts(instance));
            facts.extend(link.as_ref().map(|link| linked_server(link, cx)));
            div()
                .mt_2()
                .child(FactLine::strip("instance-facts").facts(facts))
                .into_any_element()
        }),
        None => (
            t(M::NoInstanceSelected).into(),
            div()
                .text_color(crate::theme::ink::muted())
                .font_weight(weights::BOLD.into())
                .child(t(M::NoInstanceSelectedHint))
                .into_any_element(),
        ),
    };
    let check = link.as_ref().and_then(|link| check_button(link, cx));
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
                        .child(subtitle)
                        .children(check)
                        .children(rosetta.map(rosetta_note)),
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
    // the same facts as the header's, as a line of their own; the sentence keeps only
    // the call to action, before the first game
    let (title, body, hero_facts): (SharedString, Option<SharedString>, Option<FactLine>) =
        match instance {
            Some(instance) => {
                let played = instance.last_played.is_some() && instance.play_time > 0;
                let mut hero_facts = facts::version_facts(instance);
                if played {
                    hero_facts.push(Fact::new(
                        t(FactMsg::PlayTime),
                        play_time(instance.play_time),
                    ));
                }
                (
                    instance.name.clone().into(),
                    (!played).then(|| t(M::HeroNotPlayed).into()),
                    Some(FactLine::strip("hero-facts").facts(hero_facts)),
                )
            }
            None => (
                t(M::CreateFirstInstance).into(),
                Some(t(M::InstanceExplainer).into()),
                None,
            ),
        };
    let description: SharedString = match (&body, &hero_facts) {
        (Some(body), _) => body.clone(),
        (None, Some(_)) => instance
            .map(|instance| {
                let mut labels: Vec<String> = facts::version_facts(instance)
                    .iter()
                    .map(|fact| fact.label().to_string())
                    .collect();
                labels.push(
                    Fact::new(t(FactMsg::PlayTime), play_time(instance.play_time))
                        .label()
                        .to_string(),
                );
                format::list(&labels).into()
            })
            .unwrap_or_default(),
        (None, None) => SharedString::default(),
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
                _ => this
                    .child(
                        img(crate::images::hero(cx))
                            .absolute()
                            .inset_0()
                            .size_full()
                            .rounded(radius::XL)
                            .object_fit(ObjectFit::Cover),
                    )
                    .child(crate::images::hero_fit()),
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
                        .aria_description(description)
                        .font_family(fonts::title())
                        .text_size(theme::text_size(text::XLARGE_2))
                        .line_height(theme::line_height(text::XLARGE_2, leading::TIGHT))
                        .font_weight(weights::BOLD.into())
                        .text_color(colors::WHITE)
                        .child(title),
                )
                .children(hero_facts.map(|line| {
                    // over the artwork or a picture: the strip's inks, kept light
                    div().child(line)
                }))
                .children(body.map(|body| {
                    div()
                        .text_size(theme::text_size(text::H3))
                        .line_height(theme::line_height(text::H3, leading::NORMAL))
                        .text_color(if lit {
                            colors::GRAY_300
                        } else {
                            colors::GRAY_400
                        })
                        .child(body)
                }))
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

/// "Runs under Rosetta 2: …", under the instance's name, while its game runs in an
/// Intel Java on this Apple silicon Mac, and why ([`crate::instances::Instances::rosetta_note`]):
/// the launcher never runs a game under Rosetta without saying so.
fn rosetta_note(reason: launcher_core::game::install::Rosetta) -> impl IntoElement {
    let note: SharedString = t(match reason {
        launcher_core::game::install::Rosetta::Version => M::RunsUnderRosetta,
        launcher_core::game::install::Rosetta::JavaOverride => M::RunsUnderRosettaOverride,
    })
    .into();
    div()
        .id("rosetta-note")
        .accessible(Role::Label, "rosetta-note", note.clone())
        .text_size(theme::text_size(text::SMALL))
        .text_color(crate::theme::ink::muted())
        .child(note)
}

/// The header's Server cell, for an instance linked to a server: the server as a
/// link (to the Lodestone Host page, or the Friends page, with its row scrolled into
/// sight and flashed), the host it is on, muted, and whether it runs.
fn linked_server(link: &InstanceLink, cx: &mut Context<LauncherShell>) -> Fact {
    let (core, server) = (link.core_uuid.clone(), link.server_uuid.clone());
    let shell = cx.entity().downgrade();
    let server_name = sanitize(&link.server_name, 64);
    let core_name = sanitize(&link.core_name, 64);
    let status = match link.live {
        Liveness::Unknown => None,
        Liveness::Checking => Some(StatusBadge::new(t(M::ServerChecking), Tone::Neutral)),
        Liveness::Running => Some(StatusBadge::new(t(M::ServerOnline), Tone::Success)),
        Liveness::Starting => Some(StatusBadge::new(t(M::ServerStarting), Tone::Warning)),
        Liveness::Stopped => Some(StatusBadge::new(t(M::ServerStopped), Tone::Neutral)),
        Liveness::Unreachable => Some(
            StatusBadge::new(t(M::ServerUnreachable), Tone::Danger).icon(Icon::TriangleExclamation),
        ),
    };
    // a host since removed from the Lodestone Host page
    let removed = (!link.core_known).then(|| StatusBadge::new(t(M::HostRemoved), Tone::Warning));
    let mut plain = vec![server_name.clone(), core_name.clone()];
    plain.extend(status.as_ref().map(|badge| badge.label().to_string()));
    plain.extend(removed.as_ref().map(|badge| badge.label().to_string()));
    let value = div()
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap(px(6.))
        .child(
            ObjectChip::new(server_name)
                .kind(Kind::Server)
                .id("linked-server")
                .on_click(move |_, _, cx| {
                    shell
                        .update(cx, |this, cx| {
                            // the page its row is on: the Friends page for a server
                            // from the friend list, the Lodestone Host page for the
                            // others
                            let (friends, row_core, row_server) =
                                this.servers.read(cx).link_target(&core, &server);
                            this.show(
                                if friends {
                                    Page::Friends
                                } else {
                                    Page::Servers
                                },
                                cx,
                            );
                            this.servers.update(cx, |servers, cx| {
                                servers.highlight_server(&row_core, &row_server, cx)
                            });
                        })
                        .ok();
                }),
        )
        .child(ObjectChip::new(core_name).kind(Kind::Computer).muted())
        .children(status.map(|badge| badge.id("linked-server-status")))
        .children(removed);
    Fact::new(
        t(FactMsg::Server),
        FactValue::element(value, format::list(&plain)),
    )
}

/// "Check", which asks a friend's server again when it isn't running.
fn check_button(link: &InstanceLink, cx: &mut Context<LauncherShell>) -> Option<AnyElement> {
    let checkable = link.friend
        && matches!(
            link.live,
            Liveness::Stopped | Liveness::Starting | Liveness::Unreachable
        );
    checkable.then(|| {
        let friend = link.core_uuid.clone();
        div()
            .flex()
            .mt_1()
            .child(
                Button::new("linked-server-check", t(ServersMsg::FriendCheck))
                    .size(ButtonSize::Slim)
                    .variant(ButtonVariant::Ghost)
                    .disabled(link.checking)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.servers
                            .update(cx, |servers, cx| servers.check_friend(&friend, cx))
                    })),
            )
            .into_any_element()
    })
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
        .text_color(crate::theme::ink::faint())
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
            // a friend's server that is stopped or starting: Minecraft couldn't join it
            // ("Play without joining" still plays)
            (Some(link), None) if joins && link.blocks_play => (
                PlayButtonState::Disabled,
                Some(tr!(
                    if link.live == Liveness::Starting {
                        M::ServerStartingCaption
                    } else {
                        M::ServerStoppedCaption
                    },
                    server = link.server_name
                )),
            ),
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
    // the line under the step, in the skin's progress colour, made to read on the bar
    let progress = instance
        .and_then(|i| i.play_color)
        .or(resolved.progress)
        .or(resolved.play.color)
        .map(|c| launcher_core::ui_skin::readable::suggest(c, magnet.backdrop, 3.0));
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
            div()
                .flex()
                .flex_col()
                .flex_none()
                .items_center()
                .gap_1()
                .child(
                    PlayButton::new("play")
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
}
