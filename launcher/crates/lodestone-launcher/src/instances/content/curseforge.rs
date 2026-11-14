//! The Content tab's CurseForge side: browsing CurseForge, a project's files, installing
//! them, and the files CurseForge doesn't let launchers download.
//!
//! Those files are listed in a card with a button to each file's page on
//! curseforge.com. While the card is up, the Downloads folder (or one the user picks) is
//! looked at every [`WATCH_EVERY`]: a file named like one of them whose SHA-1 matches
//! is copied into the instance (`launcher-core::content::curseforge_install`).

use std::{path::PathBuf, sync::Arc, time::Duration};

use gpui::{
    AnyElement, AppContext as _, Context, PathPromptOptions, SharedString, Task, div, prelude::*,
};
use launcher_core::content::{
    ContentError, ContentKind, CurseForge, Platforms,
    curseforge::{self, CfFile, CfMod, CfSearch, CfSearchResults, CfSort},
    curseforge_install::{self, CfInstallPlan, CfInstallRequest, CfPlannedFile},
    install::{InstallOutcome, InstallProgress, Placed},
    listing,
};

use super::{
    ContentView, Job, Load, PAGE_SIZE,
    view::{
        badge, counted, heading, icon_box, list_box, muted, pills, row_base, short_list, small,
    },
};
use crate::components::{
    Button, ButtonSize, ButtonVariant, Icon, KeyboardFocus, LoadingDots, Toggle,
    a11y::{Accessible, Role},
};
use crate::i18n::{CommonMsg, ContentMsg as M, ContentPlural as P, format, t, tr};
use crate::theme::{self, colors, fonts, text, weights};

/// How often the folder is looked at for files downloaded by hand.
const WATCH_EVERY: Duration = Duration::from_secs(2);

/// Browsing CurseForge in a section.
pub struct CfBrowse {
    pub sort: CfSort,
    pub offset: usize,
    pub results: Load<CfSearchResults>,
    pub project: Option<CfProjectView>,
    task: Option<Task<()>>,
}

impl Default for CfBrowse {
    fn default() -> Self {
        Self {
            sort: CfSort::Popularity,
            offset: 0,
            results: Load::NotLoaded,
            project: None,
            task: None,
        }
    }
}

/// A CurseForge project opened from Browse.
pub struct CfProjectView {
    pub project_id: u64,
    pub tab: super::ProjectTab,
    pub project: Load<CfMod>,
    /// The description, as HTML.
    pub description: Load<String>,
    pub files: Load<Vec<CfFile>>,
    pub show_all: bool,
    pub with_dependencies: bool,
    /// An incompatible file whose "Install anyway" was clicked once.
    pub confirm_force: Option<u64>,
    _task: Option<Task<()>>,
}

/// Files the user downloads from curseforge.com, and the folder watched for them.
pub struct BlockedDownloads {
    pub files: Vec<CfPlannedFile>,
    /// Set once a file is in the instance.
    pub added: Vec<bool>,
    pub folder: Option<PathBuf>,
    pub error: Option<SharedString>,
    _watch: Option<Task<()>>,
}

impl BlockedDownloads {
    /// The files not in the instance yet.
    pub fn unfinished(&self) -> Vec<CfPlannedFile> {
        self.files
            .iter()
            .zip(&self.added)
            .filter(|(_, added)| !**added)
            .map(|(file, _)| file.clone())
            .collect()
    }
}

impl ContentView {
    pub(super) fn curseforge(&self, cx: &gpui::App) -> Option<Arc<CurseForge>> {
        self.instances.read(cx).curseforge()
    }

    // ---- Browse ----

    pub fn cf_set_sort(&mut self, kind: ContentKind, sort: CfSort, cx: &mut Context<Self>) {
        let browse = &mut self.section_mut(kind).cf;
        browse.sort = sort;
        browse.offset = 0;
        self.cf_search(kind, cx);
    }

    pub fn cf_set_page(&mut self, kind: ContentKind, offset: usize, cx: &mut Context<Self>) {
        self.section_mut(kind).cf.offset = offset;
        self.cf_search(kind, cx);
    }

    /// Searches CurseForge with the section's text (the same box as Modrinth's), filters
    /// and page.
    pub fn cf_search(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        let (Some((id, _, target)), Some(cf)) = (self.context(cx), self.curseforge(cx)) else {
            return;
        };
        let section = self.section(kind);
        let mut search = CfSearch::new(
            curseforge::class_of(kind),
            section.browse.search.read(cx).text(),
        );
        search.sort = section.cf.sort;
        search.offset = section.cf.offset;
        search.limit = PAGE_SIZE as usize;
        if section.browse.only_compatible {
            search.game_version = Some(target.game_version.clone());
            if kind == ContentKind::Mod {
                search.loaders = curseforge::accepted_loaders(target.loader, &target.game_version);
            }
        }
        self.section_mut(kind).cf.results = Load::Loading;
        let work = cx.background_spawn(async move { cf.search(&search).await });
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                this.section_mut(kind).cf.results = match result {
                    Ok(results) => Load::Ready(results),
                    Err(err) => Load::Failed(err.to_string().into()),
                };
                cx.notify();
            })
            .ok();
        });
        self.section_mut(kind).cf.task = Some(task);
        cx.notify();
    }

    pub fn open_cf_project(&mut self, kind: ContentKind, project_id: u64, cx: &mut Context<Self>) {
        let (Some(cf), Some((_, _, target))) = (self.curseforge(cx), self.context(cx)) else {
            return;
        };
        let work = cx.background_spawn(async move {
            let project = cf.project(project_id).await;
            let description = cf.description(project_id).await;
            let files = cf.files(project_id, None, None).await;
            (project, description, files)
        });
        let task = cx.spawn(async move |this, cx| {
            let (project, description, files) = work.await;
            this.update(cx, |this, cx| {
                if let Some(view) = &mut this.section_mut(kind).cf.project
                    && view.project_id == project_id
                {
                    view.project = match project {
                        Ok(project) => Load::Ready(project),
                        Err(err) => Load::Failed(err.to_string().into()),
                    };
                    view.description = match description {
                        Ok(description) => Load::Ready(description),
                        Err(err) => Load::Failed(err.to_string().into()),
                    };
                    view.files = match files {
                        Ok(files) => Load::Ready(files),
                        Err(err) => Load::Failed(err.to_string().into()),
                    };
                }
                cx.notify();
            })
            .ok();
        });
        let _ = target;
        self.panel_closing = None;
        self.page_scroll.set_offset(gpui::Point::default());
        self.section_mut(kind).browse.project = None;
        self.section_mut(kind).cf.project = Some(CfProjectView {
            project_id,
            tab: super::ProjectTab::default(),
            project: Load::Loading,
            description: Load::Loading,
            files: Load::Loading,
            show_all: false,
            with_dependencies: true,
            confirm_force: None,
            _task: Some(task),
        });
        cx.notify();
    }

    pub fn close_cf_project(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        // The panel beside the list slides shut first.
        if self.panel_width.is_some() && self.panel_closing.is_none() {
            self.panel_closing = Some(kind);
            cx.notify();
            return;
        }
        self.section_mut(kind).cf.project = None;
        if matches!(self.section(kind).cf.results, Load::NotLoaded) {
            self.cf_search(kind, cx);
        }
        cx.notify();
    }

    fn cf_project_mut(&mut self, kind: ContentKind) -> Option<&mut CfProjectView> {
        self.section_mut(kind).cf.project.as_mut()
    }

    /// Shows a prepared CurseForge project, for the offscreen screenshots.
    pub fn preview_cf_project(
        &mut self,
        kind: ContentKind,
        project: CfMod,
        description: String,
        files: Vec<CfFile>,
        cx: &mut Context<Self>,
    ) {
        self.kind = kind;
        self.section_mut(kind).browse.project = None;
        self.section_mut(kind).cf.project = Some(CfProjectView {
            project_id: project.id,
            tab: super::ProjectTab::default(),
            project: Load::Ready(project),
            description: Load::Ready(description),
            files: Load::Ready(files),
            show_all: false,
            with_dependencies: true,
            confirm_force: None,
            _task: None,
        });
        cx.notify();
    }

    pub fn set_cf_project_tab(
        &mut self,
        kind: ContentKind,
        tab: super::ProjectTab,
        cx: &mut Context<Self>,
    ) {
        if let Some(view) = self.cf_project_mut(kind) {
            view.tab = tab;
        }
        cx.notify();
    }

    // ---- Installing ----

    /// Installs a file, with its dependencies when `with_dependencies`: a result's
    /// Install always, the project page's as its toggle says.
    pub fn install_cf_file(
        &mut self,
        kind: ContentKind,
        file: CfFile,
        project: Option<CfMod>,
        force: bool,
        with_dependencies: bool,
        cx: &mut Context<Self>,
    ) {
        self.cf_install(
            vec![CfInstallRequest {
                kind,
                file,
                project,
                install_dependencies: with_dependencies,
                force,
            }],
            cx,
        );
    }

    /// Installs `requests` one after the other in the background, with progress. Files
    /// the user has to download themselves end up in the blocked card.
    pub fn cf_install(&mut self, requests: Vec<CfInstallRequest>, cx: &mut Context<Self>) {
        if requests.is_empty() || !self.can_change(cx) {
            return;
        }
        let (Some((id, dirs, target)), Some(cf)) = (self.context(cx), self.curseforge(cx)) else {
            return;
        };
        let modrinth = self.modrinth(cx);
        let downloader = self.instances.read(cx).content_downloader();
        let busy = self.instances.read(cx).mark_files_busy(&id);
        let progress = Arc::new(InstallProgress::default());
        let cancel = launcher_core::game::download::CancelToken::new();
        let kinds: Vec<ContentKind> = requests.iter().map(|r| r.kind).collect();
        let work = {
            let progress = progress.clone();
            let cancel = cancel.clone();
            cx.background_spawn(async move {
                let mut total = InstallOutcome::default();
                let mut blocked: Vec<CfPlannedFile> = Vec::new();
                let mut target = target;
                // The tab may not have read the instance's shader mod yet: "Update all"
                // goes on here when its instance is shown again, as the tab starts over.
                target.shader_loader = listing::shader_state(&dirs)?.loader;
                for request in requests {
                    let kind = request.kind;
                    progress.set_label(match &request.project {
                        Some(project) => tr!(M::CheckingProject, name = project.name),
                        None => t(M::CheckingDependencies).to_string(),
                    });
                    let plan: CfInstallPlan = curseforge_install::plan(
                        &cf,
                        Platforms::new(modrinth.as_deref(), Some(&cf)),
                        &dirs,
                        &target,
                        request,
                    )
                    .await?;
                    let outcome = curseforge_install::execute(
                        &cf,
                        modrinth.as_deref(),
                        &downloader,
                        &dirs,
                        &target,
                        &plan,
                        &progress,
                        &cancel,
                    )
                    .await?;
                    total.installed.extend(outcome.installed);
                    total.removed.extend(outcome.removed);
                    total.skipped.extend(outcome.skipped);
                    blocked.extend(plan.blocked);
                    if kind == ContentKind::Mod {
                        target.shader_loader = listing::shader_state(&dirs)?.loader;
                    }
                }
                Ok::<_, ContentError>((total, blocked))
            })
        };
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                let Some((_, shown)) = this.finish_job(&id) else {
                    return;
                };
                if !shown {
                    let message = match result {
                        Ok((outcome, blocked)) => {
                            let said = !outcome.installed.is_empty() || blocked.is_empty();
                            // the card shows them when the instance is shown again
                            this.instances.update(cx, |instances, _| {
                                instances.put_back_pending_downloads(&id, blocked)
                            });
                            said.then(|| (super::outcome_message(&outcome).into(), false))
                        }
                        Err(err) => Some((err.to_string().into(), true)),
                    };
                    this.keep_finished(&id, message, Vec::new());
                    return;
                }
                match result {
                    Ok((outcome, blocked)) => {
                        if !outcome.installed.is_empty() || blocked.is_empty() {
                            this.set_message(super::outcome_message(&outcome), false, cx);
                        }
                        for kind in &kinds {
                            this.section_mut(*kind).updates.clear();
                            this.section_mut(*kind).cf_updates.clear();
                        }
                        if !blocked.is_empty() {
                            this.show_blocked(blocked, cx);
                        }
                    }
                    Err(err) => this.set_message(err.to_string(), true, cx),
                }
                let mut reload: Vec<ContentKind> = kinds.clone();
                reload.push(ContentKind::ShaderPack);
                reload.sort();
                reload.dedup();
                for kind in reload {
                    this.reload(kind, cx);
                }
            })
            .ok();
        });
        let ticker = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
            }
        });
        self.message = None;
        self.job = Some(Job {
            progress,
            cancel,
            queued_cf: Vec::new(),
            _task: task,
            _ticker: ticker,
            _busy: busy,
        });
        cx.notify();
    }

    /// Updates one file that came from CurseForge: from the launcher when CurseForge
    /// allows it, else through the blocked card.
    pub fn cf_update_one(&mut self, kind: ContentKind, file_name: &str, cx: &mut Context<Self>) {
        if let Some(update) = self.section(kind).cf_updates.get(file_name).cloned() {
            self.cf_install(
                vec![CfInstallRequest {
                    kind,
                    file: update.file,
                    project: None,
                    install_dependencies: true,
                    force: false,
                }],
                cx,
            );
        }
    }

    /// The CurseForge requests for every CurseForge update the last check found.
    pub(super) fn cf_update_requests(&self, kind: ContentKind) -> Vec<CfInstallRequest> {
        let mut updates: Vec<_> = self.section(kind).cf_updates.values().cloned().collect();
        updates.sort_by_key(|u| u.file.mod_id);
        updates
            .into_iter()
            .map(|update| CfInstallRequest {
                kind,
                file: update.file,
                project: None,
                install_dependencies: true,
                force: false,
            })
            .collect()
    }

    // ---- Files downloaded by hand ----

    /// Shows the card for `files` and starts watching the Downloads folder for them.
    pub fn show_blocked(&mut self, files: Vec<CfPlannedFile>, cx: &mut Context<Self>) {
        let count = files.len();
        self.blocked = Some(BlockedDownloads {
            files,
            added: vec![false; count],
            folder: curseforge_install::downloads_folder(),
            error: None,
            _watch: None,
        });
        self.watch_blocked(cx);
        cx.notify();
    }

    fn watch_blocked(&mut self, cx: &mut Context<Self>) {
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(WATCH_EVERY).await;
                let Ok(Some((folder, waiting, dirs, target))) = this.update(cx, |this, cx| {
                    let blocked = this.blocked.as_ref()?;
                    let folder = blocked.folder.clone()?;
                    let waiting: Vec<(usize, CfPlannedFile)> = blocked
                        .files
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| !blocked.added[*i])
                        .map(|(i, f)| (i, f.clone()))
                        .collect();
                    let (_, dirs, target) = this.context(cx)?;
                    Some((folder, waiting, dirs, target))
                }) else {
                    // The card was closed, or there is nothing to watch.
                    if this
                        .update(cx, |this, _| this.blocked.is_none())
                        .unwrap_or(true)
                    {
                        return;
                    }
                    continue;
                };
                if waiting.is_empty() {
                    return;
                }
                let results = cx
                    .background_spawn(async move {
                        let files: Vec<CfPlannedFile> =
                            waiting.iter().map(|(_, f)| f.clone()).collect();
                        curseforge_install::find_downloaded(&folder, &files)
                            .into_iter()
                            .map(|(at, path)| {
                                let (index, planned) = &waiting[at];
                                let result = curseforge_install::install_downloaded(
                                    &dirs, &target, planned, &path,
                                );
                                (*index, planned.title(), result)
                            })
                            .collect::<Vec<_>>()
                    })
                    .await;
                if results.is_empty() {
                    continue;
                }
                let done = this
                    .update(cx, |this, cx| this.blocked_found(results, cx))
                    .unwrap_or(true);
                if done {
                    return;
                }
            }
        });
        if let Some(blocked) = &mut self.blocked {
            blocked._watch = Some(task);
        }
    }

    /// Takes in files found in the watched folder. Returns whether the card is done.
    fn blocked_found(
        &mut self,
        results: Vec<(usize, String, Result<Placed, ContentError>)>,
        cx: &mut Context<Self>,
    ) -> bool {
        let Some(blocked) = &mut self.blocked else {
            return true;
        };
        let mut kinds = Vec::new();
        for (index, title, result) in results {
            match result {
                Ok(_) => {
                    blocked.added[index] = true;
                    kinds.push(blocked.files[index].kind);
                }
                Err(err) => {
                    blocked.error =
                        Some(tr!(M::CouldntAddDownload, name = title, error = err).into())
                }
            }
        }
        let done = blocked.added.iter().all(|added| *added);
        let names: Vec<String> = blocked.files.iter().map(|f| f.title()).collect();
        if done {
            self.blocked = None;
            self.set_message(
                tr!(M::AddedDownloads, names = format::list(&names)),
                false,
                cx,
            );
        }
        kinds.sort();
        kinds.dedup();
        for kind in kinds {
            self.reload(kind, cx);
        }
        cx.notify();
        done
    }

    pub fn open_blocked_page(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(file) = self.blocked.as_ref().and_then(|b| b.files.get(index)) {
            cx.open_url(&file.file_page());
        }
    }

    pub fn open_all_blocked_pages(&mut self, cx: &mut Context<Self>) {
        if let Some(blocked) = &self.blocked {
            for (file, added) in blocked.files.iter().zip(&blocked.added) {
                if !added {
                    cx.open_url(&file.file_page());
                }
            }
        }
    }

    /// "Watch another folder…": the folder the browser saves downloads to.
    pub fn choose_blocked_folder(&mut self, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t(M::WatchFolderPrompt).into()),
        });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(mut paths))) = picked.await else {
                return;
            };
            let Some(folder) = paths.pop() else {
                return;
            };
            this.update(cx, |this, cx| {
                if let Some(blocked) = &mut this.blocked {
                    blocked.folder = Some(folder);
                    blocked.error = None;
                }
                this.watch_blocked(cx);
                cx.notify();
            })
            .ok();
        }));
    }

    pub fn dismiss_blocked(&mut self, cx: &mut Context<Self>) {
        self.blocked = None;
        cx.notify();
    }

    // ---- Rendering ----

    /// The card for files the user downloads from curseforge.com.
    pub(super) fn render_blocked(&mut self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let blocked = self.blocked.as_ref()?;
        let folder = blocked.folder.as_ref().map(|f| f.display().to_string());
        let mut rows = list_box();
        for (index, (file, added)) in blocked.files.iter().zip(&blocked.added).enumerate() {
            let status = if *added {
                badge(t(M::BlockedAdded), colors::GREEN_300).into_any_element()
            } else {
                Button::new(("blocked-open", index), t(M::BlockedOpen))
                    .variant(ButtonVariant::Primary)
                    .icon(Icon::ArrowUpRightFromSquare)
                    .size(ButtonSize::Slim)
                    .on_click(cx.listener(move |this, _, _, cx| this.open_blocked_page(index, cx)))
                    .into_any_element()
            };
            rows = rows.child(
                row_base(("blocked-row", index), index == 0)
                    .accessible(Role::Group, format!("blocked-{index}"), file.title())
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
                                    .text_color(crate::theme::ink::text())
                                    .child(file.title()),
                            )
                            .child(
                                small(if *added {
                                    file.file.file_name.clone()
                                } else {
                                    tr!(M::BlockedWaiting, file = file.file.file_name)
                                })
                                .truncate()
                                .text_color(crate::theme::ink::faint()),
                            ),
                    )
                    .child(status),
            );
        }
        let waiting = blocked.added.iter().filter(|a| !**a).count();
        let error = blocked.error.clone();
        Some(
            super::view::card()
                .flex()
                .flex_col()
                .gap_3()
                .child(heading(t(M::BlockedTitle)))
                .child(
                    small(match &folder {
                        Some(folder) => tr!(M::BlockedLead, folder = folder),
                        None => t(M::BlockedNoFolder).to_string(),
                    })
                    .whitespace_normal()
                    .text_color(crate::theme::ink::secondary()),
                )
                .child(rows)
                .when_some(error, |this, error| {
                    this.child(small(error).text_color(colors::RED_200))
                })
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .flex_wrap()
                        .gap_2()
                        .when(waiting > 1, |this| {
                            this.child(
                                Button::new("blocked-open-all", t(M::BlockedOpenAll))
                                    .variant(ButtonVariant::Secondary)
                                    .size(ButtonSize::Slim)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_all_blocked_pages(cx)
                                    })),
                            )
                        })
                        .child(
                            Button::new("blocked-folder", t(M::BlockedChangeFolder))
                                .variant(ButtonVariant::Secondary)
                                .size(ButtonSize::Slim)
                                .on_click(
                                    cx.listener(|this, _, _, cx| this.choose_blocked_folder(cx)),
                                ),
                        )
                        .child(
                            Button::new("blocked-skip", t(M::BlockedSkip))
                                .variant(ButtonVariant::Ghost)
                                .size(ButtonSize::Slim)
                                .on_click(cx.listener(|this, _, _, cx| this.dismiss_blocked(cx))),
                        ),
                )
                .into_any_element(),
        )
    }

    /// "Install" on a CurseForge result: the newest file that fits (a release when there
    /// is one, and one the launcher may download over one it may not), with its
    /// dependencies. When none fits, its page opens instead, with every file listed.
    pub fn cf_install_hit(&mut self, kind: ContentKind, project_id: u64, cx: &mut Context<Self>) {
        let (Some((id, _, target)), Some(cf)) = (self.context(cx), self.curseforge(cx)) else {
            return;
        };
        if !self.can_change(cx) || !self.section_mut(kind).cf_resolving.insert(project_id) {
            return;
        }
        let loaders = curseforge::accepted_loaders(target.loader, &target.game_version);
        let loader = match (kind, loaders.as_slice()) {
            (ContentKind::Mod, [only]) => Some(*only),
            _ => None,
        };
        let work = cx.background_spawn(async move {
            let files = cf
                .files(project_id, Some(&target.game_version), loader)
                .await?;
            Ok::<_, launcher_core::content::CurseForgeError>(curseforge_install::pick_file(
                &target, kind, files,
            ))
        });
        cx.spawn(async move |this, cx| {
            let picked = work.await;
            this.update(cx, |this, cx| {
                this.section_mut(kind).cf_resolving.remove(&project_id);
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                match picked {
                    Ok(Some(file)) => this.install_cf_file(kind, file, None, false, true, cx),
                    Ok(None) => this.open_cf_project(kind, project_id, cx),
                    Err(err) => this.set_message(err.to_string(), true, cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Beside the views when CurseForge's is shown: its sorts and the compatibility
    /// filter (shared with Modrinth's view).
    pub(super) fn render_cf_tools(
        &mut self,
        kind: ContentKind,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sort = self.section(kind).cf.sort;
        let sorts = [
            (CfSort::Popularity, M::SortPopular),
            (CfSort::TotalDownloads, M::SortDownloads),
            (CfSort::LastUpdated, M::SortUpdated),
            (CfSort::Name, M::SortName),
        ];
        let selected = sorts.iter().position(|(s, _)| *s == sort).unwrap_or(0);
        let entity = cx.entity();
        let chip = self.compat_chip(kind, cx);
        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(pills(
                "cf-sort",
                sorts
                    .iter()
                    .map(|(_, label)| SharedString::from(t(*label)))
                    .collect(),
                selected,
                move |i, _, cx| {
                    let sort = sorts[i].0;
                    entity.update(cx, |this, cx| this.cf_set_sort(kind, sort, cx))
                },
            ))
            .child(chip)
            .child(self.installed_chip(kind, cx))
            .into_any_element()
    }

    /// Previous and next page of the CurseForge results.
    pub(super) fn render_cf_pager(
        &mut self,
        kind: ContentKind,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let cf = &self.section(kind).cf;
        let total = cf.results.ready().map_or(0, |r| r.total);
        let page_size = PAGE_SIZE as usize;
        super::view::pager(
            kind,
            ("cf-previous-page", "cf-next-page"),
            cf.offset / page_size,
            total.div_ceil(page_size),
            move |this, page, cx| this.cf_set_page(kind, page * page_size, cx),
            cx,
        )
    }

    /// A CurseForge result as a row of the list: opens its page, or installs it.
    pub(super) fn render_cf_hit(
        &mut self,
        kind: ContentKind,
        index: usize,
        hit: &CfMod,
        installed: bool,
        first: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let icon = hit.icon_url().and_then(|url| self.remote_icon(url, cx));
        let id = hit.id;
        let resolving = self.section(kind).cf_resolving.contains(&hit.id);
        let busy = self.locked(cx) || self.job.is_some();
        let author = hit
            .authors
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default();
        let open = self
            .section(kind)
            .cf
            .project
            .as_ref()
            .is_some_and(|p| p.project_id == id)
            && self.panel_closing != Some(kind);
        row_base(("cf-hit", index), first)
            .accessible(
                Role::ListItem,
                format!("{}-cf-hit-{id}", kind.dir()),
                hit.name.clone(),
            )
            .aria_description(hit.summary.clone())
            .aria_selected(open)
            .keyboard_focus()
            .cursor_pointer()
            .hover(|style| style.bg(colors::GRAY_750))
            .when(open, |this| {
                this.bg(crate::theme::accent::base().opacity(18))
            })
            .on_press(cx.listener(move |this, _, _, cx| this.open_cf_project(kind, id, cx)))
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
                                    .child(hit.name.clone()),
                            )
                            .when(!author.is_empty(), |this| {
                                this.child(
                                    small(tr!(M::ByAuthor, author = author))
                                        .flex_none()
                                        .text_color(crate::theme::ink::muted()),
                                )
                            }),
                    )
                    .child(
                        small(hit.summary.clone())
                            .truncate()
                            .text_color(crate::theme::ink::secondary()),
                    ),
            )
            .child(
                small(counted(
                    hit.download_count.max(0.0) as u64,
                    P::Downloads,
                    M::DownloadsCompact,
                ))
                .flex_none()
                .text_color(crate::theme::ink::muted()),
            )
            .map(|this| {
                if installed {
                    this.child(super::view::installed_mark())
                } else {
                    this.child(
                        Button::new(("cf-install-hit", index), t(CommonMsg::Install))
                            .variant(ButtonVariant::Primary)
                            .size(ButtonSize::Slim)
                            .loading(resolving)
                            .disabled(busy || resolving)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                // Not also the row's click, which opens the page.
                                cx.stop_propagation();
                                this.cf_install_hit(kind, id, cx)
                            })),
                    )
                }
            })
            .into_any_element()
    }

    pub(super) fn render_cf_project(
        &mut self,
        kind: ContentKind,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let Some(view) = self.section(kind).cf.project.as_ref() else {
            return div().into_any_element();
        };
        let project = view.project.clone();
        let files = view.files.clone();
        let show_all = view.show_all;
        let with_dependencies = view.with_dependencies;
        let confirm_force = view.confirm_force;
        let tab = view.tab;
        let description = view.description.clone();
        let back = self.page_bar(kind, "cf-back", None, Self::close_cf_project, cx);
        let project = match project {
            Load::Ready(project) => project,
            Load::Failed(err) => {
                return div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(back)
                    .child(small(err).text_color(colors::RED_200))
                    .into_any_element();
            }
            _ => {
                return div()
                    .flex()
                    .flex_col()
                    .gap_3()
                    .child(back)
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
        let icon = project.icon_url().and_then(|url| self.remote_icon(url, cx));
        let page = project.page_url();
        let author = project
            .authors
            .first()
            .map(|a| a.name.clone())
            .unwrap_or_default();
        let bar = self.page_bar(
            kind,
            "cf-back",
            Some(("open-curseforge", t(M::ViewOnCurseForge).into(), page)),
            Self::close_cf_project,
            cx,
        );
        let icon_size = if self.panel_width.is_some() { 56. } else { 72. };
        let mut column = div().flex().flex_col().gap_4().child(bar).child(
            div()
                .flex()
                .flex_row()
                .items_start()
                .gap_4()
                .child(icon_box(icon, icon_size))
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
                                .whitespace_normal()
                                .child(project.name.clone()),
                        )
                        .child(
                            div()
                                .text_color(crate::theme::ink::secondary())
                                .whitespace_normal()
                                .child(project.summary.clone()),
                        )
                        .child(
                            small(if author.is_empty() {
                                counted(
                                    project.download_count.max(0.0) as u64,
                                    P::Downloads,
                                    M::DownloadsCompact,
                                )
                            } else {
                                format!(
                                    "{} · {}",
                                    tr!(M::ByAuthor, author = author),
                                    counted(
                                        project.download_count.max(0.0) as u64,
                                        P::Downloads,
                                        M::DownloadsCompact
                                    )
                                )
                            })
                            .text_color(crate::theme::ink::muted()),
                        ),
                ),
        );
        if kind == ContentKind::ShaderPack && self.shaders.loader.is_none() {
            column = column.child(self.render_shader_mod(cx));
        }
        let screenshots: Vec<(String, Option<String>)> = project
            .screenshots
            .iter()
            .filter_map(|s| Some((s.url.clone()?, s.title.clone())))
            .collect();
        let mut tabs: Vec<(super::ProjectTab, SharedString)> =
            vec![(super::ProjectTab::Description, t(M::Description).into())];
        if !screenshots.is_empty() {
            tabs.push((super::ProjectTab::Gallery, t(M::Gallery).into()));
        }
        tabs.push((super::ProjectTab::Versions, t(M::Files).into()));
        let tab = if tabs.iter().any(|(t, _)| *t == tab) {
            tab
        } else {
            super::ProjectTab::Versions
        };
        column = column.child(self.page_tabs(
            kind,
            "cf-project-tabs",
            &tabs,
            tab,
            Self::set_cf_project_tab,
            cx,
        ));
        match tab {
            super::ProjectTab::Description => {
                let body = match description {
                    Load::Ready(html) if html.trim().is_empty() => {
                        muted(t(M::NoDescription)).into_any_element()
                    }
                    Load::Ready(html) => {
                        let blocks = self.parsed_description(&format!("cf-{}", project.id), &html);
                        self.description_box(
                            format!("cf-description-{}", project.id),
                            &blocks,
                            t(M::RestOnCurseForge).into(),
                            cx,
                        )
                    }
                    Load::Failed(err) => small(err).text_color(colors::RED_200).into_any_element(),
                    _ => div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .text_color(crate::theme::ink::muted())
                        .child(LoadingDots)
                        .child(t(M::LoadingDescription))
                        .into_any_element(),
                };
                return column.child(body).into_any_element();
            }
            super::ProjectTab::Gallery => {
                return column
                    .child(self.render_gallery(
                        format!("cf-gallery-{}", project.id),
                        screenshots,
                        cx,
                    ))
                    .into_any_element();
            }
            super::ProjectTab::Versions => {}
        }
        let files = match files {
            Load::Ready(files) => files,
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
        let installed_files: Vec<u64> = self
            .section(kind)
            .items()
            .iter()
            .filter_map(|i| i.curseforge.as_ref())
            .filter(|c| c.project_id == project.id)
            .map(|c| c.file_id)
            .collect();
        let shown: Vec<(CfFile, Option<String>)> = files
            .into_iter()
            .map(|f| {
                let problem = target
                    .as_ref()
                    .and_then(|t| t.curseforge_incompatibility(kind, &f));
                (f, problem)
            })
            .filter(|(_, problem)| show_all || problem.is_none())
            .collect();
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
                            Toggle::new(("cf-show-all", kind as usize), show_all)
                                .label(t(M::ShowAllVersions))
                                .on_change(cx.listener(move |this, value: &bool, _, cx| {
                                    if let Some(view) = this.cf_project_mut(kind) {
                                        view.show_all = *value;
                                    }
                                    cx.notify();
                                })),
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
                                Toggle::new(("cf-with-deps", kind as usize), with_dependencies)
                                    .label(t(M::AlsoInstallDependencies))
                                    .on_change(cx.listener(move |this, value: &bool, _, cx| {
                                        if let Some(view) = this.cf_project_mut(kind) {
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
                        project = project.name,
                        show_all = t(M::ShowVersionsShort)
                    ))
                    .whitespace_normal(),
                )
                .into_any_element();
        }
        let mut rows = list_box();
        for (index, (file, problem)) in shown.into_iter().take(40).enumerate() {
            let (type_label, type_color) = match file.release_type {
                2 => (t(M::Beta), colors::YELLOW_300),
                3 => (t(M::Alpha), colors::RED_200),
                _ => (t(M::Release), colors::GREEN_300),
            };
            let is_installed = installed_files.contains(&file.id);
            let day = file.file_date.get(..10).unwrap_or_default();
            let date = chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
                .map_or_else(|_| day.to_string(), |date| format::short_date(date, true));
            let forcing = confirm_force == Some(file.id);
            let tags = file.tags();
            let loaders: Vec<String> = tags.loaders.iter().map(|l| l.label().to_string()).collect();
            let file_for_click = file.clone();
            let project_for_click = project.clone();
            let id = file.id;
            let button = if is_installed {
                badge(t(M::InstalledBadge), colors::GREEN_300).into_any_element()
            } else if problem.is_some() && !forcing {
                Button::new(("cf-install-anyway", index), t(M::InstallAnyway))
                    .variant(ButtonVariant::Secondary)
                    .size(ButtonSize::Slim)
                    .disabled(busy)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(view) = this.cf_project_mut(kind) {
                            view.confirm_force = Some(id);
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            } else {
                Button::new(
                    ("cf-install", index),
                    if forcing {
                        t(M::InstallItAnyway).to_string()
                    } else {
                        t(CommonMsg::Install).to_string()
                    },
                )
                .variant(if forcing {
                    ButtonVariant::Danger
                } else {
                    ButtonVariant::Primary
                })
                .icon(Icon::Download)
                .size(ButtonSize::Slim)
                .disabled(busy)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.install_cf_file(
                        kind,
                        file_for_click.clone(),
                        Some(project_for_click.clone()),
                        forcing,
                        with_dependencies,
                        cx,
                    )
                }))
                .into_any_element()
            };
            rows = rows.child(
                row_base(("cf-file-row", index), index == 0)
                    .accessible(
                        Role::Group,
                        format!("{}-cf-file-{id}", kind.dir()),
                        file.display_name_or_file().to_string(),
                    )
                    .aria_description(format!("{type_label} · {date}"))
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
                                            .child(file.display_name_or_file().to_string()),
                                    )
                                    .child(badge(type_label, type_color))
                                    .when(!file.downloadable(), |this| {
                                        this.child(badge(
                                            t(M::WebsiteDownload),
                                            crate::theme::ink::muted(),
                                        ))
                                    }),
                            )
                            .child(
                                small(format!(
                                    "{} · {} · {date} · {}",
                                    short_list(&tags.game_versions, 3),
                                    if loaders.is_empty() {
                                        String::from("—")
                                    } else {
                                        short_list(&loaders, 3)
                                    },
                                    format::bytes(file.file_length)
                                ))
                                .truncate()
                                .text_color(crate::theme::ink::faint()),
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
                small(crate::i18n::trn!(P::MoreVersions, total - 40))
                    .text_color(crate::theme::ink::muted()),
            );
        }
        column.into_any_element()
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// A file CurseForge doesn't let launchers download.
    pub(in crate::instances::content) fn planned(id: u64) -> CfPlannedFile {
        let file = serde_json::json!({
            "id": id,
            "modId": id,
            "fileName": format!("lodestone-test-blocked-{id}.jar"),
        });
        CfPlannedFile {
            kind: ContentKind::Mod,
            file: serde_json::from_value(file).expect("a CurseForge file"),
            project: None,
            on_modrinth: None,
            depth: 0,
        }
    }

    #[test]
    fn unfinished_leaves_out_the_files_added() {
        let blocked = BlockedDownloads {
            files: vec![planned(1), planned(2), planned(3)],
            added: vec![false, true, false],
            folder: None,
            error: None,
            _watch: None,
        };
        let ids: Vec<u64> = blocked.unfinished().iter().map(|f| f.file.id).collect();
        assert_eq!(ids, vec![1, 3]);
    }
}
