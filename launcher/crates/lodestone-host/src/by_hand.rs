//! Files only CurseForge's website has: their authors don't let other apps download
//! them, so the user downloads each in their browser. The Host watches the folder the
//! browser saves to (the user's Downloads, unless they pick another), checks a file
//! that appears against CurseForge's hash, and uploads it to the server it was wanted
//! for (`launcher_core::cores::admin::curseforge::install_downloaded`). No address is
//! ever made up for such a file, and nothing drives the website.
//!
//! The files wait in [`HostState`], by server, so the watch goes on whatever page is
//! shown; a server's Mods tab has the card that lists them. "Open the pages one by one"
//! opens the first file's download page (which starts the download itself) and the
//! next one each time a file has arrived, so the user isn't left with a row of tabs to
//! click through. Where the default browser can be asked for a new window
//! (`launcher_core::browser`), the first page opens in one and the rest follow it
//! there: a window to close at the end, not tabs among the user's own. A server still being made from a modpack gets its files once it is
//! there.

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Duration,
};

use gpui::{AppContext as _, Context, Task};
use launcher_core::cores::admin::{
    curseforge::{self as server, PlannedFile, ServerCfError},
    mods::ModInstallResult,
};
use lodestone_ui::components::{Kind, ObjectChip, Toast};

use crate::{
    i18n::{InstanceMsg as M, t, tr},
    state::{HostEvent, HostState},
};

/// How often the folder is looked at.
const WATCH_EVERY: Duration = Duration::from_secs(2);

/// One server's files to download by hand.
#[derive(Default)]
pub struct ByHandFiles {
    pub files: Vec<PlannedFile>,
    /// Set once a file is on the server.
    pub added: Vec<bool>,
    /// Why the last file found couldn't be added.
    pub error: Option<String>,
    /// "Open the pages one by one" was pressed: the next page opens when a file arrives.
    pub one_by_one: bool,
    /// The first of those pages opened in a browser window of its own, where the rest
    /// follow it: the user closes that window at the end.
    pub own_window: bool,
    /// Downloads that were found and couldn't be added, so they aren't tried on every
    /// look: (index in `files`, path).
    tried: HashSet<(usize, PathBuf)>,
}

impl ByHandFiles {
    /// The first file not on the server yet.
    fn next_waiting(&self) -> Option<usize> {
        self.added.iter().position(|added| !added)
    }

    /// How many files aren't on the server yet.
    pub fn waiting(&self) -> usize {
        self.added.iter().filter(|added| !**added).count()
    }
}

/// The files waiting, by server, and the folder watched for them.
#[derive(Default)]
pub struct ByHand {
    servers: HashMap<String, ByHandFiles>,
    folder: Option<PathBuf>,
    /// The user picked the folder: it isn't replaced by the Downloads folder.
    chosen: bool,
    watching: bool,
    watch: Option<Task<()>>,
}

/// What a look at the folder works with.
type Look = (
    PathBuf,
    Vec<(String, usize, PlannedFile)>,
    launcher_core::cores::admin::AdminClient,
    Option<std::sync::Arc<launcher_core::content::CurseForge>>,
);

impl HostState {
    /// The files the server `uuid` waits for, if any.
    pub fn by_hand(&self, uuid: &str) -> Option<&ByHandFiles> {
        self.by_hand.servers.get(uuid)
    }

    /// The folder watched for them: `None` when the system has no Downloads folder
    /// and the user picked none.
    pub fn by_hand_folder(&self) -> Option<&Path> {
        self.by_hand.folder.as_deref()
    }

    /// Adds `files` to what the server `uuid` waits for, and watches for them.
    pub fn add_by_hand(&mut self, uuid: &str, files: Vec<PlannedFile>, cx: &mut Context<Self>) {
        if files.is_empty() {
            return;
        }
        let server = self.by_hand.servers.entry(uuid.to_string()).or_default();
        for file in files {
            if !server.files.iter().any(|f| f.file.id == file.file.id) {
                server.files.push(file);
                server.added.push(false);
            }
        }
        if self.by_hand.folder.is_none() && !self.by_hand.chosen {
            self.by_hand.folder = launcher_core::content::curseforge_install::downloads_folder();
        }
        self.watch_by_hand(cx);
        cx.notify();
    }

    /// "Skip the rest": the server stops waiting.
    pub fn dismiss_by_hand(&mut self, uuid: &str, cx: &mut Context<Self>) {
        self.by_hand.servers.remove(uuid);
        cx.notify();
    }

    /// Opens a file's download page. A download that couldn't be added is tried again.
    pub fn open_by_hand_page(&mut self, uuid: &str, index: usize, cx: &mut Context<Self>) {
        let Some(server) = self.by_hand.servers.get_mut(uuid) else {
            return;
        };
        server.tried.retain(|(tried, _)| *tried != index);
        if let Some(file) = server.files.get(index) {
            cx.open_url(&file.file_page());
        }
    }

    /// "Open the pages one by one": the first waiting file's page now, and the next
    /// each time a file arrives.
    pub fn open_by_hand_pages(&mut self, uuid: &str, cx: &mut Context<Self>) {
        let Some(server) = self.by_hand.servers.get_mut(uuid) else {
            return;
        };
        server.one_by_one = true;
        server.tried.clear();
        if let Some(file) = server.next_waiting().and_then(|at| server.files.get(at)) {
            // in a browser window of its own, where the browser can be asked for one:
            // the pages after it land there too
            let page = file.file_page();
            server.own_window = launcher_core::browser::open_in_new_window(&page);
            if !server.own_window {
                cx.open_url(&page);
            }
        }
        cx.notify();
    }

    /// "Watch another folder…": where the browser saves downloads.
    pub fn set_by_hand_folder(&mut self, folder: PathBuf, cx: &mut Context<Self>) {
        self.by_hand.folder = Some(folder);
        self.by_hand.chosen = true;
        for server in self.by_hand.servers.values_mut() {
            server.error = None;
            server.tried.clear();
        }
        self.watch_by_hand(cx);
        cx.notify();
    }

    /// What to look for now: `None` when nothing waits any more, and the watch ends.
    fn by_hand_look(&mut self, cx: &mut Context<Self>) -> Option<Option<Look>> {
        if self.by_hand.servers.values().all(|s| s.waiting() == 0) {
            self.by_hand.watching = false;
            return None;
        }
        let Some(folder) = self.by_hand.folder.clone() else {
            // the card asks for a folder
            return Some(None);
        };
        // only servers that are there: one being made from a modpack isn't yet
        let waiting: Vec<(String, usize, PlannedFile)> = self
            .by_hand
            .servers
            .iter()
            .filter(|(uuid, _)| self.instance(uuid).is_some())
            .flat_map(|(uuid, server)| {
                server
                    .files
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| !server.added[*index])
                    .map(|(index, file)| (uuid.clone(), index, file.clone()))
            })
            .collect();
        if waiting.is_empty() {
            return Some(None);
        }
        Some(Some((
            folder,
            waiting,
            self.admin.clone(),
            crate::curseforge::client(cx),
        )))
    }

    /// Sample files for the server `uuid`, one of them added already and the pages being
    /// opened one by one, for the development screenshots. Nothing is watched.
    #[cfg(feature = "screenshot")]
    pub fn demo_by_hand(&mut self, uuid: &str, cx: &mut Context<Self>) {
        let file = |id: u64, title: &str, name: &str| PlannedFile {
            file: serde_json::from_value(serde_json::json!({
                "id": id, "modId": id, "displayName": name, "fileName": name,
            }))
            .expect("a sample file"),
            project: serde_json::from_value(serde_json::json!({
                "id": id, "name": title, "slug": title.to_lowercase().replace(' ', "-"),
            }))
            .ok(),
            depth: 0,
            replaces: Vec::new(),
            installed: false,
            on_modrinth: None,
        };
        self.by_hand.servers.insert(
            uuid.to_string(),
            ByHandFiles {
                files: vec![
                    file(1, "Create", "create-1.21.1-6.0.4.jar"),
                    file(2, "Farmer's Delight", "FarmersDelight-1.21.1-1.2.7.jar"),
                    file(3, "Supplementaries", "supplementaries-1.21-3.1.20.jar"),
                ],
                added: vec![true, false, false],
                one_by_one: true,
                ..Default::default()
            },
        );
        self.by_hand.folder = launcher_core::content::curseforge_install::downloads_folder();
        cx.notify();
    }

    fn watch_by_hand(&mut self, cx: &mut Context<Self>) {
        if self.by_hand.watching {
            return;
        }
        self.by_hand.watching = true;
        let task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(WATCH_EVERY).await;
                let Ok(Some(look)) = this.update(cx, |this, cx| this.by_hand_look(cx)) else {
                    return;
                };
                let Some((folder, waiting, admin, curseforge)) = look else {
                    continue;
                };
                let files: Vec<PlannedFile> = waiting.iter().map(|(_, _, f)| f.clone()).collect();
                let found = cx
                    .background_spawn(async move { server::find_downloaded(&folder, &files) })
                    .await;
                for (at, path) in found {
                    let (uuid, index, planned) = waiting[at].clone();
                    let tried = this
                        .update(cx, |this, _| {
                            this.by_hand
                                .servers
                                .get(&uuid)
                                .is_none_or(|s| s.tried.contains(&(index, path.clone())))
                        })
                        .unwrap_or(true);
                    if tried {
                        continue;
                    }
                    let result = cx
                        .background_spawn(launcher_core::http::in_http_runtime({
                            let (admin, curseforge, uuid, planned, path) = (
                                admin.clone(),
                                curseforge.clone(),
                                uuid.clone(),
                                planned.clone(),
                                path.clone(),
                            );
                            async move {
                                let list = admin.installed_mods(&uuid, false).await?;
                                // without CurseForge's word, the file is simply added
                                let known = match &curseforge {
                                    Some(curseforge) => server::identify(curseforge, &list.mods)
                                        .await
                                        .unwrap_or_default(),
                                    None => Default::default(),
                                };
                                server::install_downloaded(
                                    &admin, &uuid, &list.mods, &known, &planned, &path,
                                )
                                .await
                            }
                        }))
                        .await;
                    let alive = this.update(cx, |this, cx| {
                        this.by_hand_found(&uuid, index, &planned, path, result, cx)
                    });
                    if alive.is_err() {
                        return;
                    }
                }
            }
        });
        self.by_hand.watch = Some(task);
    }

    /// A download was found and uploaded, or couldn't be.
    fn by_hand_found(
        &mut self,
        uuid: &str,
        index: usize,
        planned: &PlannedFile,
        path: PathBuf,
        result: Result<ModInstallResult, ServerCfError>,
        cx: &mut Context<Self>,
    ) {
        let Some(server) = self.by_hand.servers.get_mut(uuid) else {
            return;
        };
        match result {
            Ok(_) => {
                if let Some(added) = server.added.get_mut(index) {
                    *added = true;
                }
                server.error = None;
                // the next file's page, when the pages are opened one by one
                let next = server
                    .one_by_one
                    .then(|| server.next_waiting())
                    .flatten()
                    .and_then(|at| server.files.get(at))
                    .map(|file| file.file_page());
                if server.waiting() == 0 {
                    self.by_hand.servers.remove(uuid);
                }
                if let Some(page) = next {
                    cx.open_url(&page);
                }
                self.toast(
                    Toast::success(t(M::ModsByHandAddedToast))
                        .object(ObjectChip::new(planned.title()).kind(Kind::Mod)),
                    cx,
                );
                cx.emit(HostEvent::ModsChanged(uuid.to_string()));
            }
            Err(err) => {
                server.tried.insert((index, path));
                server.error = Some(tr!(
                    M::ModsByHandFailed,
                    name = planned.title(),
                    error = err
                ));
            }
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_next_page_is_the_first_file_still_waiting() {
        let mut files = ByHandFiles {
            added: vec![true, false, false],
            ..Default::default()
        };
        assert_eq!((files.next_waiting(), files.waiting()), (Some(1), 2));
        files.added[1] = true;
        assert_eq!((files.next_waiting(), files.waiting()), (Some(2), 1));
        files.added[2] = true;
        assert_eq!((files.next_waiting(), files.waiting()), (None, 0));
    }
}
