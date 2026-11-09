//! A Modrinth project picked in Browse (`ModrinthProjectView.tsx`): its header, links,
//! gallery and description, and its versions to install; for a modpack, a button that
//! makes a server from it instead.

use std::{collections::HashMap, sync::Arc};

use gpui::{
    AnyElement, AppContext as _, Context, IntoElement, ParentElement, RenderImage, SharedString,
    Styled, div, img, prelude::*,
};
use launcher_core::{
    cores::admin::mods::{
        ModInstallRequest, ModrinthProject, ModrinthSearchHit, ModrinthVersion,
        ModrinthVersionType, is_version_compatible, modrinth_project_url,
    },
    markdown::{Block, safe_link},
};
use lodestone_ui::{
    components::{
        Accessible as _, Button, ButtonSize, ButtonVariant, Checkbox, Icon, LoadingDots,
        settings::{card, link, tag},
        text_tooltip,
    },
    i18n::format,
    theme::{colors, ink, px, radius, text, weights},
};

use super::{
    ModsPage, Prompt, Target,
    installed::{mod_icon, why_not},
    muted, warning,
};
use crate::{
    i18n::{CommonMsg, InstanceMsg as M, InstancePlural as P, t, tr, trn},
    pages::create_instance::ModpackPreset,
};

/// Versions shown at first, and added by "Show more".
const VERSIONS_PAGE: usize = 20;

pub struct ProjectView {
    pub project_id: String,
    /// The version a pasted link named: listed first.
    pub linked_version_id: Option<String>,
    /// The search result it was picked from: its author (the project has none).
    pub hit: Option<ModrinthSearchHit>,
    project: Option<Result<ModrinthProject, String>>,
    blocks: Option<Arc<Vec<Block>>>,
    versions: Option<Result<Vec<ModrinthVersion>, String>>,
    linked: Option<ModrinthVersion>,
    show_all: bool,
    shown: usize,
    with_dependencies: bool,
    /// The version being installed.
    installing: Option<String>,
    /// Counts the versions' fetches, so only the newest is kept.
    serial: u64,
}

impl ProjectView {
    pub fn new(
        project_id: String,
        linked_version_id: Option<String>,
        hit: Option<ModrinthSearchHit>,
    ) -> Self {
        Self {
            project_id,
            linked_version_id,
            hit,
            project: None,
            blocks: None,
            versions: None,
            linked: None,
            show_all: false,
            shown: VERSIONS_PAGE,
            with_dependencies: true,
            installing: None,
            serial: 0,
        }
    }
}

/// The versions to list (`ProjectVersions`): compatible ones unless all are asked
/// for, the linked one first.
pub fn listed_versions(
    versions: &[ModrinthVersion],
    linked: Option<&ModrinthVersion>,
    project_id: &str,
    target: &Target,
    show_all: bool,
) -> Vec<ModrinthVersion> {
    let mut list: Vec<ModrinthVersion> = versions
        .iter()
        .filter(|v| show_all || is_version_compatible(v, target.loader, &target.game_version))
        .cloned()
        .collect();
    if let Some(linked) = linked.filter(|l| l.project_id == project_id) {
        list.retain(|v| v.id != linked.id);
        list.insert(0, linked.clone());
    }
    list
}

impl ModsPage {
    /// Opens a project picked in Browse, and fetches it, its versions and the linked one.
    pub(super) fn open_project(
        &mut self,
        project_id: String,
        linked_version_id: Option<String>,
        hit: Option<ModrinthSearchHit>,
        cx: &mut Context<Self>,
    ) {
        self.project = Some(ProjectView::new(
            project_id.clone(),
            linked_version_id.clone(),
            hit,
        ));
        let admin = self.state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let (project, linked) = cx
                .background_spawn(async move {
                    let project = admin.modrinth_project(&project_id).await;
                    let linked = match linked_version_id {
                        Some(id) => admin.modrinth_version(&id).await.ok(),
                        None => None,
                    };
                    (project, linked)
                })
                .await;
            this.update(cx, |this, cx| {
                let Some(view) = &mut this.project else {
                    return;
                };
                match project {
                    Ok(project) => {
                        view.blocks = Some(Arc::new(launcher_core::markdown::parse(&project.body)));
                        view.project_id = project.id.clone();
                        view.project = Some(Ok(project));
                    }
                    Err(err) => view.project = Some(Err(err.to_string())),
                }
                view.linked = linked;
                this.load_versions(cx);
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    fn load_versions(&mut self, cx: &mut Context<Self>) {
        let Some(target) = self.target(cx) else {
            return;
        };
        let Some(view) = &mut self.project else {
            return;
        };
        if !matches!(view.project, Some(Ok(_))) {
            return;
        }
        view.serial += 1;
        let serial = view.serial;
        let id = view.project_id.clone();
        // Modrinth filters by one loader; Paper also takes Bukkit and Spigot, so those
        // are filtered here instead
        let loaders = target.loader.compatible();
        let (game_version, loader) = if view.show_all {
            (None, None)
        } else {
            (
                Some(target.game_version.clone()),
                (loaders.len() == 1).then(|| loaders[0].to_string()),
            )
        };
        let admin = self.state.read(cx).admin.clone();
        let task = cx.spawn(async move |this, cx| {
            let versions = cx
                .background_spawn(async move {
                    admin
                        .modrinth_versions(&id, game_version.as_deref(), loader.as_deref())
                        .await
                })
                .await;
            this.update(cx, |this, cx| {
                if let Some(view) = &mut this.project
                    && view.serial == serial
                {
                    view.versions = Some(versions.map_err(|err| err.to_string()));
                    cx.notify();
                }
            })
            .ok();
        });
        self.push(task);
    }

    /// Install: at once for a compatible version, else after "Install anyway".
    fn request_install(&mut self, version: &ModrinthVersion, cx: &mut Context<Self>) {
        let Some(target) = self.target(cx) else {
            return;
        };
        if is_version_compatible(version, target.loader, &target.game_version) {
            return self.install_version(version.id.clone(), false, cx);
        }
        let title = self
            .project
            .as_ref()
            .and_then(|v| v.project.as_ref()?.as_ref().ok())
            .map(|p| p.title.clone())
            .unwrap_or_default();
        let body = tr!(
            M::ModsIncompatibleBody,
            title = title,
            version = version.version_number,
            loaders = version.loaders.join(", "),
            gameVersions = version.game_versions.join(", "),
            loader = target.loader.as_str(),
            gameVersion = target.game_version
        );
        self.open_prompt(
            Prompt::Incompatible {
                version_id: version.id.clone(),
                body,
            },
            cx,
        );
    }

    pub(super) fn install_version(
        &mut self,
        version_id: String,
        force: bool,
        cx: &mut Context<Self>,
    ) {
        let Some(view) = &mut self.project else {
            return;
        };
        let title = view
            .project
            .as_ref()
            .and_then(|p| p.as_ref().ok())
            .map(|p| p.title.clone())
            .unwrap_or_default();
        let number = view
            .versions
            .as_ref()
            .and_then(|v| v.as_ref().ok())
            .into_iter()
            .flatten()
            .chain(view.linked.iter())
            .find(|v| v.id == version_id)
            .map(|v| v.version_number.clone())
            .unwrap_or_default();
        view.installing = Some(version_id.clone());
        let request = ModInstallRequest {
            version_id,
            install_dependencies: view.with_dependencies,
            force,
        };
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move { admin.install_mod(&uuid, &request).await })
                .await;
            this.update(cx, |this, cx| {
                if let Some(view) = &mut this.project {
                    view.installing = None;
                }
                let toast = match result {
                    Ok(result) => Self::result_toast(
                        tr!(M::ModsInstalledVersion, title = title, version = number),
                        &result,
                    ),
                    Err(err) => lodestone_ui::components::Toast::error(tr!(
                        M::ModsInstallFailed,
                        title = title,
                        error = err
                    )),
                };
                this.toast(toast, cx);
                this.reload(cx);
            })
            .ok();
        });
        self.push(task);
    }

    fn version_row(
        &self,
        index: usize,
        version: &ModrinthVersion,
        target: &Target,
        installed_here: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let can_write = self.can_write(cx);
        let compatible = is_version_compatible(version, target.loader, &target.game_version);
        let installing = self
            .project
            .as_ref()
            .and_then(|v| v.installing.as_ref())
            .is_some_and(|id| *id == version.id);
        let (type_label, type_color) = match version.version_type {
            ModrinthVersionType::Release => (t(M::ModsRelease), colors::GREEN),
            ModrinthVersionType::Beta => (t(M::ModsBeta), colors::YELLOW),
            ModrinthVersionType::Alpha => (t(M::ModsAlpha), colors::RED_200),
            ModrinthVersionType::Unknown => ("?", colors::GRAY_500),
        };
        let shown: Vec<&str> = version
            .game_versions
            .iter()
            .take(3)
            .map(String::as_str)
            .collect();
        let rest = version.game_versions.len().saturating_sub(shown.len());
        let mut games = shown.join(", ");
        if rest > 0 {
            games = format!("{games} {}", trn!(P::ModsMoreGameVersions, rest));
        }
        let date = version
            .date_published
            .get(..10)
            .unwrap_or(&version.date_published)
            .to_string();
        let details = format!("{games} · {} · {date}", version.loaders.join(", "));
        let picked = version.clone();
        let id = |part: &str| SharedString::from(format!("version-{index}-{part}"));
        div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(12.))
            .px(px(16.))
            .py(px(8.))
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
                            .gap(px(8.))
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(weights::BOLD.face())
                                    .text_color(ink::text())
                                    .child(version.version_number.clone()),
                            )
                            .child(tag(type_label, type_color))
                            .when(installed_here, |this| {
                                this.child(tag(t(M::ModsInstalledTag), colors::GREEN))
                            })
                            .when(!compatible, |this| {
                                this.child(tag(
                                    tr!(
                                        M::ModsNotFor,
                                        loader = target.loader.as_str(),
                                        gameVersion = target.game_version
                                    ),
                                    colors::GRAY_400,
                                ))
                            }),
                    )
                    .child(
                        div()
                            .id(id("details"))
                            .truncate()
                            .text_size(text::SMALL)
                            .text_color(ink::muted())
                            .child(details)
                            .tooltip(text_tooltip(version.game_versions.join(", "))),
                    ),
            )
            // the exact version installed already: nothing to install
            .when(!installed_here, |this| {
                this.child(why_not(
                    id("install-why"),
                    can_write,
                    Button::new(id("install"), t(CommonMsg::Install))
                        .icon(Icon::Download)
                        .variant(if compatible {
                            ButtonVariant::Primary
                        } else {
                            ButtonVariant::Secondary
                        })
                        .size(ButtonSize::Slim)
                        .loading(installing)
                        .disabled(!can_write || installing)
                        .on_click(
                            cx.listener(move |this, _, _, cx| this.request_install(&picked, cx)),
                        ),
                ))
            })
            .into_any_element()
    }

    fn versions_section(&self, target: &Target, cx: &mut Context<Self>) -> AnyElement {
        let Some(view) = &self.project else {
            return div().into_any_element();
        };
        let can_write = self.can_write(cx);
        let installed_ids: std::collections::HashSet<String> = self
            .list
            .iter()
            .flat_map(|l| l.mods.iter())
            .filter_map(|m| m.modrinth.as_ref().map(|m| m.version_id.clone()))
            .collect();
        let header = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .child(
                div()
                    .text_size(text::H2)
                    .font_weight(weights::BOLD.face())
                    .text_color(ink::text())
                    .child(t(M::ModsVersions)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .flex_wrap()
                    .items_center()
                    .gap(px(16.))
                    .child(
                        Checkbox::new(
                            "with-dependencies",
                            view.with_dependencies,
                            t(M::ModsWithDependencies),
                        )
                        .disabled(!can_write)
                        .on_change(cx.listener(
                            |this, on: &bool, _, cx| {
                                if let Some(view) = &mut this.project {
                                    view.with_dependencies = *on;
                                }
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        Checkbox::new("show-all-versions", view.show_all, t(M::ModsShowAll))
                            .on_change(cx.listener(|this, on: &bool, _, cx| {
                                if let Some(view) = &mut this.project {
                                    view.show_all = *on;
                                    view.shown = VERSIONS_PAGE;
                                }
                                this.load_versions(cx);
                            })),
                    ),
            );
        let body: AnyElement = match &view.versions {
            None => card([div()
                .flex()
                .items_center()
                .justify_center()
                .h(px(64.))
                .child(LoadingDots)
                .into_any_element()])
            .into_any_element(),
            Some(Err(error)) => div()
                .text_color(colors::RED_200)
                .child(tr!(M::ModsVersionsFailed, error = error))
                .into_any_element(),
            Some(Ok(versions)) => {
                let list = listed_versions(
                    versions,
                    view.linked.as_ref(),
                    &view.project_id,
                    target,
                    view.show_all,
                );
                if list.is_empty() {
                    muted(if view.show_all {
                        tr!(
                            M::ModsNoVersions,
                            loader = target.loader.as_str(),
                            gameVersion = target.game_version
                        )
                    } else {
                        tr!(
                            M::ModsNoVersionsShowAll,
                            loader = target.loader.as_str(),
                            gameVersion = target.game_version
                        )
                    })
                } else {
                    let left = list.len().saturating_sub(view.shown);
                    let rows: Vec<AnyElement> = list
                        .iter()
                        .take(view.shown)
                        .enumerate()
                        .map(|(index, v)| {
                            self.version_row(index, v, target, installed_ids.contains(&v.id), cx)
                        })
                        .collect();
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(card(rows))
                        .when(left > 0, |this| {
                            this.child(
                                div().flex().justify_center().child(
                                    Button::new("show-more-versions", trn!(P::ModsShowMore, left))
                                        .on_click(cx.listener(|this, _, _, cx| {
                                            if let Some(view) = &mut this.project {
                                                view.shown += VERSIONS_PAGE;
                                            }
                                            cx.notify();
                                        })),
                                ),
                            )
                        })
                        .into_any_element()
                }
            }
        };
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(header)
            .child(body)
            .into_any_element()
    }

    pub(super) fn render_project(&self, target: &Target, cx: &mut Context<Self>) -> AnyElement {
        let Some(view) = &self.project else {
            return div().into_any_element();
        };
        let back = Button::new("project-back", t(CommonMsg::Back))
            .icon(Icon::ArrowLeft)
            .variant(ButtonVariant::Ghost)
            .on_click(cx.listener(|this, _, _, cx| {
                this.project = None;
                cx.notify();
            }));
        let column = div()
            .flex()
            .flex_col()
            .gap(px(24.))
            .child(div().flex().child(back));
        let project = match &view.project {
            None => {
                return column
                    .child(card([div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .h(px(120.))
                        .child(LoadingDots)
                        .into_any_element()]))
                    .into_any_element();
            }
            Some(Err(error)) => {
                return column
                    .child(
                        div()
                            .text_color(colors::RED_200)
                            .child(tr!(M::ModsProjectFailed, error = error)),
                    )
                    .into_any_element();
            }
            Some(Ok(project)) => project.clone(),
        };
        let is_modpack = project.project_type == "modpack";
        let installable = matches!(project.project_type.as_str(), "mod" | "plugin");
        let side = |value: &str| -> String {
            match value {
                "required" => t(M::ModsSideRequired).into(),
                "optional" => t(M::ModsSideOptional).into(),
                "unsupported" => t(M::ModsSideUnsupported).into(),
                "unknown" => t(M::ModsSideUnknown).into(),
                // a value Modrinth adds later, as it is
                other => other.into(),
            }
        };
        let author = view.hit.as_ref().map(|h| h.author.clone());
        let byline = format!(
            "{} · {}",
            author
                .map(|a| tr!(M::ModsByAuthor, author = a))
                .unwrap_or_else(|| project.slug.clone()),
            project.project_type
        );
        let stats = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(12.))
            .text_size(text::SMALL)
            .text_color(ink::muted())
            .child(
                div()
                    .id("project-downloads")
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .child(Icon::Download.render(px(10.), ink::muted()))
                    .child(format::compact(project.downloads))
                    .tooltip(text_tooltip(trn!(P::ModsDownloads, project.downloads))),
            )
            .child(
                div()
                    .id("project-followers")
                    .child(trn!(P::ModsFollowers, project.followers)),
            )
            .child(tr!(M::ModsServerSide, value = side(&project.server_side)))
            .when(installable && !target.is_plugin(), |this| {
                this.children(match project.client_side.as_str() {
                    "required" => {
                        Some(tag(t(M::ModsPlayersNeedToo), colors::BLUE_200).into_any_element())
                    }
                    "unsupported" => Some(
                        div()
                            .text_color(ink::faint())
                            .child(t(M::ModsServerOnly))
                            .into_any_element(),
                    ),
                    _ => None,
                })
            })
            .children(
                project
                    .categories
                    .iter()
                    .map(|c| tag(c.clone(), colors::GRAY_400).into_any_element()),
            );
        // Modrinth and Discord are names
        let links: Vec<(&str, String, Option<String>)> = vec![
            (
                "modrinth",
                "Modrinth".into(),
                Some(modrinth_project_url(&project.project_type, &project.slug)),
            ),
            (
                "source",
                t(M::ModsSource).into(),
                project.source_url.clone(),
            ),
            (
                "issues",
                t(M::ModsIssues).into(),
                project.issues_url.clone(),
            ),
            ("wiki", t(M::ModsWiki).into(), project.wiki_url.clone()),
            ("discord", "Discord".into(), project.discord_url.clone()),
        ];
        let links = div().flex().flex_row().flex_wrap().gap(px(12.)).children(
            links.into_iter().filter_map(|(id, label, url)| {
                let url = safe_link(url.as_deref()?)?;
                Some(
                    link(SharedString::from(format!("project-link-{id}")), label)
                        .on_press(move |_, _, cx| cx.open_url(&url))
                        .into_any_element(),
                )
            }),
        );
        let header = div()
            .flex()
            .flex_row()
            .items_start()
            .gap(px(16.))
            .child(mod_icon(project.icon_url.as_deref(), 80., cx))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .gap(px(4.))
                    .child(
                        div()
                            .truncate()
                            .text_size(text::H1)
                            .font_weight(weights::BOLD.face())
                            .text_color(ink::text())
                            .child(project.title.clone()),
                    )
                    .child(muted(byline))
                    .child(
                        div()
                            .text_color(ink::text())
                            .child(project.description.clone()),
                    )
                    .child(stats)
                    .child(links),
            );
        // https images on Modrinth's CDN; a click opens the picture in the browser
        let gallery: Vec<AnyElement> = project
            .gallery
            .iter()
            .enumerate()
            .filter_map(|(index, image)| {
                let url = safe_link(&image.url)?;
                let picture = crate::instance_icons::modrinth_image(&image.url, cx);
                Some(
                    div()
                        .id(("gallery", index))
                        .flex_none()
                        .h(px(96.))
                        .min_w(px(96.))
                        .overflow_hidden()
                        .rounded(radius::MD)
                        .border_1()
                        .border_color(ink::border())
                        .bg(colors::GRAY_900)
                        .cursor_pointer()
                        .when_some(picture, |this, picture| this.child(img(picture).h(px(96.))))
                        .when_some(image.title.clone(), |this, title| {
                            this.tooltip(text_tooltip(title))
                        })
                        .on_click(move |_, _, cx| cx.open_url(&url))
                        .into_any_element(),
                )
            })
            .collect();
        let action: AnyElement = if is_modpack {
            if self.can_create(cx) {
                let preset = ModpackPreset {
                    project_id: project.id.clone(),
                    version_id: view.linked_version_id.clone(),
                };
                Button::new("create-from-modpack", t(M::ModsCreateFromModpack))
                    .variant(ButtonVariant::Primary)
                    .size(ButtonSize::Large)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let preset = preset.clone();
                        this.state
                            .update(cx, |state, cx| state.open_create_modpack(preset, cx));
                    }))
                    .into_any_element()
            } else {
                muted(t(M::ModsModpackNoPermission))
            }
        } else if installable {
            div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .when(project.server_side == "unsupported", |this| {
                    this.child(warning(t(M::ModsNoServerSupport)))
                })
                .child(self.versions_section(target, cx))
                .into_any_element()
        } else {
            warning(if target.is_plugin() {
                tr!(M::ModsWrongTypePlugin, type = project.project_type)
            } else {
                tr!(M::ModsWrongTypeMod, type = project.project_type)
            })
        };
        let description = view.blocks.clone().map(|blocks| {
            let mut images: HashMap<String, Arc<RenderImage>> = HashMap::new();
            for url in lodestone_ui::markdown::image_urls(&blocks).iter().take(20) {
                if let Some(image) = crate::instance_icons::modrinth_image(url, cx) {
                    images.insert(url.clone(), image);
                }
            }
            lodestone_ui::markdown::render(
                format!("project-description-{}", project.id),
                &blocks,
                &images,
                t(M::ModsRestOnModrinth),
            )
        });
        column
            .child(header)
            .when(!gallery.is_empty(), |this| {
                this.child(
                    div()
                        .id("gallery")
                        .flex()
                        .flex_row()
                        .gap(px(8.))
                        .overflow_x_scroll()
                        .pb(px(4.))
                        .children(gallery),
                )
            })
            .child(action)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(text::H2)
                            .font_weight(weights::BOLD.face())
                            .text_color(ink::text())
                            .child(t(M::ModsDescription)),
                    )
                    .child(
                        div()
                            .p(px(16.))
                            .rounded(radius::LG)
                            .border_1()
                            .border_color(ink::border())
                            .bg(colors::GRAY_850)
                            .children(description),
                    ),
            )
            .into_any_element()
    }
}
