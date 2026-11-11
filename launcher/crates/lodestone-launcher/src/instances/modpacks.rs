//! Modpacks: importing a `.mrpack` (a file, or a version from Modrinth) or a CurseForge
//! pack (a `.zip` with `manifest.json`, or a file from CurseForge) as a new instance in
//! the background, updating an instance to another version of its pack, and adding a
//! pack's files to an instance that exists (one made for a server, say).
//! The work is `launcher_core::modpack`'s; this keeps the jobs and their progress.

use std::{path::PathBuf, sync::Arc, time::Duration};

use gpui::{AppContext as _, Context, SharedString, Task};
use launcher_core::{
    content::{
        InstanceDirs, Modrinth, Platforms, Target,
        curseforge::{CfFile, CfMod},
        curseforge_install::CfPlannedFile,
        install::InstallProgress,
        share::{self, ShareCode, ShareOutcome},
    },
    game::{download::CancelToken, install::Installer, loaders::LoaderKind},
    instances::{Instance, Loader},
    modpack::{
        self, CurseForgePack, ModpackError, ModpackRecord, PackInfo,
        addon::{self, AddPlan, AddedPacks},
        import::{self, ImportOptions},
        update::{self, UpdatePlan},
    },
};
use lodestone_minecraft::{
    hash,
    modrinth::{ModrinthProject, RawVersion},
};

use super::{Instances, InstancesEvent};
use crate::i18n::{InstancesMsg as M, InstancesPlural as P, t, tr, trn};

/// A modpack import's or update's progress label, in the interface's language.
/// `launcher-core` writes its labels in English ("Downloading 212 files", "Backing
/// up {world}"); the ones it is known to write are translated, others are shown as
/// they are.
pub fn progress_label(label: &str) -> String {
    let fixed = [
        ("Copying the pack's configs", M::CopyingConfigs),
        ("Recording the pack's files", M::RecordingFiles),
        (
            "Saving a snapshot of the instance's files",
            M::SavingSnapshot,
        ),
        ("Reading the modpack", M::ReadingModpack),
    ];
    if let Some((_, message)) = fixed.iter().find(|(english, _)| *english == label) {
        return t(*message).into();
    }
    if let Some(rest) = label.strip_prefix("Downloading ") {
        if let Some(count) = rest
            .strip_suffix(" files")
            .and_then(|n| n.parse::<u64>().ok())
        {
            return trn!(P::DownloadingFiles, count);
        }
        return tr!(M::DownloadingName, name = rest);
    }
    let named = [
        ("Installing ", M::InstallingName),
        ("Backing up ", M::BackingUp),
    ];
    for (prefix, message) in named {
        if let Some(name) = label.strip_prefix(prefix) {
            return tr!(message, name = name);
        }
    }
    if let Some(name) = label.strip_prefix("Adding ") {
        return tr!(M::AddingName, name = name);
    }
    if let Some(version) = label.strip_prefix("Updating to ") {
        return tr!(M::UpdatingTo, version = version);
    }
    label.to_string()
}

/// A pack being made into an instance.
pub struct ImportJob {
    pub id: u64,
    pub name: SharedString,
    pub progress: Arc<InstallProgress>,
    pub cancel: CancelToken,
    _task: Task<()>,
}

/// An update of an instance's pack in progress.
pub struct UpdateJob {
    pub progress: Arc<InstallProgress>,
    pub cancel: CancelToken,
    _task: Task<()>,
}

/// An update that was planned and waits for the user to confirm it.
#[derive(Clone, Debug)]
pub struct UpdatePrompt {
    pub instance_id: String,
    pub instance_name: String,
    pub from_version: String,
    pub plan: Arc<UpdatePlan>,
    /// The downloaded pack, removed when the prompt is answered.
    pub pack_path: PathBuf,
    pub pack: Arc<PackInfo>,
}

/// A pack to add to an instance, picked in "Add a modpack".
#[derive(Clone)]
pub enum AddSource {
    Modrinth(Box<(ModrinthProject, RawVersion)>),
    CurseForge(Box<(CfMod, CfFile)>),
    /// A `.mrpack`, or a CurseForge pack's `.zip`, on disk (kept afterwards).
    File(PathBuf),
}

impl AddSource {
    /// The record of the pack, without files.
    fn record(&self) -> ModpackRecord {
        match self {
            AddSource::Modrinth(picked) => ModpackRecord::from_modrinth(&picked.0, &picked.1),
            AddSource::CurseForge(picked) => curseforge_record(&picked.0, &picked.1),
            // Filled in from Modrinth, or from the pack, once it is read.
            AddSource::File(_) => ModpackRecord::default(),
        }
    }
}

/// What adding a pack to an instance will do, before it does it.
#[derive(Clone)]
pub struct AddPrompt {
    pub instance_id: String,
    pub instance_name: String,
    /// The server the instance was made for, whose mods the pack can't replace.
    pub server: Option<String>,
    pub plan: Arc<AddPlan>,
    pub pack_path: PathBuf,
    /// The pack is the user's own file, not a download: it stays when done.
    pub keep_pack: bool,
    /// The pack's files only CurseForge's website serves, left out.
    pub blocked: Vec<CfPlannedFile>,
}

/// The record of a CurseForge pack's `file`, without files.
fn curseforge_record(project: &CfMod, file: &CfFile) -> ModpackRecord {
    ModpackRecord {
        title: project.name.clone(),
        slug: project.slug.clone(),
        version_number: file.display_name_or_file().to_string(),
        date_published: file.file_date.clone(),
        curseforge: Some(CurseForgePack {
            project_id: project.id,
            file_id: file.id,
            file_date: file.file_date.clone(),
            page_url: project.page_url(),
        }),
        ..ModpackRecord::default()
    }
}

/// What comes before the import itself: where the pack is and where it came from.
struct Source {
    pack_path: PathBuf,
    /// Remove the pack file afterwards (it was downloaded).
    downloaded: bool,
    record: ModpackRecord,
    name: Option<String>,
    /// A CurseForge pack to download first, with its project (for the icon).
    curseforge: Option<(CfMod, CfFile)>,
    /// A share code based on a Modrinth modpack: its pack is downloaded, and its
    /// changes applied to the new instance.
    share: Option<ShareCode>,
}

/// The pack's "ready" message, and what of a share code couldn't be installed.
fn share_message(ready: String, outcome: &ShareOutcome) -> String {
    if outcome.failed.is_empty() {
        return ready;
    }
    tr!(
        M::TwoSentences,
        first = ready,
        second = tr!(
            M::ShareCodeNotInstalled,
            names = crate::i18n::format::list(
                &outcome
                    .failed
                    .iter()
                    .map(|(title, _)| title.clone())
                    .collect::<Vec<_>>()
            )
        )
    )
}

/// The loader the instance gets for a pack, with Forge's list for old Forge builds.
async fn resolve_loader(
    installer: &Installer,
    pack: &PackInfo,
) -> Result<Option<launcher_core::game::loaders::LoaderChoice>, ModpackError> {
    let Some((kind, version)) = &pack.loader else {
        return Ok(None);
    };
    let list = if *kind == LoaderKind::Forge {
        installer
            .loader_versions(LoaderKind::Forge, &pack.game_version)
            .await
            .ok()
    } else {
        None
    };
    modpack::loader_choice(*kind, version, &pack.game_version, list.as_deref())
        .map(Some)
        .map_err(ModpackError::Invalid)
}

/// The record of a pack file Modrinth knows by its SHA-1, if it is a modpack version.
async fn identify(
    api: &Modrinth,
    path: &std::path::Path,
) -> Option<(ModpackRecord, Option<String>)> {
    let sha1 = hash::sha1_of(path).ok()?;
    let version = api
        .versions_by_sha1(std::slice::from_ref(&sha1))
        .await
        .ok()?
        .remove(&sha1)?;
    let project = api.project(&version.project_id).await.ok()?;
    (project.project_type == "modpack").then(|| {
        let record = ModpackRecord::from_modrinth(&project, &version);
        let icon = record.icon_url.clone();
        (record, icon)
    })
}

impl Instances {
    pub fn imports(&self) -> &[ImportJob] {
        &self.imports
    }

    pub fn cancel_import(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(job) = self.imports.iter().find(|j| j.id == id) {
            job.cancel.cancel();
        }
        cx.notify();
    }

    /// Imports `.mrpack` files and CurseForge packs (`.zip` files with `manifest.json`),
    /// from "Import…" or a drop, and offers `.lodesnap` snapshots. Others are refused.
    pub fn import_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        for path in paths {
            let extension = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)
                .unwrap_or_default();
            if extension == launcher_core::snapshots::EXTENSION {
                cx.emit(InstancesEvent::ImportSnapshot(path));
                continue;
            }
            let is_pack = extension == "mrpack"
                || (extension == "zip" && modpack::curseforge::is_curseforge_pack(&path));
            if !is_pack {
                cx.emit(InstancesEvent::LaunchFailed {
                    title: t(M::NotAModpack).into(),
                    message: tr!(
                        M::NotMrpack,
                        name = path
                            .file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                    )
                    .into(),
                });
                continue;
            }
            let name = path
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| t(M::Modpack).into());
            self.start_import(
                name,
                Source {
                    pack_path: path,
                    downloaded: false,
                    record: ModpackRecord::default(),
                    name: None,
                    curseforge: None,
                    share: None,
                },
                None,
                cx,
            );
        }
    }

    /// Downloads the CurseForge modpack `file` of `project` and imports it, named `name`
    /// (or the project's name).
    pub fn import_curseforge(
        &mut self,
        project: CfMod,
        file: CfFile,
        name: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let record = curseforge_record(&project, &file);
        let title = name.clone().unwrap_or_else(|| project.name.clone());
        self.start_import(
            title,
            Source {
                pack_path: PathBuf::new(),
                downloaded: true,
                record,
                name,
                curseforge: Some((project, file)),
                share: None,
            },
            None,
            cx,
        );
    }

    /// Downloads `version` of the modpack `project` and imports it, named `name` (or the
    /// project's title).
    pub fn import_modrinth(
        &mut self,
        project: ModrinthProject,
        version: RawVersion,
        name: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let record = ModpackRecord::from_modrinth(&project, &version);
        let title = name.clone().unwrap_or_else(|| project.title.clone());
        self.start_import(
            title,
            Source {
                pack_path: PathBuf::new(),
                downloaded: true,
                record,
                name,
                curseforge: None,
                share: None,
            },
            Some(version),
            cx,
        );
    }

    /// Creates an instance from a share code and installs its content in the
    /// background, like a modpack import (with its progress in the sidebar). Returns
    /// the new instance's id.
    /// Makes the empty instance a share code describes: its Minecraft version and
    /// loader, named `name`, else the code's name, else "1.21.1 Fabric".
    pub fn create_from_share_code(
        &mut self,
        code: &launcher_core::content::share::ShareCode,
        name: Option<String>,
        cx: &mut Context<Self>,
    ) -> Result<launcher_core::instances::Instance, String> {
        use launcher_core::game::loaders::LoaderChoice;
        let name = name
            .or_else(|| code.name.clone())
            .unwrap_or_else(|| match code.loader.kind() {
                Some(kind) => format!("{} {}", code.game_version, kind.label()),
                None => code.game_version.clone(),
            });
        let loader = code
            .loader
            .kind()
            .zip(code.loader_version.clone())
            .map(|(kind, version)| LoaderChoice { kind, version });
        self.create(&name, &code.game_version, loader.as_ref(), cx)
    }

    /// Makes an instance from a share code. A code based on a modpack is imported like
    /// the pack (in the background) and returns no id; otherwise the empty instance is
    /// made at once and its id returned while its content installs.
    pub fn import_share_code(
        &mut self,
        code: ShareCode,
        name: Option<String>,
        cx: &mut Context<Self>,
    ) -> Result<Option<String>, String> {
        if let Some(pack) = &code.pack {
            if self.modrinth.is_none() {
                return Err(t(crate::i18n::InstanceSettingsMsg::ModrinthUnreachable).into());
            }
            let title = name
                .or_else(|| code.name.clone())
                .unwrap_or_else(|| super::share_dialog::pack_label(pack));
            self.start_import(
                title.clone(),
                Source {
                    pack_path: PathBuf::new(),
                    downloaded: true,
                    record: ModpackRecord::default(),
                    name: Some(title),
                    curseforge: None,
                    share: Some(code),
                },
                None,
                cx,
            );
            return Ok(None);
        }
        let instance = self.create_from_share_code(&code, name, cx)?;
        let id = instance.id.clone();
        let name = instance.name.clone();
        if code.items.is_empty() {
            return Ok(Some(id));
        }
        let Some(api) = self.modrinth.clone() else {
            cx.emit(InstancesEvent::LaunchFailed {
                title: t(M::ShareCodeContentFailed).into(),
                message: t(M::ShareCodeOffline).into(),
            });
            return Ok(Some(id));
        };
        let (Some(target), dirs) = (self.content_target(&id), self.content_dirs(&id)) else {
            return Ok(Some(id));
        };
        self.next_import += 1;
        let job_id = self.next_import;
        let progress = Arc::new(InstallProgress::default());
        let cancel = CancelToken::new();
        let downloader = self.content_downloader.clone();
        let work = {
            let progress = progress.clone();
            let cancel = cancel.clone();
            cx.background_spawn(async move {
                share::install(&api, &downloader, &dirs, &target, &code, &progress, &cancel).await
            })
        };
        let instance_name = name.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.imports.retain(|job| job.id != job_id);
                this.reload(cx);
                match result {
                    Ok(outcome) => {
                        let installed = trn!(P::ShareCodeInstalled, outcome.installed.len());
                        let message = match outcome.failed.len() {
                            0 => installed,
                            _ => tr!(
                                M::TwoSentences,
                                first = installed,
                                second = tr!(
                                    M::ShareCodeNotInstalled,
                                    names = crate::i18n::format::list(
                                        &outcome
                                            .failed
                                            .iter()
                                            .map(|(title, _)| title.clone())
                                            .collect::<Vec<_>>()
                                    )
                                )
                            ),
                        };
                        cx.emit(InstancesEvent::Notice {
                            title: tr!(M::ShareCodeReady, name = instance_name).into(),
                            message: message.into(),
                        });
                    }
                    Err(err) => cx.emit(InstancesEvent::LaunchFailed {
                        title: t(M::ShareCodeContentFailed).into(),
                        message: err.to_string().into(),
                    }),
                }
                cx.notify();
            })
            .ok();
        });
        self.imports.push(ImportJob {
            id: job_id,
            name: name.into(),
            progress,
            cancel,
            _task: task,
        });
        self.start_import_ticker(cx);
        cx.notify();
        Ok(Some(id))
    }

    fn start_import(
        &mut self,
        label: String,
        source: Source,
        version: Option<RawVersion>,
        cx: &mut Context<Self>,
    ) {
        let Some(api) = self.modrinth.clone() else {
            return;
        };
        let cf = self.curseforge.clone();
        self.next_import += 1;
        let id = self.next_import;
        let progress = Arc::new(InstallProgress::default());
        let cancel = CancelToken::new();
        let store = self.store.clone();
        let installer = self.installer.clone();
        let downloader = self.content_downloader.clone();
        let work = {
            let progress = progress.clone();
            let cancel = cancel.clone();
            cx.background_spawn(async move {
                let mut source = source;
                let mut version = version;
                if let Some(pack) = source.share.as_ref().and_then(|c| c.pack.clone()) {
                    let (project, found) = share::pack_version(&api, &pack)
                        .await
                        .map_err(|err| ModpackError::Invalid(err.to_string()))?;
                    source.record = ModpackRecord::from_modrinth(&project, &found);
                    version = Some(found);
                }
                if let Some(version) = &version {
                    source.pack_path = import::download_pack(
                        &api,
                        &downloader,
                        version,
                        store.root(),
                        &progress,
                        &cancel,
                    )
                    .await?;
                }
                if let Some((_, file)) = &source.curseforge {
                    let cf = cf
                        .as_deref()
                        .ok_or_else(|| ModpackError::Invalid(t(M::CurseForgeUnreachable).into()))?;
                    if !file.downloadable() {
                        return Err(ModpackError::Invalid(t(M::CurseForgePackOnWebsite).into()));
                    }
                    source.pack_path = modpack::curseforge::download(
                        cf,
                        &downloader,
                        file,
                        store.root(),
                        &progress,
                        &cancel,
                    )
                    .await?;
                }
                let result = async {
                    progress.set_label(t(M::ReadingModpack));
                    let from_curseforge =
                        modpack::curseforge::is_curseforge_pack(&source.pack_path);
                    let (pack, blocked): (PackInfo, Vec<CfPlannedFile>) = if from_curseforge {
                        let cf = cf.as_deref().ok_or_else(|| {
                            ModpackError::Invalid(t(M::CurseForgeUnreachable).into())
                        })?;
                        let read = modpack::curseforge::read_curseforge_pack(&source.pack_path)?;
                        let resolved = modpack::curseforge::resolve(cf, Some(&api), &read).await?;
                        (resolved.info, resolved.blocked)
                    } else {
                        (modpack::read_pack(&source.pack_path)?, Vec::new())
                    };
                    let mut record = source.record.clone();
                    let mut icon_url = record.icon_url.clone();
                    if !from_curseforge
                        && !record.on_modrinth()
                        && let Some((found, icon)) = identify(&api, &source.pack_path).await
                    {
                        record = found;
                        icon_url = icon;
                    }
                    if record.title.is_empty() {
                        record.title = pack.name.clone();
                    }
                    if record.version_number.is_empty() {
                        record.version_number = pack.version_number.clone().unwrap_or_default();
                    }
                    let loader = resolve_loader(&installer, &pack).await?;
                    let icon = match (&icon_url, &source.curseforge, cf.as_deref()) {
                        (Some(url), _, _) => api.image(url).await.ok(),
                        (None, Some((project, _)), Some(cf)) => match project.icon_url() {
                            Some(url) => cf.image(url).await.ok(),
                            None => None,
                        },
                        _ => None,
                    };
                    let name = source.name.clone().unwrap_or_else(|| record.title.clone());
                    import::import_with(
                        &store,
                        Platforms::new(Some(&api), cf.as_deref()),
                        &downloader,
                        &source.pack_path,
                        &pack,
                        ImportOptions {
                            name,
                            loader,
                            record,
                            icon,
                        },
                        &progress,
                        &cancel,
                    )
                    .await
                    .map(|instance| (instance, pack, blocked))
                }
                .await;
                if source.downloaded {
                    let _ = std::fs::remove_file(&source.pack_path);
                }
                let (instance, pack, blocked) = result?;
                // A share code's changes, on top of the pack it is based on.
                let shared = match &source.share {
                    Some(code) => {
                        let dirs = InstanceDirs::new(store.dir(&instance.id));
                        let target = Target::new(instance.game_version.clone(), instance.loader);
                        Some(
                            share::install(
                                &api,
                                &downloader,
                                &dirs,
                                &target,
                                code,
                                &progress,
                                &cancel,
                            )
                            .await
                            .map_err(|err| err.to_string()),
                        )
                    }
                    None => None,
                };
                Ok((instance, pack, blocked, shared))
            })
        };
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.imports.retain(|job| job.id != id);
                match result {
                    Ok((instance, pack, blocked, shared)) => {
                        this.selected = Some(instance.id.clone());
                        let to_download = blocked.len();
                        if !blocked.is_empty() {
                            this.pending_downloads.insert(instance.id.clone(), blocked);
                        }
                        this.reload(cx);
                        let ready = tr!(
                            M::ModpackReadyMessage,
                            version = instance.game_version,
                            loader = super::loader_label(&instance),
                        );
                        let mut message = match pack.skipped.first() {
                            None => ready,
                            Some(first) => tr!(
                                M::TwoSentences,
                                first = ready,
                                second = trn!(
                                    P::PackFilesLeftOut,
                                    pack.skipped.len(),
                                    path = first.path
                                )
                            ),
                        };
                        if to_download > 0 {
                            message = tr!(
                                M::TwoSentences,
                                first = message,
                                second = trn!(P::PackFilesToDownload, to_download)
                            );
                        }
                        match shared {
                            None => cx.emit(InstancesEvent::Notice {
                                title: tr!(M::ModpackReady, name = instance.name).into(),
                                message: message.into(),
                            }),
                            Some(Ok(outcome)) => cx.emit(InstancesEvent::Notice {
                                title: tr!(M::ShareCodeReady, name = instance.name).into(),
                                message: share_message(message, &outcome).into(),
                            }),
                            Some(Err(err)) => cx.emit(InstancesEvent::LaunchFailed {
                                title: t(M::ShareCodeContentFailed).into(),
                                message: err.into(),
                            }),
                        }
                    }
                    Err(ModpackError::Cancelled) => {}
                    Err(err) => cx.emit(InstancesEvent::LaunchFailed {
                        title: t(M::CouldntImportModpack).into(),
                        message: err.to_string().into(),
                    }),
                }
                cx.notify();
            })
            .ok();
        });
        self.imports.push(ImportJob {
            id,
            name: label.into(),
            progress,
            cancel,
            _task: task,
        });
        self.start_import_ticker(cx);
        cx.notify();
    }

    /// Redraws while imports or updates run.
    fn start_import_ticker(&mut self, cx: &mut Context<Self>) {
        if self.import_ticker.is_some() {
            return;
        }
        self.import_ticker = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(150))
                    .await;
                let busy = this.update(cx, |this, cx| {
                    cx.notify();
                    let busy = !this.imports.is_empty() || !this.modpack_updates.is_empty();
                    if !busy {
                        this.import_ticker = None;
                    }
                    busy
                });
                if !matches!(busy, Ok(true)) {
                    break;
                }
            }
        }));
    }

    /// The instance's pack, if it was made from one.
    pub fn modpack(&self, id: &str) -> Option<ModpackRecord> {
        ModpackRecord::load(&self.store.dir(id))
    }

    pub fn modpack_update(&self, id: &str) -> Option<&UpdateJob> {
        self.modpack_updates.get(id)
    }

    /// The pack's versions on Modrinth, newest first.
    pub fn modpack_versions(
        &self,
        project_id: &str,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<Vec<RawVersion>, String>>> {
        let api = self.modrinth.clone()?;
        let project_id = project_id.to_string();
        Some(cx.background_spawn(async move {
            api.versions(&project_id, &[], &[])
                .await
                .map_err(|e| e.to_string())
        }))
    }

    /// Downloads `version` of the instance's pack and plans the update to it.
    pub fn plan_modpack_update(
        &mut self,
        id: &str,
        version: RawVersion,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<UpdatePrompt, String>>> {
        if self.is_active(id) || self.modpack_updates.contains_key(id) {
            return None;
        }
        let api = self.modrinth.clone()?;
        let instance = self.store.get(id).ok()?;
        let current = self.modpack(id)?;
        let dirs = self.content_dirs(id);
        let installer = self.installer.clone();
        let downloader = self.content_downloader.clone();
        Some(cx.background_spawn(async move {
            let progress = InstallProgress::default();
            let project = api
                .project(&current.project_id)
                .await
                .map_err(|e| e.to_string())?;
            let pack_path = import::download_pack(
                &api,
                &downloader,
                &version,
                &dirs.instance,
                &progress,
                &CancelToken::new(),
            )
            .await
            .map_err(|e| e.to_string())?;
            let planned = async {
                let pack = modpack::read_pack(&pack_path)?;
                let loader = resolve_loader(&installer, &pack).await?;
                let record = ModpackRecord::from_modrinth(&project, &version);
                let plan = update::plan(&dirs, &current, &pack, record, loader);
                Ok::<_, ModpackError>((plan, pack))
            }
            .await;
            match planned {
                Ok((plan, pack)) => Ok(UpdatePrompt {
                    instance_id: instance.id.clone(),
                    instance_name: instance.name.clone(),
                    from_version: current.version_number.clone(),
                    plan: Arc::new(plan),
                    pack_path,
                    pack: Arc::new(pack),
                }),
                Err(err) => {
                    let _ = std::fs::remove_file(&pack_path);
                    Err(err.to_string())
                }
            }
        }))
    }

    /// Forgets a planned update (its downloaded pack).
    pub fn discard_update(&mut self, prompt: &UpdatePrompt) {
        let _ = std::fs::remove_file(&prompt.pack_path);
    }

    /// Applies a planned update, in the background.
    pub fn apply_modpack_update(
        &mut self,
        prompt: UpdatePrompt,
        backup_worlds: bool,
        cx: &mut Context<Self>,
    ) {
        let id = prompt.instance_id.clone();
        if self.is_active(&id) || self.modpack_updates.contains_key(&id) {
            self.discard_update(&prompt);
            cx.emit(InstancesEvent::LaunchFailed {
                title: t(M::CouldntUpdateModpack).into(),
                message: t(M::StopGameFirst).into(),
            });
            return;
        }
        let api = self.modrinth.clone();
        let dirs = self.content_dirs(&id);
        let downloader = self.content_downloader.clone();
        let progress = Arc::new(InstallProgress::default());
        let cancel = CancelToken::new();
        let work = {
            let progress = progress.clone();
            let cancel = cancel.clone();
            let prompt = prompt.clone();
            cx.background_spawn(async move {
                let result = update::apply(
                    api.as_deref(),
                    &downloader,
                    &dirs,
                    &prompt.plan,
                    &prompt.pack_path,
                    backup_worlds,
                    &progress,
                    &cancel,
                )
                .await;
                let _ = std::fs::remove_file(&prompt.pack_path);
                result
            })
        };
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.modpack_updates.remove(&id);
                this.content_changed(&id);
                match result {
                    Ok(outcome) => {
                        let plan = &prompt.plan;
                        let saved = this.store.get(&id).map(|mut instance: Instance| {
                            instance.game_version = plan.game_version.clone();
                            instance.loader =
                                Loader::from_kind(plan.loader.as_ref().map(|l| l.kind));
                            instance.loader_version =
                                plan.loader.as_ref().map(|l| l.version.clone());
                            instance.recommended_memory = plan.recommended_memory;
                            instance
                        });
                        let message = match saved
                            .map_err(|e| e.to_string())
                            .and_then(|i| this.save(&i, cx))
                        {
                            Ok(()) => {
                                let updated = tr!(
                                    M::ModpackUpdatedMessage,
                                    name = prompt.instance_name,
                                    pack = plan.record.title,
                                    version = plan.record.version_number,
                                    path = outcome.snapshot.display(),
                                );
                                match outcome.world_backups.len() {
                                    0 => updated,
                                    n => tr!(
                                        M::TwoSentences,
                                        first = updated,
                                        second = trn!(P::WorldsBackedUp, n)
                                    ),
                                }
                            }
                            Err(err) => tr!(M::UpdatedButNotSaved, error = err),
                        };
                        cx.emit(InstancesEvent::LoaderChanged(id.clone()));
                        cx.emit(InstancesEvent::Notice {
                            title: t(M::ModpackUpdated).into(),
                            message: message.into(),
                        });
                    }
                    Err(ModpackError::Cancelled) => {}
                    Err(err) => cx.emit(InstancesEvent::LaunchFailed {
                        title: t(M::CouldntUpdateModpack).into(),
                        message: err.to_string().into(),
                    }),
                }
                cx.notify();
            })
            .ok();
        });
        self.modpack_updates.insert(
            prompt.instance_id.clone(),
            UpdateJob {
                progress,
                cancel,
                _task: task,
            },
        );
        self.start_import_ticker(cx);
        cx.notify();
    }

    /// The packs added to the instance on top of its own.
    pub fn added_modpacks(&self, id: &str) -> AddedPacks {
        AddedPacks::load(&self.store.dir(id))
    }

    /// Downloads the pack `source` names and plans adding its files to the instance
    /// `id`. Refused (an error saying why) when the pack is for another Minecraft
    /// version or loader, or is added already.
    pub fn plan_add_modpack(
        &mut self,
        id: &str,
        source: AddSource,
        cx: &mut Context<Self>,
    ) -> Task<Result<AddPrompt, String>> {
        if self.is_active(id) || self.modpack_updates.contains_key(id) {
            return Task::ready(Err(t(M::StopGameFirst).into()));
        }
        let instance = match self.store.get(id) {
            Ok(instance) => instance,
            Err(err) => return Task::ready(Err(err.to_string())),
        };
        let record = source.record();
        if let Some(added) = self.added_modpacks(id).same_project(&record) {
            return Task::ready(Err(tr!(M::ModpackAddedAlready, pack = added.record.title)));
        }
        let api = self.modrinth.clone();
        let cf = self.curseforge.clone();
        let dirs = self.content_dirs(id);
        let server = launcher_core::cores::link::load_link(&dirs.instance).map(|l| l.server_name);
        let downloader = self.content_downloader.clone();
        cx.background_spawn(async move {
            let progress = InstallProgress::default();
            let cancel = CancelToken::new();
            let pack_path = match &source {
                AddSource::Modrinth(picked) => {
                    let version = &picked.1;
                    let api = api
                        .as_deref()
                        .ok_or_else(|| t(M::ModrinthUnreachable).to_string())?;
                    import::download_pack(
                        api,
                        &downloader,
                        version,
                        &dirs.instance,
                        &progress,
                        &cancel,
                    )
                    .await
                    .map_err(|e| e.to_string())?
                }
                AddSource::CurseForge(picked) => {
                    let file = &picked.1;
                    let cf = cf
                        .as_deref()
                        .ok_or_else(|| t(M::CurseForgeUnreachable).to_string())?;
                    if !file.downloadable() {
                        return Err(t(M::CurseForgePackOnWebsite).into());
                    }
                    modpack::curseforge::download(
                        cf,
                        &downloader,
                        file,
                        &dirs.instance,
                        &progress,
                        &cancel,
                    )
                    .await
                    .map_err(|e| e.to_string())?
                }
                AddSource::File(path) => path.clone(),
            };
            let keep_pack = matches!(source, AddSource::File(_));
            let planned = async {
                let (pack, blocked) = if modpack::curseforge::is_curseforge_pack(&pack_path) {
                    let cf = cf
                        .as_deref()
                        .ok_or_else(|| ModpackError::Invalid(t(M::CurseForgeUnreachable).into()))?;
                    let read = modpack::curseforge::read_curseforge_pack(&pack_path)?;
                    let resolved = modpack::curseforge::resolve(cf, api.as_deref(), &read).await?;
                    (resolved.info, resolved.blocked)
                } else {
                    (modpack::read_pack(&pack_path)?, Vec::new())
                };
                if let Some(reason) = addon::incompatibility(&instance, &pack) {
                    return Err(ModpackError::Invalid(reason));
                }
                let mut record = record;
                if keep_pack
                    && !modpack::curseforge::is_curseforge_pack(&pack_path)
                    && let Some(api) = api.as_deref()
                    && let Some((found, _)) = identify(api, &pack_path).await
                {
                    record = found;
                }
                if record.title.is_empty() {
                    record.title = pack.name.clone();
                }
                if record.version_number.is_empty() {
                    record.version_number = pack.version_number.clone().unwrap_or_default();
                }
                if let Some(added) = AddedPacks::load(&dirs.instance).same_project(&record) {
                    return Err(ModpackError::Invalid(tr!(
                        M::ModpackAddedAlready,
                        pack = added.record.title
                    )));
                }
                Ok((addon::plan(&dirs, &pack, record), blocked))
            }
            .await;
            match planned {
                Ok((plan, blocked)) => Ok(AddPrompt {
                    instance_id: instance.id.clone(),
                    instance_name: instance.name.clone(),
                    server,
                    plan: Arc::new(plan),
                    pack_path,
                    keep_pack,
                    blocked,
                }),
                Err(err) => {
                    if !keep_pack {
                        let _ = std::fs::remove_file(&pack_path);
                    }
                    Err(err.to_string())
                }
            }
        })
    }

    /// Forgets a planned add (its downloaded pack).
    pub fn discard_add(&mut self, prompt: &AddPrompt) {
        if !prompt.keep_pack {
            let _ = std::fs::remove_file(&prompt.pack_path);
        }
    }

    /// Adds a planned pack's files, in the background.
    pub fn apply_add_modpack(&mut self, prompt: AddPrompt, cx: &mut Context<Self>) {
        let id = prompt.instance_id.clone();
        if self.is_active(&id) || self.modpack_updates.contains_key(&id) {
            self.discard_add(&prompt);
            cx.emit(InstancesEvent::LaunchFailed {
                title: t(M::CouldntAddModpack).into(),
                message: t(M::StopGameFirst).into(),
            });
            return;
        }
        let api = self.modrinth.clone();
        let cf = self.curseforge.clone();
        let dirs = self.content_dirs(&id);
        let downloader = self.content_downloader.clone();
        let progress = Arc::new(InstallProgress::default());
        let cancel = CancelToken::new();
        let work = {
            let progress = progress.clone();
            let cancel = cancel.clone();
            let prompt = prompt.clone();
            cx.background_spawn(async move {
                let result = addon::apply(
                    Platforms::new(api.as_deref(), cf.as_deref()),
                    &downloader,
                    &dirs,
                    &prompt.plan,
                    &prompt.pack_path,
                    &progress,
                    &cancel,
                )
                .await;
                if !prompt.keep_pack {
                    let _ = std::fs::remove_file(&prompt.pack_path);
                }
                result
            })
        };
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.modpack_updates.remove(&id);
                this.content_changed(&id);
                match result {
                    Ok(added) => {
                        let to_download = prompt.blocked.len();
                        if to_download > 0 {
                            this.pending_downloads
                                .entry(id.clone())
                                .or_default()
                                .extend(prompt.blocked.clone());
                        }
                        let mut message = tr!(
                            M::ModpackAddedMessage,
                            pack = added.record.title,
                            name = prompt.instance_name,
                        );
                        if to_download > 0 {
                            message = tr!(
                                M::TwoSentences,
                                first = message,
                                second = trn!(P::PackFilesToDownload, to_download)
                            );
                        }
                        cx.emit(InstancesEvent::Notice {
                            title: t(M::ModpackAdded).into(),
                            message: message.into(),
                        });
                    }
                    Err(ModpackError::Cancelled) => {}
                    Err(err) => cx.emit(InstancesEvent::LaunchFailed {
                        title: t(M::CouldntAddModpack).into(),
                        message: err.to_string().into(),
                    }),
                }
                cx.notify();
            })
            .ok();
        });
        self.modpack_updates.insert(
            prompt.instance_id.clone(),
            UpdateJob {
                progress,
                cancel,
                _task: task,
            },
        );
        self.start_import_ticker(cx);
        cx.notify();
    }

    /// Removes the added pack `pack_id` from the instance `id`: its files that are as it
    /// wrote them. The window is told how it went; so is the caller.
    pub fn remove_added_modpack(
        &mut self,
        id: &str,
        pack_id: &str,
        cx: &mut Context<Self>,
    ) -> Result<addon::RemoveOutcome, String> {
        if self.is_active(id) || self.modpack_updates.contains_key(id) {
            cx.emit(InstancesEvent::LaunchFailed {
                title: t(M::CouldntRemoveModpack).into(),
                message: t(M::StopGameFirst).into(),
            });
            return Err(t(M::StopGameFirst).into());
        }
        let dirs = self.content_dirs(id);
        let title = self
            .added_modpacks(id)
            .get(pack_id)
            .map(|p| p.record.title.clone())
            .unwrap_or_default();
        let removed = addon::remove(&dirs, pack_id);
        self.content_changed(id);
        let result = match &removed {
            Ok(outcome) => Ok(outcome.clone()),
            Err(err) => Err(err.to_string()),
        };
        match removed {
            Ok(outcome) => {
                let removed = trn!(
                    P::AddedPackFilesRemoved,
                    outcome.removed.len(),
                    pack = title
                );
                let message = match outcome.kept_changed.len() {
                    0 => removed,
                    n => tr!(
                        M::TwoSentences,
                        first = removed,
                        second = trn!(P::AddedPackFilesKept, n)
                    ),
                };
                cx.emit(InstancesEvent::Notice {
                    title: t(M::ModpackRemoved).into(),
                    message: message.into(),
                });
            }
            Err(err) => cx.emit(InstancesEvent::LaunchFailed {
                title: t(M::CouldntRemoveModpack).into(),
                message: err.to_string().into(),
            }),
        }
        cx.notify();
        result
    }

    /// Shows an import in progress, for the offscreen screenshots.
    pub fn preview_import(
        &mut self,
        name: &str,
        progress: Arc<InstallProgress>,
        cx: &mut Context<Self>,
    ) {
        self.next_import += 1;
        self.imports.push(ImportJob {
            id: self.next_import,
            name: name.to_string().into(),
            progress,
            cancel: CancelToken::new(),
            _task: Task::ready(()),
        });
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_labels_are_read_back() {
        assert_eq!(
            progress_label("Downloading 212 files"),
            "Downloading 212 files"
        );
        assert_eq!(progress_label("Downloading 1 files"), "Downloading 1 file");
        assert_eq!(
            progress_label("Downloading sodium.jar"),
            "Downloading sodium.jar"
        );
        assert_eq!(
            progress_label("Backing up New World"),
            "Backing up New World"
        );
        assert_eq!(progress_label("Updating to 6.1.0"), "Updating to 6.1.0");
        assert_eq!(
            progress_label("Copying the pack's configs"),
            "Copying the pack's configs"
        );
        assert_eq!(progress_label("Something else"), "Something else");
    }
}
