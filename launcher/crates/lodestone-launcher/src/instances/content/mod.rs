//! The Content tab: an instance's mods, resource packs and shader packs. The list shows
//! what is installed, then Modrinth's projects to browse (and CurseForge's, in builds
//! with Lodestone's CurseForge key; see [`curseforge`]), each a page at a time. One
//! search field filters what is installed and, as it is typed, searches the platforms
//! too; Modrinth's results also have filters (see [`filters`]).
//!
//! The work is done by `launcher-core::content`; this view keeps the state of each
//! section, runs that work in the background and refuses changes while the game runs.

pub(crate) mod curseforge;
pub(crate) mod filters;
pub(crate) mod view;

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use gpui::{
    AppContext as _, Context, Entity, EventEmitter, RenderImage, SharedString, Subscription, Task,
};
use launcher_core::{
    content::{
        ContentKind, InstanceDirs, Modrinth, Platforms, Target,
        curseforge_install::CfInstallRequest,
        files,
        install::{self, InstallOutcome, InstallProgress, InstallRequest},
        listing::{self, CurseForgeUpdate, Item, Listing, ShaderState},
        options::PackChange,
        packs::Compatibility,
        shaders::{self, ShaderModOffer},
    },
    game::{download::CancelToken, loaders::LoaderKind},
};
use lodestone_minecraft::modrinth::{
    ClientSearchParams, ModrinthProject, ModrinthSearchIndex, ModrinthSearchResponse, RawVersion,
};

use super::Instances;
use super::jobs::{BusyFiles, Parked};
use crate::components::{Icon, TextInput};
use crate::file_dialog::{self, Pick};
use crate::i18n::{ContentMsg as M, format, t, tr};

/// The "Add …" rows of a tab's menu, given `cx.can_select_mixed_files_and_dirs()`:
/// each row's id, label and dialog. Mods are only files; resource and shader packs
/// are `.zip` files or folders, which get a row each where the dialog can't show both.
pub fn add_rows(kind: ContentKind, mixed: bool) -> Vec<(&'static str, M, Pick)> {
    let picks = if kind.allows_folders() {
        Pick::file_or_folder(mixed)
    } else {
        &[Pick::Files]
    };
    picks
        .iter()
        .map(|&pick| match pick {
            Pick::Files | Pick::FilesOrFolders => ("menu-add-file", M::AddFile, pick),
            Pick::Folders => ("menu-add-folder", M::AddFolder, pick),
        })
        .collect()
}

/// Results per page.
pub const PAGE_SIZE: u32 = 20;
/// How long typing must pause before a search starts.
const SEARCH_DELAY: Duration = Duration::from_millis(300);

/// Something loading in the background.
#[derive(Clone, Debug, Default)]
pub enum Load<T> {
    #[default]
    NotLoaded,
    Loading,
    Ready(T),
    Failed(SharedString),
}

impl<T> Load<T> {
    pub fn ready(&self) -> Option<&T> {
        match self {
            Load::Ready(value) => Some(value),
            _ => None,
        }
    }

    pub fn is_loading(&self) -> bool {
        matches!(self, Load::Loading)
    }
}

/// The parts of a project's page. A page without a description or gallery skips
/// those tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProjectTab {
    #[default]
    Description,
    Gallery,
    Versions,
}

/// What a section's list shows: what is installed, or one platform's projects to
/// browse. Each view has the list to itself, so a long installed list never pushes the
/// projects to browse out of reach.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ListView {
    #[default]
    Installed,
    Modrinth,
    CurseForge,
}

impl ListView {
    /// The search field's hint in this view.
    fn placeholder(self) -> M {
        match self {
            ListView::Installed => M::SearchInstalled,
            ListView::Modrinth => M::SearchModrinth,
            ListView::CurseForge => M::SearchCurseForge,
        }
    }
}

/// A Modrinth project opened from Browse or a link.
pub struct ProjectView {
    pub project_id: String,
    pub tab: ProjectTab,
    pub project: Load<ModrinthProject>,
    pub versions: Load<Vec<RawVersion>>,
    /// The version a link pointed to; it is listed first.
    pub linked_version: Option<String>,
    pub show_all: bool,
    pub with_dependencies: bool,
    /// An incompatible version whose "Install anyway" was clicked once.
    pub confirm_force: Option<String>,
    _task: Option<Task<()>>,
}

pub struct Browse {
    pub search: Entity<TextInput>,
    /// The text the current results are for.
    text: String,
    pub only_compatible: bool,
    /// Whether results the instance has already stay in the Modrinth and CurseForge
    /// views, marked Installed.
    pub show_installed: bool,
    /// Modrinth's categories, where a mod runs and open source.
    pub filters: filters::Filters,
    /// The filter panel is open.
    pub filters_open: bool,
    pub index: ModrinthSearchIndex,
    pub offset: u32,
    pub results: Load<ModrinthSearchResponse>,
    pub project: Option<ProjectView>,
    task: Option<Task<()>>,
}

pub struct Section {
    pub kind: ContentKind,
    pub listing: Load<Listing>,
    /// The decoded icons of the listed files, by file name.
    pub icons: HashMap<String, Arc<RenderImage>>,
    /// Newer versions, by file name.
    pub updates: HashMap<String, RawVersion>,
    /// Newer CurseForge files, by file name.
    pub cf_updates: HashMap<String, CurseForgeUpdate>,
    /// The CurseForge results and project page.
    pub cf: curseforge::CfBrowse,
    /// CurseForge projects whose Install was clicked, while their file is picked.
    pub cf_resolving: HashSet<u64>,
    pub checking_updates: bool,
    /// Whether updates were looked for once on their own, after the first listing.
    auto_checked: bool,
    pub browse: Browse,
    /// What the list shows.
    pub view: ListView,
    /// Modrinth projects whose Install was clicked, while their version is picked.
    pub resolving: HashSet<String>,
    /// The Installed list's scroll state and the entries it was last told about.
    rows: gpui::ListState,
    plan: Vec<view::Entry>,
    /// The page of the Installed list shown, from 0 (see [`view::page_of_plan`]). It is
    /// brought back to the last page when the list gets shorter.
    installed_page: usize,
    /// A file to show once the section has been read again after a change to it: a
    /// resource pack moved, or turned on or off, which can land on another page.
    follow: Option<String>,
    /// The file the list's next frame scrolls to, and whether its page was turned to
    /// show it (see [`ContentView::follow`]).
    reveal: Option<(String, bool)>,
    load_task: Option<Task<()>>,
    updates_task: Option<Task<()>>,
}

impl Section {
    fn new(kind: ContentKind, cx: &mut Context<ContentView>) -> Self {
        let search = cx.new(|cx| {
            TextInput::new(cx)
                .with_placeholder(ListView::default().placeholder())
                .with_leading_icon(Icon::MagnifyingGlass)
                .with_clear_button()
                .with_automation_id(format!("{}-search", kind.dir()))
        });
        Self {
            kind,
            listing: Load::NotLoaded,
            icons: HashMap::new(),
            updates: HashMap::new(),
            cf_updates: HashMap::new(),
            cf: curseforge::CfBrowse::default(),
            cf_resolving: HashSet::new(),
            checking_updates: false,
            auto_checked: false,
            resolving: HashSet::new(),
            browse: Browse {
                search,
                text: String::new(),
                only_compatible: true,
                show_installed: true,
                filters: filters::Filters::default(),
                filters_open: false,
                index: ModrinthSearchIndex::Relevance,
                offset: 0,
                results: Load::NotLoaded,
                project: None,
                task: None,
            },
            view: ListView::default(),
            rows: gpui::ListState::new(0, gpui::ListAlignment::Top, crate::theme::px(400.)),
            plan: Vec::new(),
            installed_page: 0,
            follow: None,
            reveal: None,
            load_task: None,
            updates_task: None,
        }
    }

    pub fn items(&self) -> &[Item] {
        self.listing.ready().map_or(&[], |l| l.items.as_slice())
    }

    /// The installed Modrinth projects, by id, with their version ids.
    pub fn installed_projects(&self) -> HashMap<String, Vec<String>> {
        let mut map: HashMap<String, Vec<String>> = HashMap::new();
        for item in self.items() {
            if let Some(source) = &item.modrinth {
                map.entry(source.project_id.clone())
                    .or_default()
                    .push(source.version_id.clone());
            }
        }
        map
    }
}

/// An install in progress.
pub struct Job {
    pub progress: Arc<InstallProgress>,
    pub cancel: CancelToken,
    /// CurseForge updates to install once the Modrinth ones of "Update all" are in.
    queued_cf: Vec<CfInstallRequest>,
    _task: Task<()>,
    _ticker: Task<()>,
    /// Play waits while the install runs.
    _busy: BusyFiles,
}

/// How an install went that finished while its instance wasn't shown, for when it is
/// again.
struct Finished {
    message: Option<(SharedString, bool)>,
    /// The CurseForge half of "Update all", which starts then.
    queued_cf: Vec<CfInstallRequest>,
}

/// An icon from Modrinth's CDN.
pub enum RemoteIcon {
    Loading,
    Ready(Arc<RenderImage>),
    Failed,
}

/// What the Content tab asks the window to do.
pub enum ContentEvent {
    /// Open "New instance" with Fabric chosen.
    NewFabricInstance,
    /// Open "Copy with another version or loader" for the instance, with the loader
    /// chosen: a vanilla instance's way to mods and shaders.
    CopyWithLoader(String, LoaderKind),
    /// Open "Add a modpack" for the instance.
    AddModpack(String),
    /// A mod being turned off has enabled mods that can't load without it, or one being
    /// turned on needs mods that are off: ask whether to turn those off or on too (then
    /// [`ContentView::set_mods_enabled`]).
    ConfirmToggle(TogglePrompt),
}

/// "Turn off Fabric API?" or "Turn on Sodium?": a mod being turned off or on, and the
/// mods to turn off or on with it.
#[derive(Clone, Debug, PartialEq)]
pub struct TogglePrompt {
    pub instance_id: String,
    pub file_name: String,
    pub name: String,
    /// Whether the mod is being turned on.
    pub enable: bool,
    /// Turning off: the enabled mods that need it. Turning on: the mods that are off
    /// that it needs.
    pub others: Vec<launcher_core::content::mod_finder::LinkedMod>,
}

pub struct ContentView {
    instances: Entity<Instances>,
    instance_id: Option<String>,
    /// The selected instance's loader and loader version, which decide what fits it
    /// (filters, compatibility, the shader mod offer).
    loader: Option<(launcher_core::instances::Loader, Option<String>)>,
    pub kind: ContentKind,
    sections: [Section; 3],
    /// The shader mod and pack, from the last listing.
    pub shaders: ShaderState,
    pub shader_offer: Load<ShaderModOffer>,
    shader_offer_task: Option<Task<()>>,
    /// For a vanilla instance: the mod loader "Copy with …" would give a copy (Fabric, or
    /// Forge where Fabric doesn't run), or `None` when no loader supports its version.
    pub mod_loader_offer: Load<Option<LoaderKind>>,
    mod_loader_offer_task: Option<Task<()>>,
    /// The shown instance's install.
    pub job: Option<Job>,
    /// The installs of instances not shown, which go on meanwhile.
    parked: Parked<Job>,
    /// Installs that finished while their instance wasn't shown, by instance.
    finished: HashMap<String, Finished>,
    /// Files to download from curseforge.com by hand, and the folder watched for them.
    pub blocked: Option<curseforge::BlockedDownloads>,
    /// The last result or error.
    pub message: Option<(SharedString, bool)>,
    /// The file whose Delete was clicked once.
    pub confirm_delete: Option<(ContentKind, String)>,
    /// The file being moved to the Trash, off the window's thread, and its instance.
    pub deleting: Option<(String, ContentKind, String)>,
    /// The mod whose toggle was pressed, while the mods that need it (or that it
    /// needs) are looked for: every jar is read.
    pub checking_dependents: Option<String>,
    _dependents: Option<Task<()>>,
    _delete: Option<Task<()>>,
    remote_icons: HashMap<String, RemoteIcon>,
    /// Images of project descriptions and galleries, by URL.
    description_images: HashMap<String, RemoteIcon>,
    /// The width of the project panel beside the list, or `None` when a project's page
    /// takes the whole area (a narrow window). Set on each frame.
    pub panel_width: Option<f32>,
    /// The panel of this section is sliding shut; its project is let go once it has.
    pub panel_closing: Option<ContentKind>,
    /// Where the open project's page is scrolled.
    pub page_scroll: gpui::ScrollHandle,
    /// Parsed project descriptions, by project id.
    descriptions: HashMap<String, Arc<Vec<launcher_core::markdown::Block>>>,
    /// Whether the game was running at the last check, to reload when it stops.
    was_active: bool,
    /// The instance's content revision when its lists were last read (see
    /// [`Instances::content_revision`]).
    revision: u64,
    /// The instance's "Beta and alpha updates" at the last check.
    prerelease: bool,
    /// The "⋯" menu beside the search is open.
    pub menu_open: bool,
    /// The Modrinth sort menu is open.
    pub sort_open: bool,
    _prompt: Option<Task<()>>,
    _observe: Vec<Subscription>,
}

impl EventEmitter<ContentEvent> for ContentView {}

fn index_of(kind: ContentKind) -> usize {
    match kind {
        ContentKind::Mod => 0,
        ContentKind::ResourcePack => 1,
        ContentKind::ShaderPack => 2,
    }
}

impl file_dialog::ShowsError for ContentView {
    fn show_error(&mut self, message: SharedString, cx: &mut Context<Self>) {
        self.set_message(message, true, cx);
    }
}

impl ContentView {
    /// Adds the decoded icons of the listed content, for the performance monitor.
    pub fn tally_images(&self, tally: &mut crate::perf::ImageTally) {
        for section in &self.sections {
            for image in section.icons.values() {
                tally.add(image);
            }
        }
    }

    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let sections = ContentKind::ALL.map(|kind| Section::new(kind, cx));
        let mut observe = vec![cx.observe(&instances, |this, _, cx| this.sync(cx))];
        for section in &sections {
            let kind = section.kind;
            observe.push(cx.observe(&section.browse.search, move |this, _, cx| {
                this.search_changed(kind, cx)
            }));
        }
        let mut this = Self {
            instances,
            instance_id: None,
            loader: None,
            kind: ContentKind::Mod,
            sections,
            shaders: ShaderState::default(),
            shader_offer: Load::NotLoaded,
            shader_offer_task: None,
            mod_loader_offer: Load::NotLoaded,
            mod_loader_offer_task: None,
            job: None,
            parked: Parked::default(),
            finished: HashMap::new(),
            blocked: None,
            message: None,
            confirm_delete: None,
            deleting: None,
            checking_dependents: None,
            _dependents: None,
            _delete: None,
            remote_icons: HashMap::new(),
            description_images: HashMap::new(),
            panel_width: None,
            panel_closing: None,
            page_scroll: gpui::ScrollHandle::new(),
            descriptions: HashMap::new(),
            was_active: false,
            revision: 0,
            prerelease: false,
            menu_open: false,
            sort_open: false,
            _prompt: None,
            _observe: observe,
        };
        this.sync(cx);
        this
    }

    pub fn section(&self, kind: ContentKind) -> &Section {
        &self.sections[index_of(kind)]
    }

    pub(crate) fn section_mut(&mut self, kind: ContentKind) -> &mut Section {
        &mut self.sections[index_of(kind)]
    }

    pub fn instance_id(&self) -> Option<&str> {
        self.instance_id.as_deref()
    }

    /// Whether the instance has a mod loader, so a modpack can be added to it.
    pub fn has_loader(&self) -> bool {
        self.loader
            .as_ref()
            .is_some_and(|(loader, _)| loader.kind().is_some())
    }

    /// Follows the selected instance: a new one, or a new loader for the same one (a
    /// modpack update, a linked server's pack, or another loader version in Settings),
    /// starts over; a game that stopped may have changed its options, so the lists are
    /// read again.
    fn sync(&mut self, cx: &mut Context<Self>) {
        let (selected, loader, active, prerelease, revision) = {
            let instances = self.instances.read(cx);
            // how an install went goes with its instance, not to a new one given its id
            if !self.finished.is_empty() {
                let list = instances.list();
                self.finished
                    .retain(|id, _| list.iter().any(|instance| &instance.id == id));
            }
            let revision = instances
                .selected()
                .map_or(0, |i| instances.content_revision(&i.id));
            let selected = instances.selected();
            let prerelease = selected.is_some_and(|i| i.prerelease_updates);
            let loader = selected.map(|i| (i.loader, i.loader_version.clone()));
            let selected = selected.map(|i| i.id.clone());
            let active = selected
                .as_deref()
                .is_some_and(|id| instances.is_active(id));
            (selected, loader, active, prerelease, revision)
        };
        let changed = revision != self.revision;
        self.revision = revision;
        if selected != self.instance_id || loader != self.loader {
            let from = std::mem::replace(&mut self.instance_id, selected);
            let same = from == self.instance_id;
            // the card of files to download by hand stays with its instance
            let blocked = self.blocked.take();
            if !same && let (Some(from), Some(blocked)) = (from.as_deref(), &blocked) {
                let files = blocked.unfinished();
                self.instances.update(cx, |instances, _| {
                    instances.put_back_pending_downloads(from, files)
                });
            }
            // an install goes on, and is the tab's again when its instance is
            self.parked
                .switch(&mut self.job, from.as_deref(), self.instance_id.as_deref());
            self.loader = loader;
            self.prerelease = prerelease;
            self.reset(cx);
            if same {
                self.blocked = blocked;
            }
            self.resume_finished(cx);
        } else if prerelease != self.prerelease {
            // "Beta and alpha updates" changed: the updates found no longer apply.
            self.prerelease = prerelease;
            for kind in ContentKind::ALL {
                let checked =
                    !self.section(kind).updates.is_empty() || self.section(kind).auto_checked;
                self.section_mut(kind).updates.clear();
                if checked {
                    self.look_for_updates(kind, true, cx);
                }
            }
        } else if (self.was_active && !active) || changed {
            for kind in ContentKind::ALL {
                if !matches!(self.section(kind).listing, Load::NotLoaded) {
                    self.reload(kind, cx);
                }
            }
            if self.was_active && !active {
                self.resume_finished(cx);
            }
        }
        self.was_active = active;
        // Files an imported CurseForge pack left for the user to download.
        if self.blocked.is_none()
            && let Some(id) = self.instance_id.clone()
            && self.instances.read(cx).has_pending_downloads(&id)
        {
            let files = self
                .instances
                .update(cx, |instances, _| instances.take_pending_downloads(&id))
                .unwrap_or_default();
            if !files.is_empty() {
                self.show_blocked(files, cx);
            }
        }
        cx.notify();
    }

    fn reset(&mut self, cx: &mut Context<Self>) {
        for kind in ContentKind::ALL {
            let fresh = Section::new(kind, cx);
            let section = self.section_mut(kind);
            // Keep the search box (its observer).
            let search = section.browse.search.clone();
            *section = fresh;
            section.browse.search = search;
            section
                .browse
                .search
                .update(cx, |input, cx| input.set_text("", cx));
        }
        self.shaders = ShaderState::default();
        self.shader_offer = Load::NotLoaded;
        self.shader_offer_task = None;
        self.mod_loader_offer = Load::NotLoaded;
        self.mod_loader_offer_task = None;
        self.blocked = None;
        self.message = None;
        self.confirm_delete = None;
    }

    /// Says how the shown instance's install went if it finished while another was
    /// shown, and starts the CurseForge half of its "Update all" (when its game, if
    /// started meanwhile, has stopped).
    fn resume_finished(&mut self, cx: &mut Context<Self>) {
        let Some(finished) = self
            .instance_id
            .as_ref()
            .and_then(|id| self.finished.remove(id))
        else {
            return;
        };
        if !finished.queued_cf.is_empty() {
            let idle = self.job.is_none();
            self.message = None;
            self.cf_install(finished.queued_cf.clone(), cx);
            // refused with a reason (the game was started meanwhile): the reason stays
            // shown, and the updates and how the rest went wait for the game to stop
            if !(idle && self.job.is_some())
                && self.message.is_some()
                && let Some(id) = self.instance_id.clone()
            {
                self.finished.insert(id, finished);
                return;
            }
        }
        if let Some((message, error)) = finished.message {
            self.set_message(message, error, cx);
        }
    }

    /// The install of instance `id` finished: takes its job from where it is, with
    /// whether its instance is shown.
    pub(super) fn finish_job(&mut self, id: &str) -> Option<(Job, bool)> {
        self.parked
            .finish(&mut self.job, self.instance_id.as_deref(), id)
    }

    /// Keeps how the install of `id`, not shown, went, for when it is.
    pub(super) fn keep_finished(
        &mut self,
        id: &str,
        message: Option<(SharedString, bool)>,
        queued_cf: Vec<CfInstallRequest>,
    ) {
        self.finished
            .insert(id.to_string(), Finished { message, queued_cf });
    }

    /// The instance's folders and what it accepts, with its shader mod.
    pub fn context(&self, cx: &gpui::App) -> Option<(String, InstanceDirs, Target)> {
        let id = self.instance_id.clone()?;
        let instances = self.instances.read(cx);
        let mut target = instances.content_target(&id)?;
        target.shader_loader = self.shaders.loader;
        Some((id.clone(), instances.content_dirs(&id), target))
    }

    /// The instance's "Beta and alpha updates".
    fn prerelease_updates(&self, cx: &gpui::App) -> bool {
        let id = self.instance_id.as_deref();
        self.instances
            .read(cx)
            .list()
            .iter()
            .find(|i| Some(i.id.as_str()) == id)
            .is_some_and(|i| i.prerelease_updates)
    }

    pub(crate) fn modrinth(&self, cx: &gpui::App) -> Option<Arc<Modrinth>> {
        self.instances.read(cx).modrinth()
    }

    /// Whether changes are refused: the game is starting or running.
    pub fn locked(&self, cx: &gpui::App) -> bool {
        self.instance_id
            .as_deref()
            .is_some_and(|id| self.instances.read(cx).is_active(id))
    }

    /// Whether a change can be made now, saying why not if it can't.
    pub(crate) fn can_change(&mut self, cx: &mut Context<Self>) -> bool {
        if self.locked(cx) {
            self.set_message(t(M::StopGameFirst), true, cx);
            return false;
        }
        if self.job.is_some() {
            self.set_message(t(M::WaitForInstall), true, cx);
            return false;
        }
        true
    }

    pub fn set_message(
        &mut self,
        message: impl Into<SharedString>,
        error: bool,
        cx: &mut Context<Self>,
    ) {
        self.message = Some((message.into(), error));
        cx.notify();
    }

    pub fn select_kind(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        self.kind = kind;
        self.confirm_delete = None;
        cx.notify();
    }

    /// Shows `view` in the list of `kind`, from its top. What is typed stays: it
    /// searches the view switched to.
    pub fn set_view(&mut self, kind: ContentKind, view: ListView, cx: &mut Context<Self>) {
        if self.section(kind).view == view {
            return;
        }
        let section = self.section_mut(kind);
        section.view = view;
        section.rows.scroll_to(gpui::ListOffset::default());
        let search = section.browse.search.clone();
        search.update(cx, |input, cx| {
            input.set_placeholder(view.placeholder(), cx)
        });
        cx.notify();
    }

    /// Scrolls the list of `kind` back to its top.
    pub fn scroll_list_to_top(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        self.section(kind)
            .rows
            .scroll_to(gpui::ListOffset::default());
        cx.notify();
    }

    /// Shows page `page` (from 0) of the Installed list of `kind`, from its top.
    pub fn set_installed_page(&mut self, kind: ContentKind, page: usize, cx: &mut Context<Self>) {
        let section = self.section_mut(kind);
        section.installed_page = page;
        section.follow = None;
        section.reveal = None;
        section.rows.scroll_to(gpui::ListOffset::default());
        cx.notify();
    }

    /// Scrolls the open project's page back to its top.
    pub fn scroll_page_to_top(&mut self, _kind: ContentKind, cx: &mut Context<Self>) {
        self.page_scroll.set_offset(gpui::Point::default());
        cx.notify();
    }

    /// What is typed in the section's search, trimmed.
    pub fn query(&self, kind: ContentKind, cx: &gpui::App) -> String {
        self.section(kind)
            .browse
            .search
            .read(cx)
            .text()
            .trim()
            .to_string()
    }

    /// Empties the search, as Escape in the field does.
    pub fn clear_search(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        let search = self.section(kind).browse.search.clone();
        search.update(cx, |input, cx| input.set_text("", cx));
    }

    /// "Install" on a Modrinth result: the newest version that fits (a release when
    /// there is one), with its dependencies. When none fits, its page opens instead,
    /// with every version listed.
    pub fn install_hit(&mut self, kind: ContentKind, project_id: String, cx: &mut Context<Self>) {
        let (Some((id, _, target)), Some(api)) = (self.context(cx), self.modrinth(cx)) else {
            return;
        };
        if !self.can_change(cx) || !self.section_mut(kind).resolving.insert(project_id.clone()) {
            return;
        }
        let project = project_id.clone();
        let work = cx.background_spawn(async move {
            let versions = api.versions(&project, &[], &[]).await?;
            Ok::<_, launcher_core::content::modrinth::ModrinthError>(install::pick_version(
                &target, kind, versions,
            ))
        });
        cx.spawn(async move |this, cx| {
            let picked = work.await;
            this.update(cx, |this, cx| {
                this.section_mut(kind).resolving.remove(&project_id);
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                match picked {
                    Ok(Some(version)) => this.install_version(kind, version, None, false, true, cx),
                    Ok(None) => this.open_project(kind, project_id, None, cx),
                    Err(err) => this.set_message(err.to_string(), true, cx),
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }

    /// Loads the section's list if it hasn't been.
    pub fn ensure_loaded(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        if matches!(self.section(kind).listing, Load::NotLoaded) {
            self.reload(kind, cx);
        }
    }

    /// Mods of `instance_id` were turned on or off somewhere else (the crash dialog):
    /// reads them again if they are the ones on screen.
    pub fn mods_changed(&mut self, instance_id: &str, cx: &mut Context<Self>) {
        if self.instance_id.as_deref() != Some(instance_id) {
            return;
        }
        if !matches!(self.section(ContentKind::Mod).listing, Load::NotLoaded) {
            self.reload(ContentKind::Mod, cx);
        }
        // A shader mod may have been turned on or off.
        if !matches!(
            self.section(ContentKind::ShaderPack).listing,
            Load::NotLoaded
        ) {
            self.reload(ContentKind::ShaderPack, cx);
        }
    }

    /// Reads the section's folder again (and matches new files on Modrinth).
    pub fn reload(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        let Some((id, dirs, target)) = self.context(cx) else {
            return;
        };
        let api = self.modrinth(cx);
        let cf = self.curseforge(cx);
        if matches!(
            self.section(kind).listing,
            Load::NotLoaded | Load::Failed(_)
        ) {
            self.section_mut(kind).listing = Load::Loading;
        }
        let work = cx.background_spawn(async move {
            let shaders = listing::shader_state(&dirs)?;
            let mut target = target;
            target.shader_loader = shaders.loader;
            let listing = listing::list(
                Platforms::new(api.as_deref(), cf.as_deref()),
                &dirs,
                &target,
                kind,
            )
            .await?;
            Ok::<_, launcher_core::content::ContentError>((listing, shaders))
        });
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                match result {
                    Ok((listing, shaders)) => {
                        let loader_changed = this.shaders.loader != shaders.loader;
                        let shader_pack = shaders.selection.pack.clone();
                        this.shaders = shaders;
                        let section = this.section_mut(kind);
                        let first = section.listing.ready().is_none();
                        section.icons = listing
                            .items
                            .iter()
                            .filter_map(|item| {
                                let icon = item.file.meta.icon.as_ref()?;
                                Some((
                                    item.file.file_name.clone(),
                                    crate::images::render_image((**icon).clone()),
                                ))
                            })
                            .collect();
                        let from_platform = listing.items.iter().any(|i| i.identified());
                        section.listing = Load::Ready(listing);
                        // What a change moved is shown where it landed, and the shader
                        // pack in use on its page when the packs are first listed.
                        let follow = section.follow.take().or_else(|| {
                            (first && kind == ContentKind::ShaderPack)
                                .then_some(shader_pack)
                                .flatten()
                        });
                        if from_platform && !section.auto_checked {
                            section.auto_checked = true;
                            this.look_for_updates(kind, true, cx);
                        }
                        if this.shaders.loader.is_none()
                            && (loader_changed || matches!(this.shader_offer, Load::NotLoaded))
                        {
                            this.load_shader_offer(cx);
                        }
                        if let Some(file) = follow {
                            this.follow(kind, file, cx);
                        }
                    }
                    Err(err) => {
                        let section = this.section_mut(kind);
                        section.listing = Load::Failed(err.to_string().into());
                        section.follow = None;
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.section_mut(kind).load_task = Some(task);
        cx.notify();
    }

    /// Asks Modrinth which shader mod fits the instance.
    pub fn load_shader_offer(&mut self, cx: &mut Context<Self>) {
        let Some((id, _, target)) = self.context(cx) else {
            return;
        };
        if target.loader == launcher_core::instances::Loader::None {
            self.shader_offer = Load::Ready(ShaderModOffer::NeedsModLoader);
            return;
        }
        let Some(api) = self.modrinth(cx) else {
            return;
        };
        self.shader_offer = Load::Loading;
        let work =
            cx.background_spawn(async move { shaders::find_shader_mod(&api, &target).await });
        self.shader_offer_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                this.shader_offer = match result {
                    Ok(offer) => Load::Ready(offer),
                    Err(err) => Load::Failed(err.to_string().into()),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    pub fn check_updates(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        self.look_for_updates(kind, false, cx);
    }

    /// Asks Modrinth for newer versions. `quiet` (the check after the first listing)
    /// says nothing when there are none or Modrinth can't be reached.
    fn look_for_updates(&mut self, kind: ContentKind, quiet: bool, cx: &mut Context<Self>) {
        let Some((id, dirs, target)) = self.context(cx) else {
            return;
        };
        let (api, cf) = (self.modrinth(cx), self.curseforge(cx));
        if api.is_none() && cf.is_none() {
            return;
        }
        let prerelease = self.prerelease_updates(cx);
        self.section_mut(kind).checking_updates = true;
        let work = cx.background_spawn(async move {
            let modrinth = match &api {
                Some(api) => listing::check_updates(api, &dirs, &target, kind, prerelease).await?,
                None => HashMap::new(),
            };
            let curseforge = match &cf {
                Some(cf) => {
                    listing::check_curseforge_updates(cf, &dirs, &target, kind, prerelease).await?
                }
                None => HashMap::new(),
            };
            Ok::<_, launcher_core::content::ContentError>((modrinth, curseforge))
        });
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                let section = this.section_mut(kind);
                section.checking_updates = false;
                match result {
                    Ok((updates, cf_updates)) => {
                        let count = updates.len() + cf_updates.len();
                        section.updates = updates;
                        section.cf_updates = cf_updates;
                        if count == 0 && !quiet {
                            let message = match kind {
                                ContentKind::Mod => M::AllModsUpToDate,
                                ContentKind::ResourcePack => M::AllResourcePacksUpToDate,
                                ContentKind::ShaderPack => M::AllShaderPacksUpToDate,
                            };
                            this.set_message(t(message), false, cx);
                        }
                    }
                    Err(_) if quiet => {}
                    Err(err) => {
                        this.set_message(tr!(M::CouldntCheckUpdates, error = err), true, cx)
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.section_mut(kind).updates_task = Some(task);
        cx.notify();
    }

    /// Installs `requests` one after the other in the background, with progress.
    pub fn install(&mut self, requests: Vec<InstallRequest>, cx: &mut Context<Self>) {
        self.install_then(requests, Vec::new(), cx);
    }

    /// Like [`Self::install`], then the CurseForge requests `queued_cf` once they are in.
    fn install_then(
        &mut self,
        requests: Vec<InstallRequest>,
        queued_cf: Vec<CfInstallRequest>,
        cx: &mut Context<Self>,
    ) {
        if requests.is_empty() || !self.can_change(cx) {
            return;
        }
        let (Some((id, dirs, target)), Some(api)) = (self.context(cx), self.modrinth(cx)) else {
            return;
        };
        let downloader = self.instances.read(cx).content_downloader();
        let busy = self.instances.read(cx).mark_files_busy(&id);
        let progress = Arc::new(InstallProgress::default());
        let cancel = CancelToken::new();
        let kinds: Vec<ContentKind> = requests.iter().map(|r| r.kind).collect();
        let work = {
            let progress = progress.clone();
            let cancel = cancel.clone();
            cx.background_spawn(async move {
                let mut total = InstallOutcome::default();
                let mut target = target;
                for request in requests {
                    let kind = request.kind;
                    progress.set_label(match &request.project {
                        Some(project) => tr!(M::CheckingProject, name = project.title),
                        None => t(M::CheckingDependencies).to_string(),
                    });
                    let plan = install::plan(&api, &dirs, &target, request).await?;
                    let outcome = install::execute(
                        &api,
                        &downloader,
                        &dirs,
                        &target,
                        plan,
                        &progress,
                        &cancel,
                    )
                    .await?;
                    total.installed.extend(outcome.installed);
                    total.removed.extend(outcome.removed);
                    total.skipped.extend(outcome.skipped);
                    if kind == ContentKind::Mod {
                        // A shader mod installed first changes what fits next.
                        target.shader_loader = listing::shader_state(&dirs)?.loader;
                    }
                }
                Ok::<_, launcher_core::content::ContentError>(total)
            })
        };
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                let Some((job, shown)) = this.finish_job(&id) else {
                    return;
                };
                let succeeded = result.is_ok();
                // "Update all": the CurseForge updates follow the Modrinth ones.
                let queued = if succeeded { job.queued_cf } else { Vec::new() };
                if !shown {
                    let message = match &result {
                        Ok(outcome) => (outcome_message(outcome).into(), false),
                        Err(err) => (err.to_string().into(), true),
                    };
                    let message = Some(message);
                    this.keep_finished(&id, message, queued);
                    return;
                }
                match result {
                    Ok(outcome) => {
                        let message = outcome_message(&outcome);
                        this.set_message(message, false, cx);
                        for kind in &kinds {
                            this.section_mut(*kind).updates.clear();
                        }
                    }
                    Err(err) => this.set_message(err.to_string(), true, cx),
                }
                if !queued.is_empty() {
                    this.cf_install(queued, cx);
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
            queued_cf,
            _task: task,
            _ticker: ticker,
            _busy: busy,
        });
        cx.notify();
    }

    pub fn cancel_install(&mut self, cx: &mut Context<Self>) {
        if let Some(job) = &self.job {
            job.cancel.cancel();
        }
        cx.notify();
    }

    /// Installs a version, with its dependencies when `with_dependencies`: a result's
    /// Install always, the project page's as its toggle says (the toggle is that page's
    /// alone, and results stay clickable beside it). A shader pack on an instance
    /// without a shader mod installs the offered shader mod first.
    pub fn install_version(
        &mut self,
        kind: ContentKind,
        version: RawVersion,
        project: Option<ModrinthProject>,
        force: bool,
        with_dependencies: bool,
        cx: &mut Context<Self>,
    ) {
        let mut requests = Vec::new();
        if kind == ContentKind::ShaderPack
            && self.shaders.loader.is_none()
            && let Some(ShaderModOffer::Install { version, .. }) = self.shader_offer.ready()
        {
            requests.push(InstallRequest {
                kind: ContentKind::Mod,
                version: (**version).clone(),
                project: None,
                install_dependencies: true,
                force: false,
            });
        }
        requests.push(InstallRequest {
            kind,
            version,
            project,
            install_dependencies: with_dependencies,
            force,
        });
        self.install(requests, cx);
    }

    /// Installs the offered shader mod.
    pub fn install_shader_mod(&mut self, cx: &mut Context<Self>) {
        if let Some(ShaderModOffer::Install { version, .. }) = self.shader_offer.ready() {
            let request = InstallRequest {
                kind: ContentKind::Mod,
                version: (**version).clone(),
                project: None,
                install_dependencies: true,
                force: false,
            };
            self.install(vec![request], cx);
        }
    }

    /// Installs every update the last check found.
    pub fn update_all(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        // Modrinth's first, then CurseForge's: one install runs at a time.
        let cf_requests = self.cf_update_requests(kind);
        if self.section(kind).updates.is_empty() {
            self.cf_install(cf_requests, cx);
            return;
        }
        let mut versions: Vec<RawVersion> = self.section(kind).updates.values().cloned().collect();
        versions.sort_by(|a, b| a.project_id.cmp(&b.project_id));
        let requests = versions
            .into_iter()
            .map(|version| InstallRequest {
                kind,
                version,
                project: None,
                install_dependencies: true,
                force: false,
            })
            .collect();
        self.install_then(requests, cf_requests, cx);
    }

    pub fn update_one(&mut self, kind: ContentKind, file_name: &str, cx: &mut Context<Self>) {
        if !self.section(kind).updates.contains_key(file_name) {
            self.cf_update_one(kind, file_name, cx);
            return;
        }
        if let Some(version) = self.section(kind).updates.get(file_name).cloned() {
            self.install(
                vec![InstallRequest {
                    kind,
                    version,
                    project: None,
                    install_dependencies: true,
                    force: false,
                }],
                cx,
            );
        }
    }

    /// Runs a quick change to the files, then reads the section again. Says whether
    /// the change was made.
    fn change(
        &mut self,
        kind: ContentKind,
        change: impl FnOnce(&InstanceDirs, &Target) -> Result<(), launcher_core::content::ContentError>,
        cx: &mut Context<Self>,
    ) -> bool {
        if !self.can_change(cx) {
            return false;
        }
        let Some((_, dirs, target)) = self.context(cx) else {
            return false;
        };
        let changed = match change(&dirs, &target) {
            Ok(()) => {
                self.message = None;
                true
            }
            Err(err) => {
                self.message = Some((err.to_string().into(), true));
                false
            }
        };
        self.reload(kind, cx);
        changed
    }

    /// A mod's toggle. Turning one off first looks for the enabled mods that can't
    /// load without it, and turning one on for the mods that are off that it needs;
    /// when there are some, the window asks whether to turn them off or on too
    /// ([`ContentEvent::ConfirmToggle`]) instead.
    pub fn set_mod_enabled(&mut self, file_name: String, enabled: bool, cx: &mut Context<Self>) {
        if self.checking_dependents.is_some() || !self.can_change(cx) {
            return;
        }
        let Some((id, dirs, _)) = self.context(cx) else {
            return;
        };
        let name = self
            .section(ContentKind::Mod)
            .items()
            .iter()
            .find(|item| item.file.file_name == file_name)
            .map_or_else(|| file_name.clone(), |item| item.display_name());
        self.checking_dependents = Some(file_name.clone());
        let work = {
            let file_name = file_name.clone();
            cx.background_spawn(async move {
                use launcher_core::content::mod_finder;
                if enabled {
                    mod_finder::requirements(&dirs, &file_name)
                } else {
                    mod_finder::dependents(&dirs, &file_name)
                }
            })
        };
        self._dependents = Some(cx.spawn(async move |this, cx| {
            let others = work.await;
            this.update(cx, |this, cx| {
                this.checking_dependents = None;
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    cx.notify();
                    return;
                }
                if others.is_empty() {
                    this.apply_mods_enabled(&[file_name], enabled, cx);
                } else {
                    cx.emit(ContentEvent::ConfirmToggle(TogglePrompt {
                        instance_id: id,
                        file_name,
                        name,
                        enable: enabled,
                        others,
                    }));
                    cx.notify();
                }
            })
            .ok();
        }));
        cx.notify();
    }

    /// Turns mods of `instance_id` off or on, from the prompt that asked about the
    /// mods linked to one: that mod alone, or with them.
    pub fn set_mods_enabled(
        &mut self,
        instance_id: &str,
        file_names: &[String],
        enabled: bool,
        cx: &mut Context<Self>,
    ) {
        if self.instance_id.as_deref() == Some(instance_id) {
            self.apply_mods_enabled(file_names, enabled, cx);
        }
    }

    /// Turns mods on or off, then reads the folder again.
    fn apply_mods_enabled(&mut self, file_names: &[String], enabled: bool, cx: &mut Context<Self>) {
        if !self.can_change(cx) {
            return;
        }
        let Some((_, dirs, _)) = self.context(cx) else {
            return;
        };
        self.message = None;
        let mut errors = Vec::new();
        for file_name in file_names {
            match files::set_mod_enabled(&dirs, file_name, enabled) {
                Ok(new_name) => {
                    // Show it at once: reading a large folder again takes a moment, and
                    // until then the row would show the old state and the old name.
                    if let Load::Ready(listing) = &mut self.section_mut(ContentKind::Mod).listing
                        && let Some(item) = listing
                            .items
                            .iter_mut()
                            .find(|item| item.file.file_name == *file_name)
                    {
                        item.file.file_name = new_name;
                        item.enabled = enabled;
                    }
                }
                Err(err) => errors.push(err.to_string()),
            }
        }
        if !errors.is_empty() {
            self.message = Some((errors.join(" ").into(), true));
        }
        cx.notify();
        self.reload(ContentKind::Mod, cx);
        // A shader mod may have been turned on or off.
        self.reload(ContentKind::ShaderPack, cx);
    }

    pub fn set_pack_enabled(
        &mut self,
        file_name: String,
        enabled: bool,
        compatibility: Compatibility,
        cx: &mut Context<Self>,
    ) {
        // A pack turned on goes to the top, and one turned off among those not in use:
        // either can land on another page, which is turned to.
        let changed = self.change(
            ContentKind::ResourcePack,
            |dirs, target| {
                files::set_pack_enabled(dirs, target, &file_name, enabled, compatibility)
                    .map(|_| ())
            },
            cx,
        );
        if changed {
            self.section_mut(ContentKind::ResourcePack).follow = Some(file_name);
        }
    }

    /// Raises (`up`) or lowers a resource pack's priority. A pack moved past the top or
    /// bottom of its page is followed onto the page it lands on.
    pub fn move_pack(&mut self, file_name: String, up: bool, cx: &mut Context<Self>) {
        let moved = self.change(
            ContentKind::ResourcePack,
            |dirs, target| {
                let name = file_name.clone();
                let change = if up {
                    PackChange::Raise { name }
                } else {
                    PackChange::Lower { name }
                };
                files::change_pack(dirs, target, &change).map(|_| ())
            },
            cx,
        );
        if moved {
            self.section_mut(ContentKind::ResourcePack).follow = Some(file_name);
        }
    }

    pub fn select_shader(&mut self, file_name: Option<String>, cx: &mut Context<Self>) {
        let Some(loader) = self.shaders.loader else {
            return;
        };
        self.change(
            ContentKind::ShaderPack,
            |dirs, _| files::select_shader(dirs, loader, file_name.as_deref()),
            cx,
        );
    }

    /// The first click asks; the second moves the file to the Trash.
    pub fn delete(&mut self, kind: ContentKind, file_name: String, cx: &mut Context<Self>) {
        if self.confirm_delete.as_ref() != Some(&(kind, file_name.clone())) {
            self.confirm_delete = Some((kind, file_name));
            cx.notify();
            return;
        }
        if self.deleting.is_some() || !self.can_change(cx) {
            return;
        }
        let Some((id, dirs, target)) = self.context(cx) else {
            return;
        };
        // The Trash can take a while (on Windows it walks a folder's every file).
        self.deleting = Some((id.clone(), kind, file_name.clone()));
        let work = cx.background_spawn(async move {
            files::delete(&dirs, &target, kind, &file_name).map_err(|err| err.to_string())
        });
        self._delete = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.deleting = None;
                this.confirm_delete = None;
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    cx.notify();
                    return;
                }
                match result {
                    Ok(()) => this.message = None,
                    Err(err) => this.message = Some((err.into(), true)),
                }
                this.reload(kind, cx);
                if kind == ContentKind::Mod {
                    this.reload(ContentKind::ShaderPack, cx);
                }
            })
            .ok();
        }));
        cx.notify();
    }

    pub fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.confirm_delete = None;
        cx.notify();
    }

    /// Copies files the user picked or dropped into the section's folder.
    pub fn add_files(&mut self, kind: ContentKind, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if paths.is_empty() || !self.can_change(cx) {
            return;
        }
        let Some((id, dirs, target)) = self.context(cx) else {
            return;
        };
        let work = cx.background_spawn(async move {
            paths
                .into_iter()
                .map(|path| {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    (name, files::add(&dirs, &target, kind, &path))
                })
                .collect::<Vec<_>>()
        });
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let results = work.await;
            this.update(cx, |this, cx| {
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                let added: Vec<String> = results
                    .iter()
                    .filter_map(|(_, r)| r.as_ref().ok().cloned())
                    .collect();
                let failed: Vec<String> = results
                    .iter()
                    .filter_map(|(name, r)| {
                        r.as_ref()
                            .err()
                            .map(|err| tr!(M::FileError, name = name, error = err))
                    })
                    .collect();
                let message = match (added.is_empty(), failed.is_empty()) {
                    (false, true) => tr!(M::AddedFiles, files = format::list(&added)),
                    (true, false) => tr!(M::NotAddedFiles, files = failed.join("; ")),
                    (false, false) => tr!(
                        M::AddedAndNotAdded,
                        added = format::list(&added),
                        failed = failed.join("; ")
                    ),
                    (true, true) => String::new(),
                };
                this.set_message(message, !failed.is_empty(), cx);
                this.reload(kind, cx);
                if kind == ContentKind::Mod {
                    this.reload(ContentKind::ShaderPack, cx);
                }
            })
            .ok();
        }));
    }

    /// "Add file…" or "Add folder…": the system's file dialog.
    pub fn pick_files(&mut self, kind: ContentKind, pick: Pick, cx: &mut Context<Self>) {
        if !self.can_change(cx) {
            return;
        }
        let picked = cx.prompt_for_paths(pick.options(
            true,
            t(match kind {
                ContentKind::Mod => M::AddToMods,
                ContentKind::ResourcePack => M::AddToResourcePacks,
                ContentKind::ShaderPack => M::AddToShaders,
            }),
        ));
        self._prompt = Some(cx.spawn(async move |this, cx| {
            let Some(paths) = file_dialog::chosen(picked, &this, cx).await else {
                return;
            };
            this.update(cx, |this, cx| this.add_files(kind, paths, cx))
                .ok();
        }));
    }

    pub fn open_folder(&self, kind: ContentKind, cx: &mut Context<Self>) {
        if let Some((_, dirs, _)) = self.context(cx) {
            let dir = dirs.content_dir(kind);
            let _ = std::fs::create_dir_all(&dir);
            cx.open_with_system(&dir);
        }
    }

    /// "Copy with Fabric…": a vanilla instance keeps its loader, so a copy gets one.
    pub fn copy_with_loader(&mut self, kind: LoaderKind, cx: &mut Context<Self>) {
        if let Some(id) = self.instance_id.clone() {
            cx.emit(ContentEvent::CopyWithLoader(id, kind));
        }
    }

    /// Finds which mod loader a copy of a vanilla instance can have, once.
    pub fn ensure_mod_loader_offer(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.mod_loader_offer, Load::NotLoaded) {
            return;
        }
        let Some((id, _, target)) = self.context(cx) else {
            return;
        };
        self.mod_loader_offer = Load::Loading;
        let work = self.instances.update(cx, |instances, cx| {
            instances.mod_loader_for(&target.game_version, cx)
        });
        self.mod_loader_offer_task = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                this.mod_loader_offer = match result {
                    Ok(pick) => Load::Ready(pick.map(|(kind, _)| kind)),
                    Err(err) => Load::Failed(err.to_string().into()),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    pub fn new_fabric_instance(&mut self, cx: &mut Context<Self>) {
        cx.emit(ContentEvent::NewFabricInstance);
    }

    // ---- Browse ----

    fn search_changed(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        let text = self.section(kind).browse.search.read(cx).text().to_string();
        if text == self.section(kind).browse.text {
            return;
        }
        self.section_mut(kind).browse.text = text.clone();
        self.section_mut(kind).browse.offset = 0;
        // What is installed is filtered again, from its first page.
        let section = self.section_mut(kind);
        section.installed_page = 0;
        section.follow = None;
        section.reveal = None;
        // Typing leaves a project's page for the list it filters (unless the page is
        // beside the list).
        if self.panel_width.is_none() {
            self.section_mut(kind).browse.project = None;
            self.section_mut(kind).cf.project = None;
        }
        self.section_mut(kind).cf.offset = 0;
        if text.trim().is_empty() {
            // Back to the whole installed list, with the platforms browsed below it
            // (searched again on the next frame).
            let section = self.section_mut(kind);
            section.browse.task = None;
            section.browse.results = Load::NotLoaded;
            section.cf.results = Load::NotLoaded;
            cx.notify();
            return;
        }
        let task = cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SEARCH_DELAY).await;
            this.update(cx, |this, cx| {
                if text.contains("modrinth.com/") {
                    this.open_link(kind, text.trim(), cx);
                } else {
                    this.search(kind, cx);
                    this.cf_search(kind, cx);
                }
            })
            .ok();
        });
        self.section_mut(kind).browse.task = Some(task);
        cx.notify();
    }

    pub fn set_only_compatible(&mut self, kind: ContentKind, value: bool, cx: &mut Context<Self>) {
        let browse = &mut self.section_mut(kind).browse;
        browse.only_compatible = value;
        browse.offset = 0;
        self.search(kind, cx);
        // The filter is shared with the CurseForge results.
        if !matches!(self.section(kind).cf.results, Load::NotLoaded) {
            self.section_mut(kind).cf.offset = 0;
            self.cf_search(kind, cx);
        }
    }

    pub fn set_index(
        &mut self,
        kind: ContentKind,
        index: ModrinthSearchIndex,
        cx: &mut Context<Self>,
    ) {
        let browse = &mut self.section_mut(kind).browse;
        browse.index = index;
        browse.offset = 0;
        self.search(kind, cx);
    }

    pub fn toggle_filters_open(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        let section = self.section_mut(kind);
        section.browse.filters_open = !section.browse.filters_open;
        // The panel opens at the top of the list: brought into sight from further down.
        if section.browse.filters_open {
            section.rows.scroll_to(gpui::ListOffset::default());
        }
        cx.notify();
    }

    /// Shows or hides the results the instance has already.
    pub fn set_show_installed(&mut self, kind: ContentKind, value: bool, cx: &mut Context<Self>) {
        self.section_mut(kind).browse.show_installed = value;
        cx.notify();
    }

    /// Changes the Modrinth filters of `kind` and searches again from the first page.
    pub fn change_filters(
        &mut self,
        kind: ContentKind,
        change: impl FnOnce(&mut filters::Filters),
        cx: &mut Context<Self>,
    ) {
        let browse = &mut self.section_mut(kind).browse;
        let before = browse.filters.clone();
        change(&mut browse.filters);
        if browse.filters == before {
            return;
        }
        browse.offset = 0;
        self.search(kind, cx);
    }

    pub fn set_page(&mut self, kind: ContentKind, offset: u32, cx: &mut Context<Self>) {
        self.section_mut(kind).browse.offset = offset;
        self.search(kind, cx);
    }

    /// Searches Modrinth with the section's text, filters and page.
    pub fn search(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        let (Some((id, _, target)), Some(api)) = (self.context(cx), self.modrinth(cx)) else {
            return;
        };
        let browse = &self.section(kind).browse;
        let text = browse.search.read(cx).text().trim().to_string();
        let (game_version, loaders) = if browse.only_compatible {
            (
                Some(target.game_version.clone()),
                target.search_loaders(kind),
            )
        } else {
            (None, Vec::new())
        };
        let filters = &browse.filters;
        let params = match ClientSearchParams::new(
            Some(text),
            kind.project_type(),
            game_version,
            loaders,
            Some(browse.index),
            Some(browse.offset),
            Some(PAGE_SIZE),
        )
        .and_then(|params| {
            params.with_filters(
                filters.category_facets(kind),
                filters.environment_for(kind),
                filters.open_source,
            )
        }) {
            Ok(params) => params,
            Err(err) => {
                self.section_mut(kind).browse.results = Load::Failed(err.to_string().into());
                cx.notify();
                return;
            }
        };
        self.section_mut(kind).browse.results = Load::Loading;
        let work = cx.background_spawn(async move { api.search(&params).await });
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                if this.instance_id.as_deref() != Some(id.as_str()) {
                    return;
                }
                this.section_mut(kind).browse.results = match result {
                    Ok(results) => Load::Ready(results),
                    Err(err) => Load::Failed(err.to_string().into()),
                };
                cx.notify();
            })
            .ok();
        });
        self.section_mut(kind).browse.task = Some(task);
        cx.notify();
    }

    /// Opens what a pasted modrinth.com link points to, in the section of its kind.
    fn open_link(&mut self, kind: ContentKind, link: &str, cx: &mut Context<Self>) {
        let Some(api) = self.modrinth(cx) else {
            return;
        };
        let link = link.to_string();
        self.section_mut(kind).browse.results = Load::Loading;
        let work = cx.background_spawn(async move { api.resolve_link(&link).await });
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| match result {
                Ok(target) => {
                    this.section_mut(kind).browse.results = Load::NotLoaded;
                    // Modrinth calls data packs mods; only their loader tells them apart.
                    let loaders = &target.project.loaders;
                    let data_pack = target.project.project_type == "datapack"
                        || (!loaders.is_empty()
                            && loaders
                                .iter()
                                .all(|l| l == launcher_core::datapacks::MODRINTH_LOADER));
                    match ContentKind::from_project_type(&target.project.project_type) {
                        _ if data_pack => {
                            this.section_mut(kind).browse.results =
                                Load::Failed(t(M::LinkIsDataPack).into());
                        }
                        Some(project_kind) => {
                            this.kind = project_kind;
                            let version = target.version.map(|v| v.id);
                            this.open_project(project_kind, target.project.id.clone(), version, cx);
                        }
                        None => {
                            this.section_mut(kind).browse.results = Load::Failed(
                                tr!(M::LinkNotInstallable, kind = target.project.project_type)
                                    .into(),
                            );
                        }
                    }
                    cx.notify();
                }
                Err(err) => {
                    this.section_mut(kind).browse.results = Load::Failed(err.to_string().into());
                    cx.notify();
                }
            })
            .ok();
        });
        self.section_mut(kind).browse.task = Some(task);
        cx.notify();
    }

    /// Opens a project with its versions.
    pub fn open_project(
        &mut self,
        kind: ContentKind,
        project_id: String,
        linked_version: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.modrinth(cx) else {
            return;
        };
        let id = project_id.clone();
        let opened = project_id.clone();
        let work = cx.background_spawn(async move {
            let project = api.project(&id).await;
            let versions = api.versions(&id, &[], &[]).await;
            (project, versions)
        });
        let task = cx.spawn(async move |this, cx| {
            let (project, versions) = work.await;
            this.update(cx, |this, cx| {
                if let Some(view) = &mut this.section_mut(kind).browse.project
                    && view.project_id == opened
                {
                    view.project = match project {
                        Ok(project) => Load::Ready(project),
                        Err(err) => Load::Failed(err.to_string().into()),
                    };
                    view.versions = match versions {
                        Ok(versions) => Load::Ready(versions),
                        Err(err) => Load::Failed(err.to_string().into()),
                    };
                }
                cx.notify();
            })
            .ok();
        });
        self.panel_closing = None;
        self.page_scroll.set_offset(gpui::Point::default());
        // A link to a version opens on the versions.
        let tab = if linked_version.is_some() {
            ProjectTab::Versions
        } else {
            ProjectTab::default()
        };
        self.section_mut(kind).cf.project = None;
        self.section_mut(kind).browse.project = Some(ProjectView {
            project_id,
            tab,
            project: Load::Loading,
            versions: Load::Loading,
            linked_version,
            show_all: false,
            with_dependencies: true,
            confirm_force: None,
            _task: Some(task),
        });
        if kind == ContentKind::ShaderPack
            && self.shaders.loader.is_none()
            && matches!(self.shader_offer, Load::NotLoaded)
        {
            self.load_shader_offer(cx);
        }
        cx.notify();
    }

    pub fn close_project(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        // The panel beside the list slides shut first (see `finish_closing`).
        if self.panel_width.is_some() && self.panel_closing.is_none() {
            self.panel_closing = Some(kind);
            cx.notify();
            return;
        }
        self.section_mut(kind).browse.project = None;
        if matches!(self.section(kind).browse.results, Load::NotLoaded) {
            self.search(kind, cx);
        }
        cx.notify();
    }

    /// Lets go of the project whose panel has slid shut.
    pub(super) fn finish_closing(&mut self, kind: ContentKind, cx: &mut Context<Self>) {
        self.panel_closing = None;
        let section = self.section_mut(kind);
        section.browse.project = None;
        section.cf.project = None;
        if matches!(self.section(kind).browse.results, Load::NotLoaded) {
            self.search(kind, cx);
        }
        if matches!(self.section(kind).cf.results, Load::NotLoaded) {
            self.cf_search(kind, cx);
        }
        cx.notify();
    }

    pub fn project_mut(&mut self, kind: ContentKind) -> Option<&mut ProjectView> {
        self.section_mut(kind).browse.project.as_mut()
    }

    /// The icon at a Modrinth CDN URL, fetched the first time it is asked for.
    pub fn remote_icon(&mut self, url: &str, cx: &mut Context<Self>) -> Option<Arc<RenderImage>> {
        match self.remote_icons.get(url) {
            Some(RemoteIcon::Ready(image)) => return Some(image.clone()),
            Some(_) => return None,
            None => {}
        }
        // CurseForge's logos come from its media host; everything else from Modrinth's.
        let from_curseforge = url.starts_with(&format!(
            "https://{}/",
            launcher_core::content::curseforge::MEDIA_HOST
        ));
        let (api, cf) = if from_curseforge {
            (None, Some(self.curseforge(cx)?))
        } else {
            (Some(self.modrinth(cx)?), None)
        };
        self.remote_icons
            .insert(url.to_string(), RemoteIcon::Loading);
        let key = url.to_string();
        let work = cx.background_spawn(async move {
            let bytes = match (api, cf) {
                (Some(api), _) => api.image(&key).await.ok()?,
                (_, Some(cf)) => cf.image(&key).await.ok()?,
                _ => return None,
            };
            let icon = launcher_core::content::scan::decode_icon(&bytes)?;
            Some((key, icon))
        });
        let url = url.to_string();
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                let icon = match result {
                    Some((_, icon)) => {
                        RemoteIcon::Ready(crate::images::render_image((*icon).clone()))
                    }
                    None => RemoteIcon::Failed,
                };
                this.remote_icons.insert(url, icon);
                cx.notify();
            })
            .ok();
        })
        .detach();
        None
    }

    /// A project's description, parsed once.
    pub fn description(
        &mut self,
        project: &ModrinthProject,
    ) -> Arc<Vec<launcher_core::markdown::Block>> {
        self.parsed_description(&project.id, &project.body)
    }

    /// A description (Markdown or HTML) parsed once, by a key naming its project.
    pub fn parsed_description(
        &mut self,
        key: &str,
        body: &str,
    ) -> Arc<Vec<launcher_core::markdown::Block>> {
        self.descriptions
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(launcher_core::markdown::parse(body)))
            .clone()
    }

    /// Opens an installed file's page beside the list: its Modrinth project, else its
    /// CurseForge one, else (with neither platform reachable) its page in the browser.
    pub fn open_item_page(&mut self, kind: ContentKind, file_name: &str, cx: &mut Context<Self>) {
        let Some(item) = self
            .section(kind)
            .items()
            .iter()
            .find(|i| i.file.file_name == file_name)
            .cloned()
        else {
            return;
        };
        if let Some(source) = item.modrinth.as_ref().filter(|m| !m.project_id.is_empty())
            && self.modrinth(cx).is_some()
        {
            self.open_project(kind, source.project_id.clone(), None, cx);
        } else if let Some(source) = &item.curseforge
            && self.curseforge(cx).is_some()
            && self.context(cx).is_some()
        {
            self.open_cf_project(kind, source.project_id, cx);
        } else if let Some(page) = item
            .modrinth
            .as_ref()
            .filter(|m| !m.slug.is_empty())
            .map(|m| view::modrinth_page(view::page_type(kind), &m.slug))
            .or_else(|| {
                item.curseforge
                    .as_ref()
                    .map(|c| c.page_url.clone())
                    .filter(|p| !p.is_empty())
            })
        {
            cx.open_url(&page);
        }
    }

    /// Whether the open project is the one `file_name` came from, to mark its row.
    pub fn is_open_page(&self, kind: ContentKind, item: &Item) -> bool {
        if self.panel_closing == Some(kind) {
            return false;
        }
        let section = self.section(kind);
        let modrinth = section.browse.project.as_ref().is_some_and(|p| {
            item.modrinth
                .as_ref()
                .is_some_and(|m| m.project_id == p.project_id)
        });
        let curseforge = section.cf.project.as_ref().is_some_and(|p| {
            item.curseforge
                .as_ref()
                .is_some_and(|c| c.project_id == p.project_id)
        });
        modrinth || curseforge
    }

    /// The loaded images among `urls` (from Modrinth's CDN), fetching the others the
    /// first time they are asked for.
    pub fn description_images(
        &mut self,
        urls: &[String],
        cx: &mut Context<Self>,
    ) -> HashMap<String, Arc<RenderImage>> {
        const MAX_IMAGES: usize = 40;
        let mut ready = HashMap::new();
        for url in urls.iter().take(MAX_IMAGES) {
            match self.description_images.get(url) {
                Some(RemoteIcon::Ready(image)) => {
                    ready.insert(url.clone(), image.clone());
                    continue;
                }
                Some(_) => continue,
                None => {}
            }
            let Some(api) = self.modrinth(cx) else {
                continue;
            };
            self.description_images
                .insert(url.clone(), RemoteIcon::Loading);
            let key = url.clone();
            let work = cx.background_spawn(async move {
                let bytes = api.image(&key).await.ok()?;
                crate::images::decode_large(&bytes)
            });
            let url = url.clone();
            cx.spawn(async move |this, cx| {
                let result = work.await;
                this.update(cx, |this, cx| {
                    let image = match result {
                        Some(image) => RemoteIcon::Ready(crate::images::render_image(image)),
                        None => RemoteIcon::Failed,
                    };
                    this.description_images.insert(url, image);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
        ready
    }

    pub fn set_project_tab(&mut self, kind: ContentKind, tab: ProjectTab, cx: &mut Context<Self>) {
        if let Some(view) = self.project_mut(kind) {
            view.tab = tab;
        }
        cx.notify();
    }

    // ---- previews ----

    /// Shows a description image without fetching it, for the offscreen screenshots.
    pub fn preview_description_image(&mut self, url: &str, image: Arc<RenderImage>) {
        self.description_images
            .insert(url.to_string(), RemoteIcon::Ready(image));
    }

    /// Shows prepared Browse results, for the offscreen screenshots.
    pub fn preview_results(
        &mut self,
        kind: ContentKind,
        text: &str,
        results: ModrinthSearchResponse,
        cx: &mut Context<Self>,
    ) {
        self.kind = kind;
        let section = self.section_mut(kind);
        section.browse.text = text.to_string();
        section.browse.results = Load::Ready(results);
        let search = section.browse.search.clone();
        search.update(cx, |input, cx| input.set_text(text.to_string(), cx));
        cx.notify();
    }

    /// Shows a prepared project, for the offscreen screenshots.
    pub fn preview_project(
        &mut self,
        kind: ContentKind,
        project: ModrinthProject,
        versions: Vec<RawVersion>,
        cx: &mut Context<Self>,
    ) {
        self.kind = kind;
        let section = self.section_mut(kind);
        section.browse.project = Some(ProjectView {
            project_id: project.id.clone(),
            tab: ProjectTab::default(),
            project: Load::Ready(project),
            versions: Load::Ready(versions),
            linked_version: None,
            show_all: false,
            with_dependencies: true,
            confirm_force: None,
            _task: None,
        });
        cx.notify();
    }

    /// Filters what is installed by `query`, on page `page`, for the offscreen
    /// screenshots; the platforms aren't searched.
    pub fn preview_installed(
        &mut self,
        kind: ContentKind,
        query: &str,
        page: usize,
        cx: &mut Context<Self>,
    ) {
        self.kind = kind;
        let section = self.section_mut(kind);
        section.browse.text = query.to_string();
        let search = section.browse.search.clone();
        search.update(cx, |input, cx| input.set_text(query.to_string(), cx));
        self.section_mut(kind).installed_page = page;
        cx.notify();
    }

    /// Shows a prepared shader mod offer and updates, for the offscreen screenshots.
    pub fn preview_state(
        &mut self,
        kind: ContentKind,
        offer: Option<ShaderModOffer>,
        updates: HashMap<String, RawVersion>,
        icons: HashMap<String, Arc<RenderImage>>,
        cx: &mut Context<Self>,
    ) {
        self.kind = kind;
        if let Some(offer) = offer {
            self.shader_offer = Load::Ready(offer);
        }
        self.section_mut(kind).updates = updates;
        for (url, icon) in icons {
            self.remote_icons.insert(url, RemoteIcon::Ready(icon));
        }
        cx.notify();
    }
}

/// "Installed Iris and Sodium. Replaced …", for the message line.
pub(crate) fn outcome_message(outcome: &InstallOutcome) -> String {
    let titles: Vec<&str> = outcome
        .installed
        .iter()
        .map(|(title, _)| title.as_str())
        .collect();
    let mut message = match titles.as_slice() {
        [] => t(M::NothingInstalled).to_string(),
        [one] => tr!(M::InstalledOne, name = one),
        [rest @ .., last] => tr!(M::InstalledMany, names = format::list(rest), last = last),
    };
    let gap = match crate::i18n::locale() {
        crate::i18n::Locale::ZhHans | crate::i18n::Locale::ZhHant => "",
        crate::i18n::Locale::En | crate::i18n::Locale::Fr => " ",
    };
    if !outcome.removed.is_empty() {
        message.push_str(gap);
        message.push_str(&tr!(M::Replaced, files = format::list(&outcome.removed)));
    }
    for skipped in &outcome.skipped {
        message.push_str(gap);
        message.push_str(&tr!(
            M::Skipped,
            name = skipped.project,
            reason = skipped.reason.trim_end_matches('.')
        ));
    }
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::content::install::Skipped;

    /// Where the dialog can't show files and folders together (Linux, Windows), a
    /// pack's `.zip` gets "Add file…" and an unzipped pack "Add folder…"; on macOS
    /// "Add file…" takes either, as before. Mods are always files.
    #[test]
    fn add_rows_offer_files_wherever_the_dialog_cant_show_both() {
        let rows = |kind: ContentKind, mixed: bool| -> Vec<_> {
            add_rows(kind, mixed)
                .into_iter()
                .map(|(id, label, pick)| {
                    let options = pick.options(true, "Add");
                    (id, label, options.files, options.directories)
                })
                .collect()
        };
        for mixed in [false, true] {
            assert_eq!(
                rows(ContentKind::Mod, mixed),
                [("menu-add-file", M::AddFile, true, false)]
            );
        }
        for kind in [ContentKind::ResourcePack, ContentKind::ShaderPack] {
            assert_eq!(
                rows(kind, false),
                [
                    ("menu-add-file", M::AddFile, true, false),
                    ("menu-add-folder", M::AddFolder, false, true),
                ]
            );
            assert_eq!(
                rows(kind, true),
                [("menu-add-file", M::AddFile, true, true)]
            );
        }
    }

    #[test]
    fn outcome_messages() {
        let outcome = InstallOutcome {
            installed: vec![
                ("Iris".into(), "iris.jar".into()),
                ("Sodium".into(), "sodium.jar".into()),
                ("Fabric API".into(), "fabric-api.jar".into()),
            ],
            removed: vec!["sodium-old.jar".into()],
            skipped: vec![Skipped {
                project: "x".into(),
                reason: "Not found on Modrinth".into(),
            }],
        };
        assert_eq!(
            outcome_message(&outcome),
            "Installed Iris, Sodium and Fabric API. Replaced sodium-old.jar. Skipped x: Not found on Modrinth."
        );
        assert_eq!(
            outcome_message(&InstallOutcome::default()),
            "Nothing was installed."
        );
    }
}

/// The card of files to download by hand across a change of the selected instance,
/// with the sample instances of the offscreen screenshots.
#[cfg(all(test, feature = "screenshot"))]
mod blocked_tests {
    use gpui::TestAppContext;

    use super::curseforge::tests::planned;
    use super::*;

    fn waiting(view: &Entity<ContentView>, cx: &mut TestAppContext) -> Option<Vec<u64>> {
        view.read_with(cx, |view, _| {
            let files = view.blocked.as_ref()?.unfinished();
            Some(files.iter().map(|f| f.file.id).collect())
        })
    }

    #[test]
    fn the_card_comes_back_with_its_instance() {
        let mut cx = TestAppContext::single();
        let instances = cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            crate::preview::instances("worlds", &accounts, cx)
        });
        let (a, b) = instances.read_with(&cx, |instances, _| {
            (
                instances.list()[0].id.clone(),
                instances.list()[1].id.clone(),
            )
        });
        instances.update(&mut cx, |instances, cx| instances.select(&a, cx));
        let view = cx.new(|cx| ContentView::new(instances.clone(), cx));
        cx.run_until_parked();

        // an imported pack left two files to download from curseforge.com
        instances.update(&mut cx, |instances, cx| {
            instances
                .pending_downloads
                .insert(a.clone(), vec![planned(1), planned(2)]);
            cx.notify();
        });
        cx.run_until_parked();
        assert_eq!(waiting(&view, &mut cx), Some(vec![1, 2]));
        view.update(&mut cx, |view, _| {
            view.blocked.as_mut().expect("the card").added[0] = true
        });

        // LS-25: another instance is shown, and then this one again
        let select = |id: &str, cx: &mut TestAppContext| {
            instances.update(cx, |instances, cx| instances.select(id, cx));
            cx.run_until_parked();
        };
        select(&b, &mut cx);
        assert_eq!(waiting(&view, &mut cx), None);
        select(&a, &mut cx);
        assert_eq!(
            waiting(&view, &mut cx),
            Some(vec![2]),
            "the file not added yet"
        );
    }
}

/// How an install went that finished while another instance was shown, with the
/// sample instances of the offscreen screenshots (kept off the network).
#[cfg(all(test, feature = "screenshot"))]
mod finished_tests {
    use gpui::TestAppContext;

    use super::curseforge::tests::planned;
    use super::*;
    use crate::instances::RunState;

    struct Setup {
        cx: TestAppContext,
        instances: Entity<Instances>,
        view: Entity<ContentView>,
        a: String,
        b: String,
    }

    fn setup() -> Setup {
        let mut cx = TestAppContext::single();
        let instances = cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            crate::preview::instances("worlds", &accounts, cx)
        });
        let (a, b) = instances.update(&mut cx, |instances, cx| {
            instances.preview_offline();
            let (a, b) = (
                instances.list()[0].id.clone(),
                instances.list()[1].id.clone(),
            );
            instances.select(&b, cx);
            (a, b)
        });
        let view = cx.new(|cx| ContentView::new(instances.clone(), cx));
        cx.run_until_parked();
        Setup {
            cx,
            instances,
            view,
            a,
            b,
        }
    }

    /// "Update all" of `id` finished its Modrinth half while it wasn't shown.
    fn finished_update_all(s: &mut Setup, id: &str) {
        let request = CfInstallRequest {
            kind: ContentKind::Mod,
            file: planned(1).file,
            project: None,
            install_dependencies: true,
            force: false,
        };
        s.view.update(&mut s.cx, |view, _| {
            view.keep_finished(id, Some(("Updated".into(), false)), vec![request])
        });
    }

    fn message(s: &mut Setup) -> Option<(SharedString, bool)> {
        s.view.read_with(&s.cx, |view, _| view.message.clone())
    }

    #[test]
    fn a_refused_curseforge_half_waits_for_the_game_to_stop() {
        let mut s = setup();
        let a = s.a.clone();
        finished_update_all(&mut s, &a);
        // the game of the instance was started meanwhile
        s.instances.update(&mut s.cx, |instances, cx| {
            instances.runs.entry(a.clone()).or_default().state = RunState::Preparing {
                progress: Arc::default(),
                cancel: CancelToken::new(),
            };
            instances.select(&a, cx);
        });
        s.cx.run_until_parked();
        assert_eq!(message(&mut s), Some((t(M::StopGameFirst).into(), true)));
        let kept = s
            .view
            .read_with(&s.cx, |view, _| view.finished.contains_key(&a));
        assert!(kept, "the CurseForge updates wait");

        s.instances.update(&mut s.cx, |instances, cx| {
            instances.runs.entry(a.clone()).or_default().state = RunState::Idle;
            cx.notify();
        });
        s.cx.run_until_parked();
        // offline, so the CurseForge half can't start: how the rest went is said
        assert_eq!(message(&mut s), Some(("Updated".into(), false)));
    }

    #[test]
    fn how_it_went_goes_with_its_instance() {
        let mut s = setup();
        let a = s.a.clone();
        let b = s.b.clone();
        s.instances
            .update(&mut s.cx, |instances, cx| instances.select(&a, cx));
        s.cx.run_until_parked();
        finished_update_all(&mut s, &b);
        // the instance is deleted, and a new one may get its id
        s.instances.update(&mut s.cx, |instances, cx| {
            instances.list.retain(|instance| instance.id != b);
            cx.notify();
        });
        s.cx.run_until_parked();
        let kept = s
            .view
            .read_with(&s.cx, |view, _| view.finished.contains_key(&b));
        assert!(!kept);
    }
}

/// The Installed list's page as files come and go, the search changes and another
/// kind or instance is shown, with the sample instances of the offscreen screenshots
/// (kept off the network).
#[cfg(all(test, feature = "screenshot"))]
mod page_tests {
    use gpui::TestAppContext;
    use launcher_core::content::scan::ScannedFile;

    use super::view::Entry;
    use super::*;

    struct Setup {
        cx: TestAppContext,
        instances: Entity<Instances>,
        view: Entity<ContentView>,
        other: String,
    }

    fn setup() -> Setup {
        let mut cx = TestAppContext::single();
        let instances = cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            crate::preview::instances("worlds", &accounts, cx)
        });
        let other = instances.update(&mut cx, |instances, cx| {
            instances.preview_offline();
            let id = instances.list()[0].id.clone();
            instances.select(&id, cx);
            instances.list()[1].id.clone()
        });
        let view = cx.new(|cx| ContentView::new(instances.clone(), cx));
        cx.run_until_parked();
        Setup {
            cx,
            instances,
            view,
            other,
        }
    }

    /// The resource packs of the `content-packs-paged` screenshot, read from its files:
    /// 23 in use and 9 not, two pages.
    fn packs() -> Setup {
        filled("content-packs-paged", ContentKind::ResourcePack)
    }

    /// The files of `kind` the screenshot `scene` has, read from them.
    fn filled(scene: &str, kind: ContentKind) -> Setup {
        let mut cx = TestAppContext::single();
        let instances = cx.update(|cx| {
            let accounts = crate::preview::accounts("signed-out", cx);
            crate::preview::instances(scene, &accounts, cx)
        });
        let other = instances.update(&mut cx, |instances, cx| {
            instances.preview_offline();
            let id = instances
                .list()
                .iter()
                .find(|instance| instance.name == "Survival 1.20.1")
                .expect("the filled instance")
                .id
                .clone();
            instances.select(&id, cx);
            id
        });
        let view = cx.new(|cx| ContentView::new(instances.clone(), cx));
        view.update(&mut cx, |view, cx| {
            view.select_kind(kind, cx);
            view.ensure_loaded(kind, cx);
        });
        cx.run_until_parked();
        Setup {
            cx,
            instances,
            view,
            other,
        }
    }

    /// The packs' list as the next frame shows it, on `page` when it is given: the
    /// page, the files of its rows, and whether a row is still to be scrolled to.
    fn show_packs(s: &mut Setup, page: Option<usize>) -> (usize, Vec<String>, bool) {
        show_kind(s, ContentKind::ResourcePack, page)
    }

    fn show_kind(
        s: &mut Setup,
        kind: ContentKind,
        page: Option<usize>,
    ) -> (usize, Vec<String>, bool) {
        s.cx.run_until_parked();
        s.view.update(&mut s.cx, |view, cx| {
            if let Some(page) = page {
                view.set_installed_page(kind, page, cx);
            }
            view.update_installed_list(kind, cx);
            let section = view.section(kind);
            let rows = section
                .plan
                .iter()
                .filter_map(|entry| match entry {
                    Entry::Row { file, .. } => Some(file.clone()),
                    _ => None,
                })
                .collect();
            (
                section.installed_page,
                rows,
                section.follow.is_some() || section.reveal.is_some(),
            )
        })
    }

    /// The pack `file`'s place in the game's options.
    fn pack(s: &mut Setup, file: &str) -> (bool, Compatibility) {
        s.view.read_with(&s.cx, |view, _| {
            let item = view
                .section(ContentKind::ResourcePack)
                .items()
                .iter()
                .find(|item| item.file.file_name == file)
                .expect("a listed pack");
            (item.priority.is_some(), item.compatibility)
        })
    }

    /// `count` mods, "Extra mod 0" on.
    fn mods(count: usize) -> Listing {
        let items = (0..count)
            .map(|i| Item {
                file: ScannedFile {
                    kind: ContentKind::Mod,
                    file_name: format!("extra-mod-{i:02}.jar"),
                    is_dir: false,
                    size: 1,
                    modified: None,
                    meta: Default::default(),
                },
                sha512: None,
                modrinth: None,
                curseforge: None,
                enabled: true,
                priority: None,
                compatibility: Compatibility::Unknown,
            })
            .collect();
        Listing {
            kind: ContentKind::Mod,
            items,
            lookup_error: None,
            options_error: None,
            shaders: ShaderState::default(),
        }
    }

    /// The mods listed as `listing`, on `page` when it is given, as a frame shows
    /// them: the page shown and the listing indexes of its rows.
    fn show(s: &mut Setup, listing: Option<Listing>, page: Option<usize>) -> (usize, Vec<usize>) {
        s.view.update(&mut s.cx, |view, cx| {
            let section = view.section_mut(ContentKind::Mod);
            if let Some(listing) = listing {
                section.listing = Load::Ready(listing);
            }
            if let Some(page) = page {
                section.installed_page = page;
            }
            view.update_installed_list(ContentKind::Mod, cx);
            let section = view.section(ContentKind::Mod);
            let rows = section
                .plan
                .iter()
                .filter_map(|entry| match entry {
                    Entry::Row { index, .. } => Some(*index),
                    _ => None,
                })
                .collect();
            (section.installed_page, rows)
        })
    }

    fn page(s: &mut Setup) -> usize {
        s.view.read_with(&s.cx, |view, _| {
            view.section(ContentKind::Mod).installed_page
        })
    }

    #[test]
    fn the_page_is_brought_back_when_files_are_removed() {
        let mut s = setup();
        let (page, rows) = show(&mut s, Some(mods(45)), Some(2));
        assert_eq!((page, rows), (2, (40..45).collect()));
        // ten files removed: two pages left, the last one shown
        let (page, rows) = show(&mut s, Some(mods(35)), None);
        assert_eq!((page, rows), (1, (20..35).collect()));
        // and it stays there when files are added again
        assert_eq!(show(&mut s, Some(mods(45)), None).0, 1);
        // twenty or fewer: one page, without a pager
        let (page, rows) = show(&mut s, Some(mods(20)), None);
        assert_eq!((page, rows), (0, (0..20).collect()));
        let pager = s.view.read_with(&s.cx, |view, _| {
            view.section(ContentKind::Mod)
                .plan
                .iter()
                .any(|entry| matches!(entry, Entry::InstalledPager { .. }))
        });
        assert!(!pager);
    }

    #[test]
    fn a_toggled_mod_stays_on_its_page() {
        let mut s = setup();
        show(&mut s, Some(mods(45)), Some(1));
        // turned off: renamed, and listed in the same place
        let mut listing = mods(45);
        listing.items[25].file.file_name = "extra-mod-25.jar.disabled".into();
        listing.items[25].enabled = false;
        let (page, rows) = show(&mut s, Some(listing), None);
        assert_eq!((page, rows), (1, (20..40).collect()));
    }

    #[test]
    fn a_search_or_another_instance_starts_from_the_first_page() {
        let mut s = setup();
        show(&mut s, Some(mods(45)), Some(2));
        // another kind shown and back: each kind keeps its page, as Modrinth's do
        s.view.update(&mut s.cx, |view, cx| {
            view.select_kind(ContentKind::ResourcePack, cx);
            view.select_kind(ContentKind::Mod, cx);
        });
        assert_eq!(page(&mut s), 2);

        let search = s.view.read_with(&s.cx, |view, _| {
            view.section(ContentKind::Mod).browse.search.clone()
        });
        search.update(&mut s.cx, |input, cx| input.set_text("extra-mod-1", cx));
        s.cx.run_until_parked();
        assert_eq!(page(&mut s), 0);
        // "extra-mod-10" to "extra-mod-19": one page
        assert_eq!(show(&mut s, None, None), (0, (10..20).collect()));

        search.update(&mut s.cx, |input, cx| input.set_text("", cx));
        s.cx.run_until_parked();
        show(&mut s, None, Some(2));
        let other = s.other.clone();
        s.instances
            .update(&mut s.cx, |instances, cx| instances.select(&other, cx));
        s.cx.run_until_parked();
        assert_eq!(page(&mut s), 0);
    }

    #[test]
    fn a_pack_turned_on_or_off_is_followed_onto_its_page() {
        let mut s = packs();
        let (page, rows, _) = show_packs(&mut s, Some(1));
        assert_eq!((page, rows.len()), (1, 12));
        // in use: from the second page to the top of the first
        let file = "Extra pack 20.zip".to_string();
        let (enabled, compatibility) = pack(&mut s, &file);
        assert!(!enabled && rows.contains(&file));
        s.view.update(&mut s.cx, |view, cx| {
            view.set_pack_enabled(file.clone(), true, compatibility, cx)
        });
        let (page, rows, pending) = show_packs(&mut s, None);
        assert_eq!((page, rows[0].as_str(), pending), (0, file.as_str(), false));
        assert!(pack(&mut s, &file).0);

        // and out of use again: among those not in use, on the second page
        let (_, compatibility) = pack(&mut s, &file);
        s.view.update(&mut s.cx, |view, cx| {
            view.set_pack_enabled(file.clone(), false, compatibility, cx)
        });
        let (page, rows, pending) = show_packs(&mut s, None);
        assert_eq!((page, pending), (1, false));
        assert!(rows.contains(&file));
    }

    #[test]
    fn a_pack_moved_past_its_page_is_followed() {
        let mut s = packs();
        let (_, rows, _) = show_packs(&mut s, Some(1));
        // the first pack of the second page, raised: the last of the first
        let file = rows[0].clone();
        s.view
            .update(&mut s.cx, |view, cx| view.move_pack(file.clone(), true, cx));
        let (page, rows, pending) = show_packs(&mut s, None);
        assert_eq!(
            (page, rows[19].as_str(), pending),
            (0, file.as_str(), false)
        );
        // lowered back: the first of the second
        s.view.update(&mut s.cx, |view, cx| {
            view.move_pack(file.clone(), false, cx)
        });
        let (page, rows, pending) = show_packs(&mut s, None);
        assert_eq!((page, rows[0].as_str(), pending), (1, file.as_str(), false));
        // moved on its page: the page stays
        s.view.update(&mut s.cx, |view, cx| {
            view.move_pack(file.clone(), false, cx)
        });
        let (page, rows, _) = show_packs(&mut s, None);
        assert_eq!((page, rows[1].as_str()), (1, file.as_str()));
    }

    #[test]
    fn a_followed_file_that_does_not_come_back_is_dropped() {
        let mut s = packs();
        show_packs(&mut s, Some(1));
        // changed from outside before it was read again: gone
        s.view.update(&mut s.cx, |view, cx| {
            view.section_mut(ContentKind::ResourcePack).follow = Some("Gone.zip".into());
            view.reload(ContentKind::ResourcePack, cx);
        });
        let (page, _, pending) = show_packs(&mut s, None);
        assert_eq!((page, pending), (1, false));
    }

    #[test]
    fn the_shader_packs_open_on_the_page_of_the_one_in_use() {
        // 25 packs, the last in use
        let mut s = filled("content-shaders-paged", ContentKind::ShaderPack);
        let (page, rows, pending) = show_kind(&mut s, ContentKind::ShaderPack, None);
        assert_eq!(
            (page, rows.last().map(String::as_str), pending),
            (1, Some("MakeUp-UltraFast-9.2.zip"), false)
        );
        // turned back to the first page, it stays there as the packs are read again
        show_kind(&mut s, ContentKind::ShaderPack, Some(0));
        s.view.update(&mut s.cx, |view, cx| {
            view.reload(ContentKind::ShaderPack, cx)
        });
        assert_eq!(show_kind(&mut s, ContentKind::ShaderPack, None).0, 0);
    }
}
