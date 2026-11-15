//! Moving a server's images between computers (`core/src/handlers/backups.rs`'s export,
//! `core/src/handlers/instance_images.rs`): a backup saved as a `.lodeimg` file with its
//! secrets taken out, and a new server made from a `.lodeimg` a Host or a Launcher made.
//!
//! Making a server from a file is three steps: [`AdminClient::upload_image`] streams it to
//! the core, which reads and checks it and answers what it holds ([`ImageSummary`]);
//! [`AdminClient::create_from_image`] makes the server, which comes as an
//! `InstanceCreation` progression; [`AdminClient::discard_image_upload`] forgets an upload
//! the person backed out of (the core forgets it after an hour anyway).
//!
//! Values a newer core adds read as `Unknown`, and missing fields as their defaults, so an
//! older Host keeps working.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use http_client::{AsyncBody, Method};
use serde::{Deserialize, Serialize};

use super::backups::MAX_BACKUP_DOWNLOAD_BYTES;
use super::files::{MultipartUpload, UPLOAD_TIMEOUT};
use super::{AdminClient, segment};
use crate::cores::{
    CoreError,
    client::{Auth, MAX_JSON_BYTES, check},
    download::save_via_partial,
};
use crate::game::download::{CancelToken, Progress};

/// An image export's file extension.
pub const EXTENSION: &str = "lodeimg";

/// How long the core may take to write an export before the request gives up: a world of
/// tens of gigabytes takes a while, and Cancel ends it sooner.
const EXPORT_TIMEOUT: Duration = Duration::from_secs(4 * 60 * 60);

/// How long the core may take to answer a creation: before it answers it opens the upload
/// again (its index and manifests, which for a file of a million entries take a while),
/// checks the disk and looks up the server software for the game version over the
/// network, which together can take longer than an ordinary request's limit.
const CREATE_TIMEOUT: Duration = Duration::from_secs(15 * 60);

/// The slowest upload an upload's time limit allows for, on top of [`UPLOAD_TIMEOUT`]:
/// a fixed hour would end a large world sent over a home connection.
const SLOWEST_UPLOAD_BYTES_PER_SECOND: u64 = 256 * 1024;

/// An upload of `bytes`' time limit.
fn upload_timeout(bytes: u64) -> Duration {
    UPLOAD_TIMEOUT + Duration::from_secs(bytes / SLOWEST_UPLOAD_BYTES_PER_SECOND)
}

/// Whether `path` names a `.lodeimg` file (any letter case): the core refuses other
/// names, which a client checks before sending gigabytes.
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case(EXTENSION))
}

/// Who made a file (`ImageMaker`): it only changes what the dialog says.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImageMaker {
    /// A Lodestone Host's server image (a backup's export).
    Host,
    /// A Lodestone Launcher's client image (a snapshot's export).
    Launcher,
    #[default]
    #[serde(other)]
    Unknown,
}

/// What a new server never takes from a file (`NotTaken`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotTaken {
    /// The server jar, libraries and every program or script at the top: Lodestone
    /// installs its own.
    ServerSoftware,
    /// The EULA is accepted by whoever runs the server.
    Eula,
    /// A player's own files (options, resource packs, screenshots).
    ClientFiles,
    /// The operators (and Bukkit's permissions), unless kept.
    Operators,
    /// Scripts, which only the owner adds while code execution is on.
    Scripts,
    #[serde(other)]
    Unknown,
}

/// Names, at most 64 of them, and how many there are (`NameList`). The core writes their
/// control and invisible characters visibly.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct NameList {
    pub names: Vec<String>,
    pub count: u64,
}

impl NameList {
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The first [`SHOWN_NAMES`] names joined with commas, "…" when there are more. A
    /// core may be anyone's, so whatever it sent is shown safely: each name on one line
    /// with its hidden characters written out (`lodestone_image::display_text`) and cut
    /// at [`SHOWN_NAME_CHARS`], however many names it sent.
    pub fn joined(&self) -> String {
        let shown: Vec<String> = self
            .names
            .iter()
            .take(SHOWN_NAMES)
            .map(|name| {
                let name = lodestone_image::display_text(name);
                match name.char_indices().nth(SHOWN_NAME_CHARS) {
                    Some((cut, _)) => format!("{}…", &name[..cut]),
                    None => name.into_owned(),
                }
            })
            .collect();
        let mut text = shown.join(", ");
        if self.count.max(self.names.len() as u64) > shown.len() as u64 {
            // a core that counted names it didn't list gets "…" alone, not ", …"
            text.push_str(if shown.is_empty() { "…" } else { ", …" });
        }
        text
    }
}

/// How many of a [`NameList`]'s names are shown, and how many characters of each.
pub const SHOWN_NAMES: usize = 8;
pub const SHOWN_NAME_CHARS: usize = 120;

/// What a server made from a file didn't get (`CreationOutcome`), the data beside the
/// English message at the end of its `InstanceCreation` progression: a client says it in
/// its own languages. A core from before milestone 6's second part sends none.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct CreationOutcome {
    /// Files whose download failed and whose bytes the file didn't carry, by path.
    pub not_downloaded: NameList,
    /// Files the core can't download (CurseForge's, a pack's) and the file didn't carry.
    pub no_source: NameList,
    /// Mods deleted because their jar says they run only on clients.
    pub client_jars: NameList,
    /// Files holding a key to the server, a password or a token, never placed.
    pub secrets: NameList,
    /// Settings files deleted because they couldn't be checked for keys to the server.
    pub settings_removed: NameList,
    /// Scripts left out: only the owner adds them, while code execution is on.
    pub scripts: u64,
}

impl CreationOutcome {
    /// Whether the server got everything it could take.
    pub fn is_empty(&self) -> bool {
        self.not_downloaded.is_empty()
            && self.no_source.is_empty()
            && self.client_jars.is_empty()
            && self.secrets.is_empty()
            && self.settings_removed.is_empty()
            && self.scripts == 0
    }
}

/// A world in a file (`ImageWorld`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageWorld {
    /// Its folder as the file names it: what [`CreateFromImageRequest::world`] sends back.
    /// Show [`Self::shown_name`] instead.
    pub folder: String,
    /// Its folder, written visibly (control and invisible characters as `\u{…}`).
    pub folder_shown: String,
    /// Its own name, when it differs from the folder.
    pub name: Option<String>,
    pub size: u64,
    /// Unix seconds.
    pub last_played: Option<i64>,
}

impl ImageWorld {
    /// How the world is called: its own name, else its folder (written visibly).
    pub fn shown_name(&self) -> &str {
        self.name.as_deref().unwrap_or(self.shown_folder())
    }

    /// Its folder, written visibly (an older core sent only `folder`, already so).
    pub fn shown_folder(&self) -> &str {
        if self.folder_shown.is_empty() {
            &self.folder
        } else {
            &self.folder_shown
        }
    }
}

/// What a `.lodeimg` file holds (`ImageSummary`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageSummary {
    /// The image's id, for [`CreateFromImageRequest::image`].
    pub image: String,
    pub file_name: String,
    /// A backup's or snapshot's name.
    pub label: Option<String>,
    pub made_by: ImageMaker,
    pub producer_version: String,
    /// Unix seconds.
    pub created: i64,
    pub minecraft: String,
    pub loader: String,
    pub loader_version: Option<String>,
    /// The server software the file names (`paper`, `purpur`…).
    pub flavour: Option<String>,
    /// The server Lodestone makes from it unless another is chosen (`Vanilla`, `Fabric`,
    /// `Paper`…).
    pub server: Option<String>,
    /// The servers [`CreateFromImageRequest::flavour`] may choose: `Vanilla` and `Paper`
    /// for a vanilla game, else only [`Self::server`].
    pub server_choices: Vec<String>,
    /// Why no server can be made from it.
    pub unsupported: Option<String>,
    /// A note on the server it makes ("Lodestone makes purpur servers as Paper servers…").
    pub server_note: Option<String>,
    /// The pack it is built on.
    pub base: Option<String>,
    pub mods_from_modrinth: NameList,
    pub mods_from_curseforge: NameList,
    /// Mod and plugin files whose bytes the file carries: they come from whoever made it.
    pub carried_mods: NameList,
    /// Mods only clients run: left out.
    pub client_only: NameList,
    /// The mod and plugin files the server takes, from any source: what the trust warning
    /// counts.
    pub server_mods: u64,
    pub worlds: Vec<ImageWorld>,
    /// The world picked unless another is.
    pub default_world: Option<String>,
    pub files: u64,
    pub total_bytes: u64,
    pub download_bytes: u64,
    pub unpacked_bytes: u64,
    /// What making the server writes; the core keeps 1 GB to spare besides.
    pub needed_bytes: u64,
    pub disk_free: Option<u64>,
    /// Files that can't be downloaded there: left out.
    pub not_downloadable: NameList,
    /// What the sender left out on purpose (patterns).
    pub left_out: NameList,
    /// Files the sender's copy couldn't hold.
    pub withheld: NameList,
    /// Secrets the sender's Lodestone made empty, as `<file>: <key>`.
    pub blanked: NameList,
    /// Files that would keep the sender a way into the server, or hold a service's login
    /// (proxy forwarding secrets, Floodgate's folder, LuckPerms' database settings, Discord
    /// bots' tokens, and any settings file with a password or token in it): never taken.
    pub secrets_not_taken: NameList,
    pub not_taken: Vec<NotTaken>,
    /// The file's operators (and its `permissions.yml`, `commands.yml` and LuckPerms'
    /// folder): what
    /// [`CreateFromImageRequest::keep_operators`] takes.
    pub operators: NameList,
    /// Players can join without a Minecraft account.
    pub offline_mode: bool,
    /// Making the server needs permission to change global files.
    pub needs_global_files: bool,
    pub scripts_skipped: u64,
}

impl ImageSummary {
    /// The free-space margin the core keeps on top of [`Self::needed_bytes`].
    pub const SPARE_BYTES: u64 = 1 << 30;

    /// Whether the disk has room, when its free space is known.
    pub fn has_room(&self) -> bool {
        self.has_room_for(self.default_world.as_deref())
    }

    /// What making the server writes with `world` picked
    /// ([`CreateFromImageRequest::world`]; `None`: no world). [`Self::needed_bytes`] is
    /// worked out for [`Self::default_world`]: a Launcher's file places only the world
    /// picked, so the default's size comes off and the picked one's goes on, each twice
    /// (unpacked into the core's store, then placed). A Host's file places every world
    /// whichever is the level. An estimate: the core checks again.
    pub fn needed_bytes_for(&self, world: Option<&str>) -> u64 {
        if self.made_by == ImageMaker::Host || world == self.default_world.as_deref() {
            return self.needed_bytes;
        }
        let size = |folder: Option<&str>| {
            folder
                .and_then(|folder| self.worlds.iter().find(|w| w.folder == folder))
                .map_or(0, |w| w.size.saturating_mul(2))
        };
        self.needed_bytes
            .saturating_sub(size(self.default_world.as_deref()))
            .saturating_add(size(world))
    }

    /// Whether the disk has room with `world` picked (see [`Self::needed_bytes_for`]),
    /// when its free space is known.
    pub fn has_room_for(&self, world: Option<&str>) -> bool {
        self.disk_free.is_none_or(|free| {
            free >= self
                .needed_bytes_for(world)
                .saturating_add(Self::SPARE_BYTES)
        })
    }
}

/// A backup exported as a `.lodeimg` (`ExportedBackup`): a key for `GET /file/{key}`, and
/// what the file leaves out.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExportedBackup {
    pub key: String,
    pub file_name: String,
    /// The file's size; once saved, the bytes saved.
    pub size: u64,
    /// Files taken out (Lodestone's, the EULA, server software, crash files, plugin
    /// settings), by path.
    pub left_out: NameList,
    /// What of `left_out` a player would miss (not the server software, the EULA,
    /// Lodestone's files or what a player's instance never takes, such as plugins' data
    /// and the server's settings), which "Download this world" shows: listed by the core,
    /// as `left_out`'s first names are mostly `libraries/`' and plugins' files. It may
    /// name a `config/` file not shared with players, which they wouldn't get anyway.
    pub left_out_for_players: NameList,
    /// Secrets made empty, as `<file>: <key>`.
    pub blanked: NameList,
    /// CurseForge files whoever opens the file won't get.
    pub not_downloadable: NameList,
    /// Files taken out because they hold a password or token (also in `left_out`): the
    /// other server writes its defaults for them.
    pub secrets_withheld: NameList,
}

/// `POST /instance/image/upload`'s answer (`ImageUpload`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageUpload {
    pub upload_id: String,
    #[serde(default)]
    pub summary: ImageSummary,
}

/// `POST /instance/create_from_image` (`CreateFromImageRequest`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateFromImageRequest {
    pub upload_id: String,
    /// [`ImageSummary::image`]: a file that changed since is refused.
    pub image: String,
    pub name: String,
    pub port: u32,
    /// Only when the person ticked the EULA box.
    pub accept_eula: bool,
    /// The world that becomes the server's world: a Launcher's file gives only it (none
    /// when `None`), a Host's every world, this one the level.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub world: Option<String>,
    /// Take the file's operators.
    pub keep_operators: bool,
    /// One of [`ImageSummary::server_choices`]; `None` makes [`ImageSummary::server`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flavour: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_ram: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_ram: Option<u32>,
}

/// An upload's body read through: counts what is sent into a [`Progress`], and fails once
/// the upload is cancelled, which ends the request (the core then deletes what arrived).
/// The error is never `Interrupted`: the async adapter the body goes through retries
/// those at once, which would spin the HTTP client's only worker forever.
struct Counting<R> {
    inner: R,
    progress: Arc<Progress>,
    cancel: CancelToken,
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(std::io::Error::other("cancelled"));
        }
        let read = self.inner.read(buf)?;
        self.progress.add_bytes(read as u64);
        Ok(read)
    }
}

/// The error a cancelled transfer returns.
fn cancelled() -> CoreError {
    CoreError::Invalid("cancelled".into())
}

/// Lets go of a download link of this user's ([`AdminClient::release_download`]) when the
/// download is done with: at once by [`Self::now`], or, when the future downloading is
/// dropped unfinished (a page or window closed mid-download), from a small thread, since
/// launcher-core has no executor of its own. Otherwise a world-sized file would wait an
/// hour on the core's disk and hold one of the user's few places for backup files. The
/// core keeps a file an hour anyway (and an older core has no way to let go), so a
/// failure is ignored.
pub(super) struct ReleaseOnDrop {
    client: Option<AdminClient>,
    key: String,
}

impl ReleaseOnDrop {
    pub(super) fn new(client: &AdminClient, key: &str) -> ReleaseOnDrop {
        ReleaseOnDrop {
            client: (!key.is_empty()).then(|| client.clone()),
            key: key.to_string(),
        }
    }

    /// Lets go of the link now.
    pub(super) async fn now(mut self) {
        if let Some(client) = self.client.take() {
            let _ = client.release_download(&self.key).await;
        }
    }
}

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            let key = std::mem::take(&mut self.key);
            std::thread::spawn(move || {
                let _ = futures::executor::block_on(client.release_download(&key));
            });
        }
    }
}

/// Sets its flag when dropped.
struct SetOnDrop(Arc<AtomicBool>);

impl Drop for SetOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

/// `request`'s result, or `None` when `cancel` was set first (the request is then
/// dropped, which closes its connection).
async fn race_cancel<T>(
    request: impl std::future::Future<Output = T>,
    cancel: &CancelToken,
) -> Option<T> {
    let done = Arc::new(AtomicBool::new(false));
    // the watcher ends when this future does, also when it is dropped unfinished (a
    // dialog closed mid-upload), not only when it finishes
    let _done = SetOnDrop(done.clone());
    let (sender, receiver) = futures::channel::oneshot::channel::<()>();
    {
        // launcher-core has no timer of its own: a small thread watches the flag
        let (done, cancel) = (done.clone(), cancel.clone());
        std::thread::spawn(move || {
            while !done.load(Ordering::Relaxed) && !cancel.is_cancelled() {
                std::thread::sleep(Duration::from_millis(100));
            }
            let _ = sender.send(());
        });
    }
    let request = std::pin::pin!(request);
    match futures::future::select(request, receiver).await {
        futures::future::Either::Left((result, _)) => Some(result),
        futures::future::Either::Right(_) => None,
    }
}

/// What a request Cancel can end came to: the core's answer, or a cancelled request, with
/// what the core made anyway when Cancel was pressed just as it answered (an upload it
/// kept, an export it wrote), which must be let go of or it stays on the core for an hour.
#[derive(Debug, PartialEq)]
enum Ended<T> {
    Done(T),
    Cancelled { late: Option<T> },
}

/// What the answer `answer` (`None`: cancelled before the core answered) comes to, with
/// Cancel `cancelled` or not. A failure after Cancel is the cancellation's.
fn ended<T>(answer: Option<Result<T, CoreError>>, cancelled: bool) -> Result<Ended<T>, CoreError> {
    match (answer, cancelled) {
        (None, _) | (Some(Err(_)), true) => Ok(Ended::Cancelled { late: None }),
        (Some(Err(err)), false) => Err(err),
        (Some(Ok(answer)), true) => Ok(Ended::Cancelled { late: Some(answer) }),
        (Some(Ok(answer)), false) => Ok(Ended::Done(answer)),
    }
}

/// The body of an export request.
fn export_body(include_plugin_data: bool) -> serde_json::Value {
    serde_json::json!({ "include_plugin_data": include_plugin_data })
}

/// The default name of a backup's export: "<server> - <backup>.lodeimg", without
/// characters file names can't have.
pub fn export_file_name(server: &str, backup: &str) -> String {
    let clean = |text: &str| -> String {
        text.chars()
            .map(|c| {
                if c.is_control() || "/\\:*?\"<>|".contains(c) {
                    '_'
                } else {
                    c
                }
            })
            .collect::<String>()
            .trim()
            .trim_matches('.')
            .to_string()
    };
    let server = clean(server);
    let backup = clean(backup);
    match (server.is_empty(), backup.is_empty()) {
        (false, false) => format!("{server} - {backup}.{EXTENSION}"),
        (false, true) => format!("{server}.{EXTENSION}"),
        (true, false) => format!("{backup}.{EXTENSION}"),
        (true, true) => format!("server.{EXTENSION}"),
    }
}

impl AdminClient {
    /// `POST /instance/{uuid}/backups/{id}/export`: writes the backup as a `.lodeimg` file
    /// on the core, with its secrets taken out (the plugins' folders too unless
    /// `include_plugin_data`), and returns a key for `GET /file/{key}` with what it left
    /// out. Writing a large server takes minutes; dropping the request stops it.
    pub async fn export_backup_key(
        &self,
        uuid: &str,
        id: &str,
        include_plugin_data: bool,
    ) -> Result<ExportedBackup, CoreError> {
        let path = format!("instance/{}/backups/{}/export", segment(uuid), segment(id));
        let body = serde_json::to_vec(&export_body(include_plugin_data))
            .map_err(|err| CoreError::Invalid(err.to_string()))?;
        let bytes = self
            .send_slow(
                Method::POST,
                &path,
                Some((body, "application/json".into())),
                MAX_JSON_BYTES,
                EXPORT_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// Exports the backup (see [`Self::export_backup_key`]) into `out`; what it left out,
    /// with the bytes saved as its `size`.
    /// `progress` counts the download's bytes; until the core has written the file it
    /// stays at zero. `cancel` stops the core's work or the download; the file is written
    /// beside `out` and only replaces it once complete. The core's copy is let go of once
    /// the download ends, however it ends (Cancel pressed just as the core answered too),
    /// so a world-sized file doesn't wait an hour on the core's disk.
    pub async fn export_backup_to(
        &self,
        uuid: &str,
        id: &str,
        include_plugin_data: bool,
        out: &Path,
        progress: &Progress,
        cancel: &CancelToken,
    ) -> Result<ExportedBackup, CoreError> {
        let download = self
            .export_backup_ready(uuid, id, include_plugin_data, cancel)
            .await?;
        self.download_export(download, out, progress, cancel).await
    }

    /// [`Self::export_backup_key`] with Cancel: a key the core made just as Cancel was
    /// pressed is let go of. The caller then downloads it ([`Self::download_export`]) or
    /// lets it go ([`Self::release_download`]), say when the size it states won't fit.
    pub async fn export_backup_ready(
        &self,
        uuid: &str,
        id: &str,
        include_plugin_data: bool,
        cancel: &CancelToken,
    ) -> Result<ExportedBackup, CoreError> {
        let answer = race_cancel(
            self.export_backup_key(uuid, id, include_plugin_data),
            cancel,
        )
        .await;
        match ended(answer, cancel.is_cancelled())? {
            Ended::Done(download) => Ok(download),
            Ended::Cancelled { late } => {
                if let Some(late) = late.filter(|late| !late.key.is_empty()) {
                    let _ = self.release_download(&late.key).await;
                }
                Err(cancelled())
            }
        }
    }

    /// Downloads the export `download` names into `out` (see [`Self::export_backup_to`])
    /// and lets the core's copy go however the download ends. The core states the exact
    /// size of the file it wrote, and a caller checks free space against it, so a download
    /// of any other size is refused: a core sending more than it said would otherwise fill
    /// the disk past that check.
    pub async fn download_export(
        &self,
        mut download: ExportedBackup,
        out: &Path,
        progress: &Progress,
        cancel: &CancelToken,
    ) -> Result<ExportedBackup, CoreError> {
        progress.reset(1, download.size);
        let key = download.key.clone();
        let stated = download.size.min(MAX_BACKUP_DOWNLOAD_BYTES);
        // let go of however the download ends, this future dropped unfinished too
        let release = ReleaseOnDrop::new(self, &key);
        let saved = save_via_partial(out, |partial| {
            let key = key.clone();
            async move {
                self.core()
                    .download_file(&key, &partial, stated, progress, cancel)
                    .await
                    .and_then(|saved| {
                        if saved == stated {
                            Ok(saved)
                        } else {
                            Err(CoreError::Invalid(format!(
                                "the host sent {saved} bytes, not the {stated} it said"
                            )))
                        }
                    })
            }
        })
        .await;
        release.now().await;
        download.size = saved?;
        Ok(download)
    }

    /// `DELETE /file/{key}`: lets go of a download link of this user's, deleting the file
    /// the core made for it (a backup's zip or export) at once instead of an hour later.
    pub async fn release_download(&self, key: &str) -> Result<(), CoreError> {
        self.call(
            Method::DELETE,
            &format!("file/{}", segment(key)),
            None::<()>,
        )
        .await
    }

    /// `POST /instance/image/upload`: streams the file at `path` to the core, which reads
    /// and checks it and says what it holds. `progress` counts the bytes sent; `cancel`
    /// ends the upload at once, while the file is sent or while the core reads it (the
    /// core then drops what arrived); a file the core took just as Cancel was pressed is
    /// discarded. The core calls the file by its name in `path`; see
    /// [`Self::upload_image_named`] for a file whose name isn't the one to show.
    pub async fn upload_image(
        &self,
        path: &Path,
        progress: Arc<Progress>,
        cancel: &CancelToken,
    ) -> Result<ImageUpload, CoreError> {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.upload_image_named(path, &name, progress, cancel).await
    }

    /// [`Self::upload_image`] of the file at `path`, called `name` (a `.lodeimg` name): the
    /// core shows it, and the new server is "Created from" it. For a file written to a
    /// temporary place first, such as an instance exported to be hosted.
    pub async fn upload_image_named(
        &self,
        path: &Path,
        name: &str,
        progress: Arc<Progress>,
        cancel: &CancelToken,
    ) -> Result<ImageUpload, CoreError> {
        let upload = MultipartUpload::named(&[(PathBuf::from(path), name.to_string())])?;
        let timeout = upload_timeout(upload.len());
        let body = || {
            progress.reset(1, upload.len());
            let reader = Counting {
                inner: upload.reader(),
                progress: progress.clone(),
                cancel: cancel.clone(),
            };
            Ok((
                AsyncBody::from_reader(futures::io::AllowStdIo::new(reader)),
                upload.content_type(),
                upload.len(),
            ))
        };
        let auth = Auth::Bearer(self.token());
        let request = async {
            let response = self
                .core()
                .send_streamed(
                    Method::POST,
                    "instance/image/upload",
                    auth,
                    body,
                    MAX_JSON_BYTES,
                    timeout,
                )
                .await?;
            check(&response, auth)?;
            serde_json::from_slice::<ImageUpload>(&response.body)
                .map_err(|err| CoreError::Invalid(err.to_string()))
        };
        let answer = race_cancel(request, cancel).await;
        match ended(answer, cancel.is_cancelled())? {
            Ended::Done(upload) => Ok(upload),
            Ended::Cancelled { late } => {
                if let Some(late) = late.filter(|late| !late.upload_id.is_empty()) {
                    // the core forgets it after an hour anyway
                    let _ = self.discard_image_upload(&late.upload_id).await;
                }
                Err(cancelled())
            }
        }
    }

    /// `DELETE /instance/image/upload/{upload_id}`: forgets an upload.
    pub async fn discard_image_upload(&self, upload_id: &str) -> Result<(), CoreError> {
        self.call(
            Method::DELETE,
            &format!("instance/image/upload/{}", segment(upload_id)),
            None::<()>,
        )
        .await
    }

    /// `POST /instance/create_from_image`: makes the server; returns its uuid. It comes as
    /// an `InstanceCreation` progression, whose end says what it didn't get.
    pub async fn create_from_image(
        &self,
        request: &CreateFromImageRequest,
    ) -> Result<String, CoreError> {
        let body =
            serde_json::to_vec(request).map_err(|err| CoreError::Invalid(err.to_string()))?;
        let bytes = self
            .send_slow(
                Method::POST,
                "instance/create_from_image",
                Some((body, "application/json".into())),
                MAX_JSON_BYTES,
                CREATE_TIMEOUT,
            )
            .await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_summary_with_values_this_build_doesnt_know() {
        let upload: ImageUpload = serde_json::from_str(
            r#"{
              "upload_id": "abc",
              "summary": {
                "image": "d18e", "file_name": "My world.lodeimg", "label": "My world",
                "made_by": "a_new_program", "producer_version": "1.0.0", "created": 5,
                "minecraft": "1.21.1", "loader": "fabric", "loader_version": "0.16.10",
                "server": "Fabric",
                "mods_from_modrinth": {"names": ["lithium.jar"], "count": 1},
                "carried_mods": {"names": ["carried.jar"], "count": 70},
                "worlds": [{"folder": "A", "name": "A level", "size": 23, "last_played": 200},
                           {"folder": "B"}],
                "default_world": "A", "needed_bytes": 595, "disk_free": 100,
                "not_taken": ["client_files", "something_new"],
                "needs_global_files": true, "a_field_from_later": [1]
              }
            }"#,
        )
        .unwrap();
        let summary = &upload.summary;
        assert_eq!(upload.upload_id, "abc");
        assert_eq!(summary.made_by, ImageMaker::Unknown);
        assert_eq!(summary.server.as_deref(), Some("Fabric"));
        assert_eq!(summary.worlds[0].shown_name(), "A level");
        assert_eq!(summary.worlds[1].shown_name(), "B");
        let hidden: ImageWorld = serde_json::from_value(serde_json::json!({
            "folder": "a\u{2028}b", "folder_shown": "a\\u{2028}b"
        }))
        .unwrap();
        assert_eq!(
            hidden.folder, "a\u{2028}b",
            "sent back as the file names it"
        );
        assert_eq!(hidden.shown_name(), "a\\u{2028}b");
        assert_eq!(summary.worlds[1].size, 0);
        assert_eq!(
            summary.not_taken,
            [NotTaken::ClientFiles, NotTaken::Unknown]
        );
        assert_eq!(summary.carried_mods.joined(), "carried.jar, …");
        assert!(summary.client_only.is_empty());
        assert!(!summary.has_room(), "100 bytes free, 1 GB to spare");
        assert!(ImageSummary::default().has_room(), "free space unknown");
    }

    #[test]
    fn a_cores_names_are_shown_on_one_line_and_bounded() {
        let names = NameList {
            names: vec![
                "mods/a.jar\n\nNothing was left out".into(),
                "evil\u{202E}gpj.exe".into(),
            ],
            count: 2,
        };
        assert_eq!(
            names.joined(),
            "mods/a.jar\\u{000A}\\u{000A}Nothing was left out, evil\\u{202E}gpj.exe"
        );
        let many = NameList {
            names: (0..10_000).map(|i| format!("f{i}")).collect(),
            count: 3,
        };
        assert_eq!(many.joined(), "f0, f1, f2, f3, f4, f5, f6, f7, …");
        let long = NameList {
            names: vec!["x".repeat(10_000)],
            count: 1,
        };
        assert_eq!(long.joined().chars().count(), SHOWN_NAME_CHARS + 1);
        let unlisted = NameList {
            names: Vec::new(),
            count: 56,
        };
        assert_eq!(unlisted.joined(), "…");
    }

    /// The core's own sample: `core/src/images/share/tests.rs`'s summaries come from the
    /// same types.
    #[test]
    fn reads_an_export_answer() {
        let exported: ExportedBackup = serde_json::from_str(
            r#"{"key": "k1", "file_name": "Survival - 1.lodeimg", "size": 9,
                "left_out": {"names": ["eula.txt"], "count": 1},
                "left_out_for_players": {"names": ["config/voice.toml"], "count": 1},
                "blanked": {"names": ["server.properties: rcon.password"], "count": 1}, "later": true}"#,
        )
        .unwrap();
        assert_eq!(exported.left_out.joined(), "eula.txt");
        assert_eq!(exported.left_out_for_players.names, ["config/voice.toml"]);
        assert_eq!(exported.blanked.names, ["server.properties: rcon.password"]);
        assert!(exported.not_downloadable.is_empty());
    }

    #[test]
    fn requests_are_the_cores_json() {
        let request = CreateFromImageRequest {
            upload_id: "abc".into(),
            image: "d18e".into(),
            name: "Copy".into(),
            port: 25600,
            accept_eula: false,
            world: Some("A".into()),
            keep_operators: false,
            ..Default::default()
        };
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            serde_json::json!({
                "upload_id": "abc", "image": "d18e", "name": "Copy", "port": 25600,
                "accept_eula": false, "world": "A", "keep_operators": false
            })
        );
        let no_world = CreateFromImageRequest {
            world: None,
            ..request
        };
        assert!(
            serde_json::to_value(&no_world)
                .unwrap()
                .get("world")
                .is_none()
        );
        assert_eq!(
            export_body(true),
            serde_json::json!({ "include_plugin_data": true })
        );
    }

    #[test]
    fn the_counting_reader_counts_and_stops_on_cancel() {
        let progress = Arc::new(Progress::default());
        let cancel = CancelToken::new();
        let mut reader = Counting {
            inner: std::io::Cursor::new(vec![7u8; 10_000]),
            progress: progress.clone(),
            cancel: cancel.clone(),
        };
        let mut buffer = [0u8; 4096];
        assert_eq!(reader.read(&mut buffer).unwrap(), 4096);
        assert_eq!(progress.snapshot().bytes_done, 4096);
        cancel.cancel();
        assert!(reader.read(&mut buffer).is_err());
        assert_eq!(progress.snapshot().bytes_done, 4096);
    }

    #[test]
    fn a_cancelled_upload_ends_through_the_async_body() {
        // the body is read through `AllowStdIo`, which retries `Interrupted` forever
        use futures::AsyncReadExt as _;
        let cancel = CancelToken::new();
        let reader = Counting {
            inner: std::io::Cursor::new(vec![7u8; 10_000]),
            progress: Arc::new(Progress::default()),
            cancel: cancel.clone(),
        };
        let mut body = futures::io::AllowStdIo::new(reader);
        let mut buffer = [0u8; 4096];
        assert_eq!(
            futures::executor::block_on(futures::AsyncReadExt::read(&mut body, &mut buffer))
                .unwrap(),
            4096
        );
        cancel.cancel();
        assert!(
            futures::executor::block_on(futures::AsyncReadExt::read(&mut body, &mut buffer))
                .is_err()
        );
        let mut through_body = AsyncBody::from_reader(futures::io::AllowStdIo::new(Counting {
            inner: std::io::Cursor::new(vec![7u8; 10]),
            progress: Arc::new(Progress::default()),
            cancel,
        }));
        let mut rest = Vec::new();
        assert!(futures::executor::block_on(through_body.read_to_end(&mut rest)).is_err());
    }

    #[test]
    fn the_space_needed_follows_the_world_picked() {
        let world = |folder: &str, size: u64| ImageWorld {
            folder: folder.into(),
            size,
            ..Default::default()
        };
        let mut summary = ImageSummary {
            made_by: ImageMaker::Launcher,
            worlds: vec![world("Big", 20_000), world("Small", 5)],
            default_world: Some("Big".into()),
            needed_bytes: 50_000,
            disk_free: Some(ImageSummary::SPARE_BYTES + 20_000),
            ..Default::default()
        };
        assert_eq!(summary.needed_bytes_for(Some("Big")), 50_000);
        assert_eq!(summary.needed_bytes_for(Some("Small")), 10_010);
        assert_eq!(summary.needed_bytes_for(None), 10_000);
        assert!(!summary.has_room());
        assert!(summary.has_room_for(Some("Small")));
        // a Host's file places every world, whichever is the level
        summary.made_by = ImageMaker::Host;
        assert_eq!(summary.needed_bytes_for(Some("Small")), 50_000);
        assert!(!summary.has_room_for(Some("Small")));
    }

    #[test]
    fn uploads_have_time_for_their_size() {
        assert_eq!(upload_timeout(0), UPLOAD_TIMEOUT);
        let ten_gigabytes = 10 * 1024 * 1024 * 1024;
        assert!(upload_timeout(ten_gigabytes) > Duration::from_secs(10 * 60 * 60));
    }

    #[test]
    fn a_cancelled_request_is_dropped() {
        let cancel = CancelToken::new();
        cancel.cancel();
        let never = futures::future::pending::<Result<(), CoreError>>();
        assert!(futures::executor::block_on(race_cancel(never, &cancel)).is_none());
        let ready = futures::future::ready(Ok::<u8, CoreError>(3));
        assert_eq!(
            futures::executor::block_on(race_cancel(ready, &CancelToken::new()))
                .unwrap()
                .unwrap(),
            3
        );
    }

    /// An HTTP client that answers every request with an empty 200 and records them.
    struct Recorded(std::sync::Mutex<Vec<String>>);

    impl http_client::HttpClient for Recorded {
        fn user_agent(&self) -> Option<&http_client::http::HeaderValue> {
            None
        }

        fn proxy(&self) -> Option<&url::Url> {
            None
        }

        fn send(
            &self,
            req: http_client::Request<AsyncBody>,
        ) -> futures::future::BoxFuture<'static, anyhow::Result<http_client::Response<AsyncBody>>>
        {
            self.0
                .lock()
                .unwrap()
                .push(format!("{} {}", req.method(), req.uri().path()));
            Box::pin(async {
                Ok(http_client::Response::builder()
                    .status(200)
                    .body(AsyncBody::empty())?)
            })
        }
    }

    #[test]
    fn a_download_dropped_unfinished_still_lets_go_of_its_file() {
        let http = Arc::new(Recorded(Default::default()));
        let client = AdminClient::new(
            crate::cores::CoreClient::new(
                http.clone(),
                url::Url::parse("http://127.0.0.1:16662").unwrap(),
            ),
            crate::Secret::new("token"),
        );
        // let go of at once when the download ends
        futures::executor::block_on(ReleaseOnDrop::new(&client, "first").now());
        // and from a thread when the downloading future is dropped
        drop(ReleaseOnDrop::new(&client, "second"));
        // a link with no key has nothing to let go of
        drop(ReleaseOnDrop::new(&client, ""));
        let deadline = std::time::Instant::now() + Duration::from_secs(10);
        while http.0.lock().unwrap().len() < 2 && std::time::Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(50));
        let sent = http.0.lock().unwrap().clone();
        assert_eq!(sent.len(), 2, "{sent:?}");
        assert!(sent[0].starts_with("DELETE ") && sent[0].ends_with("/file/first"));
        assert!(sent[1].starts_with("DELETE ") && sent[1].ends_with("/file/second"));
    }

    #[test]
    fn what_the_core_made_as_cancel_was_pressed_is_let_go_of() {
        let upload = || ImageUpload {
            upload_id: "abc".into(),
            summary: ImageSummary::default(),
        };
        assert_eq!(
            ended(Some(Ok(upload())), false).unwrap(),
            Ended::Done(upload())
        );
        assert_eq!(
            ended(Some(Ok(upload())), true).unwrap(),
            Ended::Cancelled {
                late: Some(upload())
            }
        );
        assert_eq!(
            ended::<ImageUpload>(None, true).unwrap(),
            Ended::Cancelled { late: None }
        );
        assert_eq!(
            ended::<ImageUpload>(Some(Err(CoreError::Invalid("x".into()))), true).unwrap(),
            Ended::Cancelled { late: None }
        );
        assert!(ended::<ImageUpload>(Some(Err(CoreError::Invalid("x".into()))), false).is_err());
    }

    #[test]
    fn only_lodeimg_files_are_sent() {
        assert!(is_image_file(Path::new("/a/World.lodeimg")));
        assert!(is_image_file(Path::new("World.LODEIMG")));
        assert!(!is_image_file(Path::new("world.zip")));
        assert!(!is_image_file(Path::new("lodeimg")));
    }

    #[test]
    fn export_file_names_are_safe() {
        assert_eq!(
            export_file_name("Survival", "20261112-101500-ab12"),
            "Survival - 20261112-101500-ab12.lodeimg"
        );
        assert_eq!(
            export_file_name("a/b: c", "Before \"x\""),
            "a_b_ c - Before _x_.lodeimg"
        );
        assert_eq!(export_file_name("", ""), "server.lodeimg");
    }
}
