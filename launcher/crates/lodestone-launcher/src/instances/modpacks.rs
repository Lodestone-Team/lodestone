//! Modpacks: importing a `.mrpack` (a file, or a version from Modrinth) as a new
//! instance in the background, and updating an instance to another version of its pack.
//! The work is `launcher_core::modpack`'s; this keeps the jobs and their progress.

use std::{path::PathBuf, sync::Arc, time::Duration};

use gpui::{AppContext as _, Context, SharedString, Task};
use launcher_core::{
    content::{Modrinth, install::InstallProgress},
    game::{download::CancelToken, install::Installer, loaders::LoaderKind},
    instances::{Instance, Loader},
    modpack::{
        self, ModpackError, ModpackRecord, PackInfo,
        import::{self, ImportOptions},
        update::{self, UpdatePlan},
    },
};
use lodestone_minecraft::{
    hash,
    modrinth::{ModrinthProject, RawVersion},
};

use super::{Instances, InstancesEvent};

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

/// What comes before the import itself: where the pack is and where it came from.
struct Source {
    pack_path: PathBuf,
    /// Remove the pack file afterwards (it was downloaded).
    downloaded: bool,
    record: ModpackRecord,
    name: Option<String>,
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

    /// Imports `.mrpack` files (from "Import .mrpack…" or a drop). Others are refused.
    pub fn import_files(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        for path in paths {
            let is_mrpack = path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e.eq_ignore_ascii_case("mrpack"));
            if !is_mrpack {
                cx.emit(InstancesEvent::LaunchFailed {
                    title: "Not a modpack".into(),
                    message: format!(
                        "{} isn't a .mrpack file. Modrinth modpacks end in .mrpack.",
                        path.file_name()
                            .map_or_else(String::new, |n| n.to_string_lossy().into_owned())
                    )
                    .into(),
                });
                continue;
            }
            let name = path
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Modpack".into());
            self.start_import(
                name,
                Source {
                    pack_path: path,
                    downloaded: false,
                    record: ModpackRecord::default(),
                    name: None,
                },
                None,
                cx,
            );
        }
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
            },
            Some(version),
            cx,
        );
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
                let result = async {
                    progress.set_label("Reading the modpack");
                    let pack = modpack::read_pack(&source.pack_path)?;
                    let mut record = source.record.clone();
                    let mut icon_url = record.icon_url.clone();
                    if !record.on_modrinth()
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
                    let icon = match &icon_url {
                        Some(url) => api.image(url).await.ok(),
                        None => None,
                    };
                    let name = source.name.clone().unwrap_or_else(|| record.title.clone());
                    import::import(
                        &store,
                        Some(&api),
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
                    .map(|instance| (instance, pack))
                }
                .await;
                if source.downloaded {
                    let _ = std::fs::remove_file(&source.pack_path);
                }
                result
            })
        };
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.imports.retain(|job| job.id != id);
                match result {
                    Ok((instance, pack)) => {
                        this.selected = Some(instance.id.clone());
                        this.reload(cx);
                        let skipped = match pack.skipped.len() {
                            0 => String::new(),
                            n => format!(" {n} file(s) of the pack were left out, such as {}.", pack.skipped[0].path),
                        };
                        cx.emit(InstancesEvent::Notice {
                            title: format!("{} is ready", instance.name).into(),
                            message: format!(
                                "Minecraft {} with {}. Press Play to download the game and start it.{skipped}",
                                instance.game_version,
                                instance.loader_label(),
                            )
                            .into(),
                        });
                    }
                    Err(ModpackError::Cancelled) => {}
                    Err(err) => cx.emit(InstancesEvent::LaunchFailed {
                        title: "Couldn't import the modpack".into(),
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
                title: "Couldn't update the modpack".into(),
                message: "Stop the game first.".into(),
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
                match result {
                    Ok(outcome) => {
                        let plan = &prompt.plan;
                        let saved = this.store.get(&id).map(|mut instance: Instance| {
                            instance.game_version = plan.game_version.clone();
                            instance.loader =
                                Loader::from_kind(plan.loader.as_ref().map(|l| l.kind));
                            instance.loader_version =
                                plan.loader.as_ref().map(|l| l.version.clone());
                            instance
                        });
                        let message = match saved
                            .map_err(|e| e.to_string())
                            .and_then(|i| this.save(&i, cx))
                        {
                            Ok(()) => format!(
                                "{} now runs {} {}. The files it replaced are saved in {}.{}",
                                prompt.instance_name,
                                plan.record.title,
                                plan.record.version_number,
                                outcome.snapshot.display(),
                                if outcome.world_backups.is_empty() {
                                    String::new()
                                } else {
                                    format!(
                                        " {} world(s) were backed up first.",
                                        outcome.world_backups.len()
                                    )
                                }
                            ),
                            Err(err) => format!(
                                "The files were updated, but the instance couldn't be saved: {err}"
                            ),
                        };
                        cx.emit(InstancesEvent::LoaderChanged(id.clone()));
                        cx.emit(InstancesEvent::Notice {
                            title: "Modpack updated".into(),
                            message: message.into(),
                        });
                    }
                    Err(ModpackError::Cancelled) => {}
                    Err(err) => cx.emit(InstancesEvent::LaunchFailed {
                        title: "Couldn't update the modpack".into(),
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
