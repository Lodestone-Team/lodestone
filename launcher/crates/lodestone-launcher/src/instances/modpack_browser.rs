//! "New instance" → Modpack: Modrinth's modpacks (search, sort, a pasted link), the
//! picked pack's versions, and its description.

use std::{collections::HashMap, sync::Arc, time::Duration};

use gpui::{
    AnyElement, AppContext as _, Context, Entity, IntoElement, Render, RenderImage, SharedString,
    Subscription, Task, Window, div, prelude::*, px,
};
use launcher_core::content::Modrinth;
use lodestone_minecraft::modrinth::{
    ClientProjectType, ClientSearchParams, ModrinthProject, ModrinthSearchIndex,
    ModrinthSearchResponse, ModrinthVersionType, RawVersion,
};

use super::{
    Instances,
    content::{
        Load, RemoteIcon,
        view::{badge, icon_box, pills, short_list, small, version_label},
    },
};
use crate::components::KeyboardFocus;
use crate::{
    components::{LoadingDots, TextInput},
    i18n::{InstancesMsg as M, format, t, tr},
    theme::{self, colors, fonts, radius, text, weights},
};

const SEARCH_DELAY: Duration = Duration::from_millis(300);
const PAGE_SIZE: u32 = 20;
const SORTS: [(ModrinthSearchIndex, M); 4] = [
    (ModrinthSearchIndex::Relevance, M::SortRelevance),
    (ModrinthSearchIndex::Downloads, M::SortDownloads),
    (ModrinthSearchIndex::Follows, M::SortFollows),
    (ModrinthSearchIndex::Updated, M::SortUpdated),
];

/// The search column's width: wider in French, whose sort labels are longer.
fn list_width() -> f32 {
    match crate::i18n::locale() {
        crate::i18n::Locale::Fr => 430.,
        _ => 360.,
    }
}

/// A Modrinth project type in a sentence ("a mod"), or the type as Modrinth names it.
fn project_kind(project_type: &str) -> String {
    let message = match project_type {
        "mod" => M::KindMod,
        "resourcepack" => M::KindResourcePack,
        "shader" => M::KindShaderPack,
        "datapack" => M::KindDataPack,
        "plugin" => M::KindPlugin,
        other => return other.to_string(),
    };
    t(message).into()
}

/// When a version was published: "Mar 12, 2025", from Modrinth's ISO 8601 time.
fn published(date: &str) -> String {
    date.get(..10)
        .and_then(|day| chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d").ok())
        .map_or_else(String::new, |day| format::short_date(day, true))
}

/// The picked pack.
pub struct Picked {
    pub project: Load<ModrinthProject>,
    pub versions: Load<Vec<RawVersion>>,
    pub version: Option<String>,
    pub show_description: bool,
    _task: Option<Task<()>>,
}

pub struct ModpackBrowser {
    instances: Entity<Instances>,
    pub search: Entity<TextInput>,
    text: String,
    pub index: ModrinthSearchIndex,
    pub results: Load<ModrinthSearchResponse>,
    pub picked: Option<Picked>,
    icons: HashMap<String, RemoteIcon>,
    images: HashMap<String, RemoteIcon>,
    descriptions: HashMap<String, Arc<Vec<launcher_core::markdown::Block>>>,
    task: Option<Task<()>>,
    _observe: Vec<Subscription>,
}

impl ModpackBrowser {
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let search = cx.new(|cx| TextInput::new(cx).with_placeholder(M::SearchModpacks));
        let observe = vec![cx.observe(&search, |this, _, cx| this.search_changed(cx))];
        Self {
            instances,
            search,
            text: String::new(),
            index: ModrinthSearchIndex::Relevance,
            results: Load::NotLoaded,
            picked: None,
            icons: HashMap::new(),
            images: HashMap::new(),
            descriptions: HashMap::new(),
            task: None,
            _observe: observe,
        }
    }

    fn api(&self, cx: &gpui::App) -> Option<Arc<Modrinth>> {
        self.instances.read(cx).modrinth()
    }

    /// The pack and version to install, once both are known.
    pub fn selection(&self) -> Option<(ModrinthProject, RawVersion)> {
        let picked = self.picked.as_ref()?;
        let project = picked.project.ready()?.clone();
        let id = picked.version.as_ref()?;
        let version = picked
            .versions
            .ready()?
            .iter()
            .find(|v| &v.id == id)?
            .clone();
        Some((project, version))
    }

    fn search_changed(&mut self, cx: &mut Context<Self>) {
        let text = self.search.read(cx).text().to_string();
        if text == self.text {
            return;
        }
        self.text = text.clone();
        self.task = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_DELAY).await;
            this.update(cx, |this, cx| {
                if text.contains("modrinth.com/") {
                    this.open_link(&text, cx);
                } else {
                    this.run_search(cx);
                }
            })
            .ok();
        }));
    }

    pub fn ensure_searched(&mut self, cx: &mut Context<Self>) {
        if matches!(self.results, Load::NotLoaded) {
            self.run_search(cx);
        }
    }

    pub fn set_index(&mut self, index: ModrinthSearchIndex, cx: &mut Context<Self>) {
        self.index = index;
        self.run_search(cx);
    }

    fn run_search(&mut self, cx: &mut Context<Self>) {
        let Some(api) = self.api(cx) else {
            return;
        };
        let params = match ClientSearchParams::new(
            Some(self.text.clone()),
            ClientProjectType::Modpack,
            None,
            Vec::new(),
            Some(self.index),
            Some(0),
            Some(PAGE_SIZE),
        ) {
            Ok(params) => params,
            Err(err) => {
                self.results = Load::Failed(err.to_string().into());
                cx.notify();
                return;
            }
        };
        self.results = Load::Loading;
        let work = cx.background_spawn(async move { api.search(&params).await });
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.results = match result {
                    Ok(results) => Load::Ready(results),
                    Err(err) => Load::Failed(err.to_string().into()),
                };
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    fn open_link(&mut self, link: &str, cx: &mut Context<Self>) {
        let Some(api) = self.api(cx) else {
            return;
        };
        let link = link.to_string();
        self.results = Load::Loading;
        let work = cx.background_spawn(async move { api.resolve_link(&link).await });
        self.task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                match result {
                    Ok(target) if target.project.project_type == "modpack" => {
                        this.results = Load::NotLoaded;
                        this.run_search(cx);
                        this.pick(target.project.id.clone(), target.version.map(|v| v.id), cx);
                    }
                    Ok(target) => {
                        this.results = Load::Failed(
                            tr!(
                                M::LinkNotModpack,
                                kind = project_kind(&target.project.project_type)
                            )
                            .into(),
                        )
                    }
                    Err(err) => this.results = Load::Failed(err.to_string().into()),
                }
                cx.notify();
            })
            .ok();
        }));
        cx.notify();
    }

    /// Opens a pack with its versions; `version` (from a link) is picked, else the newest.
    pub fn pick(&mut self, project_id: String, version: Option<String>, cx: &mut Context<Self>) {
        let Some(api) = self.api(cx) else {
            return;
        };
        let id = project_id.clone();
        let work = cx.background_spawn(async move {
            let project = api.project(&id).await;
            let versions = api.versions(&id, &[], &[]).await;
            (project, versions)
        });
        let task = cx.spawn(async move |this, cx| {
            let (project, versions) = work.await;
            this.update(cx, |this, cx| {
                if let Some(picked) = &mut this.picked {
                    picked.project = match project {
                        Ok(project) => Load::Ready(project),
                        Err(err) => Load::Failed(err.to_string().into()),
                    };
                    match versions {
                        Ok(versions) => {
                            if picked.version.is_none() {
                                picked.version = default_version(&versions);
                            }
                            picked.versions = Load::Ready(versions);
                        }
                        Err(err) => picked.versions = Load::Failed(err.to_string().into()),
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.picked = Some(Picked {
            project: Load::Loading,
            versions: Load::Loading,
            version,
            show_description: false,
            _task: Some(task),
        });
        cx.notify();
    }

    fn icon(&mut self, url: &str, cx: &mut Context<Self>) -> Option<Arc<RenderImage>> {
        let api = self.api(cx);
        fetch_image(&mut self.icons, url, false, api, cx, |this| &mut this.icons)
    }

    /// Shows prepared results and a picked pack, for the offscreen screenshots.
    pub fn preview(
        &mut self,
        text: &str,
        results: ModrinthSearchResponse,
        project: ModrinthProject,
        versions: Vec<RawVersion>,
        icons: HashMap<String, Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        self.text = text.to_string();
        self.search
            .update(cx, |input, cx| input.set_text(text.to_string(), cx));
        self.results = Load::Ready(results);
        self.picked = Some(Picked {
            project: Load::Ready(project),
            version: default_version(&versions),
            versions: Load::Ready(versions),
            show_description: false,
            _task: None,
        });
        for (url, icon) in icons {
            self.icons.insert(url, RemoteIcon::Ready(icon));
        }
        cx.notify();
    }
}

/// The image at a Modrinth CDN URL from `cache`, fetched the first time (`large` for
/// description images, else icons).
fn fetch_image<V: 'static>(
    cache: &mut HashMap<String, RemoteIcon>,
    url: &str,
    large: bool,
    api: Option<Arc<Modrinth>>,
    cx: &mut Context<V>,
    field: fn(&mut V) -> &mut HashMap<String, RemoteIcon>,
) -> Option<Arc<RenderImage>> {
    match cache.get(url) {
        Some(RemoteIcon::Ready(image)) => return Some(image.clone()),
        Some(_) => return None,
        None => {}
    }
    let api = api?;
    cache.insert(url.to_string(), RemoteIcon::Loading);
    let key = url.to_string();
    let work = cx.background_spawn(async move {
        let bytes = api.image(&key).await.ok()?;
        if large {
            crate::images::decode_large(&bytes)
        } else {
            launcher_core::content::scan::decode_icon(&bytes).map(|i| (*i).clone())
        }
    });
    let url = url.to_string();
    cx.spawn(async move |this, cx| {
        let result = work.await;
        this.update(cx, |this, cx| {
            let image = match result {
                Some(image) => RemoteIcon::Ready(crate::images::render_image(image)),
                None => RemoteIcon::Failed,
            };
            field(this).insert(url, image);
            cx.notify();
        })
        .ok();
    })
    .detach();
    None
}

fn loading(label: M) -> AnyElement {
    div()
        .flex()
        .flex_row()
        .items_center()
        .gap_2()
        .p_3()
        .text_color(colors::GRAY_500)
        .child(LoadingDots)
        .child(t(label))
        .into_any_element()
}

impl ModpackBrowser {
    fn render_results(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let results = match &self.results {
            Load::NotLoaded | Load::Loading => return loading(M::SearchingModrinth),
            Load::Failed(err) => {
                return small(err.clone())
                    .p_3()
                    .text_color(colors::RED_200)
                    .into_any_element();
            }
            Load::Ready(results) => results.clone(),
        };
        if results.hits.is_empty() {
            return small(t(M::NoModpacksMatch))
                .p_3()
                .text_color(colors::GRAY_500)
                .into_any_element();
        }
        let picked_id = self
            .picked
            .as_ref()
            .and_then(|p| p.project.ready().map(|p| p.id.clone()));
        let mut list = div().flex().flex_col().gap_1();
        for (index, hit) in results.hits.iter().enumerate() {
            let icon = hit.icon_url.as_deref().and_then(|url| self.icon(url, cx));
            let selected = picked_id.as_deref() == Some(hit.project_id.as_str());
            let id = hit.project_id.clone();
            list = list.child(
                div()
                    .id(("pack-hit", index))
                    .keyboard_focus()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap_3()
                    .px_2()
                    .py_1p5()
                    .rounded(radius::MD)
                    .cursor_pointer()
                    .map(|this| {
                        if selected {
                            this.bg(colors::BLUE.opacity(35))
                        } else {
                            this.hover(|style| style.bg(colors::GRAY_800))
                        }
                    })
                    .on_click(cx.listener(move |this, _, _, cx| this.pick(id.clone(), None, cx)))
                    .child(icon_box(icon, 36.))
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
                                    .child(hit.title.clone()),
                            )
                            .child(
                                small(tr!(
                                    M::ByAuthor,
                                    author = hit.author,
                                    downloads = format::compact(hit.downloads)
                                ))
                                .truncate()
                                .text_color(colors::GRAY_500),
                            ),
                    ),
            );
        }
        list.into_any_element()
    }

    fn render_picked(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(picked) = &self.picked else {
            return div()
                .flex()
                .items_center()
                .justify_center()
                .size_full()
                .p_4()
                .text_center()
                .text_color(colors::GRAY_500)
                .child(t(M::PickModpack))
                .into_any_element();
        };
        let project = match &picked.project {
            Load::Ready(project) => project.clone(),
            Load::Failed(err) => {
                return small(err.clone())
                    .p_3()
                    .text_color(colors::RED_200)
                    .into_any_element();
            }
            _ => return loading(M::LoadingModpack),
        };
        let versions = picked.versions.clone();
        let chosen = picked.version.clone();
        let show_description = picked.show_description;
        let icon = project
            .icon_url
            .as_deref()
            .and_then(|url| self.icon(url, cx));
        let header = div()
            .flex()
            .flex_row()
            .items_start()
            .gap_3()
            .child(icon_box(icon, 48.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .font_family(fonts::HEADING)
                            .text_size(theme::text_size(text::H3))
                            .font_weight(weights::BOLD.into())
                            .text_color(colors::GRAY_300)
                            .child(project.title.clone()),
                    )
                    .child(
                        small(project.description.clone())
                            .text_color(colors::GRAY_400)
                            .line_clamp(2),
                    ),
            );
        let has_body = !project.body.trim().is_empty();
        let entity = cx.entity();
        let switch = has_body.then(|| {
            pills(
                "pack-tabs",
                vec![t(M::Versions).into(), t(M::Description).into()],
                usize::from(show_description),
                move |index, _, cx| {
                    entity.update(cx, |this, cx| {
                        if let Some(picked) = &mut this.picked {
                            picked.show_description = index == 1;
                        }
                        cx.notify();
                    })
                },
            )
        });
        let body: AnyElement = if show_description && has_body {
            let blocks = self
                .descriptions
                .entry(project.id.clone())
                .or_insert_with(|| Arc::new(launcher_core::markdown::parse(&project.body)))
                .clone();
            let mut images = HashMap::new();
            for url in crate::markdown::image_urls(&blocks).iter().take(20) {
                let api = self.api(cx);
                if let Some(image) = fetch_image(&mut self.images, url, true, api, cx, |this| {
                    &mut this.images
                }) {
                    images.insert(url.clone(), image);
                }
            }
            crate::markdown::render(format!("pack-description-{}", project.id), &blocks, &images)
        } else {
            match versions {
                Load::Ready(list) if list.is_empty() => small(t(M::NoVersions))
                    .text_color(colors::GRAY_500)
                    .into_any_element(),
                Load::Ready(list) => {
                    let mut rows = div().flex().flex_col().gap_0p5();
                    for (index, version) in list.iter().take(50).enumerate() {
                        let selected = chosen.as_deref() == Some(version.id.as_str());
                        let (label, color) = version_label(version.version_type);
                        let size = launcher_core::modpack::import::pack_file(version)
                            .map_or(0, |f| f.size);
                        let id = version.id.clone();
                        rows = rows.child(
                            div()
                                .id(("pack-version", index))
                                .keyboard_focus()
                                .flex()
                                .flex_row()
                                .items_center()
                                .gap_2()
                                .px_2()
                                .py_1()
                                .rounded(radius::MD)
                                .cursor_pointer()
                                .map(|this| {
                                    if selected {
                                        this.bg(colors::BLUE.opacity(35))
                                    } else {
                                        this.hover(|style| style.bg(colors::GRAY_800))
                                    }
                                })
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if let Some(picked) = &mut this.picked {
                                        picked.version = Some(id.clone());
                                    }
                                    cx.notify();
                                }))
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
                                                .child(badge(label, color)),
                                        )
                                        .child(
                                            small(format!(
                                                "{} · {} · {} · {}",
                                                short_list(&version.game_versions, 2),
                                                short_list(&version.loaders, 2),
                                                published(&version.date_published),
                                                format::bytes_binary(size)
                                            ))
                                            .truncate()
                                            .text_color(colors::GRAY_500),
                                        ),
                                ),
                        );
                    }
                    rows.into_any_element()
                }
                Load::Failed(err) => small(err).text_color(colors::RED_200).into_any_element(),
                _ => loading(M::LoadingVersions),
            }
        };
        div()
            .flex()
            .flex_col()
            .gap_2()
            .size_full()
            .p_2()
            .child(header)
            .children(switch.map(|s| div().flex().child(s)))
            .child(
                div()
                    .id("pack-detail")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
            .into_any_element()
    }
}

impl Render for ModpackBrowser {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.ensure_searched(cx);
        let entity = cx.entity();
        let selected = SORTS
            .iter()
            .position(|(i, _)| *i == self.index)
            .unwrap_or(0);
        let sorts = pills(
            "pack-sort",
            SORTS
                .iter()
                .map(|(_, label)| SharedString::from(t(*label)))
                .collect(),
            selected,
            move |index, _, cx| {
                entity.update(cx, |this, cx| this.set_index(SORTS[index].0, cx));
            },
        );
        div()
            .flex()
            .flex_row()
            .gap_4()
            .h(px(380.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .w(px(list_width()))
                    .flex_none()
                    .child(self.search.clone())
                    .child(div().flex().child(sorts))
                    .child(
                        div()
                            .id("pack-results")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .p_1()
                            .rounded(radius::LG)
                            .border_1()
                            .border_color(colors::GRAY_FADED.opacity(30))
                            .bg(colors::GRAY_900)
                            .child(self.render_results(cx)),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .rounded(radius::LG)
                    .border_1()
                    .border_color(colors::GRAY_FADED.opacity(30))
                    .bg(colors::GRAY_900)
                    .child(self.render_picked(cx)),
            )
    }
}

/// The version picked when a modpack opens: the newest release, as the dashboard does,
/// or the newest version when the pack has no releases. Modrinth lists newest first.
fn default_version(versions: &[RawVersion]) -> Option<String> {
    versions
        .iter()
        .find(|v| v.version_type == ModrinthVersionType::Release)
        .or_else(|| versions.first())
        .map(|v| v.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(id: &str, kind: &str) -> RawVersion {
        serde_json::from_value(serde_json::json!({
            "id": id, "project_id": "p", "version_type": kind
        }))
        .unwrap()
    }

    #[test]
    fn a_modpack_opens_on_its_newest_release() {
        // Modrinth's order, newest first: a beta above the newest release.
        let versions = [
            version("beta3", "beta"),
            version("rel2", "release"),
            version("beta2", "beta"),
            version("rel1", "release"),
        ];
        assert_eq!(default_version(&versions).as_deref(), Some("rel2"));
        // No release at all: the newest version.
        let betas = [version("b2", "beta"), version("a1", "alpha")];
        assert_eq!(default_version(&betas).as_deref(), Some("b2"));
        assert_eq!(default_version(&[]), None);
    }
}
