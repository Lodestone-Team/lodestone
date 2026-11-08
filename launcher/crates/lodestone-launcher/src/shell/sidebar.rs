use gpui::{
    AnyElement, Context, IntoElement, SharedString, Transformation, div, img, prelude::*, px,
    radians,
};
use launcher_core::instances::{Instance, grouped};

use super::{LauncherShell, Page, account_box};
use crate::{
    components::{Align, Button, ButtonVariant, Icon},
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
                .child(instance_list(shell, cx)),
        )
        .child(footer(shell, cx))
}

/// One instance: its icon, name, version and loader, and a dot while it runs.
fn instance_row(
    shell: &LauncherShell,
    instance: &Instance,
    cx: &mut Context<LauncherShell>,
) -> AnyElement {
    let instances = shell.instances.read(cx);
    let selected = instances.selected().map(|i| i.id.clone());
    let on_instance_page = shell.page == Page::Instance;
    let id = instance.id.clone();
    let active = instances.is_active(&id);
    let running = instances.is_running(&id);
    let is_selected = on_instance_page && selected.as_deref() == Some(id.as_str());
    let icon = match instances.icon(&id) {
        Some(image) => img(image),
        None => img("images/placeholder-cube.png"),
    };
    let detail = match instance.loader.kind() {
        None => instance.game_version.clone(),
        Some(kind) => format!("{} · {}", instance.game_version, kind.label()),
    };
    div()
        .id(SharedString::from(format!("instance-{id}")))
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
        .child(icon.flex_none().size(px(20.)).rounded(radius::DEFAULT))
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
        .on_click(cx.listener(move |this, _, _, cx| this.select_instance(&id, cx)))
        .into_any_element()
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
                .child(name.to_uppercase()),
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

/// The instances: those without a group, then each group under a heading that
/// folds it.
fn instance_list(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let list: Vec<Instance> = shell.instances.read(cx).list().to_vec();
    let mut rows: Vec<AnyElement> = Vec::new();
    for (group, members) in grouped(&list) {
        if let Some(name) = &group {
            let collapsed = shell.instances.read(cx).is_group_collapsed(name);
            rows.push(group_header(name, members.len(), collapsed, cx));
            if collapsed {
                continue;
            }
        }
        for instance in members {
            rows.push(instance_row(shell, instance, cx));
        }
    }
    let empty = list.is_empty();
    div()
        .flex()
        .flex_col()
        .gap_y_1()
        .mx_1()
        .px_1()
        .child(section_label("INSTANCES"))
        .children(rows)
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
                            .child("No instances yet."),
                    ),
            )
        })
        .child(
            div().w_full().pt_1().pb_8().child(
                Button::new("new-instance", "New instance...")
                    .variant(ButtonVariant::Ghost)
                    .icon(Icon::SquarePlus)
                    .align(Align::Start)
                    .full_width()
                    .on_click(
                        cx.listener(|this, _, window, cx| this.open_new_instance(window, cx)),
                    ),
            ),
        )
}

/// A small uppercase heading, like the dashboard's "ALL INSTANCES".
fn section_label(label: &'static str) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::SNUG))
        .font_weight(weights::BOLD.into())
        .text_color(colors::GRAY_FADED.opacity(30))
        .child(label)
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
        .child(label)
        .on_click(cx.listener(move |this, _, _, cx| {
            this.show(if open { Page::Instance } else { page }, cx)
        }))
}

/// Servers and Settings, pinned to the bottom, and the version.
fn footer(shell: &LauncherShell, cx: &mut Context<LauncherShell>) -> impl IntoElement {
    let settings_open = shell.page == Page::Settings;
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
            "Servers",
            Icon::Cube,
            Page::Servers,
            servers_open,
            cx,
        ))
        .child(
            div()
                .id("settings")
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
                .child("Settings")
                .on_click(cx.listener(move |this, _, _, cx| {
                    let page = if settings_open {
                        Page::Instance
                    } else {
                        Page::Settings
                    };
                    this.show(page, cx);
                })),
        )
        .child(
            div()
                .px_2()
                .text_size(theme::text_size(text::CAPTION))
                .font_weight(weights::MEDIUM.into())
                .text_color(colors::GRAY_FADED.opacity(30))
                .child(concat!("Lodestone Launcher v", env!("CARGO_PKG_VERSION"))),
        )
}
