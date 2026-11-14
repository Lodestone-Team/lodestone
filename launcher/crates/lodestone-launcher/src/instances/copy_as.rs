//! "Copy with another version or loader": what the dialog and the control API read
//! before a copy (the instance's contents, its mods' builds for the new version and
//! loader), and the job that makes it, run like a modpack import: its progress in the
//! sidebar with Cancel, and the copy selected when it is done. The original never
//! changes. The work is `launcher_core::content::copy_as`'s.

use std::sync::Arc;

use gpui::{AppContext as _, Context, Task};
use launcher_core::{
    content::{
        Target,
        copy_as::{
            self, Contents, CopyAsError, CopyAsRequest, ResolvedMods, SourceMod, WorldsRule,
        },
        install::InstallProgress,
    },
    game::download::CancelToken,
};

use super::{About, Instances, InstancesEvent, Versions, modpacks::ImportJob};
use crate::i18n::{InstancesMsg as M, InstancesPlural as P, t, tr, trn};

impl Instances {
    /// What instance `id` has that a copy can take, read in the background. Mods
    /// `content.json` doesn't know yet are looked up on Modrinth first, when it can be
    /// reached.
    pub fn copy_contents(
        &self,
        id: &str,
        cx: &mut Context<Self>,
    ) -> Task<Result<Contents, String>> {
        let api = self.modrinth.clone();
        let dirs = self.content_dirs(id);
        cx.background_spawn(async move {
            copy_as::contents(api.as_deref(), &dirs)
                .await
                .map_err(|err| err.to_string())
        })
    }

    /// Finds each of `mods`' build for `target` on Modrinth, in the background. `None`
    /// when Modrinth can't be reached.
    pub fn resolve_copy_mods(
        &self,
        mods: Vec<SourceMod>,
        target: Target,
        cx: &mut Context<Self>,
    ) -> Option<Task<Result<ResolvedMods, String>>> {
        let api = self.modrinth.clone()?;
        Some(cx.background_spawn(async move {
            copy_as::resolve_mods(&api, &mods, &target)
                .await
                .map_err(|err| err.to_string())
        }))
    }

    /// Whether worlds can go from Minecraft `from` to `to`, placed by the version list
    /// once it has loaded.
    pub fn worlds_rule(&self, from: &str, to: &str) -> WorldsRule {
        let ids: Vec<&str> = match &self.versions {
            Versions::Ready(list) => list.iter().map(|v| v.id.as_str()).collect(),
            _ => Vec::new(),
        };
        copy_as::worlds_rule(from, to, &ids)
    }

    /// Starts making the copy of instance `id` that `request` describes, in the
    /// background. Returns the job's id, the one [`Self::imports`] lists it under.
    /// Refused while the instance's game runs or its files are changing.
    pub fn copy_as(
        &mut self,
        id: &str,
        request: CopyAsRequest,
        cx: &mut Context<Self>,
    ) -> Result<u64, String> {
        let source = self
            .list
            .iter()
            .find(|i| i.id == id)
            .cloned()
            .ok_or_else(|| {
                launcher_core::instances::InstanceError::NotFound(id.into()).to_string()
            })?;
        if self.is_active(id) {
            return Err(t(M::StopGameFirst).into());
        }
        if self.files_busy(id) {
            return Err(t(M::FilesBusy).into());
        }
        if !request.plan.mods.is_empty() && self.modrinth.is_none() {
            return Err(t(crate::i18n::InstanceSettingsMsg::ModrinthUnreachable).into());
        }
        self.next_import += 1;
        let job_id = self.next_import;
        let progress = Arc::new(InstallProgress::default());
        let cancel = CancelToken::new();
        let name = request.name.clone().unwrap_or_else(|| {
            copy_as::default_name(
                &source.name,
                &request.plan.game_version,
                request.plan.loader,
            )
        });
        let left_behind = request.plan.left_behind();
        // the original's files stay as they are while they are read: Play waits, and it
        // can't be deleted
        let busy = self.mark_files_busy(id);
        let work = {
            let store = self.store.clone();
            let api = self.modrinth.clone();
            let downloader = self.content_downloader.clone();
            let progress = progress.clone();
            let cancel = cancel.clone();
            cx.background_spawn(async move {
                let result = copy_as::execute(
                    &store,
                    api.as_deref(),
                    &downloader,
                    &source,
                    &request,
                    &progress,
                    &cancel,
                )
                .await;
                drop(busy);
                result
            })
        };
        let task = cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.imports.retain(|job| job.id != job_id);
                match result {
                    Ok(outcome) => {
                        let copy = outcome.instance;
                        this.selected = Some(copy.id.clone());
                        this.reload(cx);
                        let ready = tr!(
                            M::ModpackReadyMessage,
                            version = copy.game_version,
                            loader = super::loader_label(&copy),
                        );
                        let mut behind = left_behind;
                        behind.extend(outcome.skipped.into_iter().map(|(title, _)| title));
                        let message = match behind.len() {
                            0 => ready,
                            count => tr!(
                                M::TwoSentences,
                                first = ready,
                                second = trn!(
                                    P::CopyModsLeftBehind,
                                    count,
                                    names = crate::i18n::format::list(&behind)
                                )
                            ),
                        };
                        cx.emit(InstancesEvent::Notice {
                            title: tr!(M::CopyReady, name = copy.name).into(),
                            message: message.into(),
                            about: About::Import(job_id),
                        });
                    }
                    Err(CopyAsError::Cancelled) => {}
                    Err(err) => cx.emit(InstancesEvent::LaunchFailed {
                        title: t(M::CouldntCopy).into(),
                        message: err.to_string().into(),
                        about: About::Import(job_id),
                    }),
                }
                cx.notify();
            })
            .ok();
        });
        self.imports
            .push(ImportJob::new(job_id, name.into(), progress, cancel, task));
        self.start_import_ticker(cx);
        cx.notify();
        Ok(job_id)
    }
}
