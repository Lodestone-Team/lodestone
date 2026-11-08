//! The Content tab (mods, resource packs, shader packs from Modrinth or files) and
//! modpacks (the New instance dialog's Modpack mode, and the Settings tab's update).
//!
//! The Content tab keeps its steps in its view, for the selected instance; these are
//! the same `launcher_core::content` calls in the same order, for any instance, with
//! the tab's checks: nothing changes while the game runs, and one job at a time.

use std::{path::PathBuf, rc::Rc, sync::Arc, time::Duration};

use gpui::{App, AppContext as _, AsyncApp};
use launcher_control::{ErrorObject, codes};
use launcher_core::{
    content::{
        ContentKind, InstanceDirs, Modrinth, Target, files,
        install::{self, InstallOutcome, InstallProgress, InstallRequest},
        listing::{self, Item},
        loader_change,
        shaders::{self, ShaderModOffer},
    },
    game::download::CancelToken,
    instances::{Instance, Loader},
};
use lodestone_minecraft::modrinth::{
    ClientProjectType, ClientSearchParams, ModrinthProject, ModrinthSearchIndex,
    ModrinthVersionType, RawVersion,
};
use serde::Deserialize;
use serde_json::{Value, json};

use super::{
    ApiResult, Call, Control, Reply, api_error,
    instances::{absolute, failed, parse_loader, params, tri},
    jobs::{Cancel, JobSpec, Progress},
    poll_until, ready,
};
use crate::instances::settings::update_candidate;

/// Results per search page, as the Content tab shows.
const PAGE_SIZE: u32 = 20;

/// `mod`, `resourcepack` or `shader` (and their plurals and spellings).
pub fn parse_kind(text: &str) -> Result<ContentKind, ErrorObject> {
    match text
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '_', ' '], "")
        .as_str()
    {
        "mod" | "mods" => Ok(ContentKind::Mod),
        "resourcepack" | "resourcepacks" | "texturepack" | "pack" => Ok(ContentKind::ResourcePack),
        "shader" | "shaders" | "shaderpack" | "shaderpacks" => Ok(ContentKind::ShaderPack),
        other => Err(api_error(
            codes::INVALID_PARAMS,
            format!("No content kind \"{other}\". Kinds: mod, resourcepack, shader."),
        )),
    }
}

fn kind_name(kind: ContentKind) -> &'static str {
    match kind {
        ContentKind::Mod => "mod",
        ContentKind::ResourcePack => "resourcepack",
        ContentKind::ShaderPack => "shader",
    }
}

fn sort(text: Option<&str>) -> Result<Option<ModrinthSearchIndex>, ErrorObject> {
    Ok(Some(match text.map(str::to_ascii_lowercase).as_deref() {
        None | Some("relevance") => ModrinthSearchIndex::Relevance,
        Some("downloads") => ModrinthSearchIndex::Downloads,
        Some("follows") => ModrinthSearchIndex::Follows,
        Some("newest") => ModrinthSearchIndex::Newest,
        Some("updated") => ModrinthSearchIndex::Updated,
        Some(other) => {
            return Err(api_error(
                codes::INVALID_PARAMS,
                format!("No sort \"{other}\". Sorts: relevance, downloads, follows, newest, updated."),
            ));
        }
    }))
}

fn modrinth(control: &Control, cx: &App) -> Result<Arc<Modrinth>, ErrorObject> {
    control
        .instances
        .read(cx)
        .modrinth()
        .ok_or_else(|| api_error(codes::UNAVAILABLE, "Modrinth isn't available."))
}

/// The instance's folders and what it accepts, with its shader mod, as the Content tab
/// works it out.
fn context(control: &Control, instance: &Instance, cx: &App) -> Result<(InstanceDirs, Target), ErrorObject> {
    let instances = control.instances.read(cx);
    let dirs = instances.content_dirs(&instance.id);
    let mut target = instances
        .content_target(&instance.id)
        .ok_or_else(|| api_error(codes::NOT_FOUND, "The instance is gone."))?;
    target.shader_loader = listing::shader_state(&dirs)
        .map_err(|err| api_error(codes::FAILED, err.to_string()))?
        .loader;
    Ok((dirs, target))
}

/// The Content tab's lock: nothing changes while the game runs or another change does.
fn can_change(control: &Control, instance: &Instance, cx: &App) -> Result<(), ErrorObject> {
    control.idle(instance, cx)?;
    if let Some(job) = control.jobs.borrow().running_for("content", &instance.id) {
        return Err(api_error(
            codes::BUSY,
            format!("Job {job} is changing {}'s content; wait for it (`jobs wait {job}`).", instance.name),
        ));
    }
    Ok(())
}

/// Shows the change in the window, if the instance is the one shown.
fn refresh_view(control: &Control, instance: &str, kinds: &[ContentKind], cx: &mut App) {
    let selected = control
        .instances
        .read(cx)
        .selected()
        .is_some_and(|s| s.id == instance);
    if let (true, Some(view)) = (selected, control.content.clone()) {
        view.update(cx, |view, cx| {
            for kind in kinds {
                view.reload(*kind, cx);
            }
        });
    }
}

fn item_json(item: &Item) -> Value {
    json!({
        "file": item.file.file_name,
        "name": item.display_name(),
        "version": item.display_version(),
        "enabled": item.enabled,
        "folder": item.file.is_dir,
        "size": item.file.size,
        "priority": item.priority,
        "compatibility": format!("{:?}", item.compatibility).to_lowercase(),
        "problem": item.file.meta.problem,
        "modrinth": item.modrinth.as_ref().map(|m| json!({
            "project_id": m.project_id,
            "slug": m.slug,
            "title": m.title,
            "version_id": m.version_id,
            "version_number": m.version_number,
        })),
    })
}

#[derive(Deserialize)]
struct ListParams {
    #[serde(default)]
    kind: Option<String>,
}

fn kinds(kind: Option<&str>) -> Result<Vec<ContentKind>, ErrorObject> {
    match kind {
        Some(kind) => Ok(vec![parse_kind(kind)?]),
        None => Ok(ContentKind::ALL.to_vec()),
    }
}

pub fn list(call: Call, cx: &mut App) -> Reply {
    let params: ListParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    let kinds = tri!(kinds(params.kind.as_deref()));
    let (dirs, target) = tri!(context(&control, &instance, cx));
    let api = control.instances.read(cx).modrinth();
    let work = cx.background_spawn(async move {
        let mut out = serde_json::Map::new();
        let mut shaders = Value::Null;
        for kind in kinds {
            let listing = listing::list(api.as_deref(), &dirs, &target, kind)
                .await
                .map_err(|err| api_error(codes::FAILED, err.to_string()))?;
            if kind == ContentKind::ShaderPack {
                shaders = json!({
                    "loader": listing.shaders.loader.map(|l| l.label()),
                    "selected": listing.shaders.selection.pack.clone(),
                });
            }
            out.insert(
                kind_name(kind).to_string(),
                json!({
                    "items": listing.items.iter().map(item_json).collect::<Vec<_>>(),
                    "lookup_error": listing.lookup_error,
                    "options_error": listing.options_error,
                }),
            );
        }
        Ok(json!({ "instance": target.game_version, "content": out, "shaders": shaders }))
    });
    let id = instance.id.clone();
    cx.spawn(async move |_| {
        let mut value = work.await?;
        value["instance"] = json!(id);
        Ok(value)
    })
}

#[derive(Deserialize)]
struct SearchParams {
    query: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    /// Without it, nothing is filtered by version or loader.
    #[serde(default)]
    instance: Option<String>,
    /// Also show content that doesn't fit the instance.
    #[serde(default)]
    all: bool,
    #[serde(default)]
    sort: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    offset: Option<u32>,
}

fn hits_json(response: &lodestone_minecraft::modrinth::ModrinthSearchResponse) -> Value {
    json!({
        "total": response.total_hits,
        "offset": response.offset,
        "hits": response.hits.iter().map(|h| json!({
            "project_id": h.project_id,
            "slug": h.slug,
            "title": h.title,
            "description": h.description,
            "author": h.author,
            "downloads": h.downloads,
            "follows": h.follows,
            "categories": h.categories,
            "versions": h.versions.iter().rev().take(5).collect::<Vec<_>>(),
            "updated": h.date_modified,
            "client_side": h.client_side,
            "url": format!("https://modrinth.com/{}/{}", h.project_type, h.slug),
        })).collect::<Vec<_>>(),
    })
}

pub fn search(call: Call, cx: &mut App) -> Reply {
    let params: SearchParams = params!(call);
    let control = call.control.clone();
    let kind = tri!(parse_kind(params.kind.as_deref().unwrap_or("mod")));
    let api = tri!(modrinth(&control, cx));
    let (game_version, loaders) = match &params.instance {
        Some(reference) if !params.all => {
            let instance = tri!(control.instance(reference, cx));
            let (_, target) = tri!(context(&control, &instance, cx));
            (Some(target.game_version.clone()), target.search_loaders(kind))
        }
        _ => (None, Vec::new()),
    };
    let search = tri!(
        ClientSearchParams::new(
            params.query.filter(|q| !q.trim().is_empty()),
            kind.project_type(),
            game_version.clone(),
            loaders.clone(),
            tri!(sort(params.sort.as_deref())),
            params.offset,
            Some(params.limit.unwrap_or(PAGE_SIZE)),
        )
        .map_err(|err| api_error(codes::INVALID_PARAMS, err.to_string()))
    );
    cx.background_spawn(async move {
        let response = api
            .search(&search)
            .await
            .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
        let mut value = hits_json(&response);
        value["kind"] = json!(kind_name(kind));
        value["filtered_by"] = json!({ "game_version": game_version, "loaders": loaders });
        Ok(value)
    })
}

/// A project by slug, id or modrinth.com link, with the version a link names.
async fn resolve_project(
    api: &Modrinth,
    reference: &str,
) -> Result<(ModrinthProject, Option<RawVersion>), ErrorObject> {
    let reference = reference.trim();
    if reference.contains("modrinth.com") || reference.starts_with("http") {
        let target = api
            .resolve_link(reference)
            .await
            .map_err(|err| api_error(codes::NOT_FOUND, err.to_string()))?;
        return Ok((target.project, target.version));
    }
    let project = api
        .project(reference)
        .await
        .map_err(|err| api_error(codes::NOT_FOUND, format!("Modrinth has no project \"{reference}\": {err}")))?;
    Ok((project, None))
}

fn version_json(version: &RawVersion, target: Option<(&Target, ContentKind)>) -> Value {
    json!({
        "id": version.id,
        "version_number": version.version_number,
        "name": version.name,
        "type": format!("{:?}", version.version_type).to_lowercase(),
        "game_versions": version.game_versions,
        "loaders": version.loaders,
        "published": version.date_published,
        "downloads": version.downloads,
        "incompatible": target.and_then(|(target, kind)| target.incompatibility(kind, version)),
    })
}

#[derive(Deserialize)]
struct VersionsParams {
    project: String,
    #[serde(default)]
    instance: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

pub fn versions(call: Call, cx: &mut App) -> Reply {
    let params: VersionsParams = params!(call);
    let control = call.control.clone();
    let api = tri!(modrinth(&control, cx));
    let target = match &params.instance {
        Some(reference) => {
            let instance = tri!(control.instance(reference, cx));
            Some(tri!(context(&control, &instance, cx)).1)
        }
        None => None,
    };
    cx.background_spawn(async move {
        let (project, _) = resolve_project(&api, &params.project).await?;
        let kind = ContentKind::from_project_type(&project.project_type);
        let versions = api
            .versions(&project.id, &[], &[])
            .await
            .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
        let fit = target.as_ref().zip(kind);
        Ok(json!({
            "project": { "id": project.id, "slug": project.slug, "title": project.title, "type": project.project_type },
            "versions": versions
                .iter()
                .take(params.limit.unwrap_or(usize::MAX))
                .map(|v| version_json(v, fit))
                .collect::<Vec<_>>(),
        }))
    })
}

#[derive(Deserialize)]
struct InstallParams {
    project: String,
    /// A version id or version number; the newest that fits without it.
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    kind: Option<String>,
    #[serde(default)]
    no_deps: bool,
    /// Install a version that doesn't fit the instance.
    #[serde(default)]
    force: bool,
}

/// The Content tab's install: each request planned and run in turn (a shader mod
/// before its first shader pack), with the shader loader read again after mods.
async fn run_requests(
    api: Arc<Modrinth>,
    downloader: launcher_core::game::download::Downloader,
    dirs: InstanceDirs,
    mut target: Target,
    requests: Vec<InstallRequest>,
    progress: Arc<InstallProgress>,
    cancel: CancelToken,
) -> Result<InstallOutcome, ErrorObject> {
    let content_error = |err: launcher_core::content::ContentError| {
        let text = err.to_string();
        if cancel.is_cancelled() {
            api_error(codes::CANCELLED, "Cancelled.")
        } else {
            api_error(codes::FAILED, text)
        }
    };
    let mut total = InstallOutcome::default();
    for request in requests {
        let kind = request.kind;
        progress.set_label(format!(
            "Checking {}",
            request
                .project
                .as_ref()
                .map_or("the dependencies", |p| p.title.as_str())
        ));
        let plan = install::plan(&api, &dirs, &target, request)
            .await
            .map_err(content_error)?;
        let outcome = install::execute(&api, &downloader, &dirs, &target, plan, &progress, &cancel)
            .await
            .map_err(content_error)?;
        total.installed.extend(outcome.installed);
        total.removed.extend(outcome.removed);
        total.skipped.extend(outcome.skipped);
        if kind == ContentKind::Mod {
            target.shader_loader = listing::shader_state(&dirs).map_err(content_error)?.loader;
        }
    }
    Ok(total)
}

fn outcome_json(outcome: &InstallOutcome) -> Value {
    json!({
        "installed": outcome.installed.iter().map(|(title, file)| json!({ "title": title, "file": file })).collect::<Vec<_>>(),
        "removed": outcome.removed,
        "skipped": outcome.skipped.iter().map(|s| json!({ "project": s.project, "reason": s.reason })).collect::<Vec<_>>(),
    })
}

/// Runs install requests as a content job of the instance.
fn start_install(
    call: &Call,
    instance: &Instance,
    label: String,
    plan: impl AsyncFnOnce(&mut AsyncApp) -> Result<Vec<InstallRequest>, ErrorObject> + 'static,
    kinds: Vec<ContentKind>,
    cx: &mut App,
) -> Reply {
    let control = call.control.clone();
    let api = tri!(modrinth(&control, cx));
    let (dirs, target) = tri!(context(&control, instance, cx));
    let downloader = control.instances.read(cx).content_downloader();
    let progress = Arc::new(InstallProgress::default());
    let cancel = CancelToken::new();
    let job_progress = progress.clone();
    let job_cancel = cancel.clone();
    let refresh = Rc::downgrade(&control);
    let id = instance.id.clone();
    let work = cx.spawn(async move |cx| {
        let requests = plan(cx).await?;
        let outcome = cx
            .background_spawn(run_requests(api, downloader, dirs, target, requests, progress, cancel))
            .await;
        if let Some(control) = refresh.upgrade() {
            let mut all = kinds.clone();
            all.push(ContentKind::ShaderPack);
            cx.update(|cx| refresh_view(&control, &id, &all, cx));
        }
        Ok(outcome_json(&outcome?))
    });
    let job = control.start_job(
        JobSpec::new("content", label)
            .instance(instance)
            .progress(Progress::Install(job_progress))
            .cancel(Cancel::Token(job_cancel)),
        work,
        cx,
    );
    control.reply_job(call, job, cx)
}

pub fn install(call: Call, cx: &mut App) -> Reply {
    let params: InstallParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    tri!(can_change(&control, &instance, cx));
    let api = tri!(modrinth(&control, cx));
    let (_, target) = tri!(context(&control, &instance, cx));
    let wanted_kind = match &params.kind {
        Some(kind) => Some(tri!(parse_kind(kind))),
        None => None,
    };
    let label = format!("Installing {} in {}", params.project, instance.name);
    let kinds = wanted_kind.into_iter().collect::<Vec<_>>();
    start_install(
        &call,
        &instance,
        label,
        async move |_cx| {
            let (project, linked_version) = resolve_project(&api, &params.project).await?;
            let kind = match (wanted_kind, ContentKind::from_project_type(&project.project_type)) {
                (Some(kind), _) => kind,
                (None, Some(kind)) => kind,
                (None, None) => {
                    return Err(api_error(
                        codes::INVALID_PARAMS,
                        format!(
                            "{} is a {}, not a mod, resource pack or shader. Install modpacks with `modpacks install`.",
                            project.title, project.project_type
                        ),
                    ));
                }
            };
            let version = match (params.version.as_deref(), linked_version) {
                (None, Some(version)) => version,
                (wanted, _) => {
                    let versions = api
                        .versions(&project.id, &[], &[])
                        .await
                        .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
                    match wanted {
                        Some(wanted) => versions
                            .into_iter()
                            .find(|v| v.id == wanted || v.version_number == wanted)
                            .ok_or_else(|| {
                                api_error(
                                    codes::NOT_FOUND,
                                    format!("{} has no version \"{wanted}\". List them with `content versions {}`.", project.title, project.slug),
                                )
                            })?,
                        None if params.force => versions.into_iter().next().ok_or_else(|| {
                            api_error(codes::NOT_FOUND, format!("{} has no versions.", project.title))
                        })?,
                        None => install::pick_version(&target, kind, versions).ok_or_else(|| {
                            api_error(
                                codes::NOT_FOUND,
                                format!(
                                    "{} has no version for Minecraft {} with {}. Pick one with --version and --force, or see `content versions {}`.",
                                    project.title,
                                    target.game_version,
                                    target.loader.label(),
                                    project.slug
                                ),
                            )
                        })?,
                    }
                }
            };
            if !params.force
                && let Some(reason) = target.incompatibility(kind, &version)
            {
                return Err(api_error(
                    codes::CONFLICT,
                    format!("{} {} doesn't fit this instance: {reason} Pass --force to install it anyway.", project.title, version.version_number),
                ));
            }
            let mut requests = Vec::new();
            if kind == ContentKind::ShaderPack && target.shader_loader.is_none() {
                // shaders need a shader mod: the Content tab installs it first
                match shaders::find_shader_mod(&api, &target)
                    .await
                    .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?
                {
                    ShaderModOffer::Install { version, .. } => requests.push(InstallRequest {
                        kind: ContentKind::Mod,
                        version: *version,
                        project: None,
                        install_dependencies: true,
                        force: false,
                    }),
                    ShaderModOffer::NeedsModLoader => {
                        return Err(api_error(
                            codes::CONFLICT,
                            "Shader packs need a mod loader, and this instance is vanilla. Switch it to Fabric first: `content switch-loader <instance> fabric`.",
                        ));
                    }
                    ShaderModOffer::Unavailable { reason } => {
                        return Err(api_error(codes::UNAVAILABLE, reason));
                    }
                }
            }
            requests.push(InstallRequest {
                kind,
                version,
                project: Some(project),
                install_dependencies: !params.no_deps,
                force: params.force,
            });
            Ok(requests)
        },
        kinds,
        cx,
    )
}

#[derive(Deserialize)]
struct AddFileParams {
    path: PathBuf,
    #[serde(default)]
    kind: Option<String>,
}

pub fn add_file(call: Call, cx: &mut App) -> Reply {
    let params: AddFileParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    tri!(absolute(&params.path));
    tri!(can_change(&control, &instance, cx));
    let kind = match &params.kind {
        Some(kind) => tri!(parse_kind(kind)),
        None => match params.path.extension().and_then(|e| e.to_str()) {
            Some("jar") => ContentKind::Mod,
            _ => {
                return ready(Err(api_error(
                    codes::INVALID_PARAMS,
                    "Say what the file is with --kind (mod, resourcepack or shader).",
                )));
            }
        },
    };
    let (dirs, target) = tri!(context(&control, &instance, cx));
    let path = params.path.clone();
    let id = instance.id.clone();
    let work = cx.background_spawn(async move { files::add(&dirs, &target, kind, &path) });
    cx.spawn(async move |cx| {
        let name = work
            .await
            .map_err(|err| api_error(codes::FAILED, err.to_string()))?;
        cx.update(|cx| refresh_view(&control, &id, &[kind], cx));
        Ok(json!({ "instance": id, "kind": kind_name(kind), "file": name }))
    })
}

#[derive(Deserialize)]
struct ItemParams {
    /// File names, slugs, Modrinth titles or names.
    items: Vec<String>,
    #[serde(default)]
    kind: Option<String>,
}

/// The installed items named by `names`, of `kind` or any kind.
async fn find_items(
    api: Option<Arc<Modrinth>>,
    dirs: &InstanceDirs,
    target: &Target,
    kind: Option<ContentKind>,
    names: &[String],
) -> Result<Vec<Item>, ErrorObject> {
    let kinds = match kind {
        Some(kind) => vec![kind],
        None => ContentKind::ALL.to_vec(),
    };
    let mut items = Vec::new();
    for kind in kinds {
        let listing = listing::list(api.as_deref(), dirs, target, kind)
            .await
            .map_err(|err| api_error(codes::FAILED, err.to_string()))?;
        items.extend(listing.items);
    }
    let mut found = Vec::new();
    for name in names {
        let wanted = name.trim();
        let plain = launcher_core::content::enabled_name(wanted);
        let matches: Vec<&Item> = items
            .iter()
            .filter(|item| {
                item.file.file_name == wanted
                    || launcher_core::content::enabled_name(&item.file.file_name) == plain
                    || item.display_name().eq_ignore_ascii_case(wanted)
                    || item.modrinth.as_ref().is_some_and(|m| {
                        m.slug.eq_ignore_ascii_case(wanted)
                            || m.project_id == wanted
                            || m.title.eq_ignore_ascii_case(wanted)
                    })
                    || item.file.meta.id.as_deref().is_some_and(|id| id.eq_ignore_ascii_case(wanted))
            })
            .collect();
        match matches.as_slice() {
            [item] => found.push((*item).clone()),
            [] => {
                return Err(api_error(
                    codes::NOT_FOUND,
                    format!("Nothing installed is called \"{wanted}\". List it with `content list`."),
                ));
            }
            _ => {
                return Err(api_error(
                    codes::AMBIGUOUS,
                    format!("More than one installed file matches \"{wanted}\"; use the file name."),
                )
                .with_data(json!({ "matches": matches.iter().map(|i| &i.file.file_name).collect::<Vec<_>>() })));
            }
        }
    }
    Ok(found)
}

/// Finds the items, then changes each on the main thread, as the Content tab does.
fn change_items(
    call: Call,
    cx: &mut App,
    confirm: Option<&str>,
    change: fn(&InstanceDirs, &Target, &Item) -> Result<Value, ErrorObject>,
) -> Reply {
    let params: ItemParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    if let Some(what) = confirm {
        tri!(call.confirm(format!("{what} {}", params.items.join(", "))));
    }
    tri!(can_change(&control, &instance, cx));
    if params.items.is_empty() {
        return ready(Err(api_error(codes::INVALID_PARAMS, "Name what to change.")));
    }
    let kind = match &params.kind {
        Some(kind) => Some(tri!(parse_kind(kind))),
        None => None,
    };
    let (dirs, target) = tri!(context(&control, &instance, cx));
    let api = control.instances.read(cx).modrinth();
    let id = instance.id.clone();
    cx.spawn(async move |cx| {
        let find_dirs = dirs.clone();
        let find_target = target.clone();
        let names = params.items.clone();
        let items = cx
            .background_spawn(async move { find_items(api, &find_dirs, &find_target, kind, &names).await })
            .await?;
        let results: Vec<Value> = cx.update(|cx| {
            let instance = control.instance(&id, cx)?;
            can_change(&control, &instance, cx)?;
            let results = items
                .iter()
                .map(|item| change(&dirs, &target, item))
                .collect::<Result<Vec<_>, _>>();
            let kinds: Vec<ContentKind> = items.iter().map(|i| i.file.kind).collect();
            refresh_view(&control, &id, &kinds, cx);
            results
        })?;
        Ok(json!({ "instance": id, "changed": results }))
    })
}

fn content_failed(err: launcher_core::content::ContentError) -> ErrorObject {
    api_error(codes::FAILED, err.to_string())
}

pub fn remove(call: Call, cx: &mut App) -> Reply {
    change_items(call, cx, Some("moves to the Trash:"), |dirs, target, item| {
        files::delete(dirs, target, item.file.kind, &item.file.file_name).map_err(content_failed)?;
        Ok(json!({ "file": item.file.file_name, "removed": true, "trash": true }))
    })
}

fn set_enabled(dirs: &InstanceDirs, target: &Target, item: &Item, enabled: bool) -> Result<Value, ErrorObject> {
    match item.file.kind {
        ContentKind::Mod => {
            let name = files::set_mod_enabled(dirs, &item.file.file_name, enabled).map_err(content_failed)?;
            Ok(json!({ "file": name, "enabled": enabled }))
        }
        ContentKind::ResourcePack => {
            files::set_pack_enabled(dirs, target, &item.file.file_name, enabled, item.compatibility)
                .map_err(content_failed)?;
            Ok(json!({ "file": item.file.file_name, "enabled": enabled }))
        }
        ContentKind::ShaderPack => {
            // a shader pack is on when the shader mod uses it
            let loader = target.shader_loader.ok_or_else(|| {
                api_error(
                    codes::CONFLICT,
                    "There is no shader mod to use the pack: install Iris (or OptiFine) first.",
                )
            })?;
            files::select_shader(dirs, loader, enabled.then_some(item.file.file_name.as_str()))
                .map_err(content_failed)?;
            Ok(json!({ "file": item.file.file_name, "enabled": enabled, "selected_shader": enabled }))
        }
    }
}

pub fn enable(call: Call, cx: &mut App) -> Reply {
    change_items(call, cx, None, |dirs, target, item| set_enabled(dirs, target, item, true))
}

pub fn disable(call: Call, cx: &mut App) -> Reply {
    change_items(call, cx, None, |dirs, target, item| set_enabled(dirs, target, item, false))
}

#[derive(Deserialize)]
struct UpdatesParams {
    #[serde(default)]
    kind: Option<String>,
}

/// Newer compatible versions of installed content, by file, for each kind.
async fn find_updates(
    api: Arc<Modrinth>,
    dirs: InstanceDirs,
    target: Target,
    kinds: Vec<ContentKind>,
) -> Result<Vec<(ContentKind, String, RawVersion)>, ErrorObject> {
    let mut all = Vec::new();
    for kind in kinds {
        let updates = listing::check_updates(&api, &dirs, &target, kind)
            .await
            .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
        let mut updates: Vec<(String, RawVersion)> = updates.into_iter().collect();
        // "Update all" goes by project
        updates.sort_by(|a, b| a.1.project_id.cmp(&b.1.project_id));
        all.extend(updates.into_iter().map(|(file, version)| (kind, file, version)));
    }
    Ok(all)
}

pub fn updates(call: Call, cx: &mut App) -> Reply {
    let params: UpdatesParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    let kinds = tri!(kinds(params.kind.as_deref()));
    let api = tri!(modrinth(&control, cx));
    let (dirs, target) = tri!(context(&control, &instance, cx));
    let id = instance.id.clone();
    cx.background_spawn(async move {
        let updates = find_updates(api, dirs, target, kinds).await?;
        Ok(json!({
            "instance": id,
            "updates": updates.iter().map(|(kind, file, version)| json!({
                "kind": kind_name(*kind),
                "file": file,
                "project_id": version.project_id,
                "version_id": version.id,
                "version_number": version.version_number,
            })).collect::<Vec<_>>(),
        }))
    })
}

#[derive(Deserialize)]
struct UpdateParams {
    /// File names to update; every update without it.
    #[serde(default)]
    items: Vec<String>,
    #[serde(default)]
    kind: Option<String>,
}

pub fn update(call: Call, cx: &mut App) -> Reply {
    let params: UpdateParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    tri!(can_change(&control, &instance, cx));
    let kinds = tri!(kinds(params.kind.as_deref()));
    let api = tri!(modrinth(&control, cx));
    let (dirs, target) = tri!(context(&control, &instance, cx));
    let label = if params.items.is_empty() {
        format!("Updating {}'s content", instance.name)
    } else {
        format!("Updating {} in {}", params.items.join(", "), instance.name)
    };
    let wanted = params.items.clone();
    let refresh_kinds = kinds.clone();
    start_install(
        &call,
        &instance,
        label,
        async move |_cx| {
            let updates = find_updates(api, dirs, target, kinds).await?;
            let chosen: Vec<_> = updates
                .into_iter()
                .filter(|(_, file, version)| {
                    wanted.is_empty()
                        || wanted.iter().any(|w| {
                            w == file
                                || launcher_core::content::enabled_name(w)
                                    == launcher_core::content::enabled_name(file)
                                || w == &version.project_id
                        })
                })
                .collect();
            if chosen.is_empty() {
                return Err(api_error(
                    codes::NOT_FOUND,
                    if wanted.is_empty() {
                        "Everything is up to date.".to_string()
                    } else {
                        format!("No update for {}. See `content updates`.", wanted.join(", "))
                    },
                ));
            }
            Ok(chosen
                .into_iter()
                .map(|(kind, _, version)| InstallRequest {
                    kind,
                    version,
                    project: None,
                    install_dependencies: true,
                    force: false,
                })
                .collect())
        },
        refresh_kinds,
        cx,
    )
}

#[derive(Deserialize)]
struct SwitchLoaderParams {
    /// fabric, quilt, forge, neoforge or vanilla; the Content tab's offer (Fabric, or
    /// Forge before 1.14) without it.
    #[serde(default)]
    loader: Option<String>,
    #[serde(default)]
    loader_version: Option<String>,
    /// Turn off the mods made for another loader (the window asks).
    #[serde(default)]
    disable_mismatched: bool,
}

pub fn switch_loader(call: Call, cx: &mut App) -> Reply {
    let params: SwitchLoaderParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    tri!(can_change(&control, &instance, cx));
    if control.instances.read(cx).loader_switch_running() {
        return ready(Err(api_error(codes::BUSY, "A loader switch is running.")));
    }
    let Some(loader) = params.loader.clone() else {
        // "Switch to Fabric" on a vanilla instance
        if instance.loader != Loader::None {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                format!("{} has {} already; name the loader to change to.", instance.name, instance.loader_label()),
            )));
        }
        let seq = control.notices.borrow().last_seq();
        let id = instance.id.clone();
        control
            .instances
            .update(cx, |instances, cx| instances.switch_to_mod_loader(&id, cx));
        let outbox = call.outbox.clone();
        return cx.spawn(async move |cx| {
            poll_until(cx, Some(&outbox), Duration::from_millis(200), Duration::from_secs(120), |cx| {
                (!control.instances.read(cx).loader_switch_running()).then_some(())
            })
            .await?;
            let notice = control
                .notices
                .borrow()
                .find_since(seq, |n| matches!(n.kind, "notice" | "launch_failed"));
            match notice {
                Some(n) if n.is_failure() => Err(api_error(codes::FAILED, format!("{}: {}", n.title, n.message))),
                Some(n) => Ok(json!({ "instance": id, "title": n.title, "message": n.message })),
                None => Ok(json!({ "instance": id })),
            }
        });
    };
    let kind = tri!(parse_loader(&loader));
    let installer = control.instances.read(cx).installer();
    let dirs = control.instances.read(cx).content_dirs(&instance.id);
    cx.spawn(async move |cx| {
        let choice = match kind {
            None => None,
            Some(kind) => {
                let game_version = instance.game_version.clone();
                let list = cx
                    .background_spawn(async move { installer.loader_versions(kind, &game_version).await })
                    .await
                    .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
                let version = match &params.loader_version {
                    Some(wanted) => list.iter().find(|v| &v.id == wanted || &v.display == wanted),
                    None => list.iter().find(|v| v.recommended).or_else(|| list.first()),
                }
                .ok_or_else(|| {
                    api_error(
                        codes::NOT_FOUND,
                        format!("{} has no such build for Minecraft {}.", kind.label(), instance.game_version),
                    )
                })?;
                Some(launcher_core::game::loaders::LoaderChoice { kind, version: version.id.clone() })
            }
        };
        let to = Loader::from_kind(choice.as_ref().map(|c| c.kind));
        // the Settings tab's check before a loader change
        let check = loader_change::check(&dirs, instance.loader, to, &instance.game_version)
            .map_err(|err| api_error(codes::FAILED, err.to_string()))?;
        let mismatched: Vec<String> = check.mismatched.iter().map(|m| m.file_name.clone()).collect();
        if check.needs_confirmation() && !params.disable_mismatched && !super::confirmed(&call.params) {
            return Err(api_error(
                codes::CONFIRMATION_REQUIRED,
                format!(
                    "Changing the loader leaves {} mod(s) behind{}. Ask the user, then pass --yes to keep them as they are, or --disable-mismatched to turn them off.",
                    mismatched.len(),
                    if check.worlds_at_risk { " and may damage this instance's worlds (back them up first)" } else { "" }
                ),
            )
            .with_data(json!({
                "mismatched": check.mismatched.iter().map(|m| json!({ "file": m.file_name, "name": m.name, "made_for": m.made_for })).collect::<Vec<_>>(),
                "worlds": check.worlds,
                "worlds_at_risk": check.worlds_at_risk,
            })));
        }
        if params.disable_mismatched {
            loader_change::disable_mods(&dirs, &mismatched)
                .map_err(|err| api_error(codes::FAILED, err.to_string()))?;
        }
        let mut changed = instance.clone();
        changed.loader = to;
        changed.loader_version = choice.map(|c| c.version);
        let saved = control
            .instances
            .update(cx, |instances, cx| instances.save(&changed, cx));
        failed(saved)?;
        Ok(json!({
            "instance": changed.id,
            "loader": changed.loader,
            "loader_version": changed.loader_version,
            "disabled": if params.disable_mismatched { mismatched } else { Vec::new() },
            "message": "The loader installs the next time the game starts.",
        }))
    })
}

// --- modpacks ---

#[derive(Deserialize)]
struct ModpackSearchParams {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    sort: Option<String>,
    #[serde(default)]
    limit: Option<u32>,
    #[serde(default)]
    offset: Option<u32>,
}

pub fn modpack_search(call: Call, cx: &mut App) -> Reply {
    let params: ModpackSearchParams = params!(call);
    let api = tri!(modrinth(&call.control, cx));
    let search = tri!(
        ClientSearchParams::new(
            params.query.filter(|q| !q.trim().is_empty()),
            ClientProjectType::Modpack,
            None,
            Vec::new(),
            tri!(sort(params.sort.as_deref())),
            params.offset.or(Some(0)),
            Some(params.limit.unwrap_or(PAGE_SIZE)),
        )
        .map_err(|err| api_error(codes::INVALID_PARAMS, err.to_string()))
    );
    cx.background_spawn(async move {
        let response = api
            .search(&search)
            .await
            .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
        Ok(hits_json(&response))
    })
}

/// The version the modpack browser starts at: the newest release, else the newest.
fn default_modpack_version(versions: Vec<RawVersion>) -> Option<RawVersion> {
    let release = versions
        .iter()
        .position(|v| v.version_type == ModrinthVersionType::Release);
    match release {
        Some(i) => versions.into_iter().nth(i),
        None => versions.into_iter().next(),
    }
}

#[derive(Deserialize)]
struct ModpackInstallParams {
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    version: Option<String>,
    /// A `.mrpack` file instead of a Modrinth project.
    #[serde(default)]
    file: Option<PathBuf>,
    #[serde(default)]
    name: Option<String>,
}

pub fn modpack_install(call: Call, cx: &mut App) -> Reply {
    let params: ModpackInstallParams = params!(call);
    let control = call.control.clone();
    match (&params.project, &params.file) {
        (Some(_), Some(_)) | (None, None) => {
            return ready(Err(api_error(
                codes::INVALID_PARAMS,
                "Give a Modrinth project or a .mrpack file.",
            )));
        }
        (None, Some(file)) => {
            tri!(absolute(file));
            if file.extension().and_then(|e| e.to_str()) != Some("mrpack") {
                return ready(Err(api_error(codes::INVALID_PARAMS, "A modpack file ends in .mrpack.")));
            }
            let seq = control.notices.borrow().last_seq();
            let before = last_import(&control, cx);
            let file = file.clone();
            control
                .instances
                .update(cx, |instances, cx| instances.import_files(vec![file.clone()], cx));
            return follow_import(&call, before, seq, format!("Importing {}", file.display()), cx);
        }
        (Some(_), None) => {}
    }
    let api = tri!(modrinth(&control, cx));
    let reference = params.project.clone().unwrap_or_default();
    let call_for_job = Rc::new(call);
    cx.spawn(async move |cx| {
        let (project, linked) = resolve_project(&api, &reference).await?;
        if project.project_type != "modpack" {
            return Err(api_error(
                codes::INVALID_PARAMS,
                format!("{} is a {}, not a modpack. Install it into an instance with `content install`.", project.title, project.project_type),
            ));
        }
        let version = match (params.version.as_deref(), linked) {
            (None, Some(version)) => version,
            (wanted, _) => {
                let versions = api
                    .versions(&project.id, &[], &[])
                    .await
                    .map_err(|err| api_error(codes::UNAVAILABLE, err.to_string()))?;
                match wanted {
                    Some(wanted) => versions
                        .into_iter()
                        .find(|v| v.id == wanted || v.version_number == wanted)
                        .ok_or_else(|| api_error(codes::NOT_FOUND, format!("{} has no version \"{wanted}\".", project.title)))?,
                    None => default_modpack_version(versions)
                        .ok_or_else(|| api_error(codes::NOT_FOUND, format!("{} has no versions.", project.title)))?,
                }
            }
        };
        let label = format!("Installing {} {}", project.title, version.version_number);
        let reply = cx.update(|cx| {
            let seq = control.notices.borrow().last_seq();
            let before = last_import(&control, cx);
            control.instances.update(cx, |instances, cx| {
                instances.import_modrinth(project, version, params.name.clone(), cx)
            });
            follow_import(&call_for_job, before, seq, label, cx)
        });
        reply.await
    })
}

fn last_import(control: &Control, cx: &App) -> Option<u64> {
    control.instances.read(cx).imports().iter().map(|j| j.id).max()
}

/// Follows the window's modpack import that just started, until it ends.
fn follow_import(call: &Call, before: Option<u64>, seq: u64, label: String, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let started = control
        .instances
        .read(cx)
        .imports()
        .iter()
        .filter(|j| Some(j.id) > before)
        .map(|j| (j.id, j.progress.clone()))
        .max_by_key(|(id, _)| *id);
    let Some((import, progress)) = started else {
        // refused at once: the window said why
        let notice = control.notices.borrow().find_since(seq, |n| n.is_failure());
        return ready(Err(match notice {
            Some(n) => api_error(codes::FAILED, format!("{}: {}", n.title, n.message)),
            None => api_error(codes::UNAVAILABLE, "The import didn't start."),
        }));
    };
    let watch = control.clone();
    let work = cx.spawn(async move |cx| {
        poll_until(cx, None, Duration::from_millis(300), Duration::from_secs(6 * 3600), |cx| {
            (!watch.instances.read(cx).imports().iter().any(|j| j.id == import)).then_some(())
        })
        .await?;
        let notice = watch
            .notices
            .borrow()
            .find_since(seq, |n| matches!(n.kind, "notice" | "launch_failed"));
        match notice {
            Some(n) if n.is_failure() => Err(api_error(codes::FAILED, format!("{}: {}", n.title, n.message))),
            Some(n) => Ok(cx.update(|cx| {
                let selected = watch.instances.read(cx).selected().cloned();
                json!({
                    "instance": selected.as_ref().map(|i| json!({ "id": i.id, "name": i.name, "game_version": i.game_version, "loader": i.loader })),
                    "title": n.title,
                    "message": n.message,
                })
            })),
            None => Err(api_error(codes::CANCELLED, "Cancelled.")),
        }
    });
    let job = control.start_job(
        JobSpec::new("modpack_install", label)
            .progress(Progress::Install(progress))
            .cancel(Cancel::Import(import)),
        work,
        cx,
    );
    control.reply_job(call, job, cx)
}

pub fn modpack_check(call: Call, cx: &mut App) -> Reply {
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    let Some(record) = control.instances.read(cx).modpack(&instance.id) else {
        return ready(Err(api_error(
            codes::NOT_FOUND,
            format!("{} wasn't made from a modpack.", instance.name),
        )));
    };
    if !record.on_modrinth() {
        return ready(Ok(json!({
            "instance": instance.id,
            "modpack": { "title": record.title, "version_number": record.version_number },
            "on_modrinth": false,
            "update_available": false,
        })));
    }
    let Some(fetch) = control
        .instances
        .update(cx, |instances, cx| instances.modpack_versions(&record.project_id, cx))
    else {
        return ready(Err(api_error(codes::UNAVAILABLE, "Modrinth isn't available.")));
    };
    cx.spawn(async move |_| {
        let versions = fetch.await.map_err(|err| api_error(codes::UNAVAILABLE, err))?;
        let candidate = update_candidate(&versions, &record);
        Ok(json!({
            "instance": instance.id,
            "modpack": {
                "title": record.title,
                "project_id": record.project_id,
                "version_id": record.version_id,
                "version_number": record.version_number,
            },
            "on_modrinth": true,
            "update_available": candidate.is_some(),
            "latest": candidate.as_ref().map(|v| version_json(v, None)),
        }))
    })
}

#[derive(Deserialize)]
struct ModpackUpdateParams {
    /// A version id or number; the newer version `modpacks.check` finds without it.
    #[serde(default)]
    version: Option<String>,
    #[serde(default = "yes")]
    backup_worlds: bool,
}

fn yes() -> bool {
    true
}

fn plan_json(prompt: &crate::instances::modpacks::UpdatePrompt) -> Value {
    let plan = &prompt.plan;
    json!({
        "instance": prompt.instance_id,
        "from_version": prompt.from_version,
        "to_version": plan.record.version_number,
        "game_version": plan.game_version,
        "loader": plan.loader.as_ref().map(|l| json!({ "kind": l.kind.label(), "version": l.version })),
        "added": plan.added,
        "updated": plan.updated.iter().map(|(old, new)| json!({ "from": old, "to": new })).collect::<Vec<_>>(),
        "removed": plan.removed,
        "kept_changed": plan.kept_changed,
        "replaced_own": plan.replaced_own,
        "overrides_written": plan.overrides_written,
        "overrides_kept": plan.overrides_kept,
        "download_bytes": plan.download_size(),
    })
}

pub fn modpack_update(call: Call, cx: &mut App) -> Reply {
    let params: ModpackUpdateParams = params!(call);
    let control = call.control.clone();
    let instance = tri!(control.instance_param(&call, cx));
    tri!(control.idle(&instance, cx));
    let Some(record) = control.instances.read(cx).modpack(&instance.id) else {
        return ready(Err(api_error(
            codes::NOT_FOUND,
            format!("{} wasn't made from a modpack.", instance.name),
        )));
    };
    if !record.on_modrinth() {
        return ready(Err(api_error(
            codes::UNAVAILABLE,
            "The modpack isn't from Modrinth, so it can't be updated.",
        )));
    }
    let Some(fetch) = control
        .instances
        .update(cx, |instances, cx| instances.modpack_versions(&record.project_id, cx))
    else {
        return ready(Err(api_error(codes::UNAVAILABLE, "Modrinth isn't available.")));
    };
    let call = Rc::new(call);
    cx.spawn(async move |cx| {
        let versions = fetch.await.map_err(|err| api_error(codes::UNAVAILABLE, err))?;
        let version = match &params.version {
            Some(wanted) => versions
                .into_iter()
                .find(|v| &v.id == wanted || &v.version_number == wanted)
                .ok_or_else(|| api_error(codes::NOT_FOUND, format!("The modpack has no version \"{wanted}\".")))?,
            None => update_candidate(&versions, &record).ok_or_else(|| {
                api_error(codes::NOT_FOUND, "The modpack is up to date.")
            })?,
        };
        let id = instance.id.clone();
        let planned = cx.update(|cx| {
            control
                .instances
                .update(cx, |instances, cx| instances.plan_modpack_update(&id, version, cx))
        });
        let Some(planned) = planned else {
            return Err(api_error(codes::BUSY, "The instance is running or updating."));
        };
        let prompt = planned.await.map_err(|err| api_error(codes::FAILED, err))?;
        let plan = plan_json(&prompt);
        if !super::confirmed(&call.params) {
            // the window asks before applying; so does the API
            control.instances.update(cx, |instances, _| instances.discard_update(&prompt));
            return Err(api_error(
                codes::CONFIRMATION_REQUIRED,
                "This changes the instance's files as the plan says. Show the user the plan, then pass --yes.",
            )
            .with_data(json!({ "plan": plan })));
        }
        let reply = cx.update(|cx| -> Reply {
            let seq = control.notices.borrow().last_seq();
            let id = prompt.instance_id.clone();
            control.instances.update(cx, |instances, cx| {
                instances.apply_modpack_update(prompt, params.backup_worlds, cx)
            });
            let Some(job) = control.instances.read(cx).modpack_update(&id) else {
                let notice = control.notices.borrow().find_since(seq, |n| n.is_failure());
                return ready(Err(match notice {
                    Some(n) => api_error(codes::FAILED, format!("{}: {}", n.title, n.message)),
                    None => api_error(codes::FAILED, "The update didn't start."),
                }));
            };
            let progress = job.progress.clone();
            let cancel = job.cancel.clone();
            let watch = control.clone();
            let plan = plan.clone();
            let work = cx.spawn(async move |cx| {
                poll_until(cx, None, Duration::from_millis(300), Duration::from_secs(6 * 3600), |cx| {
                    watch.instances.read(cx).modpack_update(&id).is_none().then_some(())
                })
                .await?;
                let notice = watch
                    .notices
                    .borrow()
                    .find_since(seq, |n| matches!(n.kind, "notice" | "launch_failed"));
                match notice {
                    Some(n) if n.is_failure() => Err(api_error(codes::FAILED, format!("{}: {}", n.title, n.message))),
                    Some(n) => Ok(json!({ "instance": id, "title": n.title, "message": n.message, "plan": plan })),
                    None => Err(api_error(codes::CANCELLED, "Cancelled.")),
                }
            });
            let job = control.start_job(
                JobSpec::new("modpack_update", format!("Updating {}'s modpack", instance.name))
                    .instance(&instance)
                    .progress(Progress::Install(progress))
                    .cancel(Cancel::Token(cancel)),
                work,
                cx,
            );
            control.reply_job(&call, job, cx)
        });
        reply.await
    })
}

#[allow(dead_code)]
fn _assert_result(_: ApiResult) {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_kinds() {
        assert_eq!(parse_kind("mods").unwrap(), ContentKind::Mod);
        assert_eq!(parse_kind("resource-pack").unwrap(), ContentKind::ResourcePack);
        assert_eq!(parse_kind("Shader_Packs").unwrap(), ContentKind::ShaderPack);
        assert!(parse_kind("datapack").is_err());
        assert!(sort(Some("downloads")).is_ok());
        assert!(sort(Some("stars")).is_err());
    }
}
