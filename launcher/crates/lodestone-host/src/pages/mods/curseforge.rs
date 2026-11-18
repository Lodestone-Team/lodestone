//! CurseForge on the Mods tab. The core has no CurseForge client, so the Host asks
//! CurseForge itself (`launcher_core::cores::admin::curseforge`): which of the
//! installed jars are its files (by the fingerprints the core reports), which have a
//! newer file, and, to install one, it downloads the file here and uploads it to the
//! core as a jar. A build without Lodestone's key shows none of this.

use std::collections::HashSet;

use gpui::{AppContext as _, Context};
use launcher_core::{
    content::{
        CurseForgeError,
        curseforge::{CfFile, CfMod},
    },
    cores::admin::{
        curseforge::{self as server, CfInstalled, PlannedFile},
        curseforge_pack::ByHand,
        mods::InstalledMod,
    },
};
use lodestone_ui::components::{Kind, ObjectChip, Toast};

use super::ModsPage;
use crate::i18n::{InstanceMsg as M, t, tr};

/// How long the note about files to download by hand stays: there is a page to open
/// and a file to fetch before it can go.
const BY_HAND_SECONDS: u64 = 30;

/// The toast for files whose authors keep them on CurseForge's website: their names,
/// and a button that opens their pages. The user downloads them there and uploads them.
pub(super) fn by_hand_toast(blocked: &[PlannedFile]) -> Option<Toast> {
    by_hand(
        blocked.iter().map(|file| file.title()).collect(),
        blocked.iter().map(|file| file.file_page()).collect(),
    )
}

/// The same, for the files a CurseForge modpack's update leaves out.
pub(super) fn pack_by_hand_toast(files: &[ByHand]) -> Option<Toast> {
    by_hand(
        files.iter().map(|file| file.title.clone()).collect(),
        files.iter().map(|file| file.page_url.clone()).collect(),
    )
}

fn by_hand(names: Vec<String>, pages: Vec<String>) -> Option<Toast> {
    if names.is_empty() {
        return None;
    }
    let mut lines = vec![t(M::ModsCurseForgeByHand).to_string()];
    lines.extend(names);
    Some(
        Toast::warning(lines.join("\n"))
            .duration(std::time::Duration::from_secs(BY_HAND_SECONDS))
            .action(t(M::ModsOpenCurseForge), move |_, cx| {
                for page in &pages {
                    cx.open_url(page);
                }
            }),
    )
}

impl ModsPage {
    /// The CurseForge file an installed jar is, where Modrinth doesn't know the jar:
    /// the core's own word (and its updates) is shown for the ones it does.
    pub(super) fn on_curseforge(&self, m: &InstalledMod) -> Option<&CfInstalled> {
        if m.modrinth.is_some() {
            return None;
        }
        self.curseforge.get(&m.file_name)
    }

    /// The name a row shows: Modrinth's title, else CurseForge's, else the jar's own.
    pub(super) fn mod_name(&self, m: &InstalledMod) -> String {
        match self.on_curseforge(m) {
            Some(curseforge) => curseforge.title.clone(),
            None => m.name().to_string(),
        }
    }

    /// Asks CurseForge which of the listed jars are its files and, with `check`, which
    /// have a newer one for the server. `serial` is the list's fetch this belongs to.
    pub(super) fn load_curseforge(&mut self, check: bool, serial: u64, cx: &mut Context<Self>) {
        let (Some(curseforge), Some(list)) = (crate::curseforge::client(cx), &self.list) else {
            self.curseforge.clear();
            self.curseforge_loading = false;
            return;
        };
        let mods = list.mods.clone();
        let target = server::ServerTarget {
            kind: list.kind,
            loader: list.loader,
            game_version: list.game_version.clone(),
        };
        // what is known stays for the jars still there, until CurseForge answers
        let names: HashSet<&str> = list.mods.iter().map(|m| m.file_name.as_str()).collect();
        self.curseforge
            .retain(|name, _| names.contains(name.as_str()));
        self.curseforge_loading = true;
        let task = cx.spawn(async move |this, cx| {
            let known = cx
                .background_spawn(launcher_core::http::in_http_runtime(async move {
                    let mut known = server::identify(&curseforge, &mods).await?;
                    if check {
                        server::find_updates(&curseforge, &target, &mods, &mut known).await?;
                    }
                    Ok::<_, CurseForgeError>(known)
                }))
                .await;
            this.update(cx, |this, cx| {
                if serial != this.list_serial {
                    return;
                }
                this.curseforge_loading = false;
                match known {
                    Ok(known) => this.curseforge = known,
                    Err(err) => {
                        log::warn!("Could not look up mods on CurseForge: {err}");
                        // a plain listing just shows the jars as they are
                        if check {
                            this.toast(Toast::error(tr!(M::ModsCurseForgeFailed, error = err)), cx);
                        }
                    }
                }
                cx.notify();
            })
            .ok();
        });
        self.push(task);
    }

    /// Installs a CurseForge file on the server: its required dependencies with it when
    /// asked, older files of the same projects replaced. The toast is about `object`
    /// (a name and a version) and says it was installed, or updated with `update`.
    /// `busy` is the installed file whose row waits for it.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn install_curseforge(
        &mut self,
        file: CfFile,
        project: Option<CfMod>,
        with_dependencies: bool,
        force: bool,
        object: (String, String),
        update: bool,
        busy: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let (Some(curseforge), Some(downloader), Some(list)) = (
            crate::curseforge::client(cx),
            crate::curseforge::downloader(cx),
            self.list.as_ref(),
        ) else {
            if let Some(view) = &mut self.project {
                view.installing = None;
            }
            if let Some(file) = &busy {
                self.busy_files.remove(file);
            }
            return self.toast(Toast::error(t(M::ModsCurseForgeUnavailable)), cx);
        };
        let target = server::ServerTarget {
            kind: list.kind,
            loader: list.loader,
            game_version: list.game_version.clone(),
        };
        let (mods, known) = (list.mods.clone(), self.curseforge.clone());
        let admin = self.state.read(cx).admin.clone();
        let uuid = self.uuid.clone();
        let task = cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(launcher_core::http::in_http_runtime(async move {
                    server::install_file(
                        &curseforge,
                        &downloader,
                        &admin,
                        &uuid,
                        &target,
                        &mods,
                        &known,
                        file,
                        project,
                        with_dependencies,
                        force,
                    )
                    .await
                }))
                .await;
            this.update(cx, |this, cx| {
                if let Some(view) = &mut this.project {
                    view.installing = None;
                }
                if let Some(file) = &busy {
                    this.busy_files.remove(file);
                }
                match result {
                    Ok(outcome) => {
                        // nothing was put on the server when the file itself is one
                        // the user has to fetch
                        let changed = !outcome.result.installed.is_empty()
                            || !outcome.result.removed.is_empty();
                        if changed || outcome.blocked.is_empty() {
                            let said = t(if update {
                                M::ModsUpdated
                            } else {
                                M::ModsInstalledVersion
                            });
                            this.toast(
                                Self::result_toast(Some(object), said.to_string(), &outcome.result),
                                cx,
                            );
                        }
                        if let Some(toast) = by_hand_toast(&outcome.blocked) {
                            this.toast(toast, cx);
                        }
                    }
                    Err(err) => {
                        let text = if update {
                            tr!(M::ModsUpdateFailed, error = err)
                        } else {
                            tr!(M::ModsInstallFailed, error = err)
                        };
                        this.toast(
                            Toast::error(text).object(ObjectChip::new(object.0).kind(Kind::Mod)),
                            cx,
                        );
                    }
                }
                this.reload(cx);
            })
            .ok();
        });
        self.push(task);
    }
}
