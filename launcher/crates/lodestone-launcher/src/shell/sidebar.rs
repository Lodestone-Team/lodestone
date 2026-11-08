use gpui::{
    AnyElement, Context, ExternalPaths, IntoElement, Render, SharedString, Transformation, Window,
    div, prelude::*, px, radians,
};
use launcher_core::instances::{AutoGroup, Instance, last_played};

use super::{LauncherShell, Page, account_box};
use crate::components::KeyboardFocus;
use crate::{
    components::{Align, Button, ButtonVariant, Icon},
    i18n::{ShellMsg as M, format, t, tr},
    instances::{Ago, play_time},
    theme::{self, colors, layout, leading, radius, text, weights},
};

/// The left sidebar: the account box at the top, the instance list and "New
/// instance" in the middle, and Settings pinned at the bottom.
pub fn render(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    div()
        .id("sidebar")
        .flex()
        .flex_col()
        .flex_none()
        .w(layout::SIDEBAR_WIDTH)
        .h_full()
        .bg(colors::GRAY_850)
        .border_r_1()
        .border_color(colors::FADE_700)
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
                        .border_color(colors::BLUE_200.opacity(60))
                        .bg(colors::BLUE.opacity(8))
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
        .border_color(colors::BLUE_200.opacity(30))
        .bg(colors::BLUE.opacity(8))
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
                        .text_color(colors::GRAY_300)
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
                .text_color(colors::GRAY_500)
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
                            .bg(colors::BLUE_200)
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

/// A small label shown while the pointer rests on a row.
struct TextTooltip(SharedString);

impl Render for TextTooltip {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .px_2()
            .py_1()
            .rounded(radius::MD)
            .border_1()
            .border_color(colors::FADE_700)
            .bg(colors::GRAY_850)
            .text_size(theme::text_size(text::SMALL))
            .font_weight(weights::MEDIUM.into())
            .text_color(colors::GRAY_300)
            .child(self.0.clone())
    }
}

/// "Played 2 hours ago · 2 h 5 min in total", for an instance that has been played.
fn played_summary(instance: &Instance) -> Option<String> {
    let when = Ago::since(instance.last_played?).played();
    Some(match instance.play_time {
        0 => when,
        seconds => tr!(M::PlayedInTotal, when = when, time = play_time(seconds)),
    })
}

/// One instance: its icon, name, version and loader (or when it was played), and a
/// dot while it runs. When it was played shows on hover.
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
    let icon = crate::instances::appearance::instance_icon(instances, instance, px(26.));
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
    div()
        .id(SharedString::from(element_id))
        .keyboard_focus()
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
        .hover(|style| style.bg(colors::GRAY_800))
        .when(is_selected, |this| {
            this.bg(colors::GRAY_800).border_color(colors::FADE_700)
        })
        .child(icon)
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
                        .text_color(colors::GRAY_300)
                        .child(instance.name.clone()),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(theme::text_size(text::SMALL))
                        .text_color(colors::GRAY_500)
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
            this.tooltip(move |_, cx| cx.new(|_| TextTooltip(tooltip.clone())).into())
        })
        .on_click(cx.listener(move |this, _, _, cx| this.select_instance(&id, cx)))
        .into_any_element()
}

/// A group's name as the sidebar shows it: the automatic groups' names in the
/// current language, the user's own as they are. The name itself stays the group's
/// key (for folding it away).
fn group_label(name: &str) -> String {
    match AutoGroup::ALL
        .into_iter()
        .find(|group| group.name() == name)
    {
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
    let caret = Icon::CaretDown.render(px(10.), colors::GRAY_FADED.opacity(50));
    let caret = if collapsed {
        caret.with_transformation(Transformation::rotate(radians(
            -std::f32::consts::FRAC_PI_2,
        )))
    } else {
        caret
    };
    div()
        .id(SharedString::from(format!("group-{name}")))
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
        .hover(|style| style.bg(colors::GRAY_800))
        .child(caret)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(theme::text_size(text::SMALL))
                .font_weight(weights::BOLD.into())
                .text_color(colors::GRAY_400)
                .child(group_label(name).to_uppercase()),
        )
        .child(
            div()
                .text_size(theme::text_size(text::CAPTION))
                .font_weight(weights::BOLD.into())
                .text_color(colors::GRAY_FADED.opacity(30))
                .child(count.to_string()),
        )
        .on_click(cx.listener(move |this, _, _, cx| {
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
                    .child(Icon::Expand.render(px(28.), colors::GRAY_FADED.opacity(30)))
                    .child(
                        div()
                            .text_center()
                            .italic()
                            .font_weight(weights::BOLD.into())
                            .text_color(colors::GRAY_FADED.opacity(30))
                            .child(t(M::NoInstancesYet)),
                    ),
            )
        })
        .child(
            div().w_full().pt_1().child(
                Button::new("new-instance", t(M::NewInstance))
                    .variant(ButtonVariant::Ghost)
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
        .text_color(colors::GRAY_FADED.opacity(30))
        .child(label.to_uppercase())
}

/// A page link in the footer: Servers or Settings.
fn page_link(
    id: &'static str,
    label: &'static str,
    icon: Icon,
    page: Page,
    open: bool,
    cx: &mut Context<LauncherShell>,
) -> impl IntoElement {
    div()
        .id(id)
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
                this.bg(colors::GRAY_800).text_color(colors::GRAY_300)
            } else {
                this.text_color(colors::GRAY_400)
                    .hover(|style| style.bg(colors::GRAY_800).text_color(colors::GRAY_300))
            }
        })
        .child(icon.render(px(14.), colors::GRAY_FADED.opacity(50)))
        .child(div().min_w_0().truncate().child(label))
        .on_click(cx.listener(move |this, _, _, cx| {
            this.show(if open { Page::Instance } else { page }, cx)
        }))
}

/// Servers and Settings, pinned to the bottom, and the version.
fn footer(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let settings_open = shell.settings_open();
    let servers_open = shell.page == Page::Servers;
    div()
        .flex()
        .flex_col()
        .flex_none()
        .gap_1()
        .w_full()
        .px_2()
        .py_2()
        .border_t_1()
        .border_color(colors::FADE_700)
        .child(page_link(
            "servers",
            t(M::Servers),
            Icon::Cube,
            Page::Servers,
            servers_open,
            cx,
        ))
        .child(
            div()
                .id("settings")
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
                        this.bg(colors::GRAY_800).text_color(colors::GRAY_300)
                    } else {
                        this.text_color(colors::GRAY_400)
                            .hover(|style| style.bg(colors::GRAY_800).text_color(colors::GRAY_300))
                    }
                })
                .child(Icon::Gear.render(px(14.), colors::GRAY_FADED.opacity(50)))
                .child(div().min_w_0().truncate().child(t(M::Settings)))
                .on_click(
                    cx.listener(|this, _, window, cx| this.open_settings(None, Some(window), cx)),
                ),
        )
        .child(
            div()
                .px_2()
                .text_size(theme::text_size(text::CAPTION))
                .font_weight(weights::MEDIUM.into())
                .text_color(colors::GRAY_FADED.opacity(30))
                .truncate()
                .child(tr!(M::AppVersion, version = env!("CARGO_PKG_VERSION"))),
        )
}
