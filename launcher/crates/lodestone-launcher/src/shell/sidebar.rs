use crate::components::RichText;
use crate::theme::px;
use gpui::{
    AnyElement, Context, ElementId, ExternalPaths, IntoElement, ObjectFit, SharedString,
    Transformation, div, img, prelude::*, radians,
};
use launcher_core::instances::{AutoGroup, Instance, last_played};
use launcher_core::ui_skin::{FontPart, SurfaceId};

use super::{LauncherShell, Page, account_box};
use crate::components::{
    KeyboardFocus,
    a11y::{Accessible, AccessibleAction, Role},
};
use crate::skin::Skinned;
use crate::{
    components::{Align, Button, ButtonVariant, Icon, PlayButtonState},
    i18n::{ShellMsg as M, format, t, tr},
    instances::{Ago, play_time},
    theme::{self, colors, layout, leading, radius, text, weights},
};

/// The left sidebar: the account box at the top, the instance list and "New
/// instance" in the middle, and Settings pinned at the bottom. Its right edge drags
/// to resize it; dragged narrow enough it folds into a rail of icons.
pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let (width, collapsed) = shell.sidebar_state(cx);
    let dragging = shell.sidebar_dragging();
    div()
        .id("sidebar")
        .role(Role::Navigation)
        .accessibility_id("sidebar")
        .relative()
        .flex()
        .flex_col()
        .flex_none()
        .w(px(if collapsed {
            layout::SIDEBAR_RAIL
        } else {
            width
        }))
        .h_full()
        .bg(colors::GRAY_850)
        .border_r_1()
        .border_color(crate::theme::ink::border())
        .overflow_hidden()
        .surface(SurfaceId::Sidebar)
        .when_some(theme::fonts::own(FontPart::Sidebar), |this, font| {
            this.font_family(font)
        })
        // a backing, when the skin's text colours alone can't stay readable
        .when_some(crate::skin::sidebar_inks().backing, |this, backing| {
            this.child(div().absolute().inset_0().bg(backing))
        })
        .map(|this| {
            if collapsed {
                this.child(rail(shell, cx))
            } else {
                this.child(full(shell, cx))
            }
        })
        .child(resize_handle(dragging, cx))
}

/// The sidebar's right edge: drag it to resize, double-click it for the default.
fn resize_handle(dragging: bool, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    div()
        .id("sidebar-resize")
        .absolute()
        .top_0()
        .bottom_0()
        .right_0()
        .w(px(6.))
        .flex()
        .justify_end()
        .cursor(gpui::CursorStyle::ResizeLeftRight)
        .child(div().w(px(2.)).h_full().when(dragging, |this| {
            this.bg(crate::theme::accent::light().opacity(60))
        }))
        .hover(|style| style.bg(crate::theme::accent::light().opacity(20)))
        .on_mouse_down(
            gpui::MouseButton::Left,
            cx.listener(|this, _, _, cx| {
                cx.stop_propagation();
                this.start_sidebar_drag(cx);
            }),
        )
        .on_press(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
            if event.click_count() >= 2 {
                this.reset_sidebar(cx);
            }
        }))
}

/// The sidebar at full width: the account box, the list, and the footer.
fn full(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .size_full()
        .child(
            div()
                .flex_none()
                .px_2()
                .pt_3()
                .pb_4()
                .child(account_box::render(shell, cx)),
        )
        .child(
            div()
                .id("sidebar-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .px_2()
                .border_2()
                .border_color(colors::TRANSPARENT)
                // A dropped .mrpack becomes a new instance.
                .drag_over::<ExternalPaths>(|style, _, _, _| {
                    style
                        .border_color(crate::theme::accent::light().opacity(60))
                        .bg(crate::theme::accent::base().opacity(8))
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    let paths = paths.paths().to_vec();
                    this.instances
                        .update(cx, |instances, cx| instances.import_files(paths, cx));
                }))
                // Up and Down move between the groups and instances.
                .child(
                    crate::components::ArrowGroup::new("sidebar-list")
                        .flex()
                        .flex_col()
                        .child(instance_list(shell, cx)),
                ),
        )
        .child(footer(shell, cx))
}

/// The folded sidebar: the account's avatar, every instance as its icon (the last
/// played first, under a line), New instance and Import, then Servers and Settings.
/// The names show as tooltips.
fn rail(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let list: Vec<Instance> = shell.instances.read(cx).list().to_vec();
    let last = last_played(&list).cloned();
    // Every instance, in the list's order; a folded group's too, since the rail
    // has no headings to unfold it with.
    let ordered: Vec<Instance> = shell
        .instances
        .read(cx)
        .grouped()
        .into_iter()
        .flat_map(|(_, members)| members.into_iter().cloned())
        .collect();
    let importing: Vec<SharedString> = shell
        .instances
        .read(cx)
        .imports()
        .iter()
        .map(|job| job.name.clone())
        .collect();
    let settings_open = shell.settings_open();
    let servers_open = shell.page == Page::Servers;
    div()
        .flex()
        .flex_col()
        .items_center()
        .size_full()
        .child(
            div()
                .flex_none()
                .pt_3()
                .pb_3()
                .child(account_box::render_compact(shell, cx)),
        )
        .child(
            div()
                .id("sidebar-scroll")
                .flex()
                .flex_col()
                .items_center()
                .flex_1()
                .min_h_0()
                .w_full()
                .overflow_y_scroll()
                .border_2()
                .border_color(colors::TRANSPARENT)
                .drag_over::<ExternalPaths>(|style, _, _, _| {
                    style
                        .border_color(crate::theme::accent::light().opacity(60))
                        .bg(crate::theme::accent::base().opacity(8))
                })
                .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                    let paths = paths.paths().to_vec();
                    this.instances
                        .update(cx, |instances, cx| instances.import_files(paths, cx));
                }))
                .child(
                    crate::components::ArrowGroup::new("sidebar-list")
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .when_some(last, |this, instance| {
                            this.child(rail_instance(shell, &instance, RowPlace::LastPlayed, cx))
                                .child(div().w(px(28.)).h(px(1.)).my_1().bg(colors::FADE_700))
                        })
                        .children(
                            ordered
                                .iter()
                                .map(|instance| rail_instance(shell, instance, RowPlace::List, cx)),
                        )
                        .children(importing.into_iter().enumerate().map(|(index, name)| {
                            div()
                                .id(("rail-import", index))
                                .size(px(44.))
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(radius::MD)
                                .tooltip(crate::components::text_tooltip(name))
                                .child(crate::components::LoadingDots)
                        }))
                        .child(div().h_1())
                        .child(rail_button(
                            "new-instance",
                            Glyph::Icon(Icon::SquarePlus),
                            t(M::NewInstance),
                            false,
                            cx.listener(|this, _, window, cx| this.open_new_instance(window, cx)),
                        ))
                        .child(rail_button(
                            "import-instances",
                            Glyph::Icon(Icon::Download),
                            t(M::ImportInstances),
                            false,
                            cx.listener(|this, _, window, cx| this.open_import(window, cx)),
                        ))
                        .child(div().h_4()),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .flex_none()
                .justify_center()
                .gap_1()
                .w_full()
                .min_h(px(super::play::BOTTOM_BAR_HEIGHT))
                .py_2()
                .border_t_1()
                .border_color(crate::theme::ink::border())
                .child(rail_button(
                    "servers",
                    Glyph::HostMark(crate::images::mark(false, cx)),
                    t(M::Servers),
                    servers_open,
                    cx.listener(move |this, _, _, cx| {
                        this.show(
                            if servers_open {
                                Page::Instance
                            } else {
                                Page::Servers
                            },
                            cx,
                        )
                    }),
                ))
                .child(rail_button(
                    "settings",
                    Glyph::Icon(Icon::Gear),
                    t(M::Settings),
                    settings_open,
                    cx.listener(|this, _, window, cx| this.open_settings(None, Some(window), cx)),
                )),
        )
}

/// What a sidebar link shows beside (or, in the rail, instead of) its label: an icon
/// in the sidebar's ink, or the green Lodestone mark, which stands for Lodestone Host.
enum Glyph {
    Icon(Icon),
    HostMark(gpui::ImageSource),
}

impl Glyph {
    fn render(self, size: gpui::Pixels, color: impl Into<gpui::Hsla>) -> AnyElement {
        match self {
            Glyph::Icon(icon) => icon.render(size, color).into_any_element(),
            Glyph::HostMark(mark) => img(mark).size(size).flex_none().into_any_element(),
        }
    }
}

/// A square icon button of the rail, its label as a tooltip.
fn rail_button(
    id: &'static str,
    icon: Glyph,
    label: &'static str,
    open: bool,
    on_click: impl Fn(&gpui::ClickEvent, &mut gpui::Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    div()
        .id(id)
        .accessible(Role::Button, id, label)
        .keyboard_focus()
        .size(px(40.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(radius::MD)
        .cursor_pointer()
        .map(|this| {
            if open {
                this.bg(crate::skin::sidebar_inks().fill)
            } else {
                this.hover(|style| style.bg(crate::skin::sidebar_inks().fill))
            }
        })
        .tooltip(crate::components::text_tooltip(label))
        .child(icon.render(
            px(16.),
            if open {
                crate::skin::sidebar_inks().primary
            } else {
                crate::skin::sidebar_inks().icon
            },
        ))
        .on_press(on_click)
}

/// An instance in the rail: its icon, highlighted when it's the one shown, with a
/// dot while it runs; its name (and when it was played) as a tooltip.
fn rail_instance(
    shell: &LauncherShell,
    instance: &Instance,
    place: RowPlace,
    cx: &mut Context<LauncherShell>,
) -> AnyElement {
    let instances = shell.instances.read(cx);
    let id = instance.id.clone();
    let is_selected = shell.page == Page::Instance
        && instances
            .selected()
            .is_some_and(|selected| selected.id == id);
    let active = instances.is_active(&id);
    let running = instances.is_running(&id);
    let icon = crate::instances::appearance::instance_icon(instances, instance, px(28.));
    let tooltip: SharedString = match played_summary(instance) {
        Some(played) => format!("{}\n{played}", instance.name).into(),
        None => instance.name.clone().into(),
    };
    let element_id = match place {
        RowPlace::List => format!("instance-{id}"),
        RowPlace::LastPlayed => format!("last-played-{id}"),
    };
    let launching = instances.is_launching(&id);
    let description: Option<String> = match (running, active) {
        (true, _) if launching => Some(t(M::Launching).to_string()),
        (true, _) => Some(t(M::Playing).to_string()),
        _ => played_summary(instance),
    };
    let linked = linked_to(shell, &id, cx);
    let description: Option<SharedString> = match (description, &linked) {
        (Some(description), Some(linked)) => Some(format!("{description} · {linked}").into()),
        (description, linked) => description.or_else(|| linked.clone()).map(Into::into),
    };
    let element_id = SharedString::from(element_id);
    div()
        .id(element_id.clone())
        .accessible(Role::ListItem, element_id, instance.name.clone())
        .aria_selected(is_selected)
        .when_some(description, |this, description| {
            this.aria_description(description)
        })
        .keyboard_focus()
        .relative()
        .size(px(44.))
        .flex()
        .items_center()
        .justify_center()
        .rounded(radius::MD)
        .border_1()
        .border_color(colors::TRANSPARENT)
        .cursor_pointer()
        .hover(|style| style.bg(crate::skin::sidebar_inks().fill))
        .when(is_selected, |this| {
            this.bg(crate::skin::sidebar_inks().fill)
                .border_color(crate::theme::ink::border())
        })
        .child(icon)
        .when(linked.is_some(), |this| {
            this.child(linked_badge(px(14.), px(3.), cx))
        })
        .when(active, |this| {
            this.child(
                div()
                    .absolute()
                    .top(px(4.))
                    .right(px(4.))
                    .size(px(8.))
                    .rounded_full()
                    .border_1()
                    .border_color(colors::GRAY_850)
                    .bg(if running {
                        colors::GREEN_200
                    } else {
                        colors::YELLOW
                    }),
            )
        })
        .tooltip(crate::components::text_tooltip(tooltip))
        .on_press(cx.listener(move |this, _, _, cx| this.select_instance(&id, cx)))
        .into_any_element()
}

/// A modpack being made into an instance: its name, what it is doing, and Cancel.
fn import_row(
    id: u64,
    name: SharedString,
    progress: &launcher_core::content::install::InstallProgress,
    cx: &mut Context<LauncherShell>,
) -> AnyElement {
    let snapshot = progress.downloads.snapshot();
    let fraction = (snapshot.bytes_total > 0)
        .then(|| snapshot.bytes_done as f32 / snapshot.bytes_total as f32)
        .or_else(|| {
            (snapshot.files_total > 0)
                .then(|| snapshot.files_done as f32 / snapshot.files_total as f32)
        });
    let label = crate::instances::modpacks::progress_label(&progress.label());
    div()
        .id(("import", id as usize))
        .flex()
        .flex_col()
        .gap_1()
        .w_full()
        .px_2()
        .py_1p5()
        .rounded(radius::MD)
        .border_1()
        .border_color(crate::theme::accent::light().opacity(30))
        .bg(crate::theme::accent::base().opacity(8))
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(crate::components::LoadingDots)
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .font_weight(weights::BOLD.into())
                        .text_color(crate::skin::sidebar_inks().primary)
                        .child(name),
                )
                .child(
                    crate::components::IconButton::new(("cancel-import", id as usize), Icon::Xmark)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.instances
                                .update(cx, |instances, cx| instances.cancel_import(id, cx))
                        })),
                ),
        )
        .child(
            div()
                .truncate()
                .text_size(theme::text_size(text::SMALL))
                .text_color(crate::skin::sidebar_inks().muted)
                .child(match fraction {
                    Some(f) => tr!(
                        M::ImportProgress,
                        percent = format::percent((f * 100.0).clamp(0.0, 100.0).round() as u64),
                        label = label
                    ),
                    None => label,
                }),
        )
        .when_some(fraction, |this, fraction| {
            this.child(
                div()
                    .h(px(3.))
                    .w_full()
                    .rounded_full()
                    .bg(colors::GRAY_700)
                    .child(
                        div()
                            .h_full()
                            .rounded_full()
                            .bg(crate::theme::accent::light())
                            .surface(launcher_core::ui_skin::SurfaceId::Progress)
                            .w(gpui::relative(fraction.clamp(0.0, 1.0))),
                    ),
            )
        })
        .into_any_element()
}

/// Where an instance row is listed.
#[derive(Clone, Copy, PartialEq, Eq)]
enum RowPlace {
    /// In its group (or loose): the detail line is the version and loader.
    List,
    /// Under "Last played": the detail line is when.
    LastPlayed,
}

/// "Played 2 hours ago · 2 h 5 min in total", for an instance that has been played.
fn played_summary(instance: &Instance) -> Option<String> {
    let when = Ago::since(instance.last_played?).played();
    Some(match instance.play_time {
        0 => when,
        seconds => tr!(M::PlayedInTotal, when = when, time = play_time(seconds)),
    })
}

/// The size of an instance's icon in its sidebar row, in logical pixels.
const ROW_ICON: f32 = 26.;

/// How dark the scrim over an instance's icon is, in percent: dark enough that the
/// white Play icon reads over a white icon too.
const SCRIM: u32 = 60;
const SCRIM_INK: theme::Color = theme::Color::rgb(0x000000);

/// What shows over an instance's icon while the pointer is on its row, as the Play
/// bar would show for it: Play, which starts the instance as the bar's Play does
/// (and doesn't open the row); loading dots while it gets ready or starts; nothing
/// while it runs (Stop stays on the Play bar, away from a stray click) or while
/// Play waits.
///
/// Play is drawn on every row, invisible until the pointer is on it, so assistive
/// technology finds it as a button named "Play {name}". It isn't a tab stop: Up and
/// Down still step one row at a time, and the Play bar is one Tab away.
fn icon_overlay(
    shell: &LauncherShell,
    instance: &Instance,
    row: &SharedString,
    hovered: bool,
    size: f32,
    cx: &mut Context<LauncherShell>,
) -> Option<AnyElement> {
    let target = shell.play_target(&instance.id, cx);
    let scrim = |id: SharedString| {
        div()
            .id(id)
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .rounded(px(size * 0.2))
            .bg(SCRIM_INK.opacity(SCRIM))
    };
    match target.state {
        PlayButtonState::Ready => {
            let button_id = SharedString::from(format!("play-{row}"));
            let id = instance.id.clone();
            let joins = target.joins;
            let selector = button_id.to_string();
            let button = scrim(button_id.clone())
                .debug_selector(move || selector.clone())
                .accessible(
                    Role::Button,
                    button_id.clone(),
                    tr!(M::PlayNamed, name = instance.name),
                )
                .when_some(target.caption, |this, caption| {
                    this.aria_description(caption)
                })
                .child(Icon::Play.render(px(size * 0.5), colors::WHITE))
                // assistive technology's Click, whether Play shows or not
                .on_a11y_action(AccessibleAction::Click, {
                    let (id, shell) = (id.clone(), cx.entity().downgrade());
                    move |_, _, cx| {
                        shell
                            .update(cx, |this, cx| this.press_play(&id, joins, cx))
                            .ok();
                    }
                })
                // A click plays only once Play shows: until the shell has seen the
                // pointer on the row (after a scroll, say), it opens the row.
                .when(hovered, |this| {
                    this.cursor_pointer()
                        .hover(|style| style.bg(SCRIM_INK.opacity(SCRIM + 15)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            // Play, not the row's own click, which opens the instance.
                            cx.stop_propagation();
                            this.press_play(&id, joins, cx);
                        }))
                });
            // Fades in and out with the pointer.
            Some(
                crate::components::motion::Animated::new(
                    ElementId::Name(format!("{button_id}-shown").into()),
                    hovered,
                    button,
                    |this, t| this.opacity(t),
                )
                .into_any_element(),
            )
        }
        // A click on the dots opens the instance, where its progress shows.
        PlayButtonState::Preparing { .. } if hovered => Some(
            scrim(format!("preparing-{row}").into())
                .child(crate::components::loading_dots(colors::WHITE))
                .into_any_element(),
        ),
        PlayButtonState::Preparing { .. }
        | PlayButtonState::Playing
        | PlayButtonState::Disabled => None,
    }
}

/// One instance: its icon, name, version and loader (or when it was played), and a
/// dot while it runs. When it was played shows on hover; so does Play, over its icon.
fn instance_row(
    shell: &LauncherShell,
    instance: &Instance,
    place: RowPlace,
    cx: &mut Context<LauncherShell>,
) -> AnyElement {
    let instances = shell.instances.read(cx);
    let selected = instances.selected().map(|i| i.id.clone());
    let on_instance_page = shell.page == Page::Instance;
    let id = instance.id.clone();
    let active = instances.is_active(&id);
    let running = instances.is_running(&id);
    let is_selected = on_instance_page && selected.as_deref() == Some(id.as_str());
    let icon = crate::instances::appearance::instance_icon(instances, instance, px(ROW_ICON));
    let detail = match (place, instance.last_played) {
        (RowPlace::LastPlayed, Some(when)) => Ago::since(when).label(),
        _ => match instance.loader.kind() {
            None => instance.game_version.clone(),
            Some(kind) => format!("{} · {}", instance.game_version, kind.label()),
        },
    };
    let tooltip: Option<SharedString> = played_summary(instance).map(Into::into);
    let element_id = match place {
        RowPlace::List => format!("instance-{id}"),
        RowPlace::LastPlayed => format!("last-played-{id}"),
    };
    // The skin can show the instance's Play background faintly behind its row, under
    // a strong scrim so the name stays readable.
    let banner = crate::skin::look(cx)
        .spec
        .banner
        .in_sidebar
        .then(|| instances.sidebar_banner(&id))
        .flatten();
    let banner_readability = crate::skin::look(cx).spec.banner.readability;
    let mut description: String = if instances.is_launching(&id) {
        format!("{detail} · {}", t(M::Launching))
    } else if running {
        format!("{detail} · {}", t(M::Playing))
    } else {
        detail.clone()
    };
    let linked = linked_to(shell, &id, cx);
    if let Some(linked) = &linked {
        description = format!("{description} · {linked}");
    }
    let element_id = SharedString::from(element_id);
    let hovered = shell.sidebar_hover() == Some(&element_id);
    let overlay = icon_overlay(shell, instance, &element_id, hovered, ROW_ICON, cx);
    let hover_row = element_id.clone();
    let selector = element_id.to_string();
    div()
        .id(element_id.clone())
        .debug_selector(move || selector.clone())
        .accessible(Role::ListItem, element_id, instance.name.clone())
        .aria_selected(is_selected)
        .aria_description(SharedString::from(description))
        .keyboard_focus()
        .on_hover(cx.listener(move |this, hovered: &bool, _, cx| {
            this.hover_sidebar_row(&hover_row, *hovered, cx)
        }))
        .relative()
        .overflow_hidden()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .w_full()
        .px_2()
        .py_1()
        .rounded(radius::MD)
        .border_1()
        .border_color(colors::TRANSPARENT)
        .cursor_pointer()
        .hover(|style| style.bg(crate::skin::sidebar_inks().fill))
        .when(is_selected, |this| {
            this.bg(crate::skin::sidebar_inks().fill)
                .border_color(crate::theme::ink::border())
        })
        .when_some(banner, |this, (banner, backdrop)| {
            this.child(
                img(banner)
                    .absolute()
                    .inset_0()
                    .size_full()
                    .object_fit(ObjectFit::Cover)
                    .opacity(0.45),
            )
            // as dark (or light) as the row's text needs over this image
            .child(div().absolute().inset_0().bg(crate::skin::row_backing(
                backdrop,
                &crate::skin::sidebar_inks(),
                banner_readability,
            )))
        })
        .child(
            div()
                .relative()
                .flex_none()
                .size(px(ROW_ICON))
                .child(icon)
                .children(overlay)
                .when(linked.is_some(), |this| {
                    this.child(linked_badge(px(13.), px(-4.), cx))
                }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .font_weight(weights::BOLD.into())
                        .text_color(crate::skin::sidebar_inks().primary)
                        .child(instance.name.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(crate::skin::sidebar_inks().muted)
                        .child(detail),
                ),
        )
        .when(active, |this| {
            this.child(
                div()
                    .flex_none()
                    .size(px(8.))
                    .rounded_full()
                    .bg(if running {
                        colors::GREEN_200
                    } else {
                        colors::YELLOW
                    }),
            )
        })
        .when_some(tooltip, |this, tooltip| {
            this.tooltip(crate::components::text_tooltip(tooltip))
        })
        .on_press(cx.listener(move |this, _, _, cx| this.select_instance(&id, cx)))
        .into_any_element()
}

/// "Linked to Survival SMP on Peter's core", for an instance linked to a server (one
/// on a Lodestone core, or a friend's shared server). Its icon carries Lodestone
/// Host's mark, and assistive technology reads this.
fn linked_to(shell: &LauncherShell, id: &str, cx: &Context<LauncherShell>) -> Option<String> {
    let link = shell.servers.read(cx).instance_link(id)?;
    Some(tr!(
        M::LinkedTo,
        server = link.server_name,
        core = link.core_name
    ))
}

/// The Lodestone Host mark on the bottom right corner of a linked instance's icon,
/// `size` wide, `inset` in from the corner (negative: out past it). A dark round
/// backing keeps it readable over any icon.
fn linked_badge(
    size: gpui::Pixels,
    inset: gpui::Pixels,
    cx: &Context<LauncherShell>,
) -> impl IntoElement + use<> {
    div()
        .absolute()
        .right(inset)
        .bottom(inset)
        .size(size + px(4.))
        .flex()
        .items_center()
        .justify_center()
        .rounded_full()
        .bg(colors::GRAY_900.opacity(90))
        .child(img(crate::images::mark(false, cx)).size(size))
}

/// A group's name as the sidebar shows it: the automatic groups' names in the
/// current language, the user's own as they are. The name itself stays the group's
/// key (for folding it away).
fn group_label(name: &str) -> String {
    match AutoGroup::ALL
        .into_iter()
        .find(|group| group.name() == name)
    {
        Some(AutoGroup::Servers) => t(M::GroupServers).to_string(),
        Some(AutoGroup::Fabric) => tr!(M::GroupModded, loader = "Fabric"),
        Some(AutoGroup::Forge) => tr!(M::GroupModded, loader = "Forge"),
        Some(AutoGroup::NeoForge) => tr!(M::GroupModded, loader = "NeoForge"),
        Some(AutoGroup::Quilt) => tr!(M::GroupModded, loader = "Quilt"),
        Some(AutoGroup::Vanilla) => t(M::Vanilla).to_string(),
        Some(AutoGroup::Imported) => t(M::GroupImported).to_string(),
        None => name.to_string(),
    }
}

/// A group's heading, which folds the group away.
fn group_header(
    name: &str,
    count: usize,
    collapsed: bool,
    cx: &mut Context<LauncherShell>,
) -> AnyElement {
    let group = name.to_string();
    let caret = Icon::CaretDown.render(px(10.), crate::skin::sidebar_inks().icon);
    let caret = if collapsed {
        caret.with_transformation(Transformation::rotate(radians(
            -std::f32::consts::FRAC_PI_2,
        )))
    } else {
        caret
    };
    let header_id = SharedString::from(format!("group-{name}"));
    let toggle = {
        let group = group.clone();
        let shell = cx.entity().downgrade();
        move |_: &mut gpui::Window, cx: &mut gpui::App| {
            let group = group.clone();
            shell
                .update(cx, |this, cx| {
                    this.instances
                        .update(cx, |instances, cx| instances.toggle_group(&group, cx))
                })
                .ok();
        }
    };
    div()
        .id(header_id.clone())
        .accessible(
            Role::Button,
            header_id,
            format!("{} ({count})", group_label(name)),
        )
        .expandable(!collapsed, toggle)
        .keyboard_focus()
        .flex()
        .flex_row()
        .items_center()
        .gap_1p5()
        .mt_2()
        .px_1()
        .py_0p5()
        .rounded(radius::MD)
        .cursor_pointer()
        .hover(|style| style.bg(crate::skin::sidebar_inks().fill))
        .child(caret)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .text_color(crate::skin::sidebar_inks().secondary)
                .child(group_label(name).to_uppercase()),
        )
        .child(
            div()
                .text_size(theme::text_size(text::CAPTION))
                .font_weight(weights::BOLD.into())
                .text_color(crate::skin::sidebar_inks().faint)
                .child(count.to_string()),
        )
        .on_press(cx.listener(move |this, _, _, cx| {
            this.instances
                .update(cx, |instances, cx| instances.toggle_group(&group, cx))
        }))
        .into_any_element()
}

/// The instance played last, under "Last played"; then the instances: those
/// without a group, then each group under a heading that folds it. Instances
/// without a group of their own are in automatic groups when that setting is on.
fn instance_list(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let list: Vec<Instance> = shell.instances.read(cx).list().to_vec();
    let last = last_played(&list).cloned();
    let groups: Vec<(Option<String>, Vec<Instance>)> = shell
        .instances
        .read(cx)
        .grouped()
        .into_iter()
        .map(|(name, members)| (name, members.into_iter().cloned().collect()))
        .collect();
    let last_played_row =
        last.map(|instance| instance_row(shell, &instance, RowPlace::LastPlayed, cx));
    let mut rows: Vec<AnyElement> = Vec::new();
    for (group, members) in &groups {
        if let Some(name) = group {
            let collapsed = shell.instances.read(cx).is_group_collapsed(name);
            rows.push(group_header(name, members.len(), collapsed, cx));
            if collapsed {
                continue;
            }
        }
        for instance in members {
            rows.push(instance_row(shell, instance, RowPlace::List, cx));
        }
    }
    let jobs: Vec<(
        u64,
        SharedString,
        std::sync::Arc<launcher_core::content::install::InstallProgress>,
    )> = shell
        .instances
        .read(cx)
        .imports()
        .iter()
        .map(|job| (job.id, job.name.clone(), job.progress.clone()))
        .collect();
    let imports: Vec<AnyElement> = jobs
        .into_iter()
        .map(|(id, name, progress)| import_row(id, name, &progress, cx))
        .collect();
    let empty = list.is_empty() && imports.is_empty();
    div()
        .flex()
        .flex_col()
        .gap_y_1()
        .mx_1()
        .px_1()
        .when_some(last_played_row, |this, row| {
            this.child(section_label(t(M::LastPlayed)))
                .child(row)
                .child(div().h_2())
        })
        .child(section_label(t(M::Instances)))
        .children(rows)
        .children(imports)
        .when(empty, |this| {
            this.child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_4()
                    .mt_2()
                    .py_4()
                    .px_6()
                    .rounded(radius::XL)
                    .border_2()
                    .border_dashed()
                    .border_color(colors::GRAY_FADED.opacity(10))
                    .child(Icon::Expand.render(px(28.), crate::skin::sidebar_inks().faint))
                    .child(
                        div()
                            .text_center()
                            .italic()
                            .font_weight(weights::BOLD.into())
                            .text_color(crate::skin::sidebar_inks().faint)
                            .child(t(M::NoInstancesYet)),
                    ),
            )
        })
        .child(
            div().w_full().pt_1().child(
                Button::new("new-instance", t(M::NewInstance))
                    .variant(ButtonVariant::Ghost)
                    .ink(
                        crate::skin::sidebar_inks().ghost,
                        crate::skin::sidebar_inks().fill,
                    )
                    .icon(Icon::SquarePlus)
                    .align(Align::Start)
                    .full_width()
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_new_instance(window, cx)),
                    ),
            ),
        )
        .child(
            div().w_full().pb_8().child(
                Button::new("import-instances", t(M::ImportInstances))
                    .variant(ButtonVariant::Ghost)
                    .ink(
                        crate::skin::sidebar_inks().ghost,
                        crate::skin::sidebar_inks().fill,
                    )
                    .icon(Icon::Download)
                    .align(Align::Start)
                    .full_width()
                    .on_click(cx.listener(|this, _, window, cx| this.open_import(window, cx))),
            ),
        )
}

/// A small uppercase heading, like the dashboard's "ALL INSTANCES".
fn section_label(label: &'static str) -> impl IntoElement {
    div()
        .truncate()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::SNUG))
        .font_weight(weights::BOLD.into())
        .text_color(crate::skin::sidebar_inks().faint)
        .child(label.to_uppercase())
}

/// A page link in the footer: Servers or Settings.
fn page_link(
    id: &'static str,
    label: &'static str,
    icon: Glyph,
    page: Page,
    open: bool,
    cx: &mut Context<LauncherShell>,
) -> impl IntoElement {
    div()
        .id(id)
        .accessible(Role::Link, id, label)
        .keyboard_focus()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .px_2()
        .py_1p5()
        .rounded(radius::MD)
        .cursor_pointer()
        .font_weight(weights::BOLD.into())
        .map(|this| {
            if open {
                this.bg(crate::skin::sidebar_inks().fill)
                    .text_color(crate::skin::sidebar_inks().primary)
            } else {
                this.text_color(crate::skin::sidebar_inks().secondary)
                    .hover(|style| {
                        style
                            .bg(crate::skin::sidebar_inks().fill)
                            .text_color(crate::skin::sidebar_inks().primary)
                    })
            }
        })
        .child(icon.render(px(14.), crate::skin::sidebar_inks().icon))
        .child(div().min_w_0().truncate().child(label))
        .on_press(cx.listener(move |this, _, _, cx| {
            this.show(if open { Page::Instance } else { page }, cx)
        }))
}

/// Servers and Settings, pinned to the bottom, and the version. As tall as the
/// Play bar beside it, so their top lines meet.
fn footer(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let settings_open = shell.settings_open();
    let servers_open = shell.page == Page::Servers;
    div()
        .flex()
        .flex_col()
        .flex_none()
        .justify_center()
        .gap_1()
        .w_full()
        .min_h(px(super::play::BOTTOM_BAR_HEIGHT))
        .px_2()
        .py_2()
        .border_t_1()
        .border_color(crate::theme::ink::border())
        .child(page_link(
            "servers",
            t(M::Servers),
            Glyph::HostMark(crate::images::mark(false, cx)),
            Page::Servers,
            servers_open,
            cx,
        ))
        .child(
            div()
                .id("settings")
                .accessible(Role::Button, "settings", t(M::Settings))
                .keyboard_focus()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_2()
                .py_1p5()
                .rounded(radius::MD)
                .cursor_pointer()
                .font_weight(weights::BOLD.into())
                .map(|this| {
                    if settings_open {
                        this.bg(crate::skin::sidebar_inks().fill)
                            .text_color(crate::skin::sidebar_inks().primary)
                    } else {
                        this.text_color(crate::skin::sidebar_inks().secondary)
                            .hover(|style| {
                                style
                                    .bg(crate::skin::sidebar_inks().fill)
                                    .text_color(crate::skin::sidebar_inks().primary)
                            })
                    }
                })
                .child(Icon::Gear.render(px(14.), crate::skin::sidebar_inks().icon))
                .child(div().min_w_0().truncate().child(t(M::Settings)))
                .on_press(
                    cx.listener(|this, _, window, cx| this.open_settings(None, Some(window), cx)),
                ),
        )
        .child(
            div()
                .px_2()
                .text_size(theme::text_size(text::CAPTION))
                .line_height(theme::line_height(text::CAPTION, leading::NORMAL))
                .font_weight(weights::MEDIUM.into())
                .text_color(crate::skin::sidebar_inks().faint)
                .truncate()
                .child(RichText::new(tr!(
                    M::AppVersion,
                    version = env!("CARGO_PKG_VERSION")
                ))),
        )
}

#[cfg(all(test, feature = "screenshot"))]
mod tests {
    use gpui::{Entity, Modifiers, TestAppContext, VisualTestContext};

    use super::*;

    /// The launcher's window with the sidebar's sample instances, signed out, so a
    /// Play stops at "add an account first" and nothing is downloaded or started.
    fn open(cx: &mut TestAppContext) -> (Entity<LauncherShell>, &mut VisualTestContext) {
        cx.update(crate::components::init);
        let (shell, cx) = cx.add_window_view(|_, cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            let instances = crate::preview::instances("sidebar-groups", &accounts, cx);
            LauncherShell::new(accounts, instances, cx)
        });
        cx.run_until_parked();
        (shell, cx)
    }

    fn id_of(shell: &Entity<LauncherShell>, name: &str, cx: &mut VisualTestContext) -> String {
        shell.read_with(cx, |shell, cx| {
            let instances = shell.instances.read(cx);
            let instance = instances.list().iter().find(|i| i.name == name);
            instance.expect("a sample instance").id.clone()
        })
    }

    fn selected(shell: &Entity<LauncherShell>, cx: &mut VisualTestContext) -> Option<String> {
        shell.read_with(cx, |shell, cx| {
            shell.instances.read(cx).selected().map(|i| i.id.clone())
        })
    }

    /// Whether the instance was asked to play: Play starts its log.
    fn asked_to_play(shell: &Entity<LauncherShell>, id: &str, cx: &mut VisualTestContext) -> bool {
        shell.read_with(cx, |shell, cx| {
            let instances = shell.instances.read(cx);
            instances.log(id).is_some_and(|log| !log.rows().is_empty())
        })
    }

    fn hovered(shell: &Entity<LauncherShell>, cx: &mut VisualTestContext) -> Option<String> {
        shell.read_with(cx, |shell, _| {
            shell.sidebar_hover().map(|row| row.to_string())
        })
    }

    /// Where the element with the debug selector `selector` was drawn.
    fn bounds_of(selector: &str, cx: &mut VisualTestContext) -> gpui::Bounds<gpui::Pixels> {
        // GPUI looks selectors up by a `&'static str`; a test leaks a few.
        let selector: &'static str = Box::leak(selector.to_string().into_boxed_str());
        cx.debug_bounds(selector)
            .unwrap_or_else(|| panic!("{selector} drawn"))
    }

    /// A point on the row's name, away from its icon.
    fn on_name(bounds: gpui::Bounds<gpui::Pixels>) -> gpui::Point<gpui::Pixels> {
        gpui::point(bounds.right() - px(40.), bounds.center().y)
    }

    #[test]
    fn play_over_the_icon_plays_without_opening_the_row() {
        let mut cx = TestAppContext::single();
        let (shell, cx) = open(&mut cx);

        // The rest of a row opens it, as before, without playing.
        let opened = id_of(&shell, "All the mods", cx);
        let bounds = bounds_of(&format!("instance-{opened}"), cx);
        cx.simulate_click(on_name(bounds), Modifiers::none());
        assert_eq!(selected(&shell, cx).as_deref(), Some(opened.as_str()));
        assert!(!asked_to_play(&shell, &opened, cx), "the row only opens");

        // The pointer on another row shows Play over its icon.
        let id = id_of(&shell, "Fabulously Optimized", cx);
        let row = format!("instance-{id}");
        let bounds = bounds_of(&row, cx);
        cx.simulate_mouse_move(on_name(bounds), None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(hovered(&shell, cx).as_deref(), Some(row.as_str()));

        // Play plays that instance, and its row stays closed.
        let play = bounds_of(&format!("play-{row}"), cx);
        assert!(bounds.contains(&play.center()), "over the row's icon");
        cx.simulate_click(play.center(), Modifiers::none());
        assert!(asked_to_play(&shell, &id, cx), "Play plays the instance");
        assert_eq!(selected(&shell, cx).as_deref(), Some(opened.as_str()));
    }

    #[test]
    fn play_hides_when_the_pointer_leaves_the_row() {
        let mut cx = TestAppContext::single();
        let (shell, cx) = open(&mut cx);
        let id = id_of(&shell, "Fabulously Optimized", cx);
        let row = format!("instance-{id}");
        let bounds = bounds_of(&row, cx);
        cx.simulate_mouse_move(on_name(bounds), None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(hovered(&shell, cx).as_deref(), Some(row.as_str()));
        // over the main area
        cx.simulate_mouse_move(gpui::point(px(900.), px(400.)), None, Modifiers::none());
        cx.run_until_parked();
        assert_eq!(hovered(&shell, cx), None);
    }
}
