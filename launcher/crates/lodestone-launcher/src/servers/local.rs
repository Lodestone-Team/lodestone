//! "On this computer": the Lodestone Host and bare cores running here, found on the
//! default port, and "Connect as owner" for each (`launcher_core::cores::local_claim`).
//!
//! What a claim gives (the owner key, and the owner account the launcher made for a
//! core without one) is kept in plain text in `cores/local_credentials.json`, not the
//! Keychain: it exists only because this user can already write the core's folder. A
//! claimed core is saved at its HTTPS address with its certificate pinned, and its key
//! is used only for that entry, at that address, to that certificate. The page offers
//! the key to copy; an owner account's password is never shown (it only revokes the
//! key), since the owner signs in to Lodestone Host itself.

use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use gpui::{AppContext as _, Context, PathPromptOptions, Task};
use launcher_core::{
    Secret,
    cores::{
        self, CoreInfo,
        client::display_address,
        local_claim::{self, ClaimError, CredentialsFile, LocalCredentials},
        store::{self, SavedCore},
    },
    game::download::Sleep,
};
use url::Url;

use super::{CoreEntry, Notice, Servers, core_error};
use crate::i18n::{ServersMsg as M, t, tr};

/// How long a scan stays fresh: opening the page again scans only after this.
const SCAN_FRESH: Duration = Duration::from_secs(10);

/// A core that answered on this computer.
#[derive(Clone, Debug)]
pub struct FoundCore {
    pub url: Url,
    pub info: CoreInfo,
}

/// The page's "On this computer" section.
#[derive(Default)]
pub struct LocalCores {
    pub found: Vec<FoundCore>,
    pub scanning: bool,
    /// A scan has finished at least once.
    pub scanned: bool,
    /// The core being claimed, by id.
    pub claiming: Option<String>,
    /// Why the last claim of a core failed, by its id.
    pub error: Option<(String, ClaimError)>,
    /// Data folders the user chose, looked in first.
    pub folders: Vec<PathBuf>,
    pub credentials: CredentialsFile,
    last_scan: Option<Instant>,
    scan_task: Option<Task<()>>,
    claim_task: Option<Task<()>>,
}

/// GPUI's timer, for waiting on the core's answer file.
fn gpui_sleep(cx: &gpui::App) -> Sleep {
    let executor = cx.background_executor().clone();
    Arc::new(move |duration| Box::pin(executor.timer(duration)))
}

/// A claim failure in the user's language.
pub fn claim_error(err: &ClaimError) -> String {
    match err {
        ClaimError::NoCore => t(M::ClaimNoCore).into(),
        ClaimError::Refused => t(M::ClaimRefused).into(),
        ClaimError::FolderNotFound => t(M::ClaimFolderNotFound).into(),
        ClaimError::NotThisCore => t(M::ClaimNotThisCore).into(),
        ClaimError::NoResult => t(M::ClaimNoResult).into(),
        ClaimError::NoCertificate => t(M::ClaimNoCertificate).into(),
        ClaimError::Core(err) => core_error(err),
        ClaimError::Files(error) => tr!(M::ClaimFiles, error = error),
        ClaimError::Invalid(error) => tr!(M::ClaimInvalid, error = error),
    }
}

/// What claiming did, for the control API.
pub struct ClaimedCore {
    pub uuid: String,
    pub name: String,
    pub product: cores::CoreProduct,
    pub owner_username: Option<String>,
    pub owner_created: bool,
    pub credentials_file: PathBuf,
}

impl Servers {
    fn credentials_path(&self, cx: &gpui::App) -> PathBuf {
        local_claim::credentials_path(self.accounts.read(cx).data_dir().root())
    }

    /// Reads `local_credentials.json`, and gives each claimed core its key.
    pub(super) fn load_local_credentials(&mut self, cx: &mut Context<Self>) {
        let path = self.credentials_path(cx);
        match local_claim::load_credentials(&path) {
            Ok(file) => self.local.credentials = file,
            Err(err) => {
                let err = err.to_string();
                self.notice = Some(Notice::new(move || {
                    (t(M::CredentialsReadFailed).into(), err.clone())
                }));
            }
        }
        for core in &mut self.cores {
            if !core.saved.local_claim || !core.saved.signed_in() {
                continue;
            }
            let pinned = core
                .saved
                .certificate
                .as_ref()
                .map(|pin| pin.sha256.as_str());
            core.key = self
                .local
                .credentials
                .cores
                .get(&core.saved.uuid)
                .and_then(|c| c.key_for(&core.saved.url, pinned));
        }
    }

    /// Whether core `uuid`'s key is kept in `local_credentials.json` (not the Keychain).
    pub(super) fn has_local_key(&self, uuid: &str) -> bool {
        self.cores
            .iter()
            .any(|c| c.saved.uuid == uuid && c.saved.local_claim)
    }

    /// A claimed core's id changed (its card follows it): its credentials follow too.
    pub(super) fn rename_local_credentials(
        &mut self,
        old: &str,
        new: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(entry) = self.local.credentials.cores.remove(old) {
            self.local.credentials.cores.insert(new.to_string(), entry);
            self.save_local_credentials(cx);
        }
    }

    /// Revokes core `uuid`'s claimed key on the core, best effort, over its pinned
    /// certificate: with a sign-in as the owner account the launcher made, else with
    /// the key itself (which cores that only let sessions revoke keys refuse).
    pub(super) fn revoke_local_key(&mut self, uuid: &str, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        if let Some(revoke) = self.local_revocation(uuid) {
            cx.background_spawn(revoke).detach();
        }
    }

    /// What revoking core `uuid`'s claimed key takes, as a future, or `None` when there
    /// is nothing to revoke.
    fn local_revocation(
        &self,
        uuid: &str,
    ) -> Option<impl std::future::Future<Output = ()> + use<>> {
        let core = self
            .cores
            .iter()
            .find(|c| c.saved.uuid == uuid && c.saved.local_claim)?;
        let key = core.key.clone()?;
        let id = core
            .saved
            .key_id
            .clone()
            .or_else(|| local_claim::key_id(key.expose()))?;
        let client = self.client(core);
        let owner = self.local.credentials.cores.get(uuid).and_then(|c| {
            c.owner_username
                .clone()
                .zip(c.owner_password.clone().map(Secret::new))
        });
        Some(async move {
            let revoked = match &owner {
                Some((username, password)) => client
                    .revoke_key_signed_in(username, password, &id)
                    .await
                    .is_ok(),
                None => false,
            };
            if !revoked {
                let _ = client.revoke_key(&key, &id).await;
            }
        })
    }

    /// What was kept for core `uuid`, if it was claimed here.
    pub fn local_credentials(&self, uuid: &str) -> Option<&LocalCredentials> {
        self.local.credentials.cores.get(uuid)
    }

    fn save_local_credentials(&mut self, cx: &mut Context<Self>) {
        if self.preview {
            return;
        }
        let path = self.credentials_path(cx);
        if let Err(err) = local_claim::save_credentials(&path, &self.local.credentials) {
            let err = err.to_string();
            self.notice = Some(Notice::new(move || {
                (t(M::CredentialsSaveFailed).into(), err.clone())
            }));
        }
    }

    /// Signing out of, or removing, core `uuid`: its key in the file goes too (the
    /// owner account the launcher made stays, as its only copy).
    pub(super) fn forget_local_key(&mut self, uuid: &str, cx: &mut Context<Self>) {
        if self.local.credentials.forget_key(uuid) {
            self.save_local_credentials(cx);
        }
    }

    /// Looks for cores on this computer, unless it did a moment ago (`force`: Rescan).
    pub fn scan_local(&mut self, force: bool, cx: &mut Context<Self>) {
        if self.preview || self.local.scanning {
            return;
        }
        if !force
            && self
                .local
                .last_scan
                .is_some_and(|at| at.elapsed() < SCAN_FRESH)
        {
            return;
        }
        self.local.scanning = true;
        self.local.last_scan = Some(Instant::now());
        cx.notify();
        let http = self.http.clone();
        let scan = cx.background_spawn(local_claim::discover(http));
        self.local.scan_task = Some(cx.spawn(async move |this, cx| {
            let found = scan.await;
            this.update(cx, |this, cx| {
                this.local.scanning = false;
                this.local.scanned = true;
                // what a core says it is can change (an update), so saved cores follow
                let mut changed = false;
                for (_, info) in &found {
                    for core in &mut this.cores {
                        if core.saved.uuid == info.uuid
                            && (core.saved.product != info.product
                                || core.saved.launched_by != info.launched_by)
                        {
                            core.saved.product = info.product;
                            core.saved.launched_by = info.launched_by;
                            changed = true;
                        }
                    }
                }
                if changed {
                    this.save();
                }
                this.local.found = found
                    .into_iter()
                    .map(|(url, info)| FoundCore { url, info })
                    .collect();
                cx.notify();
            })
            .ok();
        }));
    }

    /// Whether the core found at `url` with id `uuid` is saved and signed in here, at
    /// the same host and port.
    pub fn is_connected(&self, uuid: &str, url: &Url) -> bool {
        self.cores.iter().any(|c| {
            c.saved.uuid == uuid
                && c.signed_in()
                && c.url.host_str() == url.host_str()
                && c.url.port_or_known_default() == url.port_or_known_default()
        })
    }

    /// Whether core card `index` shows in "On this computer", not "Other cores": the
    /// scan found its core here, it was claimed here (and may be stopped now), or its
    /// address is this computer's (a core on another port, say).
    pub(super) fn is_local_card(&self, index: usize) -> bool {
        self.cores.get(index).is_some_and(|core| {
            core.saved.local_claim
                || core
                    .url
                    .host_str()
                    .is_some_and(cores::client::is_loopback_host)
                || self
                    .local
                    .found
                    .iter()
                    .any(|found| found.info.uuid == core.saved.uuid)
        })
    }

    /// "Connect as owner" for a core found on this computer. `folder` is a data folder
    /// the user chose ("Choose folder…").
    pub fn connect_local(&mut self, uuid: &str, folder: Option<PathBuf>, cx: &mut Context<Self>) {
        let task = self.claim_local(uuid, folder, cx);
        self.local.claim_task = Some(cx.spawn(async move |_, _| {
            let _ = task.await;
        }));
    }

    /// Claims owner access to the found core `uuid` (or the only one found, when
    /// `uuid` is empty), saves it signed in, and says so on the page.
    pub fn claim_local(
        &mut self,
        uuid: &str,
        folder: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Task<Result<ClaimedCore, ClaimError>> {
        let found = match uuid {
            "" if self.local.found.len() == 1 => self.local.found.first().cloned(),
            uuid => self
                .local
                .found
                .iter()
                .find(|f| f.info.uuid == uuid)
                .cloned(),
        };
        let http = self.http.clone();
        if let Some(folder) = folder
            && !self.local.folders.contains(&folder)
        {
            self.local.folders.insert(0, folder);
        }
        let folders = local_claim::candidate_folders(&self.local.folders);
        let name = store::device_key_name(&store::host_name());
        let sleep = gpui_sleep(cx);
        let credentials_file = self.credentials_path(cx);
        let claiming = found.as_ref().map(|f| f.info.uuid.clone());
        // a key this launcher claimed before goes first (the core replaces a key of the
        // same name too, so keys don't pile up)
        let revoke = claiming
            .as_deref()
            .and_then(|uuid| self.local_revocation(uuid));
        let wanted = (!uuid.is_empty()).then(|| uuid.to_string());
        self.local.claiming = claiming.clone().or(Some(String::new()));
        self.local.error = None;
        cx.notify();
        let work = cx.background_spawn(async move {
            if let Some(revoke) = revoke {
                revoke.await;
            }
            // without a scan (the control API), find the core first
            let found = match found {
                Some(found) => found,
                None => {
                    let all = local_claim::discover(http.clone()).await;
                    let (url, info) = all
                        .into_iter()
                        .find(|(_, info)| wanted.as_ref().is_none_or(|w| *w == info.uuid))
                        .ok_or(ClaimError::NoCore)?;
                    FoundCore { url, info }
                }
            };
            let client = cores::CoreClient::new(http, found.url.clone());
            local_claim::claim(&client, &folders, &name, sleep).await
        });
        cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.local.claiming = None;
                let claimed = match result {
                    Ok(claimed) => claimed,
                    Err(err) => {
                        this.local.error = Some((claiming.unwrap_or_default(), err.clone()));
                        cx.notify();
                        return Err(err);
                    }
                };
                let summary = this.claimed(&claimed, cx);
                Ok(ClaimedCore {
                    credentials_file,
                    ..summary
                })
            })
            .unwrap_or(Err(ClaimError::NoResult))
        })
    }

    /// Saves a claimed core, signed in with its owner key.
    fn claimed(&mut self, claimed: &local_claim::Claimed, cx: &mut Context<Self>) -> ClaimedCore {
        let uuid = claimed.info.uuid.clone();
        self.local.credentials.record(&uuid, claimed);
        self.save_local_credentials(cx);
        let address = display_address(&claimed.url);
        let saved_cores: Vec<SavedCore> = self.cores.iter().map(|c| c.saved.clone()).collect();
        let existing = store::find_existing(&saved_cores, &uuid, &address);
        let mut saved = existing
            .map(|i| self.cores[i].saved.clone())
            .unwrap_or_default();
        saved.uuid = uuid.clone();
        saved.url = address;
        // the key goes only to this certificate, over HTTPS
        saved.pin_certificate(claimed.pin.clone());
        saved.local_claim = true;
        saved.name = claimed.info.core_name.clone();
        saved.product = claimed.info.product;
        saved.launched_by = claimed.info.launched_by;
        saved.username = claimed.owner_username.clone();
        saved.key_id = claimed.key_id.clone();
        saved.key_expires_at = None;
        let entry = CoreEntry::new(saved, claimed.url.clone(), Some(claimed.api_key.clone()));
        let index = match existing {
            Some(index) => {
                self.cores[index] = entry;
                index
            }
            None => {
                self.cores.push(entry);
                self.cores.len() - 1
            }
        };
        let merged = store::dedupe(&mut self.cores, |c| &c.saved, Some(index));
        self.merged(merged, cx);
        let index = self.core_index(&uuid).unwrap_or(index);
        self.save();
        let name = claimed.info.core_name.clone();
        let created = claimed.created_owner.as_ref().map(|o| o.username.clone());
        self.notice = Some(Notice::success(move || {
            (
                tr!(M::ConnectedAsOwner, name = name),
                match &created {
                    Some(username) => tr!(M::ConnectedMadeOwner, username = username),
                    None => t(M::ConnectedOwnerKey).into(),
                },
            )
        }));
        self.refresh(index, cx);
        cx.notify();
        ClaimedCore {
            uuid,
            name: claimed.info.core_name.clone(),
            product: claimed.info.product,
            owner_username: claimed.owner_username.clone(),
            owner_created: claimed.created_owner.is_some(),
            credentials_file: PathBuf::new(),
        }
    }

    /// "Choose folder…" after the claim couldn't find the core's data folder: asks for
    /// it, then claims again with it.
    pub fn choose_local_folder(&mut self, uuid: String, cx: &mut Context<Self>) {
        let dialog = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some(t(M::ChooseDataFolderPrompt).into()),
        });
        self.local.claim_task = Some(cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = dialog.await else {
                return;
            };
            let Some(folder) = paths.into_iter().next() else {
                return;
            };
            this.update(cx, |this, cx| this.connect_local(&uuid, Some(folder), cx))
                .ok();
        }));
    }

    /// Fills "On this computer", for the offscreen screenshots.
    pub fn preview_local(
        &mut self,
        found: Vec<(&str, &str, cores::CoreProduct, cores::LaunchedBy)>,
        credentials: Vec<(&str, LocalCredentials)>,
        error: Option<(&str, ClaimError)>,
    ) {
        self.local.scanned = true;
        self.local.found = found
            .into_iter()
            .map(|(uuid, name, product, launched_by)| FoundCore {
                url: cores::parse_address("http://127.0.0.1").expect("a valid address"),
                info: CoreInfo {
                    version: "0.6.0".into(),
                    uuid: uuid.into(),
                    core_name: name.into(),
                    is_setup: true,
                    product,
                    launched_by,
                },
            })
            .collect();
        for (uuid, entry) in credentials {
            self.local.credentials.cores.insert(uuid.into(), entry);
        }
        self.local.error = error.map(|(uuid, err)| (uuid.to_string(), err));
    }
}
