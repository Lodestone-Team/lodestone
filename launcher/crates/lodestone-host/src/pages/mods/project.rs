//! A Modrinth project picked in Browse (`ModrinthProjectView.tsx`): its header, links,
//! gallery and description, and its versions to install; for a modpack, a button that
//! makes a server from it instead.
//!
//! A CurseForge project is shown the same way: the Host asks CurseForge for it and
//! turns what it gets into the values Modrinth's have
//! (`launcher_core::cores::admin::curseforge`); its files are its versions. CurseForge
//! has no followers and doesn't say which side a mod runs on, so those facts are left
//! out.

use std::{collections::HashMap, sync::Arc};

use gpui::{
    AnyElement, AppContext as _, Context, IntoElement, ParentElement, RenderImage, SharedString,
    Styled, div, img, prelude::*,
};
use launcher_core::{
    content::curseforge::{CfFile, CfMod},
    cores::admin::{
        curseforge as server,
        mods::{
            ModInstallRequest, ModrinthProject, ModrinthSearchHit, ModrinthVersion,
            ModrinthVersionType, is_version_compatible, modrinth_project_url,
        },
    },
    markdown::{Block, safe_link},
};
use lodestone_ui::{
    components::{
        Accessible as _, Button, ButtonSize, ButtonVariant, Checkbox, CountBadge, Fact, FactLine,
        FactValue, Icon, Kind, KindIcon, LoadingDots, Num, ProductPhrase, StatusBadge, VersionTag,
        settings::{card, link, tag},
        text_tooltip,
    },
    i18n::format,
    theme::{Tone, colors, ink, px, radius, text, weights},
    values::Product,
};

use super::{
    ModsPage, Prompt, Target,
    installed::{mod_icon, why_not},
    muted, warning,
};
use crate::{
    curseforge::Source,
    i18n::{CommonMsg, FactMsg, InstanceMsg as M, InstancePlural as P, t, tr, trn},
    pages::create_instance::ModpackPreset,
};

/// A loader's name as it spells it ("fabric" is Fabric, "neoforge" NeoForge), else
/// the id with a capital first letter.
pub(super) fn loader_name(id: &str) -> String {
    match Product::from_name(id) {
        Some(product) => product.name().to_string(),
        None => {
            let mut chars = id.chars();
            chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default()
        }
    }
}

/// "Minecraft 1.21.1, 1.21": the game with its versions, one plain phrase.
pub(super) fn game_versions(versions: &[String]) -> String {
    lodestone_ui::values::phrase(Product::Minecraft, &versions.join(", "))
}

/// A Modrinth date ("2026-10-02T…") as the app shows times.
fn published(date: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(date)
        .map(|time| {
            format::day_and_time(
                time.with_timezone(&chrono::Local),
                chrono::Local::now().date_naive(),
            )
        })
        .unwrap_or_else(|_| date.get(..10).unwrap_or(date).to_string())
}

/// The kind a Modrinth project type names, for its icon.
fn project_kind(project_type: &str) -> Kind {
    match project_type {
        "plugin" => Kind::Plugin,
        "modpack" => Kind::Modpack,
        "resourcepack" => Kind::ResourcePack,
        "shader" => Kind::ShaderPack,
        "datapack" => Kind::DataPack,
        _ => Kind::Mod,
    }
}

/// What a version made for something else is made for, and what the server runs.
#[derive(Clone, Debug, Default)]
pub(super) struct Mismatch {
    pub title: String,
    pub version: String,
    pub loaders: Vec<String>,
    pub game_versions: Vec<String>,
    pub loader: String,
    pub game_version: String,
}

/// Versions shown at first, and added by "Show more".
const VERSIONS_PAGE: usize = 20;

pub struct ProjectView {
    /// Where the project is. On CurseForge its id and its versions' ids are numbers.
    pub source: Source,
    /// The project as CurseForge gave it, for its page and its files' installs.
    curseforge: Option<CfMod>,
    /// The CurseForge files the listed versions are, by id.
    files: HashMap<String, CfFile>,
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
    pub(super) installing: Option<String>,
    /// Counts the versions' fetches, so only the newest is kept.
    serial: u64,
}

impl ProjectView {
    pub fn new(
        source: Source,
        project_id: String,
        linked_version_id: Option<String>,
        hit: Option<ModrinthSearchHit>,
    ) -> Self {
        Self {
            source,
            curseforge: None,
            files: HashMap::new(),
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

    /// Whether `version` runs on the server without "Install anyway": the core's rule
    /// for Modrinth's versions, the Launcher's for CurseForge's files.
    fn fits(&self, version: &ModrinthVersion, target: &Target) -> bool {
        match self.source {
            Source::Modrinth => is_version_compatible(version, target.loader, &target.game_version),
            Source::CurseForge => self
                .files
                .get(&version.id)
                .is_some_and(|file| target.server().fits(file)),
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
        source: Source,
        project_id: String,
        linked_version_id: Option<String>,
        hit: Option<ModrinthSearchHit>,
        cx: &mut Context<Self>,
    ) {
        self.project = Some(ProjectView::new(
            source,
            project_id.clone(),
            linked_version_id.clone(),
            hit,
        ));
        if source == Source::CurseForge {
            return self.open_curseforge_project(project_id, linked_version_id, cx);
        }
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

    /// Fetches a CurseForge project, its description and the file a link named.
    fn open_curseforge_project(
        &mut self,
        project_id: String,
        linked_file: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let fail = |this: &mut Self, why: String| {
            if let Some(view) = &mut this.project {
                view.project = Some(Err(why));
            }
        };
        let Some(curseforge) = crate::curseforge::client(cx) else {
            fail(self, t(M::ModsCurseForgeUnavailable).to_string());
            return cx.notify();
        };
        let Ok(id) = project_id.parse::<u64>() else {
            fail(self, t(M::ModsCurseForgeUnavailable).to_string());
            return cx.notify();
        };
        let linked_file = linked_file.and_then(|file| file.parse::<u64>().ok());
        let task = cx.spawn(async move |this, cx| {
            let fetched = cx
                .background_spawn(launcher_core::http::in_http_runtime(async move {
                    let project = curseforge.project(id).await?;
                    // a project without a description still has its files
                    let description = curseforge.description(id).await.unwrap_or_default();
                    let linked = match linked_file {
                        Some(file) => curseforge.file(id, file).await.ok(),
                        None => None,
                    };
                    Ok::<_, launcher_core::content::CurseForgeError>((project, description, linked))
                }))
                .await;
            this.update(cx, |this, cx| {
                let Some(view) = this.project.as_mut().filter(|view| {
                    view.source == Source::CurseForge && view.project_id == project_id
                }) else {
                    return;
                };
                match fetched {
                    Ok((project, description, linked)) => {
                        let shown = server::project(&project, description);
                        view.blocks = Some(Arc::new(launcher_core::markdown::parse(&shown.body)));
                        view.project = Some(Ok(shown));
                        view.curseforge = Some(project);
                        view.linked = linked.map(|file| {
                            let version = server::version(&file);
                            view.files.insert(version.id.clone(), file);
                            version
                        });
                    }
                    Err(err) => view.project = Some(Err(err.to_string())),
                }
                this.load_versions(cx);
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// Fetches a CurseForge project's files: the ones for the server, or all of them.
    fn load_curseforge_files(&mut self, target: Target, cx: &mut Context<Self>) {
        let Some(view) = &mut self.project else {
            return;
        };
        let (Some(curseforge), Ok(id)) = (
            crate::curseforge::client(cx),
            view.project_id.parse::<u64>(),
        ) else {
            return;
        };
        view.serial += 1;
        let (serial, all) = (view.serial, view.show_all);
        let task = cx.spawn(async move |this, cx| {
            let files = cx
                .background_spawn(launcher_core::http::in_http_runtime(async move {
                    server::files(&curseforge, id, &target.server(), all).await
                }))
                .await;
            this.update(cx, |this, cx| {
                if let Some(view) = &mut this.project
                    && view.serial == serial
                {
                    view.versions = Some(match files {
                        Ok(files) => Ok(files
                            .into_iter()
                            .map(|file| {
                                let version = server::version(&file);
                                view.files.insert(version.id.clone(), file);
                                version
                            })
                            .collect()),
                        Err(err) => Err(err.to_string()),
                    });
                    cx.notify();
                }
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
        if view.source == Source::CurseForge {
            return self.load_curseforge_files(target, cx);
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
        if self
            .project
            .as_ref()
            .is_some_and(|view| view.fits(version, &target))
        {
            return self.install_version(version.id.clone(), false, cx);
        }
        let title = self
            .project
            .as_ref()
            .and_then(|v| v.project.as_ref()?.as_ref().ok())
            .map(|p| p.title.clone())
            .unwrap_or_default();
        self.open_prompt(
            Prompt::Incompatible {
                version_id: version.id.clone(),
                mismatch: Mismatch {
                    title,
                    version: version.version_number.clone(),
                    loaders: version.loaders.clone(),
                    game_versions: version.game_versions.clone(),
                    loader: target.loader.as_str().to_string(),
                    game_version: target.game_version.clone(),
                },
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
        if view.source == Source::CurseForge {
            // the Host downloads the file and uploads it to the core
            let Some(file) = view.files.get(&version_id).cloned() else {
                view.installing = None;
                return;
            };
            let (project, with_dependencies) = (view.curseforge.clone(), view.with_dependencies);
            cx.notify();
            return self.install_curseforge(
                file,
                project,
                with_dependencies,
                force,
                (title, number),
                false,
                None,
                cx,
            );
        }
        let request = ModInstallRequest {
            version_id,
            install_dependencies: view.with_dependencies,
            force,
        };
        cx.notify();
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        // this tab says how the install went; the core's event needn't
        self.state
            .update(cx, |state, _| state.expect_own_file_toast(&uuid));
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
                        Some((title, number)),
                        t(M::ModsInstalledVersion).to_string(),
                        &result,
                    ),
                    Err(err) => lodestone_ui::components::Toast::error(tr!(
                        M::ModsInstallFailed,
                        error = err
                    ))
                    .object(lodestone_ui::components::ObjectChip::new(title).kind(Kind::Mod)),
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
        let compatible = self
            .project
            .as_ref()
            .is_some_and(|view| view.fits(version, target));
        let installing = self
            .project
            .as_ref()
            .and_then(|v| v.installing.as_ref())
            .is_some_and(|id| *id == version.id);
        let (type_label, type_tone) = match version.version_type {
            ModrinthVersionType::Release => (t(M::ModsRelease), Tone::Success),
            ModrinthVersionType::Beta => (t(M::ModsBeta), Tone::Warning),
            ModrinthVersionType::Alpha => (t(M::ModsAlpha), Tone::Danger),
            ModrinthVersionType::Unknown => ("?", Tone::Neutral),
        };
        // the game versions (the first few, then a count whose tooltip has them all), the
        // loaders by name, and when it was published
        let shown: Vec<String> = version.game_versions.iter().take(3).cloned().collect();
        let rest = version.game_versions.len().saturating_sub(shown.len());
        let games = game_versions(&shown);
        let loaders: Vec<String> = version.loaders.iter().map(|l| loader_name(l)).collect();
        let games_value = FactValue::element(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(4.))
                .child(div().flex_none().whitespace_nowrap().child(games.clone()))
                .when(rest > 0, |this| {
                    this.child(
                        div()
                            .id(SharedString::from(format!("version-{index}-more")))
                            .child(CountBadge::new(rest as u64))
                            .tooltip(text_tooltip(game_versions(&version.game_versions))),
                    )
                }),
            game_versions(&version.game_versions),
        );
        let details = FactLine::row(SharedString::from(format!("version-{index}-facts")))
            .fact(Fact::bare_named(t(FactMsg::Game), games_value).priority(1))
            .fact(Fact::bare_named(t(FactMsg::Loader), loaders.join(", ")).priority(2))
            .fact(Fact::new(
                t(FactMsg::Published),
                FactValue::time(published(&version.date_published)),
            ));
        let picked = version.clone();
        let id = |part: &str| SharedString::from(format!("version-{index}-{part}"));
        // A CurseForge file has a name ("Lithium mc1.21.4-0.14.8 Fabric"), not a version
        // number: shown whole, where a version tag would cut its middle out and leave
        // a project's files looking alike.
        let on_curseforge = self
            .project
            .as_ref()
            .is_some_and(|view| view.source == Source::CurseForge);
        let number: AnyElement = if on_curseforge {
            div()
                .id(id("number"))
                .min_w_0()
                .truncate()
                .font_weight(weights::MEDIUMBOLD.face())
                .text_color(ink::text())
                .child(version.version_number.clone())
                .into_any_element()
        } else {
            VersionTag::new(version.version_number.clone())
                .id(id("number"))
                .into_any_element()
        };
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
                            .min_w_0()
                            .child(number)
                            .child(StatusBadge::new(type_label, type_tone))
                            .when(installed_here, |this| {
                                this.child(StatusBadge::new(t(M::ModsInstalledTag), Tone::Success))
                            })
                            .when(!compatible, |this| {
                                this.child(StatusBadge::new(t(M::ModsNotFor), Tone::Neutral))
                            }),
                    )
                    .child(details),
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
        let installed_ids: std::collections::HashSet<String> = match view.source {
            Source::Modrinth => self
                .list
                .iter()
                .flat_map(|l| l.mods.iter())
                .filter_map(|m| m.modrinth.as_ref().map(|m| m.version_id.clone()))
                .collect(),
            Source::CurseForge => self
                .curseforge
                .values()
                .map(|installed| installed.file_id.to_string())
                .collect(),
        };
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
                // CurseForge's files come already limited to the server's, unless all
                // were asked for
                let list = listed_versions(
                    versions,
                    view.linked.as_ref(),
                    &view.project_id,
                    target,
                    view.show_all || view.source == Source::CurseForge,
                );
                if list.is_empty() {
                    // what the server runs, that no version is for
                    let game = ProductPhrase::new(Product::Minecraft, target.game_version.clone());
                    let loader = ProductPhrase::from_name(target.loader.as_str(), "");
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(muted(t(if view.show_all {
                            M::ModsNoVersions
                        } else {
                            M::ModsNoVersionsShowAll
                        })))
                        .child(
                            FactLine::strip("no-versions-for")
                                .fact(Fact::new(game.key(), game))
                                .facts(loader.map(|loader| Fact::new(loader.key(), loader))),
                        )
                        .into_any_element()
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
        let on_curseforge = view.source == Source::CurseForge;
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
        // Modrinth's project doesn't name its author (the search result does);
        // CurseForge's does
        let author = view
            .hit
            .as_ref()
            .map(|h| h.author.clone())
            .or_else(|| Some(view.curseforge.as_ref()?.authors.first()?.name.clone()))
            .unwrap_or_else(|| project.slug.clone());
        // who made it, its downloads and followers, and whether servers need it
        let facts = FactLine::strip("project-facts")
            .fact(Fact::new(t(FactMsg::Author), FactValue::text(author)))
            .fact(Fact::new(
                t(FactMsg::Downloads),
                FactValue::element(
                    div()
                        .id("project-downloads")
                        .font_features(lodestone_ui::theme::fonts::tabular())
                        .child(format::compact(project.downloads))
                        .tooltip(text_tooltip(Num::count(project.downloads).plain())),
                    Num::count(project.downloads).plain(),
                ),
            ))
            // CurseForge has neither followers nor sides
            .facts(
                (!on_curseforge)
                    .then(|| Fact::new(t(FactMsg::Followers), Num::count(project.followers))),
            )
            .facts((!on_curseforge).then(|| {
                Fact::new(
                    t(M::ModsServerSide),
                    FactValue::text(side(&project.server_side)),
                )
            }));
        let stats = div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap(px(12.))
            .text_size(text::SMALL)
            .text_color(ink::muted())
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
            match &view.curseforge {
                Some(curseforge) if on_curseforge => (
                    "curseforge",
                    Source::CurseForge.name().into(),
                    Some(curseforge.page_url()),
                ),
                _ => (
                    "modrinth",
                    Source::Modrinth.name().into(),
                    Some(modrinth_project_url(&project.project_type, &project.slug)),
                ),
            },
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
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(8.))
                            .min_w_0()
                            .child(
                                div()
                                    .truncate()
                                    .text_size(text::H1)
                                    .font_weight(weights::BOLD.face())
                                    .text_color(ink::text())
                                    .child(project.title.clone()),
                            )
                            .child(KindIcon::new(project_kind(&project.project_type))),
                    )
                    .child(facts)
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
                let picture = crate::instance_icons::project_image(&image.url, cx);
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
                    source: view.source,
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
                if let Some(image) = crate::instance_icons::project_image(url, cx) {
                    images.insert(url.clone(), image);
                }
            }
            lodestone_ui::markdown::render(
                format!("project-description-{}", project.id),
                &blocks,
                &images,
                t(if on_curseforge {
                    M::ModsRestOnCurseForge
                } else {
                    M::ModsRestOnModrinth
                }),
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
