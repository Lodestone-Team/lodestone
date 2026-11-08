//! How the Content tab looks: the section and sub-view switches, the installed lists,
//! the Shaders section's shader mod card, Browse, and a project's page.

use std::sync::Arc;

use gpui::{
    AnyElement, Context, ExternalPaths, IntoElement, ObjectFit, Render, RenderImage, SharedString,
    Window, div, img, prelude::*, px,
};
use launcher_core::{
    content::{
        ContentKind,
        listing::Item,
        packs::Compatibility,
        shaders::{ShaderLoader, ShaderModOffer},
    },
    instances::Loader,
};
use lodestone_minecraft::{
    modrinth::{ModrinthSearchHit, ModrinthSearchIndex, ModrinthVersionType, RawVersion},
    util::format_byte,
};

use super::{ContentView, Load, PAGE_SIZE, SubView};
use crate::{
    components::{Button, ButtonSize, ButtonVariant, Icon, IconButton, LoadingDots, Toggle},
    theme::{self, colors, fonts, leading, radius, text, weights},
};

/// `72230975` as `72.2M`.
pub fn compact(value: u64) -> String {
    match value {
        0..1_000 => value.to_string(),
        1_000..1_000_000 => format!("{:.1}k", value as f64 / 1_000.0),
        _ => format!("{:.1}M", value as f64 / 1_000_000.0),
    }
}

/// The project's page on modrinth.com.
pub fn modrinth_page(project_type: &str, slug: &str) -> String {
    format!("https://modrinth.com/{project_type}/{slug}")
}

fn muted(text: impl Into<SharedString>) -> gpui::Div {
    div().text_color(colors::GRAY_500).child(text.into())
}

fn small(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
        .child(text.into())
}

/// A small coloured label.
fn badge(label: impl Into<SharedString>, color: theme::Color) -> gpui::Div {
    div()
        .flex_none()
        .px_1p5()
        .rounded(radius::DEFAULT)
        .border_1()
        .border_color(color.opacity(40))
        .bg(color.opacity(12))
        .text_color(color)
        .text_size(theme::text_size(text::CAPTION))
        .font_weight(weights::BOLD.into())
        .child(label.into())
}

/// An icon image, or the cube placeholder.
fn icon_box(image: Option<Arc<RenderImage>>, size: f32) -> AnyElement {
    match image {
        Some(image) => img(image)
            .flex_none()
            .size(px(size))
            .rounded(radius::MD)
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        None => div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(size))
            .rounded(radius::MD)
            .bg(colors::GRAY_700)
            .child(Icon::Cube.render(px(size * 0.45), colors::GRAY_500))
            .into_any_element(),
    }
}

/// A row of pills; the selected one is filled.
fn pills(
    id: &'static str,
    labels: Vec<SharedString>,
    selected: usize,
    on_select: impl Fn(usize, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let on_select = std::rc::Rc::new(on_select);
    div()
        .id(id)
        .flex()
        .flex_row()
        .flex_none()
        .gap_1()
        .p_1()
        .rounded(radius::LG)
        .bg(colors::GRAY_900)
        .border_1()
        .border_color(colors::FADE_700)
        .children(labels.into_iter().enumerate().map(|(index, label)| {
            let on_select = on_select.clone();
            div()
                .id((id, index))
                .px_3()
                .py_1()
                .rounded(radius::MD)
                .font_weight(weights::BOLD.into())
                .cursor_pointer()
                .map(|this| {
                    if index == selected {
                        this.bg(colors::GRAY_700).text_color(colors::GRAY_300)
                    } else {
                        this.text_color(colors::GRAY_500)
                            .hover(|style| style.text_color(colors::GRAY_400))
                    }
                })
                .on_click(move |_, window, cx| on_select(index, window, cx))
                .child(label)
        }))
}

/// A box for a notice: the shader mod card, the vanilla card, errors.
fn card() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .rounded(radius::LG)
        .border_1()
        .border_color(colors::FADE_700)
        .bg(colors::GRAY_850)
}

fn heading(text_value: impl Into<SharedString>) -> gpui::Div {
    div()
        .font_family(fonts::HEADING)
        .text_size(theme::text_size(text::H3))
        .font_weight(weights::BOLD.into())
        .text_color(colors::GRAY_300)
        .child(text_value.into())
}

fn list_box() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .rounded(radius::LG)
        .overflow_hidden()
        .border_1()
        .border_color(colors::GRAY_FADED.opacity(30))
}

fn row_base(id: impl Into<gpui::ElementId>, first: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_2p5()
        .bg(colors::GRAY_800)
        .when(!first, |this| {
            this.border_t_1()
                .border_color(colors::GRAY_FADED.opacity(15))
        })
}

fn version_label(kind: ModrinthVersionType) -> (&'static str, theme::Color) {
    match kind {
        ModrinthVersionType::Release => ("Release", colors::GREEN_300),
        ModrinthVersionType::Beta => ("Beta", colors::YELLOW_300),
        ModrinthVersionType::Alpha => ("Alpha", colors::RED_200),
    }
}

/// "1.20.1, 1.20" with at most three versions.
fn short_list(values: &[String], max: usize) -> String {
    let mut shown: Vec<&str> = values.iter().take(max).map(String::as_str).collect();
    if values.len() > max {
        shown.push("…");
    }
    shown.join(", ")
}

impl ContentView {
    fn kind_label(&self, kind: ContentKind) -> SharedString {
        match self.section(kind).listing.ready() {
            Some(listing) if !listing.items.is_empty() => {
                format!("{} {}", kind.label(), listing.items.len()).into()
            }
            _ => kind.label().into(),
        }
    }

    fn target_label(&self, kind: ContentKind, cx: &gpui::App) -> String {
        let Some((_, _, target)) = self.context(cx) else {
            return String::new();
        };
        match kind {
            ContentKind::Mod => format!("{} / {}", target.game_version, target.loader.label()),
            ContentKind::ResourcePack => target.game_version.clone(),
            ContentKind::ShaderPack => format!(
                "{} / {}",
                target.game_version,
                self.shaders.loader.map_or("Iris", ShaderLoader::label)
            ),
        }
    }

    fn render_header(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = self.kind;
        let selected = ContentKind::ALL
            .iter()
            .position(|k| *k == kind)
            .unwrap_or(0);
        let labels: Vec<SharedString> = ContentKind::ALL
            .iter()
            .map(|k| self.kind_label(*k))
            .collect();
        let entity = cx.entity();
        let view = self.section(kind).view;
        let entity2 = entity.clone();
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap_3()
            .child(pills(
                "content-kinds",
                labels,
                selected,
                move |index, _, cx| {
                    entity.update(cx, |this, cx| this.select_kind(ContentKind::ALL[index], cx))
                },
            ))
            .child(pills(
                "content-views",
                vec!["Installed".into(), "Browse Modrinth".into()],
                if view == SubView::Installed { 0 } else { 1 },
                move |index, _, cx| {
                    let view = if index == 0 {
                        SubView::Installed
                    } else {
                        SubView::Browse
                    };
                    entity2.update(cx, |this, cx| this.set_view(kind, view, cx))
                },
            ))
    }

    fn render_banners(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let locked = self.locked(cx);
        let job = self.job.as_ref().map(|job| {
            let snapshot = job.progress.downloads.snapshot();
            let fraction = if snapshot.bytes_total > 0 {
                (snapshot.bytes_done as f32 / snapshot.bytes_total as f32).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let detail = if snapshot.files_total > 1 {
                format!(
                    "{} of {} files · {}",
                    snapshot.files_done.min(snapshot.files_total),
                    snapshot.files_total,
                    lodestone_minecraft::util::format_byte_download(
                        snapshot.bytes_done,
                        snapshot.bytes_total
                    )
                )
            } else {
                lodestone_minecraft::util::format_byte_download(
                    snapshot.bytes_done,
                    snapshot.bytes_total,
                )
            };
            (job.progress.label(), detail, fraction)
        });
        div()
            .flex()
            .flex_col()
            .gap_2()
            .when(locked, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded(radius::MD)
                        .border_1()
                        .border_color(colors::YELLOW_300.opacity(60))
                        .bg(colors::YELLOW_FADED.opacity(20))
                        .text_color(colors::WHITE)
                        .child(Icon::TriangleExclamation.render(px(14.), colors::YELLOW_300))
                        .child("The game is running. Stop it to change its content: it rewrites options.txt and the shader settings when it exits."),
                )
            })
            .when_some(job, |this, (label, detail, fraction)| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1p5()
                        .px_3()
                        .py_2()
                        .rounded(radius::MD)
                        .bg(colors::GRAY_850)
                        .border_1()
                        .border_color(colors::FADE_700)
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_3()
                                .child(LoadingDots)
                                .child(div().flex_1().font_weight(weights::BOLD.into()).child(label))
                                .child(small(detail).text_color(colors::GRAY_500))
                                .child(
                                    Button::new("cancel-install", "Cancel")
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Slim)
                                        .on_click(cx.listener(|this, _, _, cx| this.cancel_install(cx))),
                                ),
                        )
                        .child(
                            div()
                                .h(px(4.))
                                .w_full()
                                .rounded(radius::XS)
                                .bg(colors::GRAY_700)
                                .child(
                                    div()
                                        .h_full()
                                        .rounded(radius::XS)
                                        .bg(colors::BLUE_200)
                                        .w(gpui::relative(fraction)),
                                ),
                        ),
                )
            })
            .when_some(self.message.clone(), |this, (message, error)| {
                this.child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap_2()
                        .px_3()
                        .py_2()
                        .rounded(radius::MD)
                        .bg(if error {
                            colors::RED_FADED.opacity(15)
                        } else {
                            colors::GREEN_FADED.opacity(15)
                        })
                        .border_1()
                        .border_color(if error {
                            colors::RED_200.opacity(40)
                        } else {
                            colors::GREEN_300.opacity(40)
                        })
                        .child(
                            div()
                                .flex_1()
                                .whitespace_normal()
                                .text_color(if error { colors::RED_200 } else { colors::GREEN_300 })
                                .child(message),
                        )
                        .child(
                            IconButton::new("dismiss-message", Icon::Xmark)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.message = None;
                                    cx.notify();
                                })),
                        ),
                )
            })
    }

    /// The buttons over an installed list.
    fn render_toolbar(&mut self, kind: ContentKind, cx: &mut Context<Self>) -> impl IntoElement {
        let section = self.section(kind);
        let busy = self.locked(cx) || self.job.is_some();
        let count = section.items().len();
        let updates = section.updates.len();
        let checking = section.checking_updates;
        let online = self.modrinth(cx).is_some();
        let summary = match (kind, count) {
            (_, 0) => String::new(),
            (ContentKind::ResourcePack, _) => {
                let enabled = section.items().iter().filter(|i| i.enabled).count();
                format!("{count} resource packs, {enabled} enabled")
            }
            (ContentKind::Mod, _) => {
                let enabled = section.items().iter().filter(|i| i.enabled).count();
                format!("{count} mods, {enabled} enabled")
            }
            (ContentKind::ShaderPack, _) => format!("{count} shader packs"),
        };
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .text_color(colors::GRAY_500)
                    .font_weight(weights::BOLD.into())
                    .child(summary),
            )
            .when(updates > 0, |this| {
                this.child(
                    Button::new(
                        ("update-all", kind as usize),
                        format!("Update all ({updates})"),
                    )
                    .variant(ButtonVariant::Primary)
                    .icon(Icon::CircleUp)
                    .size(ButtonSize::Slim)
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| this.update_all(kind, cx))),
                )
            })
            .child(
                Button::new(("check-updates", kind as usize), "Check for updates")
                    .variant(ButtonVariant::Secondary)
                    .icon(Icon::CircleUp)
                    .size(ButtonSize::Slim)
                    .loading(checking)
                    .disabled(!online || count == 0)
                    .on_click(cx.listener(move |this, _, _, cx| this.check_updates(kind, cx))),
            )
            .child(
                Button::new(("add-file", kind as usize), "Add file…")
                    .variant(ButtonVariant::Secondary)
                    .icon(Icon::Upload)
                    .size(ButtonSize::Slim)
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| this.pick_files(kind, cx))),
            )
            .child(
                Button::new(("open-folder", kind as usize), "Open folder")
                    .variant(ButtonVariant::Ghost)
                    .icon(Icon::ArrowUpRightFromSquare)
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(move |this, _, _, cx| this.open_folder(kind, cx))),
            )
    }

    /// The icon of an installed item: its own, else its Modrinth project's.
    fn item_icon(
        &mut self,
        kind: ContentKind,
        item: &Item,
        cx: &mut Context<Self>,
    ) -> Option<Arc<RenderImage>> {
        if let Some(icon) = self.section(kind).icons.get(&item.file.file_name) {
            return Some(icon.clone());
        }
        let url = item.modrinth.as_ref()?.icon_url.clone()?;
        self.remote_icon(&url, cx)
    }

    /// One installed file.
    fn render_item(
        &mut self,
        kind: ContentKind,
        index: usize,
        item: &Item,
        first: bool,
        reorder: Option<(bool, bool)>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        // Shader rows sit inside the selectable row, which draws the background.
        let inner = kind == ContentKind::ShaderPack;
        let busy = self.locked(cx) || self.job.is_some();
        let name = item.file.file_name.clone();
        let icon = self.item_icon(kind, item, cx);
        let update = self.section(kind).updates.get(&name).cloned();
        let confirming = self.confirm_delete.as_ref() == Some(&(kind, name.clone()));
        let title = item.display_name();
        let version = item.display_version();
        let detail = if item.file.is_dir {
            format!("{name} · folder")
        } else {
            format!("{name} · {}", format_byte(item.file.size))
        };
        let warning = item
            .file
            .meta
            .problem
            .clone()
            .or_else(|| item.compatibility.warning().map(str::to_string));
        let page = item
            .modrinth
            .as_ref()
            .filter(|m| !m.slug.is_empty())
            .map(|m| {
                modrinth_page(
                    match kind {
                        ContentKind::Mod => "mod",
                        ContentKind::ResourcePack => "resourcepack",
                        ContentKind::ShaderPack => "shader",
                    },
                    &m.slug,
                )
            });
        let enabled = item.enabled;
        let compatibility = item.compatibility;
        let id_base = kind as usize * 100_000 + index;
        row_base(("content-row", id_base), first)
            .when(inner, |this| this.bg(colors::TRANSPARENT).pl_3())
            .child(icon_box(icon, 36.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(weights::BOLD.into())
                                    .text_color(if enabled || kind == ContentKind::ShaderPack {
                                        colors::GRAY_300
                                    } else {
                                        colors::WHITE.opacity(50)
                                    })
                                    .child(title),
                            )
                            .when_some(version, |this, version| {
                                this.child(
                                    small(version)
                                        .flex_none()
                                        .text_color(colors::WHITE.opacity(50)),
                                )
                            })
                            .when(
                                kind == ContentKind::ResourcePack
                                    && compatibility == Compatibility::Compatible,
                                |this| this.child(badge("Compatible", colors::GREEN_300)),
                            )
                            .when(item.compatibility.warning().is_some(), |this| {
                                this.child(badge(
                                    if compatibility == Compatibility::TooOld {
                                        "Older version"
                                    } else {
                                        "Newer version"
                                    },
                                    colors::YELLOW_300,
                                ))
                            })
                            .when(item.modrinth.is_none() && !item.file.is_dir, |this| {
                                this.child(badge("Not on Modrinth", colors::GRAY_500))
                            }),
                    )
                    .child(
                        small(detail)
                            .truncate()
                            .text_color(colors::WHITE.opacity(30)),
                    )
                    .when_some(
                        item.file
                            .meta
                            .description
                            .clone()
                            .filter(|_| kind == ContentKind::ResourcePack),
                        |this, description| {
                            this.child(small(description).truncate().text_color(colors::GRAY_400))
                        },
                    )
                    .when_some(warning, |this, warning| {
                        this.child(small(warning).text_color(colors::YELLOW_300))
                    }),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_none()
                    .items_center()
                    .gap_2()
                    .when_some(update, |this, update| {
                        let name = name.clone();
                        this.child(
                            Button::new(
                                ("update-one", id_base),
                                format!("Update to {}", update.version_number),
                            )
                            .variant(ButtonVariant::Primary)
                            .icon(Icon::CircleUp)
                            .size(ButtonSize::Slim)
                            .disabled(busy)
                            .on_click(
                                cx.listener(move |this, _, _, cx| this.update_one(kind, &name, cx)),
                            ),
                        )
                    })
                    .when_some(reorder, |this, (can_up, can_down)| {
                        let up = name.clone();
                        let down = name.clone();
                        this.child(
                            IconButton::new(("raise", id_base), Icon::ArrowUp)
                                .disabled(busy || !can_up)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.move_pack(up.clone(), true, cx)
                                })),
                        )
                        .child(
                            IconButton::new(("lower", id_base), Icon::ArrowDown)
                                .disabled(busy || !can_down)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.move_pack(down.clone(), false, cx)
                                })),
                        )
                    })
                    .when_some(page, |this, page| {
                        this.child(
                            IconButton::new(
                                ("modrinth-page", id_base),
                                Icon::ArrowUpRightFromSquare,
                            )
                            .on_click(move |_, _, cx| {
                                cx.stop_propagation();
                                cx.open_url(&page)
                            }),
                        )
                    })
                    .when(kind != ContentKind::ShaderPack, |this| {
                        let name = name.clone();
                        this.child(
                            Toggle::new(("enabled", id_base), enabled)
                                .disabled(busy)
                                .on_change(cx.listener(
                                    move |this, value: &bool, _, cx| match kind {
                                        ContentKind::Mod => {
                                            this.set_mod_enabled(name.clone(), *value, cx)
                                        }
                                        _ => this.set_pack_enabled(
                                            name.clone(),
                                            *value,
                                            compatibility,
                                            cx,
                                        ),
                                    },
                                )),
                        )
                    })
                    .map(|this| {
                        let name = name.clone();
                        if confirming {
                            this.child(
                                Button::new(("confirm-delete", id_base), "Move to Trash")
                                    .variant(ButtonVariant::Danger)
                                    .size(ButtonSize::Slim)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.delete(kind, name.clone(), cx)
                                    })),
                            )
                            .child(
                                Button::new(("cancel-delete", id_base), "Cancel")
                                    .variant(ButtonVariant::Ghost)
                                    .size(ButtonSize::Slim)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.cancel_delete(cx)
                                    })),
                            )
                        } else {
                            this.child(
                                IconButton::new(("delete", id_base), Icon::TrashCan)
                                    .disabled(busy)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.delete(kind, name.clone(), cx)
                                    })),
                            )
                        }
                    }),
            )
            .into_any_element()
    }

    fn render_empty(&self, kind: ContentKind, cx: &mut Context<Self>) -> AnyElement {
        let message = match kind {
            ContentKind::Mod => "No mods yet. Browse Modrinth, or drop .jar files here.",
            ContentKind::ResourcePack => {
                "No resource packs yet. Browse Modrinth, or drop .zip files or folders here."
            }
            ContentKind::ShaderPack => {
                "No shader packs yet. Browse Modrinth, or drop .zip files or folders here."
            }
        };
        div()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .py_12()
            .rounded(radius::XL)
            .border_2()
            .border_dashed()
            .border_color(colors::GRAY_FADED.opacity(10))
            .text_color(colors::GRAY_FADED.opacity(50))
            .font_weight(weights::BOLD.into())
            .child(message)
            .child(
                Button::new(("browse-empty", kind as usize), "Browse Modrinth")
                    .variant(ButtonVariant::Secondary)
                    .icon(Icon::MagnifyingGlass)
                    .on_click(
                        cx.listener(move |this, _, _, cx| this.set_view(kind, SubView::Browse, cx)),
                    ),
            )
            .into_any_element()
    }

    /// The card for vanilla instances: content that needs a mod loader.
    fn render_needs_loader(&mut self, what: &str, cx: &mut Context<Self>) -> AnyElement {
        self.ensure_mod_loader_offer(cx);
        let switching = self.is_switching_loader(cx);
        let busy = self.locked(cx) || self.job.is_some();
        let game_version = self
            .context(cx)
            .map(|(_, _, target)| target.game_version)
            .unwrap_or_default();
        let new_instance = Button::new("new-fabric-instance", "New Fabric instance…")
            .variant(ButtonVariant::Secondary)
            .icon(Icon::SquarePlus)
            .on_click(cx.listener(|this, _, _, cx| this.new_fabric_instance(cx)));
        let (text, buttons): (String, Option<AnyElement>) = match &self.mod_loader_offer {
            Load::Ready(Some(kind)) => {
                let label = kind.label();
                (
                    format!(
                        "This instance is vanilla Minecraft. {what} run through a mod loader such as {label}; switching keeps your worlds and settings, and {label} installs the next time you press Play."
                    ),
                    Some(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .child(
                                Button::new("switch-to-mod-loader", format!("Switch to {label}"))
                                    .variant(ButtonVariant::Primary)
                                    .loading(switching)
                                    .disabled(busy)
                                    .on_click(
                                        cx.listener(|this, _, _, cx| this.switch_to_mod_loader(cx)),
                                    ),
                            )
                            .child(new_instance)
                            .into_any_element(),
                    ),
                )
            }
            Load::Ready(None) => (
                format!(
                    "This instance is vanilla Minecraft {game_version}. {what} need a mod loader, and none that Lodestone Launcher supports runs on this version: Fabric starts at 1.14, and Forge has no build for it. A new instance on a newer version can use them."
                ),
                Some(new_instance.into_any_element()),
            ),
            Load::Failed(err) => (
                format!(
                    "This instance is vanilla Minecraft. {what} need a mod loader, but checking which one fits didn't work: {err}"
                ),
                Some(
                    Button::new("retry-mod-loader", "Try again")
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.mod_loader_offer = Load::NotLoaded;
                            this.ensure_mod_loader_offer(cx);
                        }))
                        .into_any_element(),
                ),
            ),
            Load::NotLoaded | Load::Loading => (
                format!(
                    "This instance is vanilla Minecraft. {what} run through a mod loader; checking which one runs on Minecraft {game_version}…"
                ),
                None,
            ),
        };
        card()
            .child(heading(format!("{what} need a mod loader")))
            .child(
                div()
                    .text_color(colors::GRAY_400)
                    .whitespace_normal()
                    .child(text),
            )
            .children(buttons)
            .into_any_element()
    }

    /// The Shaders section's card: the shader mod in use, or the one to install.
    fn render_shader_mod(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.locked(cx) || self.job.is_some();
        if let Some(loader) = self.shaders.loader {
            let using = match (
                &self.shaders.selection.pack,
                self.shaders.selection.internal,
            ) {
                (Some(pack), _) => format!("{} is using {pack}.", loader.label()),
                (None, true) => format!("{} is using its built-in shaders.", loader.label()),
                (None, false) => format!("{} is installed; shaders are off.", loader.label()),
            };
            return card()
                .flex_row()
                .items_center()
                .gap_3()
                .child(Icon::Check.render(px(16.), colors::GREEN_300))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .flex_1()
                        .child(div().font_weight(weights::BOLD.into()).child(using))
                        .child(small(format!(
                            "Pick a pack below. {} reads its choice from {} when the game starts.",
                            loader.label(),
                            loader.config_file()
                        ))
                        .text_color(colors::GRAY_500)),
                )
                .into_any_element();
        }
        let target_loader = self
            .context(cx)
            .map(|(_, _, t)| t.loader)
            .unwrap_or_default();
        if target_loader == Loader::None {
            return self.render_needs_loader("Shaders", cx);
        }
        let optifine_note = small(
            "OptiFine works too, but it isn't on Modrinth and its licence doesn't allow Lodestone to download it. If you prefer it, download it from optifine.net and add the jar with Add file… in Mods (Fabric also needs OptiFabric).",
        )
        .whitespace_normal()
        .text_color(colors::GRAY_500);
        let body: AnyElement = match &self.shader_offer {
            Load::NotLoaded | Load::Loading => div()
                .flex()
                .flex_row()
                .gap_2()
                .text_color(colors::GRAY_500)
                .child(LoadingDots)
                .child("Looking for a shader mod for this version on Modrinth")
                .into_any_element(),
            Load::Failed(err) => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(small(format!("Couldn't ask Modrinth: {err}")).text_color(colors::RED_200))
                .child(
                    Button::new("retry-shader-offer", "Try again")
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.load_shader_offer(cx))),
                )
                .into_any_element(),
            Load::Ready(ShaderModOffer::Install { loader, version }) => {
                let label = format!("Install {} {}", loader.label(), version.version_number);
                let with = match loader {
                    ShaderLoader::Iris => " It comes with Sodium, which Iris needs.",
                    ShaderLoader::Oculus => " It comes with Embeddium, which Oculus needs.",
                    ShaderLoader::OptiFine => "",
                };
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div().text_color(colors::GRAY_400).whitespace_normal().child(format!(
                            "Minecraft can't load shader packs by itself; a shader mod does that. {} runs on this instance's {}.{with}",
                            loader.label(),
                            target_loader.label()
                        )),
                    )
                    .child(
                        div().flex().child(
                            Button::new("install-shader-mod", label)
                                .variant(ButtonVariant::Primary)
                                .icon(Icon::Download)
                                .disabled(busy)
                                .on_click(cx.listener(|this, _, _, cx| this.install_shader_mod(cx))),
                        ),
                    )
                    .into_any_element()
            }
            Load::Ready(ShaderModOffer::Unavailable { reason }) => div()
                .text_color(colors::GRAY_400)
                .whitespace_normal()
                .child(format!(
                    "Minecraft can't load shader packs by itself; a shader mod does that. {reason} Fabric instances can always use Iris."
                ))
                .into_any_element(),
            Load::Ready(ShaderModOffer::NeedsModLoader) => return self.render_needs_loader("Shaders", cx),
        };
        card()
            .child(heading("Shaders need a shader mod"))
            .child(body)
            .child(optifine_note)
            .into_any_element()
    }

    fn render_installed(&mut self, kind: ContentKind, cx: &mut Context<Self>) -> AnyElement {
        let target = self.context(cx).map(|(_, _, t)| t);
        let listing = self.section(kind).listing.clone();
        let mut column = div().flex().flex_col().gap_3();
        if kind == ContentKind::ShaderPack {
            column = column.child(self.render_shader_mod(cx));
        }
        if kind == ContentKind::Mod && target.as_ref().is_some_and(|t| t.loader == Loader::None) {
            column = column.child(self.render_needs_loader("Mods", cx));
        }
        column = column.child(self.render_toolbar(kind, cx));
        let listing = match listing {
            Load::NotLoaded | Load::Loading => {
                return column
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .py_6()
                            .justify_center()
                            .text_color(colors::GRAY_500)
                            .child(LoadingDots)
                            .child(format!("Reading the {} folder", kind.dir())),
                    )
                    .into_any_element();
            }
            Load::Failed(err) => {
                return column
                    .child(
                        small(format!("Couldn't read the {} folder: {err}", kind.dir()))
                            .text_color(colors::RED_200),
                    )
                    .into_any_element();
            }
            Load::Ready(listing) => listing,
        };
        if let Some(error) = &listing.lookup_error {
            column = column.child(
                small(format!(
                    "Some files couldn't be matched to Modrinth: {error}"
                ))
                .text_color(colors::GRAY_500),
            );
        }
        if let Some(error) = &listing.options_error {
            column = column.child(small(error.clone()).text_color(colors::YELLOW_300));
        }
        if listing.items.is_empty() {
            return column.child(self.render_empty(kind, cx)).into_any_element();
        }
        match kind {
            ContentKind::Mod => {
                let mut rows = list_box();
                for (index, item) in listing.items.iter().enumerate() {
                    rows = rows.child(self.render_item(kind, index, item, index == 0, None, cx));
                }
                column = column.child(rows);
            }
            ContentKind::ResourcePack => {
                let mut enabled: Vec<(usize, &Item)> = listing
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(_, i)| i.priority.is_some())
                    .collect();
                enabled.sort_by_key(|(_, i)| i.priority);
                let others: Vec<(usize, &Item)> = listing
                    .items
                    .iter()
                    .enumerate()
                    .filter(|(_, i)| i.priority.is_none())
                    .collect();
                if !enabled.is_empty() {
                    column = column.child(
                        small("ENABLED · the top pack wins where packs overlap")
                            .font_weight(weights::BOLD.into())
                            .text_color(colors::GRAY_500),
                    );
                    let count = enabled.len();
                    let mut rows = list_box();
                    for (position, (index, item)) in enabled.into_iter().enumerate() {
                        let reorder = Some((position > 0, position + 1 < count));
                        rows = rows.child(self.render_item(
                            kind,
                            index,
                            item,
                            position == 0,
                            reorder,
                            cx,
                        ));
                    }
                    column = column.child(rows);
                }
                if !others.is_empty() {
                    column = column.child(
                        small("AVAILABLE")
                            .font_weight(weights::BOLD.into())
                            .text_color(colors::GRAY_500),
                    );
                    let mut rows = list_box();
                    for (position, (index, item)) in others.into_iter().enumerate() {
                        rows = rows.child(self.render_item(
                            kind,
                            index,
                            item,
                            position == 0,
                            None,
                            cx,
                        ));
                    }
                    column = column.child(rows);
                }
            }
            ContentKind::ShaderPack => {
                let loader = self.shaders.loader;
                let busy = self.locked(cx) || self.job.is_some() || loader.is_none();
                let selected_none = self.shaders.selection.pack.is_none();
                let mut rows = list_box().child(
                    row_base("shaders-off", true)
                        .when(!busy, |this| {
                            this.cursor_pointer()
                                .hover(|style| style.bg(colors::GRAY_750))
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.select_shader(None, cx)),
                                )
                        })
                        .child(radio(selected_none && loader.is_some()))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .child(div().font_weight(weights::BOLD.into()).child("Shaders off"))
                                .child(
                                    small("Minecraft's normal look.")
                                        .text_color(colors::WHITE.opacity(30)),
                                ),
                        ),
                );
                for (index, item) in listing.items.iter().enumerate() {
                    let name = item.file.file_name.clone();
                    let row = self.render_item(kind, index, item, true, None, cx);
                    let selected = item.enabled;
                    rows = rows.child(
                        div()
                            .id(("shader-select", index))
                            .flex()
                            .flex_row()
                            .items_center()
                            .bg(colors::GRAY_800)
                            .border_t_1()
                            .border_color(colors::GRAY_FADED.opacity(15))
                            .when(selected, |this| this.bg(colors::BLUE.opacity(15)))
                            .when(!busy, |this| {
                                this.cursor_pointer()
                                    .hover(|style| style.bg(colors::GRAY_750))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_shader(Some(name.clone()), cx)
                                    }))
                            })
                            .child(div().pl_4().child(radio(selected)))
                            .child(div().flex_1().min_w_0().child(row)),
                    );
                }
                column = column.child(rows);
            }
        }
        column.into_any_element()
    }

    fn render_browse(&mut self, kind: ContentKind, cx: &mut Context<Self>) -> AnyElement {
        if self.modrinth(cx).is_none()
            && matches!(self.section(kind).browse.results, Load::NotLoaded)
            && self.section(kind).browse.project.is_none()
        {
            return muted("Browsing Modrinth needs the network.").into_any_element();
        }
        if self.section(kind).browse.project.is_some() {
            return self.render_project(kind, cx);
        }
        let browse = &self.section(kind).browse;
        let search = browse.search.clone();
        let only = browse.only_compatible;
        let index = browse.index;
        let offset = browse.offset;
        let results = browse.results.clone();
        let target_label = self.target_label(kind, cx);
        let indexes = [
            (ModrinthSearchIndex::Relevance, "Relevance"),
            (ModrinthSearchIndex::Downloads, "Downloads"),
            (ModrinthSearchIndex::Follows, "Follows"),
            (ModrinthSearchIndex::Newest, "Newest"),
            (ModrinthSearchIndex::Updated, "Updated"),
        ];
        let selected_index = indexes.iter().position(|(i, _)| *i == index).unwrap_or(0);
        let entity = cx.entity();
        let installed = self.section(kind).installed_projects();
        let mut column = div()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_2()
                    .child(Icon::MagnifyingGlass.render(px(14.), colors::GRAY_500))
                    .child(div().flex_1().child(search)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                Toggle::new(("only-compatible", kind as usize), only).on_change(
                                    cx.listener(move |this, value: &bool, _, cx| {
                                        this.set_only_compatible(kind, *value, cx)
                                    }),
                                ),
                            )
                            .child(
                                small(format!("Only show {}s for {target_label}", kind.noun()))
                                    .text_color(colors::GRAY_400),
                            ),
                    )
                    .child(pills(
                        "search-index",
                        indexes
                            .iter()
                            .map(|(_, label)| SharedString::from(*label))
                            .collect(),
                        selected_index,
                        move |i, _, cx| {
                            let index = indexes[i].0;
                            entity.update(cx, |this, cx| this.set_index(kind, index, cx))
                        },
                    )),
            );
        if kind == ContentKind::ShaderPack && self.shaders.loader.is_none() {
            column = column.child(self.render_shader_mod(cx));
        }
        match results {
            Load::NotLoaded | Load::Loading => {
                column = column.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_center()
                        .gap_2()
                        .py_6()
                        .text_color(colors::GRAY_500)
                        .child(LoadingDots)
                        .child("Searching Modrinth"),
                );
            }
            Load::Failed(err) => {
                column = column.child(small(err).text_color(colors::RED_200));
            }
            Load::Ready(results) => {
                if results.hits.is_empty() {
                    column = column.child(muted("Nothing on Modrinth matches."));
                }
                let mut cards = div().flex().flex_col().gap_2();
                for (i, hit) in results.hits.iter().enumerate() {
                    let installed = installed.contains_key(&hit.project_id);
                    cards = cards.child(self.render_hit(kind, i, hit, installed, cx));
                }
                column = column.child(cards);
                let pages = results.total_hits.div_ceil(PAGE_SIZE).max(1);
                let page = offset / PAGE_SIZE + 1;
                if results.total_hits > PAGE_SIZE {
                    column = column.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .justify_center()
                            .gap_3()
                            .child(
                                Button::new(("previous-page", kind as usize), "Previous")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Slim)
                                    .disabled(offset == 0)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_page(kind, offset.saturating_sub(PAGE_SIZE), cx)
                                    })),
                            )
                            .child(
                                small(format!("Page {page} of {pages}"))
                                    .text_color(colors::GRAY_500),
                            )
                            .child(
                                Button::new(("next-page", kind as usize), "Next")
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Slim)
                                    .disabled(page >= pages)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.set_page(kind, offset + PAGE_SIZE, cx)
                                    })),
                            ),
                    );
                }
            }
        }
        column.into_any_element()
    }

    fn render_hit(
        &mut self,
        kind: ContentKind,
        index: usize,
        hit: &ModrinthSearchHit,
        installed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let icon = hit
            .icon_url
            .as_deref()
            .and_then(|url| self.remote_icon(url, cx));
        let id = hit.project_id.clone();
        let client_badge = kind == ContentKind::Mod && hit.client_side == "optional";
        div()
            .id(("hit", index))
            .flex()
            .flex_row()
            .items_center()
            .gap_3()
            .p_3()
            .rounded(radius::LG)
            .bg(colors::GRAY_800)
            .border_1()
            .border_color(colors::FADE_700)
            .cursor_pointer()
            .hover(|style| style.bg(colors::GRAY_750))
            .on_click(
                cx.listener(move |this, _, _, cx| this.open_project(kind, id.clone(), None, cx)),
            )
            .child(icon_box(icon, 48.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap_0p5()
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(weights::BOLD.into())
                                    .text_color(colors::GRAY_300)
                                    .child(hit.title.clone()),
                            )
                            .child(
                                small(format!("by {}", hit.author))
                                    .flex_none()
                                    .text_color(colors::WHITE.opacity(50)),
                            )
                            .when(installed, |this| {
                                this.child(badge("Installed", colors::GREEN_300))
                            })
                            .when(client_badge, |this| {
                                this.child(badge("Optional on servers", colors::GRAY_500))
                            }),
                    )
                    .child(
                        small(hit.description.clone())
                            .text_color(colors::GRAY_400)
                            .line_clamp(2),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .items_end()
                    .child(
                        small(format!("{} downloads", compact(hit.downloads)))
                            .text_color(colors::GRAY_400),
                    )
                    .child(
                        small(format!("{} follows", compact(hit.follows)))
                            .text_color(colors::GRAY_500),
                    ),
            )
            .into_any_element()
    }

    fn render_project(&mut self, kind: ContentKind, cx: &mut Context<Self>) -> AnyElement {
        let Some(view) = self.section(kind).browse.project.as_ref() else {
            return div().into_any_element();
        };
        let project = view.project.clone();
        let versions = view.versions.clone();
        let show_all = view.show_all;
        let with_dependencies = view.with_dependencies;
        let confirm_force = view.confirm_force.clone();
        let linked = view.linked_version.clone();
        let back = Button::new(("back", kind as usize), "Back to results")
            .variant(ButtonVariant::Ghost)
            .icon(Icon::ArrowLeft)
            .size(ButtonSize::Slim)
            .on_click(cx.listener(move |this, _, _, cx| this.close_project(kind, cx)));
        let project = match project {
            Load::Ready(project) => project,
            Load::Failed(err) => {
                return div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().flex().child(back))
                    .child(small(err).text_color(colors::RED_200))
                    .into_any_element();
            }
            _ => {
                return div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(div().flex().child(back))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .text_color(colors::GRAY_500)
                            .child(LoadingDots)
                            .child("Loading the project"),
                    )
                    .into_any_element();
            }
        };
        let icon = project
            .icon_url
            .as_deref()
            .and_then(|url| self.remote_icon(url, cx));
        let page = modrinth_page(&project.project_type, &project.slug);
        let mut column = div()
            .flex()
            .flex_col()
            .gap_4()
            .child(div().flex().child(back))
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_start()
                    .gap_4()
                    .child(icon_box(icon, 72.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                div()
                                    .font_family(fonts::TITLE)
                                    .text_size(theme::text_size(text::H2))
                                    .font_weight(weights::BOLD.into())
                                    .text_color(colors::GRAY_300)
                                    .child(project.title.clone()),
                            )
                            .child(
                                div()
                                    .text_color(colors::GRAY_400)
                                    .whitespace_normal()
                                    .child(project.description.clone()),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .flex_wrap()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        small(format!(
                                            "{} downloads · {} followers",
                                            compact(project.downloads),
                                            compact(project.followers)
                                        ))
                                        .text_color(colors::GRAY_500),
                                    )
                                    .children(
                                        project
                                            .categories
                                            .iter()
                                            .take(6)
                                            .map(|c| badge(c.clone(), colors::BLUE_200)),
                                    ),
                            ),
                    )
                    .child(
                        Button::new(("open-modrinth", kind as usize), "View on Modrinth")
                            .variant(ButtonVariant::Secondary)
                            .icon(Icon::ArrowUpRightFromSquare)
                            .size(ButtonSize::Slim)
                            .on_click(move |_, _, cx| cx.open_url(&page)),
                    ),
            );
        if kind == ContentKind::ShaderPack && self.shaders.loader.is_none() {
            column = column.child(self.render_shader_mod(cx));
        }
        let versions = match versions {
            Load::Ready(versions) => versions,
            Load::Failed(err) => {
                return column
                    .child(small(err).text_color(colors::RED_200))
                    .into_any_element();
            }
            _ => {
                return column
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .text_color(colors::GRAY_500)
                            .child(LoadingDots)
                            .child("Loading versions"),
                    )
                    .into_any_element();
            }
        };
        let target = self.context(cx).map(|(_, _, t)| t);
        let installed_versions = self
            .section(kind)
            .installed_projects()
            .remove(&project.id)
            .unwrap_or_default();
        let mut shown: Vec<(RawVersion, Option<String>)> = versions
            .into_iter()
            .map(|v| {
                let problem = target.as_ref().and_then(|t| t.incompatibility(kind, &v));
                (v, problem)
            })
            .filter(|(v, problem)| {
                show_all || problem.is_none() || linked.as_deref() == Some(v.id.as_str())
            })
            .collect();
        if let Some(linked) = &linked {
            shown.sort_by_key(|(v, _)| &v.id != linked);
        }
        let total = shown.len();
        let busy = self.locked(cx) || self.job.is_some();
        column = column.child(
            div()
                .flex()
                .flex_row()
                .flex_wrap()
                .items_center()
                .gap_4()
                .child(heading("Versions"))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            Toggle::new(("show-all", kind as usize), show_all).on_change(
                                cx.listener(move |this, value: &bool, _, cx| {
                                    if let Some(view) = this.project_mut(kind) {
                                        view.show_all = *value;
                                    }
                                    cx.notify();
                                }),
                            ),
                        )
                        .child(
                            small("Show versions for other loaders and Minecraft versions")
                                .text_color(colors::GRAY_400),
                        ),
                )
                .when(kind == ContentKind::Mod, |this| {
                    this.child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap_2()
                            .child(
                                Toggle::new(("with-deps", kind as usize), with_dependencies)
                                    .on_change(cx.listener(move |this, value: &bool, _, cx| {
                                        if let Some(view) = this.project_mut(kind) {
                                            view.with_dependencies = *value;
                                        }
                                        cx.notify();
                                    })),
                            )
                            .child(
                                small("Also install required dependencies")
                                    .text_color(colors::GRAY_400),
                            ),
                    )
                }),
        );
        if shown.is_empty() {
            return column
                .child(muted(format!("No version of {} fits this instance. Turn on \"Show versions…\" to see them all.", project.title)))
                .into_any_element();
        }
        let mut rows = list_box();
        for (index, (version, problem)) in shown.into_iter().take(40).enumerate() {
            let (type_label, type_color) = version_label(version.version_type);
            let is_installed = installed_versions.contains(&version.id);
            let size = version.primary_file().map_or(0, |f| f.size);
            let date = version
                .date_published
                .get(..10)
                .unwrap_or_default()
                .to_string();
            let forcing = confirm_force.as_deref() == Some(version.id.as_str());
            let version_for_click = version.clone();
            let project_for_click = project.clone();
            let id = version.id.clone();
            let button = if is_installed {
                badge("Installed", colors::GREEN_300).into_any_element()
            } else if problem.is_some() && !forcing {
                Button::new(("install-anyway", index), "Install anyway…")
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Slim)
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(view) = this.project_mut(kind) {
                            view.confirm_force = Some(id.clone());
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            } else {
                let label = match (
                    kind,
                    forcing,
                    self.shaders.loader,
                    self.shader_offer.ready(),
                ) {
                    (_, true, _, _) => "Install it anyway".to_string(),
                    (
                        ContentKind::ShaderPack,
                        _,
                        None,
                        Some(ShaderModOffer::Install { loader, .. }),
                    ) => {
                        format!("Install with {}", loader.label())
                    }
                    _ => "Install".to_string(),
                };
                Button::new(("install", index), label)
                    .variant(if forcing {
                        ButtonVariant::Danger
                    } else {
                        ButtonVariant::Primary
                    })
                    .icon(Icon::Download)
                    .size(ButtonSize::Slim)
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.install_version(
                            kind,
                            version_for_click.clone(),
                            Some(project_for_click.clone()),
                            forcing,
                            cx,
                        )
                    }))
                    .into_any_element()
            };
            rows = rows.child(
                row_base(("version-row", index), index == 0)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .flex_row()
                                    .items_center()
                                    .gap_2()
                                    .child(
                                        div()
                                            .truncate()
                                            .font_weight(weights::BOLD.into())
                                            .child(version.version_number.clone()),
                                    )
                                    .child(badge(type_label, type_color)),
                            )
                            .child(
                                small(format!(
                                    "{} · {} · {date} · {}",
                                    short_list(&version.game_versions, 3),
                                    short_list(&version.loaders, 3),
                                    format_byte(size)
                                ))
                                .truncate()
                                .text_color(colors::WHITE.opacity(40)),
                            )
                            .when_some(problem, |this, problem| {
                                this.child(small(problem).text_color(colors::YELLOW_300))
                            }),
                    )
                    .child(button),
            );
        }
        column = column.child(rows);
        if total > 40 {
            column = column.child(
                small(format!("{} more versions are on Modrinth.", total - 40))
                    .text_color(colors::GRAY_500),
            );
        }
        column.into_any_element()
    }
}

/// A round selection mark.
fn radio(selected: bool) -> impl IntoElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(16.))
        .rounded(px(8.))
        .border_2()
        .border_color(if selected {
            colors::BLUE_200
        } else {
            colors::GRAY_600
        })
        .when(selected, |this| {
            this.child(div().size(px(6.)).rounded(px(3.)).bg(colors::BLUE_200))
        })
}

impl Render for ContentView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let kind = self.kind;
        // Every section, for the counts in the switch; the Shaders card and shader
        // searches also need to know the shader mod.
        for kind in ContentKind::ALL {
            self.ensure_loaded(kind, cx);
        }
        let view = self.section(kind).view;
        if view == SubView::Browse
            && matches!(self.section(kind).browse.results, Load::NotLoaded)
            && self.section(kind).browse.project.is_none()
        {
            self.search(kind, cx);
        }
        let body = match view {
            SubView::Installed => self.render_installed(kind, cx),
            SubView::Browse => self.render_browse(kind, cx),
        };
        let accepts_drop = view == SubView::Installed;
        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .child(self.render_header(cx))
            .child(self.render_banners(cx))
            .child(
                div()
                    .id(("content-body", kind as usize))
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .rounded(radius::LG)
                    .border_2()
                    .border_color(colors::TRANSPARENT)
                    .when(accepts_drop, |this| {
                        this.drag_over::<ExternalPaths>(|style, _, _, _| {
                            style
                                .border_color(colors::BLUE_200.opacity(60))
                                .bg(colors::BLUE.opacity(8))
                        })
                        .on_drop(cx.listener(
                            move |this, paths: &ExternalPaths, _, cx| {
                                this.add_files(kind, paths.paths().to_vec(), cx)
                            },
                        ))
                    })
                    .child(div().pb_4().child(body)),
            )
    }
}
