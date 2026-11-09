//! How the Content tab looks: the kinds and the search over the list, the installed
//! lists (filtered by the search), the Modrinth results below them while searching,
//! the Shaders section's shader mod card, and a project's page.

use crate::skin::Skinned;
use crate::theme::px;
use launcher_core::ui_skin::SurfaceId;
use std::sync::Arc;

use gpui::{
    AnyElement, Context, ExternalPaths, IntoElement, ObjectFit, Render, RenderImage, SharedString,
    Window, div, img, list, prelude::*,
};
use launcher_core::{
    mc_text,
    content::{
        ContentKind,
        listing::Item,
        packs::Compatibility,
        shaders::{ShaderLoader, ShaderModOffer},
    },
    instances::Loader,
};
use lodestone_minecraft::modrinth::{
    ModrinthSearchHit, ModrinthSearchIndex, ModrinthVersionType, RawVersion,
};

use super::{ContentView, Load, PAGE_SIZE, ProjectTab};
use crate::components::KeyboardFocus;
use crate::i18n::{CommonMsg, ContentMsg as M, ContentPlural as P, format, t, tr, trn};
use crate::{
    components::{
        Button, ButtonSize, ButtonVariant, Icon, IconButton, ListRow, LoadingDots, RichText,
        Toggle,
        focus::Dismiss,
        settings::{popover, popover_panel, select_button},
    },
    theme::{self, colors, fonts, leading, radius, text, weights},
};

/// A count of downloads, follows or followers: the plural `exact` for a small count,
/// else `shortened` with the count shortened ("1.2k downloads").
pub(crate) fn counted(value: u64, exact: P, shortened: M) -> String {
    let short = format::compact(value);
    if short == format::number(value) {
        trn!(exact, value)
    } else {
        tr!(shortened, count = short)
    }
}

/// A content kind's name, as the section switch shows it.
fn kind_name(kind: ContentKind) -> &'static str {
    t(match kind {
        ContentKind::Mod => M::KindMods,
        ContentKind::ResourcePack => M::KindResourcePacks,
        ContentKind::ShaderPack => M::KindShaders,
    })
}

/// An instance's loader in a label: its name, or "Vanilla".
fn loader_name(loader: Loader) -> &'static str {
    match loader {
        Loader::None => t(M::Vanilla),
        other => other.label(),
    }
}

/// The project's page on modrinth.com.
pub fn modrinth_page(project_type: &str, slug: &str) -> String {
    format!("https://modrinth.com/{project_type}/{slug}")
}

pub(crate) fn muted(text: impl Into<SharedString>) -> gpui::Div {
    div()
        .text_color(crate::theme::ink::muted())
        .child(text.into())
}

pub(crate) fn small(text: impl Into<SharedString>) -> gpui::Div {
    small_box().child(text.into())
}

/// [`small`] text that can be selected and copied, such as a file name or a message.
pub(crate) fn small_rich(text: RichText) -> gpui::Div {
    small_box().child(text)
}

fn small_box() -> gpui::Div {
    div()
        .text_size(theme::text_size(text::SMALL))
        .line_height(theme::line_height(text::SMALL, leading::NORMAL))
}

/// A small coloured label.
pub(crate) fn badge(label: impl Into<SharedString>, color: theme::Color) -> gpui::Div {
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
pub(crate) fn icon_box(image: Option<Arc<RenderImage>>, size: f32) -> AnyElement {
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
pub(crate) fn pills(
    id: &'static str,
    labels: Vec<SharedString>,
    selected: usize,
    on_select: impl Fn(usize, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    let on_select = std::rc::Rc::new(on_select);
    crate::components::ArrowGroup::new(id)
        .flex()
        .flex_row()
        .flex_none()
        .gap_1()
        .p_1()
        .rounded(radius::LG)
        .bg(colors::GRAY_900)
        .border_1()
        .border_color(crate::theme::ink::border())
        .children(labels.into_iter().enumerate().map(|(index, label)| {
            let on_select = on_select.clone();
            let item = div()
                .id((id, index))
                .keyboard_focus()
                .px_3()
                .py_1()
                .rounded(radius::MD)
                .font_weight(weights::BOLD.into())
                .cursor_pointer()
                .when(index != selected, |this| {
                    this.hover(|style| style.text_color(crate::theme::ink::secondary()))
                })
                .on_click({
                    let (pills, label) = (id, label.clone());
                    move |_, window, cx| {
                        crate::components::trace_ui(
                            "ui.select",
                            "pills",
                            &gpui::ElementId::Name(pills.into()),
                            serde_json::json!({ "index": index, "label": label.as_ref() }),
                        );
                        on_select(index, window, cx)
                    }
                })
                .child(label);
            // The raised background fades over to the chosen pill.
            crate::components::motion::Animated::new(
                (id, index + 10_000),
                index == selected,
                item,
                |this, t| {
                    this.bg(colors::GRAY_700.opacity(0).mix(colors::GRAY_700, t))
                        .text_color(colors::GRAY_500.mix(colors::GRAY_300, t))
                },
            )
        }))
}

/// A box for a notice: the shader mod card, the vanilla card, errors.
pub(crate) fn card() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .gap_2()
        .p_4()
        .rounded(radius::LG)
        .border_1()
        .border_color(crate::theme::ink::border())
        .bg(colors::GRAY_850)
        .surface(SurfaceId::Card)
}

pub(crate) fn heading(text_value: impl Into<SharedString>) -> gpui::Div {
    div()
        .font_family(fonts::heading())
        .text_size(theme::text_size(text::H3))
        .font_weight(weights::BOLD.into())
        .text_color(crate::theme::ink::text())
        .child(text_value.into())
}

pub(crate) fn list_box() -> gpui::Div {
    div()
        .flex()
        .flex_col()
        .rounded(radius::LG)
        .overflow_hidden()
        .border_1()
        .border_color(colors::GRAY_FADED.opacity(30))
}

/// What one entry of the list is. See `ContentView::installed_plan`.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Entry {
    ShaderMod,
    NeedsLoader,
    Loading,
    Failed(SharedString),
    /// A note above the list; `true` for a warning.
    Note(String, bool),
    Empty,
    Heading(Heading),
    /// "Installed · 0": nothing installed matches the search.
    NoInstalledMatch,
    /// The Shaders list's "Off" row.
    ShadersOff,
    /// A file's row: `first` and `last` in its box.
    Row {
        index: usize,
        file: String,
        first: bool,
        last: bool,
        reorder: Option<(bool, bool)>,
    },
    /// "On Modrinth · n", with the sort and the compatibility filter.
    ModrinthHeading,
    ModrinthLoading,
    ModrinthFailed(SharedString),
    /// No network: Modrinth can't be searched.
    ModrinthOffline,
    ModrinthNothing,
    /// A Modrinth result, by its index in the results.
    Hit {
        index: usize,
        id: String,
        first: bool,
        last: bool,
    },
    /// Previous and next page of the Modrinth results.
    Pager,
    /// "On CurseForge · n", with its sort.
    CurseForgeHeading,
    CurseForgeLoading,
    CurseForgeFailed(SharedString),
    CurseForgeNothing,
    /// A CurseForge result, by its index in the results.
    CfHit {
        index: usize,
        id: u64,
        first: bool,
        last: bool,
    },
    /// Previous and next page of the CurseForge results.
    CfPager,
}

/// A heading over installed rows, with how many there are.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Heading {
    Installed(usize),
    /// Resource packs in use, the top one winning.
    Enabled(usize),
    Available(usize),
}

/// Whether an installed file matches what was typed: its name, file name or
/// Modrinth title.
fn item_matches(item: &Item, query: &str) -> bool {
    let query = query.to_lowercase();
    // Without formatting codes, which names of packs often have.
    let plain = |text: &str| mc_text::strip(text).to_lowercase();
    plain(&item.display_name()).contains(&query)
        || plain(&item.file.file_name).contains(&query)
        || item
            .modrinth
            .as_ref()
            .is_some_and(|m| m.title.to_lowercase().contains(&query))
}

/// A row drawn as part of a `list_box`: the box's sides, and its top or bottom edge
/// on the first or last row.
fn in_box(row: AnyElement, first: bool, last: bool) -> AnyElement {
    div()
        .overflow_hidden()
        .border_x_1()
        .border_color(colors::GRAY_FADED.opacity(30))
        .when(first, |this| this.border_t_1().rounded_t(radius::LG))
        .when(last, |this| this.border_b_1().rounded_b(radius::LG))
        .child(row)
        .into_any_element()
}

pub(crate) fn row_base(id: impl Into<gpui::ElementId>, first: bool) -> gpui::Stateful<gpui::Div> {
    div()
        .id(id)
        .flex()
        .flex_row()
        .items_center()
        .gap_3()
        .px_4()
        .py_2p5()
        .bg(colors::GRAY_800)
        .surface(SurfaceId::Row)
        .when(!first, |this| {
            this.border_t_1()
                .border_color(colors::GRAY_FADED.opacity(15))
        })
}

pub(crate) fn version_label(kind: ModrinthVersionType) -> (&'static str, theme::Color) {
    match kind {
        ModrinthVersionType::Release => (t(M::Release), colors::GREEN_300),
        ModrinthVersionType::Beta => (t(M::Beta), colors::YELLOW_300),
        ModrinthVersionType::Alpha => (t(M::Alpha), colors::RED_200),
    }
}

/// "1.20.1, 1.20" with at most three versions.
pub(crate) fn short_list(values: &[String], max: usize) -> String {
    let mut shown: Vec<&str> = values.iter().take(max).map(String::as_str).collect();
    if values.len() > max {
        shown.push("…");
    }
    format::list(&shown)
}

impl ContentView {
    fn kind_label(&self, kind: ContentKind) -> SharedString {
        match self.section(kind).listing.ready() {
            Some(listing) if !listing.items.is_empty() => format!(
                "{} {}",
                kind_name(kind),
                format::number(listing.items.len() as u64)
            )
            .into(),
            _ => kind_name(kind).into(),
        }
    }

    pub(crate) fn target_label(&self, kind: ContentKind, cx: &gpui::App) -> String {
        let Some((_, _, target)) = self.context(cx) else {
            return String::new();
        };
        match kind {
            ContentKind::Mod => format!("{} / {}", target.game_version, loader_name(target.loader)),
            ContentKind::ResourcePack => target.game_version.clone(),
            ContentKind::ShaderPack => format!(
                "{} / {}",
                target.game_version,
                self.shaders.loader.map_or("Iris", ShaderLoader::label)
            ),
        }
    }

    /// The kinds, the search over the list, "Update all" and the "⋯" menu.
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
        let section = self.section(kind);
        let updates = section.updates.len();
        let checking = section.checking_updates;
        let search = section.browse.search.clone();
        let busy = self.locked(cx) || self.job.is_some();
        // One row: the kinds, then the search with Update all and the menu. When
        // they don't fit beside the kinds, the search group wraps onto a line of its
        // own, across the whole width.
        let has_button = updates > 0 || checking;
        let tools = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .flex_1()
            // The narrowest the search group gets beside the kinds before it wraps.
            .min_w(px(if has_button { 440. } else { 300. }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    // Escape empties the search first; an empty one lets it through.
                    .on_action(cx.listener(move |this, _: &Dismiss, _, cx| {
                        if this.query(kind, cx).is_empty() {
                            cx.propagate();
                        } else {
                            this.clear_search(kind, cx);
                        }
                    }))
                    .child(search),
            )
            .map(|this| {
                if updates > 0 {
                    this.child(
                        Button::new(
                            ("update-all", kind as usize),
                            tr!(M::UpdateAll, count = format::number(updates as u64)),
                        )
                        .variant(ButtonVariant::Primary)
                        .icon(Icon::CircleUp)
                        .disabled(busy)
                        .on_click(cx.listener(move |this, _, _, cx| this.update_all(kind, cx))),
                    )
                } else if checking {
                    this.child(
                        Button::new(("checking-updates", kind as usize), t(M::CheckForUpdates))
                            .variant(ButtonVariant::Secondary)
                            .loading(true)
                            .disabled(true),
                    )
                } else {
                    this
                }
            })
            .child(self.render_menu(kind, cx));
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_x_2()
            .gap_y_3()
            .child(div().flex_none().child(pills(
                "content-kinds",
                labels,
                selected,
                move |index, _, cx| {
                    entity.update(cx, |this, cx| this.select_kind(ContentKind::ALL[index], cx))
                },
            )))
            .child(tools)
    }

    /// "⋯": Check for updates, Add file… and Open folder.
    fn render_menu(&mut self, kind: ContentKind, cx: &mut Context<Self>) -> impl IntoElement {
        let can_check = (self.modrinth(cx).is_some() || self.curseforge(cx).is_some())
            && !self.section(kind).items().is_empty();
        div()
            .flex()
            .flex_col()
            .items_end()
            .child(
                IconButton::new(("content-menu", kind as usize), Icon::Ellipsis).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.menu_open = !this.menu_open;
                        cx.notify();
                    }),
                ),
            )
            .when(self.menu_open, |this| {
                let close = cx.listener(|this, _: &(), _, cx| {
                    this.menu_open = false;
                    cx.notify();
                });
                let mut panel = popover_panel("content-menu-list")
                    .w(px(220.))
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.menu_open = false;
                        cx.notify();
                    }));
                if can_check {
                    panel = panel.child(
                        ListRow::new("menu-check-updates", t(M::CheckForUpdates))
                            .icon(Icon::CircleUp)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.menu_open = false;
                                this.check_updates(kind, cx);
                            })),
                    );
                }
                panel = panel
                    .child(
                        ListRow::new("menu-add-file", t(M::AddFile))
                            .icon(Icon::Upload)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.menu_open = false;
                                this.pick_files(kind, cx);
                            })),
                    )
                    .child(
                        ListRow::new("menu-open-folder", t(CommonMsg::OpenFolder))
                            .icon(Icon::ArrowUpRightFromSquare)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.menu_open = false;
                                this.open_folder(kind, cx);
                                cx.notify();
                            })),
                    );
                // In a box of its own under the button: the popover starts where its
                // parent does.
                this.child(div().child(popover(
                    "content-menu-popover",
                    panel,
                    move |window, cx| close(&(), window, cx),
                )))
            })
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
            let bytes = tr!(
                M::BytesProgress,
                done = format::bytes(snapshot.bytes_done),
                total = format::bytes(snapshot.bytes_total)
            );
            let detail = if snapshot.files_total > 1 {
                format!(
                    "{} · {bytes}",
                    tr!(
                        M::FilesProgress,
                        done = format::number(snapshot.files_done.min(snapshot.files_total)),
                        total = format::number(snapshot.files_total)
                    )
                )
            } else {
                bytes
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
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .whitespace_normal()
                                .child(t(M::GameRunning)),
                        ),
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
                        .border_color(crate::theme::ink::border())
                        .child(
                            div()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_3()
                                .child(LoadingDots)
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .truncate()
                                        .font_weight(weights::BOLD.into())
                                        .child(label),
                                )
                                .child(
                                    small(detail)
                                        .flex_none()
                                        .text_color(crate::theme::ink::muted()),
                                )
                                .child(
                                    Button::new("cancel-install", t(CommonMsg::Cancel))
                                        .variant(ButtonVariant::Ghost)
                                        .size(ButtonSize::Slim)
                                        .on_click(
                                            cx.listener(|this, _, _, cx| this.cancel_install(cx)),
                                        ),
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
                                        .bg(crate::theme::accent::light())
                                        .surface(launcher_core::ui_skin::SurfaceId::Progress)
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
                                .text_color(if error {
                                    colors::RED_200
                                } else {
                                    colors::GREEN_300
                                })
                                .child(RichText::minecraft(message)),
                        )
                        .child(IconButton::new("dismiss-message", Icon::Xmark).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.message = None;
                                cx.notify();
                            }),
                        )),
                )
            })
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
        let url = item
            .modrinth
            .as_ref()
            .and_then(|m| m.icon_url.clone())
            .or_else(|| item.curseforge.as_ref().and_then(|c| c.icon_url.clone()))?;
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
        // A Modrinth update takes the row's button when there are both.
        let cf_update = self
            .section(kind)
            .cf_updates
            .get(&name)
            .cloned()
            .filter(|_| update.is_none());
        let curseforge_on = self.curseforge(cx).is_some();
        let confirming = self.confirm_delete.as_ref() == Some(&(kind, name.clone()));
        let title = item.display_name();
        let version = item.display_version();
        let detail = if item.file.is_dir {
            tr!(M::ItemFolder, name = name)
        } else {
            format!("{name} · {}", format::bytes(item.file.size))
        };
        // The "Older version" or "Newer version" tag says the rest.
        let warning = item.file.meta.problem.clone();
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
            })
            .or_else(|| {
                item.curseforge
                    .as_ref()
                    .map(|c| c.page_url.clone())
                    .filter(|p| !p.is_empty())
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
                                    .child(RichText::minecraft(title)),
                            )
                            .when_some(version, |this, version| {
                                this.child(
                                    small_rich(RichText::minecraft(version))
                                        .flex_none()
                                        .text_color(colors::WHITE.opacity(50)),
                                )
                            })
                            .when(
                                kind == ContentKind::ResourcePack
                                    && compatibility == Compatibility::Compatible,
                                |this| this.child(badge(t(M::Compatible), colors::GREEN_300)),
                            )
                            .when(item.compatibility.warning().is_some(), |this| {
                                this.child(badge(
                                    if compatibility == Compatibility::TooOld {
                                        t(M::OlderVersion)
                                    } else {
                                        t(M::NewerVersion)
                                    },
                                    colors::YELLOW_300,
                                ))
                            })
                            .when(
                                item.modrinth.is_none() && item.curseforge.is_some(),
                                |this| this.child(badge(t(M::CurseForgeTab), colors::YELLOW_300)),
                            )
                            .when(!item.identified() && !item.file.is_dir, |this| {
                                this.child(badge(
                                    t(if curseforge_on {
                                        M::NotOnEitherPlatform
                                    } else {
                                        M::NotOnModrinth
                                    }),
                                    colors::GRAY_500,
                                ))
                            }),
                    )
                    .child(
                        small_rich(RichText::minecraft(detail))
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
                            this.child(
                                small_rich(RichText::minecraft(description))
                                    .truncate()
                                    .text_color(crate::theme::ink::secondary()),
                            )
                        },
                    )
                    .when_some(warning, |this, warning| {
                        this.child(
                            small_rich(RichText::new(warning)).text_color(colors::YELLOW_300),
                        )
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
                                tr!(M::UpdateTo, version = update.version_number),
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
                    .when_some(cf_update, |this, cf_update| {
                        let name = name.clone();
                        this.child(
                            Button::new(
                                ("cf-update-one", id_base),
                                if cf_update.downloadable() {
                                    tr!(
                                        M::UpdateTo,
                                        version = cf_update.file.display_name_or_file()
                                    )
                                } else {
                                    t(M::GetOnCurseForge).to_string()
                                },
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
                                Button::new(("confirm-delete", id_base), t(CommonMsg::MoveToTrash))
                                    .variant(ButtonVariant::Danger)
                                    .size(ButtonSize::Slim)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        cx.stop_propagation();
                                        this.delete(kind, name.clone(), cx)
                                    })),
                            )
                            .child(
                                Button::new(("cancel-delete", id_base), t(CommonMsg::Cancel))
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

    fn render_empty(&self, kind: ContentKind) -> AnyElement {
        let message = t(match kind {
            ContentKind::Mod => M::NoModsYet,
            ContentKind::ResourcePack => M::NoResourcePacksYet,
            ContentKind::ShaderPack => M::NoShaderPacksYet,
        });
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
            .child(
                div()
                    .px_4()
                    .text_center()
                    .whitespace_normal()
                    .child(message),
            )
            .into_any_element()
    }

    /// The card for vanilla instances: content that needs a mod loader.
    fn render_needs_loader(&mut self, what: NeedsLoader, cx: &mut Context<Self>) -> AnyElement {
        self.ensure_mod_loader_offer(cx);
        let switching = self.is_switching_loader(cx);
        let busy = self.locked(cx) || self.job.is_some();
        let game_version = self
            .context(cx)
            .map(|(_, _, target)| target.game_version)
            .unwrap_or_default();
        let new_instance = Button::new("new-fabric-instance", t(M::NewFabricInstance))
            .variant(ButtonVariant::Secondary)
            .icon(Icon::SquarePlus)
            .on_click(cx.listener(|this, _, _, cx| this.new_fabric_instance(cx)));
        let (text, buttons): (String, Option<AnyElement>) = match &self.mod_loader_offer {
            Load::Ready(Some(kind)) => {
                let label = kind.label();
                (
                    tr!(
                        what.pick(M::VanillaSwitchMods, M::VanillaSwitchShaders),
                        loader = label
                    ),
                    Some(
                        div()
                            .flex()
                            .flex_row()
                            .flex_wrap()
                            .gap_2()
                            .child(
                                Button::new(
                                    "switch-to-mod-loader",
                                    tr!(M::SwitchTo, loader = label),
                                )
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
                tr!(
                    what.pick(M::VanillaNoLoaderMods, M::VanillaNoLoaderShaders),
                    version = game_version
                ),
                Some(new_instance.into_any_element()),
            ),
            Load::Failed(err) => (
                tr!(
                    what.pick(M::VanillaCheckFailedMods, M::VanillaCheckFailedShaders),
                    error = err
                ),
                Some(
                    Button::new("retry-mod-loader", t(CommonMsg::TryAgain))
                        .variant(ButtonVariant::Secondary)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.mod_loader_offer = Load::NotLoaded;
                            this.ensure_mod_loader_offer(cx);
                        }))
                        .into_any_element(),
                ),
            ),
            Load::NotLoaded | Load::Loading => (
                tr!(
                    what.pick(M::VanillaCheckingMods, M::VanillaCheckingShaders),
                    version = game_version
                ),
                None,
            ),
        };
        card()
            .child(heading(t(
                what.pick(M::ModsNeedLoader, M::ShadersNeedLoader)
            )))
            .child(
                div()
                    .text_color(crate::theme::ink::secondary())
                    .whitespace_normal()
                    .child(text),
            )
            .children(buttons)
            .into_any_element()
    }

    /// The Shaders section's card: the shader mod in use, or the one to install.
    pub(crate) fn render_shader_mod(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let busy = self.locked(cx) || self.job.is_some();
        if let Some(loader) = self.shaders.loader {
            let using = match (
                &self.shaders.selection.pack,
                self.shaders.selection.internal,
            ) {
                (Some(pack), _) => {
                    tr!(
                        M::ShaderModUsingPack,
                        shader_mod = loader.label(),
                        pack = pack
                    )
                }
                (None, true) => tr!(M::ShaderModUsingBuiltIn, shader_mod = loader.label()),
                (None, false) => tr!(M::ShaderModInstalledOff, shader_mod = loader.label()),
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
                        .min_w_0()
                        .child(
                            div()
                                .font_weight(weights::BOLD.into())
                                .whitespace_normal()
                                .child(RichText::minecraft(using)),
                        )
                        .child(
                            small(tr!(
                                M::ShaderModPickPack,
                                shader_mod = loader.label(),
                                file = loader.config_file()
                            ))
                            .whitespace_normal()
                            .text_color(crate::theme::ink::muted()),
                        ),
                )
                .into_any_element();
        }
        let target_loader = self
            .context(cx)
            .map(|(_, _, t)| t.loader)
            .unwrap_or_default();
        if target_loader == Loader::None {
            return self.render_needs_loader(NeedsLoader::Shaders, cx);
        }
        let optifine_note = small(tr!(
            M::OptiFineNote,
            add_file = t(M::AddFile),
            mods = t(M::KindMods)
        ))
        .whitespace_normal()
        .text_color(crate::theme::ink::muted());
        let body: AnyElement = match &self.shader_offer {
            Load::NotLoaded | Load::Loading => div()
                .flex()
                .flex_row()
                .gap_2()
                .text_color(crate::theme::ink::muted())
                .child(LoadingDots)
                .child(t(M::LookingForShaderMod))
                .into_any_element(),
            Load::Failed(err) => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(small(tr!(M::CouldntAskModrinth, error = err)).text_color(colors::RED_200))
                .child(
                    Button::new("retry-shader-offer", t(CommonMsg::TryAgain))
                        .variant(ButtonVariant::Secondary)
                        .size(ButtonSize::Slim)
                        .on_click(cx.listener(|this, _, _, cx| this.load_shader_offer(cx))),
                )
                .into_any_element(),
            Load::Ready(ShaderModOffer::Install { loader, version }) => {
                let label = tr!(
                    M::InstallShaderMod,
                    shader_mod = loader.label(),
                    version = version.version_number
                );
                let runs_on = tr!(
                    M::ShaderModRunsOn,
                    shader_mod = loader.label(),
                    loader = loader_name(target_loader)
                );
                let with = match loader {
                    ShaderLoader::Iris => Some("Sodium"),
                    ShaderLoader::Oculus => Some("Embeddium"),
                    ShaderLoader::OptiFine => None,
                };
                let text = match with {
                    Some(dependency) => {
                        let gap = match crate::i18n::locale() {
                            crate::i18n::Locale::ZhHans | crate::i18n::Locale::ZhHant => "",
                            crate::i18n::Locale::En | crate::i18n::Locale::Fr => " ",
                        };
                        let comes_with = tr!(
                            M::ShaderModComesWith,
                            dependency = dependency,
                            shader_mod = loader.label()
                        );
                        format!("{runs_on}{gap}{comes_with}")
                    }
                    None => runs_on,
                };
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_color(crate::theme::ink::secondary())
                            .whitespace_normal()
                            .child(text),
                    )
                    .child(
                        div().flex().child(
                            Button::new("install-shader-mod", label)
                                .variant(ButtonVariant::Primary)
                                .icon(Icon::Download)
                                .disabled(busy)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.install_shader_mod(cx)),
                                ),
                        ),
                    )
                    .into_any_element()
            }
            Load::Ready(ShaderModOffer::Unavailable { reason }) => div()
                .text_color(crate::theme::ink::secondary())
                .whitespace_normal()
                .child(tr!(M::ShaderModUnavailable, reason = reason))
                .into_any_element(),
            Load::Ready(ShaderModOffer::NeedsModLoader) => {
                return self.render_needs_loader(NeedsLoader::Shaders, cx);
            }
        };
        card()
            .child(heading(t(M::ShadersNeedShaderMod)))
            .child(body)
            .child(optifine_note)
            .into_any_element()
    }

    /// What the Installed view lists, top to bottom: the cards and toolbar above the
    /// list, then one entry per row. Cheap to build every frame; the rows' elements
    /// are only made for the entries on screen (see [`Self::render_entry`]).
    fn installed_plan(&self, kind: ContentKind, cx: &gpui::App) -> Vec<Entry> {
        let query = self.query(kind, cx);
        let searching = !query.is_empty();
        let mut plan = Vec::new();
        if kind == ContentKind::ShaderPack && !searching {
            plan.push(Entry::ShaderMod);
        }
        if kind == ContentKind::Mod
            && !searching
            && self
                .context(cx)
                .is_some_and(|(_, _, target)| target.loader == Loader::None)
        {
            plan.push(Entry::NeedsLoader);
        }
        let listing = match &self.section(kind).listing {
            Load::NotLoaded | Load::Loading => {
                plan.push(Entry::Loading);
                if searching {
                    self.modrinth_plan(kind, cx, &mut plan);
                    self.curseforge_plan(kind, cx, &mut plan);
                }
                return plan;
            }
            Load::Failed(err) => {
                plan.push(Entry::Failed(err.clone()));
                return plan;
            }
            Load::Ready(listing) => listing,
        };
        let row = |index: usize, item: &Item, first: bool, last: bool, reorder| Entry::Row {
            index,
            file: item.file.file_name.clone(),
            first,
            last,
            reorder,
        };
        if searching {
            let matches: Vec<(usize, &Item)> = listing
                .items
                .iter()
                .enumerate()
                .filter(|(_, item)| item_matches(item, &query))
                .collect();
            if matches.is_empty() {
                plan.push(Entry::NoInstalledMatch);
            } else {
                plan.push(Entry::Heading(Heading::Installed(matches.len())));
                let count = matches.len();
                for (position, (index, item)) in matches.into_iter().enumerate() {
                    plan.push(row(index, item, position == 0, position + 1 == count, None));
                }
            }
            self.modrinth_plan(kind, cx, &mut plan);
            self.curseforge_plan(kind, cx, &mut plan);
            return plan;
        }
        if let Some(error) = &listing.lookup_error {
            plan.push(Entry::Note(error.clone(), false));
        }
        if let Some(error) = &listing.options_error {
            plan.push(Entry::Note(error.clone(), true));
        }
        if listing.items.is_empty() {
            plan.push(Entry::Empty);
            // Something to start from: the most relevant projects.
            self.modrinth_plan(kind, cx, &mut plan);
            self.curseforge_plan(kind, cx, &mut plan);
            return plan;
        }
        let count = listing.items.len();
        match kind {
            ContentKind::Mod => {
                plan.push(Entry::Heading(Heading::Installed(count)));
                for (index, item) in listing.items.iter().enumerate() {
                    plan.push(row(index, item, index == 0, index + 1 == count, None));
                }
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
                    plan.push(Entry::Heading(Heading::Enabled(enabled.len())));
                    let count = enabled.len();
                    for (position, (index, item)) in enabled.into_iter().enumerate() {
                        let reorder = Some((position > 0, position + 1 < count));
                        plan.push(row(
                            index,
                            item,
                            position == 0,
                            position + 1 == count,
                            reorder,
                        ));
                    }
                }
                if !others.is_empty() {
                    plan.push(Entry::Heading(Heading::Available(others.len())));
                    let count = others.len();
                    for (position, (index, item)) in others.into_iter().enumerate() {
                        plan.push(row(index, item, position == 0, position + 1 == count, None));
                    }
                }
            }
            ContentKind::ShaderPack => {
                plan.push(Entry::Heading(Heading::Installed(count)));
                plan.push(Entry::ShadersOff);
                for (index, item) in listing.items.iter().enumerate() {
                    plan.push(row(index, item, false, index + 1 == count, None));
                }
            }
        }
        plan
    }

    /// The Modrinth part of the list: its heading, then the results not installed yet.
    fn modrinth_plan(&self, kind: ContentKind, cx: &gpui::App, plan: &mut Vec<Entry>) {
        plan.push(Entry::ModrinthHeading);
        match &self.section(kind).browse.results {
            Load::NotLoaded | Load::Loading if self.modrinth(cx).is_none() => {
                plan.push(Entry::ModrinthOffline)
            }
            Load::NotLoaded | Load::Loading => plan.push(Entry::ModrinthLoading),
            Load::Failed(err) => plan.push(Entry::ModrinthFailed(err.clone())),
            Load::Ready(results) => {
                let installed = self.section(kind).installed_projects();
                let hits: Vec<(usize, &ModrinthSearchHit)> = results
                    .hits
                    .iter()
                    .enumerate()
                    .filter(|(_, hit)| !installed.contains_key(&hit.project_id))
                    .collect();
                if hits.is_empty() {
                    plan.push(Entry::ModrinthNothing);
                }
                let count = hits.len();
                for (position, (index, hit)) in hits.into_iter().enumerate() {
                    plan.push(Entry::Hit {
                        index,
                        id: hit.project_id.clone(),
                        first: position == 0,
                        last: position + 1 == count,
                    });
                }
                if results.total_hits > PAGE_SIZE {
                    plan.push(Entry::Pager);
                }
            }
        }
    }

    /// The CurseForge part of the list, after Modrinth's: its heading, then the results
    /// not installed yet. Only in builds with Lodestone's CurseForge key.
    fn curseforge_plan(&self, kind: ContentKind, cx: &gpui::App, plan: &mut Vec<Entry>) {
        if self.curseforge(cx).is_none() {
            return;
        }
        plan.push(Entry::CurseForgeHeading);
        match &self.section(kind).cf.results {
            Load::NotLoaded | Load::Loading => plan.push(Entry::CurseForgeLoading),
            Load::Failed(err) => plan.push(Entry::CurseForgeFailed(err.clone())),
            Load::Ready(results) => {
                let installed: Vec<u64> = self
                    .section(kind)
                    .items()
                    .iter()
                    .filter_map(|i| i.curseforge.as_ref().map(|c| c.project_id))
                    .collect();
                let hits: Vec<(usize, u64)> = results
                    .hits
                    .iter()
                    .enumerate()
                    .filter(|(_, hit)| !installed.contains(&hit.id))
                    .map(|(index, hit)| (index, hit.id))
                    .collect();
                if hits.is_empty() {
                    plan.push(Entry::CurseForgeNothing);
                }
                let count = hits.len();
                for (position, (index, id)) in hits.into_iter().enumerate() {
                    plan.push(Entry::CfHit {
                        index,
                        id,
                        first: position == 0,
                        last: position + 1 == count,
                    });
                }
                if results.total > PAGE_SIZE as usize {
                    plan.push(Entry::CfPager);
                }
            }
        }
    }

    /// Brings the list of `kind` up to date with `plan`: entries that changed are
    /// measured again, and the scroll position stays where it is.
    fn sync_installed_list(&mut self, kind: ContentKind, plan: Vec<Entry>) {
        let section = self.section_mut(kind);
        let old = &section.plan;
        if *old == plan {
            return;
        }
        let prefix = old.iter().zip(&plan).take_while(|(a, b)| a == b).count();
        let suffix = old[prefix..]
            .iter()
            .rev()
            .zip(plan[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        section
            .rows
            .splice(prefix..old.len() - suffix, plan.len() - prefix - suffix);
        section.plan = plan;
    }

    /// The element of entry `ix` of the Installed list of `kind`.
    fn render_entry(&mut self, kind: ContentKind, ix: usize, cx: &mut Context<Self>) -> AnyElement {
        let Some(entry) = self.section(kind).plan.get(ix).cloned() else {
            return div().into_any_element();
        };
        let last_entry = ix + 1 == self.section(kind).plan.len();
        let element = match &entry {
            Entry::ShaderMod => self.render_shader_mod(cx),
            Entry::NeedsLoader => self.render_needs_loader(NeedsLoader::Mods, cx),
            Entry::Loading => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .py_6()
                .justify_center()
                .text_color(crate::theme::ink::muted())
                .child(LoadingDots)
                .child(tr!(M::ReadingFolder, folder = kind.dir()))
                .into_any_element(),
            Entry::Failed(err) => {
                small(tr!(M::CouldntReadFolder, folder = kind.dir(), error = err))
                    .text_color(colors::RED_200)
                    .into_any_element()
            }
            Entry::Note(note, warning) => small(if *warning {
                note.clone()
            } else {
                tr!(M::SomeFilesNotMatched, error = note)
            })
            .text_color(if *warning {
                colors::YELLOW_300
            } else {
                colors::GRAY_500
            })
            .into_any_element(),
            Entry::Empty => self.render_empty(kind),
            Entry::Heading(heading) => {
                let (message, count) = match heading {
                    Heading::Installed(n) => (M::InstalledCount, n),
                    Heading::Enabled(n) => (M::EnabledCount, n),
                    Heading::Available(n) => (M::AvailableCount, n),
                };
                small(tr!(message, count = format::number(*count as u64)))
                    .font_weight(weights::BOLD.into())
                    .text_color(crate::theme::ink::muted())
                    .into_any_element()
            }
            Entry::NoInstalledMatch => small(tr!(
                M::NoInstalledMatch,
                count = format::number(0),
                kind = kind_name(kind).to_lowercase()
            ))
            .text_color(crate::theme::ink::muted())
            .into_any_element(),
            Entry::ModrinthHeading => self.render_modrinth_heading(kind, cx),
            Entry::ModrinthLoading => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .py_4()
                .justify_center()
                .text_color(crate::theme::ink::muted())
                .child(LoadingDots)
                .child(t(M::SearchingModrinth))
                .into_any_element(),
            Entry::ModrinthFailed(err) => small(err.clone())
                .text_color(colors::RED_200)
                .into_any_element(),
            Entry::ModrinthOffline => muted(t(M::BrowsingNeedsNetwork)).into_any_element(),
            Entry::ModrinthNothing => muted(t(M::NothingMatches)).into_any_element(),
            Entry::Hit {
                index, first, last, ..
            } => {
                let hit = match &self.section(kind).browse.results {
                    Load::Ready(results) => results.hits.get(*index).cloned(),
                    _ => None,
                };
                let Some(hit) = hit else {
                    return div().into_any_element();
                };
                let row = self.render_hit(kind, *index, &hit, *first, cx);
                in_box(row, *first, *last)
            }
            Entry::Pager => self.render_pager(kind, cx),
            Entry::CurseForgeHeading => self.render_cf_heading(kind, cx),
            Entry::CurseForgeLoading => div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .py_4()
                .justify_center()
                .text_color(crate::theme::ink::muted())
                .child(LoadingDots)
                .child(t(M::SearchingCurseForge))
                .into_any_element(),
            Entry::CurseForgeFailed(err) => small(err.clone())
                .text_color(colors::RED_200)
                .into_any_element(),
            Entry::CurseForgeNothing => muted(t(M::NothingOnCurseForge)).into_any_element(),
            Entry::CfHit {
                index, first, last, ..
            } => {
                let hit = match &self.section(kind).cf.results {
                    Load::Ready(results) => results.hits.get(*index).cloned(),
                    _ => None,
                };
                let Some(hit) = hit else {
                    return div().into_any_element();
                };
                let row = self.render_cf_hit(kind, *index, &hit, *first, cx);
                in_box(row, *first, *last)
            }
            Entry::CfPager => self.render_cf_pager(kind, cx),
            Entry::ShadersOff => {
                let loader = self.shaders.loader;
                let busy = self.locked(cx) || self.job.is_some() || loader.is_none();
                let selected_none = self.shaders.selection.pack.is_none();
                in_box(
                    row_base("shaders-off", true)
                        .when(!busy, |this| {
                            this.keyboard_focus()
                                .cursor_pointer()
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
                                .child(
                                    div()
                                        .font_weight(weights::BOLD.into())
                                        .child(t(M::ShadersOff)),
                                )
                                .child(
                                    small(t(M::NormalLook)).text_color(colors::WHITE.opacity(30)),
                                ),
                        )
                        .into_any_element(),
                    true,
                    false,
                )
            }
            Entry::Row {
                index,
                first,
                last,
                reorder,
                ..
            } => {
                let item = match &self.section(kind).listing {
                    Load::Ready(listing) => listing.items.get(*index).cloned(),
                    _ => None,
                };
                let Some(item) = item else {
                    return div().into_any_element();
                };
                if kind == ContentKind::ShaderPack {
                    let loader = self.shaders.loader;
                    let busy = self.locked(cx) || self.job.is_some() || loader.is_none();
                    let name = item.file.file_name.clone();
                    let selected = item.enabled;
                    let row = self.render_item(kind, *index, &item, true, None, cx);
                    in_box(
                        div()
                            .id(("shader-select", *index))
                            .flex()
                            .flex_row()
                            .items_center()
                            .bg(colors::GRAY_800)
                            .border_t_1()
                            .border_color(colors::GRAY_FADED.opacity(15))
                            .when(selected, |this| {
                                this.bg(crate::theme::accent::base().opacity(15))
                            })
                            .when(!busy, |this| {
                                this.keyboard_focus()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(colors::GRAY_750))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.select_shader(Some(name.clone()), cx)
                                    }))
                            })
                            .child(div().pl_4().child(radio(selected)))
                            .child(div().flex_1().min_w_0().child(row))
                            .into_any_element(),
                        false,
                        *last,
                    )
                } else {
                    let row = self.render_item(kind, *index, &item, *first, *reorder, cx);
                    in_box(row, *first, *last)
                }
            }
        };
        // The gap between blocks; rows of one box touch.
        let joined = matches!(
            entry,
            Entry::Row { first: false, .. }
                | Entry::Hit { first: false, .. }
                | Entry::CfHit { first: false, .. }
        ) || (matches!(entry, Entry::Row { .. })
            && kind == ContentKind::ShaderPack
            && self.query(kind, cx).is_empty());
        div()
            .w_full()
            .when(ix > 0 && !joined, |this| this.pt_3())
            .when(last_entry, |this| this.pb_4())
            .child(element)
            .into_any_element()
    }

    /// "On Modrinth · n", with the sort and the "1.20.1 Fabric only" filter.
    fn render_modrinth_heading(&mut self, kind: ContentKind, cx: &mut Context<Self>) -> AnyElement {
        let browse = &self.section(kind).browse;
        let only = browse.only_compatible;
        let index = browse.index;
        let total = browse.results.ready().map(|r| r.total_hits);
        let indexes = [
            (ModrinthSearchIndex::Relevance, M::SortRelevance),
            (ModrinthSearchIndex::Downloads, M::SortDownloads),
            (ModrinthSearchIndex::Follows, M::SortFollows),
            (ModrinthSearchIndex::Newest, M::SortNewest),
            (ModrinthSearchIndex::Updated, M::SortUpdated),
        ];
        let current = indexes
            .iter()
            .find(|(i, _)| *i == index)
            .map_or(M::SortRelevance, |(_, label)| *label);
        let target = self.target_label(kind, cx);
        let title = match total {
            Some(total) => tr!(M::OnModrinthCount, count = format::number(total as u64)),
            None => t(M::OnModrinth).to_string(),
        };
        let sort = div()
            .flex()
            .flex_col()
            .items_end()
            .child(
                select_button(
                    ("modrinth-sort", kind as usize),
                    small(t(current)).font_weight(weights::BOLD.into()),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.sort_open = !this.sort_open;
                    cx.notify();
                })),
            )
            .when(self.sort_open, |this| {
                let close = cx.listener(|this, _: &(), _, cx| {
                    this.sort_open = false;
                    cx.notify();
                });
                let mut panel = popover_panel("modrinth-sort-list")
                    .w(px(180.))
                    .on_mouse_down_out(cx.listener(|this, _, _, cx| {
                        this.sort_open = false;
                        cx.notify();
                    }));
                for (i, (value, label)) in indexes.into_iter().enumerate() {
                    panel = panel.child(
                        ListRow::new(("modrinth-sort-choice", i), t(label))
                            .selected(value == index)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.sort_open = false;
                                this.set_index(kind, value, cx);
                            })),
                    );
                }
                // In a box of its own under the button: the popover starts where its
                // parent does.
                this.child(div().child(popover(
                    "modrinth-sort-popover",
                    panel,
                    move |window, cx| close(&(), window, cx),
                )))
            });
        let chip = div()
            .id(("only-compatible", kind as usize))
            .keyboard_focus()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .px_2p5()
            .py_0p5()
            .rounded_full()
            .border_1()
            .cursor_pointer()
            .text_size(theme::text_size(text::SMALL))
            .font_weight(weights::BOLD.into())
            .map(|this| {
                if only {
                    this.bg(crate::theme::accent::base().opacity(25))
                        .border_color(crate::theme::accent::light().opacity(60))
                        .text_color(colors::WHITE)
                } else {
                    this.border_color(crate::theme::ink::border())
                        .text_color(crate::theme::ink::secondary())
                        .hover(|style| {
                            style
                                .text_color(crate::theme::ink::text())
                                .bg(colors::GRAY_750)
                        })
                }
            })
            .when(only, |this| {
                this.child(Icon::Check.render(px(10.), colors::WHITE))
            })
            .child(tr!(M::OnlyTarget, target = target))
            .on_click(cx.listener(move |this, _, _, cx| this.set_only_compatible(kind, !only, cx)));
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap_2()
            .child(
                div().flex_1().min_w_0().child(
                    small(title)
                        .font_weight(weights::BOLD.into())
                        .text_color(crate::theme::ink::muted()),
                ),
            )
            .child(sort)
            .child(chip)
            .into_any_element()
    }

    /// Previous and next page of the Modrinth results.
    fn render_pager(&mut self, kind: ContentKind, cx: &mut Context<Self>) -> AnyElement {
        let browse = &self.section(kind).browse;
        let offset = browse.offset;
        let total = browse.results.ready().map_or(0, |r| r.total_hits);
        let pages = total.div_ceil(PAGE_SIZE).max(1);
        let page = offset / PAGE_SIZE + 1;
        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                Button::new(("previous-page", kind as usize), t(CommonMsg::Previous))
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Slim)
                    .disabled(offset == 0)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_page(kind, offset.saturating_sub(PAGE_SIZE), cx)
                    })),
            )
            .child(
                small(tr!(
                    M::PageOf,
                    page = format::number(page as u64),
                    pages = format::number(pages as u64)
                ))
                .text_color(crate::theme::ink::muted()),
            )
            .child(
                Button::new(("next-page", kind as usize), t(CommonMsg::Next))
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Slim)
                    .disabled(page >= pages)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_page(kind, offset + PAGE_SIZE, cx)
                    })),
            )
            .into_any_element()
    }

    /// A Modrinth result as a row of the list: opens its page, or installs it.
    fn render_hit(
        &mut self,
        kind: ContentKind,
        index: usize,
        hit: &ModrinthSearchHit,
        first: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let icon = hit
            .icon_url
            .as_deref()
            .and_then(|url| self.remote_icon(url, cx));
        let id = hit.project_id.clone();
        let install_id = hit.project_id.clone();
        let resolving = self.section(kind).resolving.contains(&hit.project_id);
        let busy = self.locked(cx) || self.job.is_some();
        let client_badge = kind == ContentKind::Mod && hit.client_side == "optional";
        row_base(("hit", index), first)
            .keyboard_focus()
            .cursor_pointer()
            .hover(|style| style.bg(colors::GRAY_750))
            .on_click(
                cx.listener(move |this, _, _, cx| this.open_project(kind, id.clone(), None, cx)),
            )
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
                                    .text_color(crate::theme::ink::text())
                                    .child(hit.title.clone()),
                            )
                            .child(
                                small(tr!(M::ByAuthor, author = hit.author))
                                    .flex_none()
                                    .text_color(colors::WHITE.opacity(50)),
                            )
                            .when(client_badge, |this| {
                                this.child(badge(t(M::OptionalOnServers), colors::GRAY_500))
                            }),
                    )
                    .child(
                        small(hit.description.clone())
                            .truncate()
                            .text_color(crate::theme::ink::secondary()),
                    ),
            )
            .child(
                small(counted(hit.downloads, P::Downloads, M::DownloadsCompact))
                    .flex_none()
                    .text_color(crate::theme::ink::muted()),
            )
            .child(
                Button::new(("install-hit", index), t(CommonMsg::Install))
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Slim)
                    .loading(resolving)
                    .disabled(busy || resolving)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        // Not also the row's click, which opens the page.
                        cx.stop_propagation();
                        this.install_hit(kind, install_id.clone(), cx)
                    })),
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
        let tab = view.tab;
        let with_dependencies = view.with_dependencies;
        let confirm_force = view.confirm_force.clone();
        let linked = view.linked_version.clone();
        let back = Button::new(("back", kind as usize), t(CommonMsg::Back))
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
                            .text_color(crate::theme::ink::muted())
                            .child(LoadingDots)
                            .child(t(M::LoadingProject)),
                    )
                    .into_any_element();
            }
        };
        let icon = project
            .icon_url
            .as_deref()
            .and_then(|url| self.remote_icon(url, cx));
        let page = modrinth_page(&project.project_type, &project.slug);
        let mut column =
            div()
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
                                        .font_family(fonts::display())
                                        .text_size(theme::text_size(text::H2))
                                        .font_weight(weights::BOLD.into())
                                        .text_color(crate::theme::ink::text())
                                        .child(project.title.clone()),
                                )
                                .child(
                                    div()
                                        .text_color(crate::theme::ink::secondary())
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
                                                "{} · {}",
                                                counted(
                                                    project.downloads,
                                                    P::Downloads,
                                                    M::DownloadsCompact
                                                ),
                                                counted(
                                                    project.followers,
                                                    P::Followers,
                                                    M::FollowersCompact
                                                )
                                            ))
                                            .text_color(crate::theme::ink::muted()),
                                        )
                                        .children(project.categories.iter().take(6).map(|c| {
                                            badge(c.clone(), crate::theme::accent::light())
                                        })),
                                ),
                        )
                        .child(
                            Button::new(("open-modrinth", kind as usize), t(M::ViewOnModrinth))
                                .variant(ButtonVariant::Secondary)
                                .icon(Icon::ArrowUpRightFromSquare)
                                .size(ButtonSize::Slim)
                                .on_click(move |_, _, cx| cx.open_url(&page)),
                        ),
                );
        if kind == ContentKind::ShaderPack && self.shaders.loader.is_none() {
            column = column.child(self.render_shader_mod(cx));
        }
        let has_body = !project.body.trim().is_empty();
        if has_body {
            let entity = cx.entity();
            column = column.child(div().flex().child(pills(
                "project-tabs",
                vec![t(M::Versions).into(), t(M::Description).into()],
                match tab {
                    ProjectTab::Versions => 0,
                    ProjectTab::Description => 1,
                },
                move |index, _, cx| {
                    let tab = if index == 0 {
                        ProjectTab::Versions
                    } else {
                        ProjectTab::Description
                    };
                    entity.update(cx, |this, cx| this.set_project_tab(kind, tab, cx));
                },
            )));
        }
        if has_body && tab == ProjectTab::Description {
            let blocks = self.description(&project);
            let urls = crate::markdown::image_urls(&blocks);
            let images = self.description_images(&urls, cx);
            return column
                .child(
                    div()
                        .p_4()
                        .rounded(radius::LG)
                        .border_1()
                        .border_color(crate::theme::ink::border())
                        .bg(colors::GRAY_850)
                        .child(crate::markdown::render(
                            format!("description-{}", project.id),
                            &blocks,
                            &images,
                        )),
                )
                .into_any_element();
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
                            .text_color(crate::theme::ink::muted())
                            .child(LoadingDots)
                            .child(t(M::LoadingVersions)),
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
                .child(heading(t(M::Versions)))
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
                            small(t(M::ShowAllVersions)).text_color(crate::theme::ink::secondary()),
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
                                small(t(M::AlsoInstallDependencies))
                                    .text_color(crate::theme::ink::secondary()),
                            ),
                    )
                }),
        );
        if shown.is_empty() {
            return column
                .child(
                    muted(tr!(
                        M::NoVersionFits,
                        project = project.title,
                        show_all = t(M::ShowVersionsShort)
                    ))
                    .whitespace_normal(),
                )
                .into_any_element();
        }
        let mut rows = list_box();
        for (index, (version, problem)) in shown.into_iter().take(40).enumerate() {
            let (type_label, type_color) = version_label(version.version_type);
            let is_installed = installed_versions.contains(&version.id);
            let size = version.primary_file().map_or(0, |f| f.size);
            let day = version.date_published.get(..10).unwrap_or_default();
            let date = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
                .map_or_else(|_| day.to_string(), |date| format::short_date(date, true));
            let forcing = confirm_force.as_deref() == Some(version.id.as_str());
            let version_for_click = version.clone();
            let project_for_click = project.clone();
            let id = version.id.clone();
            let button = if is_installed {
                badge(t(M::InstalledBadge), colors::GREEN_300).into_any_element()
            } else if problem.is_some() && !forcing {
                Button::new(("install-anyway", index), t(M::InstallAnyway))
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
                    (_, true, _, _) => t(M::InstallItAnyway).to_string(),
                    (
                        ContentKind::ShaderPack,
                        _,
                        None,
                        Some(ShaderModOffer::Install { loader, .. }),
                    ) => {
                        tr!(M::InstallWith, shader_mod = loader.label())
                    }
                    _ => t(CommonMsg::Install).to_string(),
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
                                    format::bytes(size)
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
                small(trn!(P::MoreVersions, total - 40)).text_color(crate::theme::ink::muted()),
            );
        }
        column.into_any_element()
    }
}

/// What needs a mod loader, on a vanilla instance's card.
#[derive(Clone, Copy, PartialEq, Eq)]
enum NeedsLoader {
    Mods,
    Shaders,
}

impl NeedsLoader {
    fn pick(self, mods: M, shaders: M) -> M {
        match self {
            NeedsLoader::Mods => mods,
            NeedsLoader::Shaders => shaders,
        }
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
            crate::theme::accent::light()
        } else {
            colors::GRAY_600
        })
        .when(selected, |this| {
            this.child(
                div()
                    .size(px(6.))
                    .rounded(px(3.))
                    .bg(crate::theme::accent::light()),
            )
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
        let cf_project_open = self.section(kind).cf.project.is_some();
        let project_open = self.section(kind).browse.project.is_some() || cf_project_open;
        if !project_open
            && self.shows_modrinth(kind, cx)
            && matches!(self.section(kind).cf.results, Load::NotLoaded)
            && self.curseforge(cx).is_some()
            && (self.query(kind, cx).is_empty() || self.section(kind).browse.task.is_none())
        {
            self.cf_search(kind, cx);
        }
        // Nothing installed: the most relevant projects, to start from. (While typing,
        // the search runs once typing pauses.)
        if !project_open
            && self.shows_modrinth(kind, cx)
            && matches!(self.section(kind).browse.results, Load::NotLoaded)
            && self.section(kind).browse.task.is_none()
            && self.modrinth(cx).is_some()
        {
            self.search(kind, cx);
        }
        let body = if project_open {
            div()
                .id(("content-project", kind as usize))
                .size_full()
                .overflow_y_scroll()
                .child(div().pb_4().child(if cf_project_open {
                    self.render_cf_project(kind, cx)
                } else {
                    self.render_project(kind, cx)
                }))
                .into_any_element()
        } else {
            // Only the rows on screen are made: a long list scrolls as fast as a short
            // one.
            let plan = self.installed_plan(kind, cx);
            self.sync_installed_list(kind, plan);
            let entity = cx.entity();
            list(self.section(kind).rows.clone(), move |ix, _, cx| {
                entity.update(cx, |this, cx| this.render_entry(kind, ix, cx))
            })
            .size_full()
            .into_any_element()
        };
        div()
            .flex()
            .flex_col()
            .size_full()
            .gap_3()
            .child(self.render_header(cx))
            .child(self.render_banners(cx))
            .children(self.render_blocked(cx))
            .child(
                div()
                    .id(("content-body", kind as usize))
                    .flex_1()
                    .min_h_0()
                    .rounded(radius::LG)
                    .border_2()
                    .border_color(colors::TRANSPARENT)
                    .when(!project_open, |this| {
                        this.drag_over::<ExternalPaths>(|style, _, _, _| {
                            style
                                .border_color(crate::theme::accent::light().opacity(60))
                                .bg(crate::theme::accent::base().opacity(8))
                        })
                        .on_drop(cx.listener(
                            move |this, paths: &ExternalPaths, _, cx| {
                                this.add_files(kind, paths.paths().to_vec(), cx)
                            },
                        ))
                    })
                    .child(body),
            )
    }
}
