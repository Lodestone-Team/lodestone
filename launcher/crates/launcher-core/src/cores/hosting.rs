//! "Host this": a Launcher instance becomes a server on a Lodestone core
//! (`docs/instance-images.md`, "Sharing", Launcher to Host, and "M6b decisions").
//!
//! 1. The core is asked whether it makes servers from files ([`probe`]): a core from
//!    before milestone 6 can't, and says so before anything is written.
//! 2. The instance is written as a temporary `.lodeimg` ([`upload`],
//!    `Images::export_current`): its content as recorded now, with mods and packs from
//!    Modrinth and CurseForge as references, the worlds the player picked, never the
//!    player's private files (account files, typed commands, crash logs).
//! 3. It is uploaded (`AdminClient::upload_image_named`, as "<instance>.lodeimg"), and the
//!    core reads it and says what the server gets ([`Uploaded::upload`]'s summary): the
//!    server view of the image, client-only mods dropped.
//! 4. The owner picks the world, the name, the port and ticks the EULA (only the person
//!    can: it is never ticked for them), and [`create`] asks the core to make the server.
//!    The core never starts it, and nothing here does.
//! 5. [`wait_created`] follows the creation to its end, which says what the server didn't
//!    get ([`CreationEnd::outcome`]); the instance is linked to the new server
//!    ([`host_link`]) only once it was made.
//!
//! Whatever happens (success, failure, Cancel, a window closed mid-way), the temporary
//! file is deleted (as soon as the core has answered the upload: nothing reads it after)
//! and an upload the core still holds is let go of ([`Uploaded`] does it when dropped).
//! The calls need the owner key or a password sign-in's session: a
//! Launcher's own key can only view, start and stop servers. Neither is ever stored.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use http_client::Method;
use serde::Deserialize;

use super::admin::events::{
    Awaited, ProgressionEndValue, ProgressionStartValue, WaitError, Waited,
};
use super::admin::images::{CreateFromImageRequest, CreationOutcome, ImageUpload};
use super::admin::{AdminClient, login};
use super::client::MAX_JSON_BYTES;
use super::link::ServerLink;
use super::{CoreClient, CoreError};
use crate::Secret;
use crate::content::InstanceDirs;
use crate::game::download::{CancelToken, Progress, ProgressSnapshot, Sleep};
use crate::game::loaders::LoaderKind;
use crate::images::{ImageError, Images};
use crate::instances::Instance;

/// The first game port tried, and how many after it.
pub const FIRST_PORT: u16 = 25565;
pub const PORTS_TRIED: u16 = 100;
/// The server's memory, in MB, unless the user picks other values.
pub const DEFAULT_MIN_RAM: u32 = 1024;
pub const DEFAULT_MAX_RAM: u32 = 4096;
/// How long [`wait_created`] follows a creation before it says it is still being set up:
/// a large world and many downloads take a while, and the core goes on regardless.
pub const CREATION_WAIT: Duration = Duration::from_secs(30 * 60);
/// The temporary files' folder, in the Launcher's cache folder: emptied at start.
pub const UPLOADS_FOLDER: &str = "host-uploads";

/// How the Launcher signs in to the core for hosting.
#[derive(Clone)]
pub enum HostAuth {
    /// The owner key of a core claimed on this computer ("Connect as owner").
    OwnerKey(Secret),
    /// A user's password: a session is made for the dialog and dropped with it.
    Password { username: String, password: Secret },
}

impl std::fmt::Debug for HostAuth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HostAuth::OwnerKey(_) => f.write_str("OwnerKey"),
            HostAuth::Password { username, .. } => f
                .debug_struct("Password")
                .field("username", username)
                .finish_non_exhaustive(),
        }
    }
}

impl HostAuth {
    /// The core's API signed in this way.
    pub async fn admin(self, client: CoreClient) -> Result<AdminClient, CoreError> {
        match self {
            HostAuth::OwnerKey(key) => Ok(AdminClient::new(client, key)),
            HostAuth::Password { username, password } => {
                let reply = login(&client, &username, &password).await?;
                Ok(AdminClient::new(client, Secret::new(reply.token)))
            }
        }
    }
}

/// Why hosting stopped.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// The core is older than milestone 6: it can't make a server from a file.
    #[error(
        "This core is too old to make servers from files. Update Lodestone on that computer, then try again."
    )]
    NeedsUpdate,
    /// The user may not make servers there (or not with what the file carries).
    #[error("{0}")]
    Forbidden(String),
    /// The core refused to make the server from the file it read, for a reason of its own
    /// it gave (in English): the user may not add the file's protected files, say.
    #[error("{0}")]
    Refused(String),
    /// The game runs, and its worlds would be read mid-write.
    #[error("The game is running. Quit it first, so its worlds aren't read while it writes them.")]
    Running,
    /// Lodestone makes no Quilt servers, which the core says itself.
    #[error("Lodestone can't make Quilt servers.")]
    Quilt,
    #[error("Cancelled")]
    Cancelled,
    /// The core is still setting the server up after [`CREATION_WAIT`].
    #[error("The server is still being set up on the core.")]
    StillCreating,
    #[error(transparent)]
    Core(CoreError),
    #[error(transparent)]
    Image(ImageError),
}

impl From<CoreError> for HostError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::Forbidden(message) => HostError::Forbidden(message),
            other => HostError::Core(other),
        }
    }
}

impl From<ImageError> for HostError {
    fn from(err: ImageError) -> Self {
        match err {
            ImageError::Running => HostError::Running,
            ImageError::Cancelled => HostError::Cancelled,
            other => HostError::Image(other),
        }
    }
}

/// Whether the core makes servers from files, asked before anything is written: an
/// upload with no file is refused as such (400) by a core that does, as forbidden (403) to
/// a user who can't make servers there, and isn't found (404, or 405 for a path an older
/// core matches otherwise) on a core from before milestone 6.
pub async fn probe(admin: &AdminClient) -> Result<(), HostError> {
    let boundary = "lodestone-launcher-probe";
    let body = format!("--{boundary}--\r\n").into_bytes();
    let answer = admin
        .send_raw(
            Method::POST,
            "instance/image/upload",
            Some((body, format!("multipart/form-data; boundary={boundary}"))),
            MAX_JSON_BYTES,
        )
        .await;
    match answer {
        // 409: the user's uploads arriving at once are full, which only a core that takes
        // files says
        Err(CoreError::Server {
            status: 400 | 409, ..
        }) => Ok(()),
        // a core that took an empty upload: it makes servers from files, and the upload it
        // may have kept is let go of so it doesn't count against the user's
        Ok(bytes) => {
            if let Ok(kept) = serde_json::from_slice::<ImageUpload>(&bytes)
                && !kept.upload_id.is_empty()
            {
                let _ = admin.discard_image_upload(&kept.upload_id).await;
            }
            Ok(())
        }
        Err(CoreError::NotFound(_)) | Err(CoreError::Server { status: 405, .. }) => {
            Err(HostError::NeedsUpdate)
        }
        Err(err) => Err(err.into()),
    }
}

/// A world the player can send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostWorld {
    /// Its folder in `saves/`.
    pub folder: String,
    /// The name the game shows.
    pub name: String,
    pub size: u64,
    /// Unix seconds, when known.
    pub last_played: Option<i64>,
}

/// The instance's worlds, most recently played first.
pub fn worlds(dirs: &InstanceDirs) -> Vec<HostWorld> {
    crate::worlds::list(dirs)
        .into_iter()
        .map(|world| HostWorld {
            name: world.name(),
            last_played: world
                .summary
                .as_ref()
                .and_then(|s| s.last_played)
                .filter(|ms| *ms > 0)
                .map(|ms| ms / 1000),
            size: world.size,
            folder: world.folder,
        })
        .collect()
}

/// The worlds sent unless the player picks others: the last played (the one a server is
/// usually made for), none when the instance has none.
pub fn default_worlds(worlds: &[HostWorld]) -> Vec<String> {
    worlds
        .iter()
        .max_by_key(|w| w.last_played.unwrap_or(i64::MIN))
        .map(|w| vec![w.folder.clone()])
        .unwrap_or_default()
}

/// The worlds to send for a caller that names them by folder (the control API, the CLI),
/// checked against the instance's worlds `known` before anything is written: `worlds` the
/// folders to send (`None`: the default), `world` the server's world (`Some(None)`: a new
/// one, `None`: the core's choice among those sent). A folder is matched exactly, as the
/// export matches it, so a misspelt or wrongly-cased name is refused here (naming the
/// folders there are) rather than sending no world and making the server a fresh one, or
/// being refused by the core after the whole upload. When only `world` is named, it alone
/// is sent; when a new world is asked for, none is. The message is English, for the
/// control API.
pub fn worlds_to_send(
    known: &[HostWorld],
    worlds: Option<&[String]>,
    world: Option<Option<&str>>,
) -> Result<Vec<String>, String> {
    let check = |name: &str| -> Result<(), String> {
        if known.iter().any(|w| w.folder == name) {
            return Ok(());
        }
        let shown = |text: &str| format!("\"{}\"", lodestone_image::display_text(text));
        let mut message = format!("The instance has no world {}", shown(name));
        if let Some(near) = known
            .iter()
            .find(|w| w.folder.to_lowercase() == name.to_lowercase())
        {
            message.push_str(&format!(
                " (folder names match exactly: did you mean {}?)",
                shown(&near.folder)
            ));
        }
        if known.is_empty() {
            message.push_str("; it has no worlds.");
        } else {
            let folders: Vec<String> = known.iter().map(|w| shown(&w.folder)).collect();
            message.push_str(&format!("; its worlds are {}.", folders.join(", ")));
        }
        Err(message)
    };
    for name in worlds.unwrap_or_default() {
        check(name)?;
    }
    if let Some(Some(name)) = world {
        check(name)?;
        if let Some(worlds) = worlds
            && !worlds.iter().any(|w| w == name)
        {
            return Err(format!(
                "The server's world \"{}\" isn't among the worlds sent.",
                lodestone_image::display_text(name)
            ));
        }
    }
    Ok(match (worlds, world) {
        (Some(worlds), _) => {
            let mut sent: Vec<String> = Vec::new();
            for name in worlds {
                if !sent.contains(name) {
                    sent.push(name.clone());
                }
            }
            sent
        }
        (None, Some(Some(name))) => vec![name.to_string()],
        (None, Some(None)) => Vec::new(),
        (None, None) => default_worlds(known),
    })
}

/// Where hosting has got to, for a dialog to show while [`upload`] runs.
#[derive(Debug, Default)]
pub struct HostProgress {
    stage: AtomicU8,
    /// The bytes of the current stage: read and written while exporting, sent while
    /// uploading.
    pub bytes: Arc<Progress>,
}

/// A stage of [`upload`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostStage {
    /// Writing the instance as a file.
    Exporting,
    /// Sending it to the core.
    Uploading,
    /// The core reads it.
    Reading,
}

impl HostProgress {
    pub fn stage(&self) -> HostStage {
        match self.stage.load(Ordering::Relaxed) {
            0 => HostStage::Exporting,
            // every byte sent: the core reads the file before it answers
            1 => {
                let bytes = self.bytes.snapshot();
                if bytes.bytes_total > 0 && bytes.bytes_done >= bytes.bytes_total {
                    HostStage::Reading
                } else {
                    HostStage::Uploading
                }
            }
            _ => HostStage::Reading,
        }
    }

    fn set(&self, stage: HostStage) {
        self.stage.store(stage as u8, Ordering::Relaxed);
    }

    /// Half of a 180 MB file sent: for the offscreen screenshots.
    pub fn preview_uploading() -> HostProgress {
        let progress = HostProgress::default();
        progress.set(HostStage::Uploading);
        progress.bytes.set(ProgressSnapshot {
            files_total: 1,
            files_done: 0,
            bytes_total: 180_000_000,
            bytes_done: 90_000_000,
        });
        progress
    }
}

/// A file the core holds for a server not made yet: its summary, and the core's upload, let
/// go of when this is dropped (the person backed out, cancelled or closed the dialog)
/// unless [`create`] made a server from it. The temporary file is gone already: once the
/// core has it, nothing reads it again.
pub struct Uploaded {
    pub upload: ImageUpload,
    /// Whether the signed-in user may change the core's global files, which a file's own
    /// jars need (`sharing::can_create`): the owner may; a user may when given it.
    pub may_write_global: bool,
    guard: UploadGuard,
}

impl Uploaded {
    /// A file the core read, holding nothing to let go of: for the offscreen screenshots'
    /// summary. `None` when `summary` isn't an `ImageSummary`.
    pub fn preview(summary: serde_json::Value) -> Option<Uploaded> {
        Some(Uploaded {
            upload: ImageUpload {
                upload_id: String::new(),
                summary: serde_json::from_value(summary).ok()?,
            },
            may_write_global: true,
            guard: UploadGuard {
                admin: None,
                upload_id: String::new(),
            },
        })
    }
}

impl Uploaded {
    /// An upload the core holds already (`upload`), let go of through `admin` when dropped,
    /// with no temporary file of this Launcher's: for tests of the windows that hold one.
    #[doc(hidden)]
    pub fn held(upload: ImageUpload, admin: AdminClient) -> Uploaded {
        Uploaded {
            guard: UploadGuard {
                admin: Some(admin),
                upload_id: upload.upload_id.clone(),
            },
            upload,
            may_write_global: true,
        }
    }
}

impl std::fmt::Debug for Uploaded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Uploaded")
            .field("upload_id", &self.upload.upload_id)
            .finish_non_exhaustive()
    }
}

/// Lets go of an upload the core holds when dropped, from a small thread (launcher-core
/// has no executor of its own), unless disarmed: the core would keep it an hour.
struct UploadGuard {
    admin: Option<AdminClient>,
    upload_id: String,
}

impl UploadGuard {
    fn disarm(&mut self) {
        self.admin = None;
    }
}

impl Drop for UploadGuard {
    fn drop(&mut self) {
        if let Some(admin) = self.admin.take()
            && !self.upload_id.is_empty()
        {
            let upload_id = std::mem::take(&mut self.upload_id);
            std::thread::spawn(move || {
                let _ = futures::executor::block_on(crate::http::in_http_runtime(async move {
                    admin.discard_image_upload(&upload_id).await
                }));
            });
        }
    }
}

/// A temporary file, deleted when dropped.
struct TempFile(PathBuf);

impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        let _ = std::fs::remove_file(self.0.with_extension("lodeimg.part"));
    }
}

/// A file name for the instance named `name`: what the core calls the upload, and the
/// new server's first image says it was "Created from".
pub fn upload_file_name(name: &str) -> String {
    let clean: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let clean = clean.trim().trim_matches('.');
    if clean.is_empty() {
        "instance.lodeimg".into()
    } else {
        format!("{clean}.lodeimg")
    }
}

/// Writes `instance` (with the worlds `worlds`, by folder) as a temporary `.lodeimg` in
/// `temp_dir` and uploads it to the core, which reads it and says what a server would get.
/// `progress` follows the stages; `cancel` stops the export or the upload (nothing is left
/// on either side). Refuses a Quilt instance before anything is written: Lodestone makes no
/// Quilt servers. Blocks while it writes the file: run it on a background thread.
pub async fn upload(
    admin: &AdminClient,
    images: &Arc<Images>,
    instance: &Instance,
    worlds: &[String],
    temp_dir: &Path,
    progress: &HostProgress,
    cancel: &CancelToken,
) -> Result<Uploaded, HostError> {
    if instance.loader.kind() == Some(LoaderKind::Quilt) {
        return Err(HostError::Quilt);
    }
    std::fs::create_dir_all(temp_dir).map_err(|e| {
        HostError::Image(ImageError::io(format!(
            "Failed to create {}",
            temp_dir.display()
        ))(e))
    })?;
    let random = u64::from_le_bytes(crate::random_bytes());
    let file = TempFile(temp_dir.join(format!("{random:016x}.lodeimg")));
    progress.set(HostStage::Exporting);
    progress.bytes.set(ProgressSnapshot::default());
    let bytes = progress.bytes.clone();
    images.export_current(
        &instance.id,
        worlds,
        &file.0,
        &|done, total| {
            bytes.set(ProgressSnapshot {
                files_total: 1,
                files_done: 0,
                bytes_total: total,
                bytes_done: done,
            })
        },
        &|| cancel.is_cancelled(),
    )?;
    if cancel.is_cancelled() {
        return Err(HostError::Cancelled);
    }
    progress.set(HostStage::Uploading);
    let name = upload_file_name(&instance.name);
    let upload = admin
        .upload_image_named(&file.0, &name, progress.bytes.clone(), cancel)
        .await;
    // the core has its copy, or none: this one is read no more
    drop(file);
    let upload = match upload {
        Ok(upload) => upload,
        Err(_) if cancel.is_cancelled() => return Err(HostError::Cancelled),
        Err(CoreError::NotFound(message)) if message == super::client::NOT_FOUND => {
            return Err(HostError::NeedsUpdate);
        }
        Err(err) => return Err(err.into()),
    };
    progress.set(HostStage::Reading);
    let mut uploaded = Uploaded {
        guard: UploadGuard {
            admin: Some(admin.clone()),
            upload_id: upload.upload_id.clone(),
        },
        upload,
        may_write_global: true,
    };
    // An owner key may do anything; a user's permissions are read so the summary warns
    // before Create when the file's jars need one they lack. Unread, the core decides.
    // raced with Cancel: dropping `uploaded` then lets the core's upload go at once
    match super::player_image::unless_cancelled(cancel, admin.me()).await {
        None => return Err(HostError::Cancelled),
        Some(Ok(me)) => {
            uploaded.may_write_global =
                me.may_core(super::admin::models::CorePermission::WriteGlobalFile);
        }
        Some(Err(_)) => {}
    }
    Ok(uploaded)
}

/// What the person chose for the new server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostChoices {
    pub name: String,
    pub port: u16,
    /// The world (its folder in the file) that becomes the server's; `None` for a new
    /// one.
    pub world: Option<String>,
    /// The server software, one of the summary's choices; `None` for the file's own.
    pub flavour: Option<String>,
    /// In MB.
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
    /// Whether the person ticked "I agree to the Minecraft EULA"
    /// (https://aka.ms/MinecraftEULA). Only the person can: never set it for them.
    pub accept_eula: bool,
}

/// Asks the core to make the server from `uploaded` with `choices`; returns its uuid. The
/// core makes it in the background ([`wait_created`]) and never starts it; from here the
/// core owns the upload, and forgets it once the server is made. On a refusal the upload
/// is still the caller's, to try again or drop.
pub async fn create(
    admin: &AdminClient,
    uploaded: &mut Uploaded,
    choices: &HostChoices,
) -> Result<String, HostError> {
    let request = CreateFromImageRequest {
        upload_id: uploaded.upload.upload_id.clone(),
        image: uploaded.upload.summary.image.clone(),
        name: choices.name.clone(),
        port: u32::from(choices.port),
        accept_eula: choices.accept_eula,
        world: choices.world.clone(),
        keep_operators: false,
        flavour: choices.flavour.clone(),
        description: None,
        min_ram: choices.min_ram,
        max_ram: choices.max_ram,
    };
    let uuid = admin
        .create_from_image(&request)
        .await
        .map_err(|err| match err {
            // the user could upload and read the file: the core's own reason says why not
            CoreError::Forbidden(message) => HostError::Refused(message),
            other => other.into(),
        })?;
    super::client::segment(&uuid)?;
    uploaded.guard.disarm();
    Ok(uuid)
}

/// How a creation ended.
#[derive(Clone, Debug, PartialEq)]
pub struct CreationEnd {
    pub success: bool,
    /// The core's message, in English.
    pub message: Option<String>,
    /// What the server didn't get, from a core of milestone 6's second part or later;
    /// an older core says it only in `message`.
    pub outcome: Option<CreationOutcome>,
}

/// Follows the creation of the server `uuid` to its end, calling `progress` with how far
/// along it is (0 to 1), reading the core's event buffer about once a second (with
/// `sleep`). Cancel stops only the waiting: the core goes on making the server. When the
/// core's buffer lost the creation (a busy core), the server being listed counts as
/// made, without an outcome. After [`CREATION_WAIT`]: [`HostError::StillCreating`].
pub async fn wait_created(
    admin: &AdminClient,
    uuid: &str,
    progress: &(dyn Fn(f64) + Sync),
    cancel: &CancelToken,
    sleep: &Sleep,
) -> Result<CreationEnd, HostError> {
    let start = |value: &ProgressionStartValue| matches!(value, ProgressionStartValue::InstanceCreation { instance_uuid } if instance_uuid == uuid);
    let end = |value: &ProgressionEndValue| matches!(value, ProgressionEndValue::InstanceCreation(created) if created.info.uuid == uuid);
    let awaited = Awaited {
        id: None,
        start: &start,
        end: &end,
    };
    let mut left = CREATION_WAIT;
    loop {
        let started = std::time::Instant::now();
        let waited = admin
            .wait_progression(&awaited, progress, cancel, sleep, left)
            .await;
        match waited {
            Ok(Waited::Ended(ended)) => {
                let outcome = match ended.value {
                    Some(ProgressionEndValue::InstanceCreation(created)) => created.outcome,
                    _ => None,
                };
                return Ok(CreationEnd {
                    success: ended.success,
                    message: ended.message,
                    outcome,
                });
            }
            Ok(Waited::Unseen) => {
                let listed = admin
                    .instances()
                    .await?
                    .iter()
                    .any(|instance| instance.uuid == uuid);
                if listed {
                    return Ok(CreationEnd {
                        success: true,
                        message: None,
                        outcome: None,
                    });
                }
                // not there yet: look again, for the time left
                left = left.saturating_sub(started.elapsed().max(Duration::from_secs(u64::from(
                    super::admin::events::UNSEEN_POLLS,
                ))));
                if left.is_zero() {
                    return Err(HostError::StillCreating);
                }
            }
            Err(WaitError::Cancelled) => return Err(HostError::Cancelled),
            Err(WaitError::TimedOut(_)) => return Err(HostError::StillCreating),
            Err(WaitError::Core(err)) => return Err(err.into()),
        }
    }
}

/// The link from the hosted instance to its new server.
pub fn host_link(
    core_uuid: &str,
    core_url: &str,
    server_uuid: &str,
    server_name: &str,
) -> ServerLink {
    ServerLink {
        core_uuid: core_uuid.to_string(),
        core_url: core_url.to_string(),
        server_uuid: server_uuid.to_string(),
        server_name: server_name.to_string(),
        pack_etag: None,
        view: None,
    }
}

/// What following a creation came to for the hosted instance's link.
#[derive(Clone, Debug, PartialEq)]
pub enum Linked {
    /// The server was made: the instance is linked to it now. `previous` names the server
    /// it was linked to before, if any.
    Made {
        end: CreationEnd,
        previous: Option<String>,
    },
    /// The server was made, but the instance was deleted while the creation went on, so
    /// nothing is linked (a link saved then would bring back its folder).
    InstanceGone { end: CreationEnd },
    /// The creation failed (with the core's English message, if any): no link changed.
    Failed(Option<String>),
}

/// Saves `link` in the instance folder `instance_dir` only when the creation `ended` well:
/// a failed creation, one still going on, or a wait that stopped leaves the instance's
/// earlier link as it was ("M6b decisions", Linking). A failure to save the link is the
/// `Err`, as is a wait that ended without an end (`HostError::StillCreating`, a core error).
/// An instance deleted while the creation went on (up to [`CREATION_WAIT`]) gets no link:
/// saving it would make the folder again, holding only the link. The link is saved only
/// into a folder still there, so one deleted between the check and the save isn't made
/// again either.
pub fn link_when_made(
    instance_dir: &Path,
    ended: Result<CreationEnd, HostError>,
    link: &ServerLink,
) -> Result<Linked, HostError> {
    let end = ended?;
    if !end.success {
        return Ok(Linked::Failed(end.message));
    }
    if !instance_dir.join("instance.json").is_file() {
        return Ok(Linked::InstanceGone { end });
    }
    let previous = super::link::load_link(instance_dir).map(|old| old.server_name);
    match super::link::save_link_if_present(instance_dir, link) {
        Ok(()) => Ok(Linked::Made { end, previous }),
        Err(crate::storage::StorageError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            Ok(Linked::InstanceGone { end })
        }
        Err(e) => Err(HostError::Image(ImageError::Instance(e.to_string()))),
    }
}

#[derive(Deserialize)]
struct PortStatus {
    is_in_use: bool,
    is_allocated: bool,
}

impl AdminClient {
    /// The first game port from `first` that is neither in use on the core's machine nor
    /// given to another server (`/check/port/{port}`). `cancel` stops the search at once,
    /// between checks or during one: on a slow core it can be many calls.
    pub async fn free_port(&self, first: u16, cancel: &CancelToken) -> Result<u16, CoreError> {
        for port in first..first.saturating_add(PORTS_TRIED) {
            let status: PortStatus = super::player_image::unless_cancelled(
                cancel,
                self.get(&format!("check/port/{port}")),
            )
            .await
            .ok_or_else(|| CoreError::Invalid("cancelled".into()))??;
            if !status.is_in_use && !status.is_allocated {
                return Ok(port);
            }
        }
        Err(CoreError::Server {
            status: 409,
            message: format!(
                "No free game port between {first} and {}.",
                first.saturating_add(PORTS_TRIED - 1)
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world(folder: &str, last_played: Option<i64>) -> HostWorld {
        HostWorld {
            folder: folder.into(),
            name: folder.into(),
            size: 1,
            last_played,
        }
    }

    #[test]
    fn the_last_played_world_is_sent_unless_others_are_picked() {
        assert_eq!(
            default_worlds(&[world("A", Some(10)), world("B", Some(30)), world("C", None)]),
            ["B"]
        );
        assert_eq!(default_worlds(&[world("A", None)]), ["A"]);
        assert!(default_worlds(&[]).is_empty());
    }

    #[test]
    fn worlds_named_by_a_caller_are_checked_before_anything_is_sent() {
        let known = [world("Survival", Some(30)), world("Creative", Some(10))];
        let names = |names: &[&str]| names.iter().map(|n| n.to_string()).collect::<Vec<_>>();
        // the default: the last played
        assert_eq!(worlds_to_send(&known, None, None).unwrap(), ["Survival"]);
        // the server's world alone is sent when only it is named
        assert_eq!(
            worlds_to_send(&known, None, Some(Some("Creative"))).unwrap(),
            ["Creative"]
        );
        // a new world: nothing to send
        assert!(worlds_to_send(&known, None, Some(None)).unwrap().is_empty());
        assert_eq!(
            worlds_to_send(
                &known,
                Some(&names(&["Creative", "Survival", "Creative"])),
                None
            )
            .unwrap(),
            ["Creative", "Survival"]
        );
        assert_eq!(
            worlds_to_send(&known, Some(&names(&["Survival"])), Some(Some("Survival"))).unwrap(),
            ["Survival"]
        );
        // a wrongly-cased folder is refused, with a hint
        let wrong_case = worlds_to_send(&known, Some(&names(&["survival"])), None).unwrap_err();
        assert!(
            wrong_case.contains("did you mean \"Survival\""),
            "{wrong_case}"
        );
        assert!(wrong_case.contains("\"Creative\""), "{wrong_case}");
        // an unknown world, named either way
        assert!(worlds_to_send(&known, Some(&names(&["Hardcore"])), None).is_err());
        assert!(worlds_to_send(&known, None, Some(Some("Hardcore"))).is_err());
        // the server's world must be among those sent
        let not_sent = worlds_to_send(&known, Some(&names(&["Survival"])), Some(Some("Creative")))
            .unwrap_err();
        assert!(not_sent.contains("isn't among"), "{not_sent}");
        // an instance without worlds says so
        let none = worlds_to_send(&[], None, Some(Some("World"))).unwrap_err();
        assert!(none.contains("has no worlds"), "{none}");
    }

    #[test]
    fn every_byte_sent_reads_as_the_core_reading_the_file() {
        let progress = HostProgress::preview_uploading();
        assert_eq!(progress.stage(), HostStage::Uploading);
        progress.bytes.set(ProgressSnapshot {
            files_total: 1,
            files_done: 0,
            bytes_total: 180_000_000,
            bytes_done: 180_000_000,
        });
        assert_eq!(progress.stage(), HostStage::Reading);
        assert_eq!(HostProgress::default().stage(), HostStage::Exporting);
    }

    #[test]
    fn upload_names_are_file_names() {
        assert_eq!(upload_file_name("All the mods"), "All the mods.lodeimg");
        assert_eq!(upload_file_name("a/b: \"c\""), "a_b_ _c_.lodeimg");
        assert_eq!(upload_file_name(" . "), "instance.lodeimg");
    }

    #[test]
    fn the_sign_in_is_never_printed() {
        let auth = HostAuth::Password {
            username: "steve".into(),
            password: Secret::new("hunter2"),
        };
        let shown = format!("{auth:?}");
        assert!(
            shown.contains("steve") && !shown.contains("hunter2"),
            "{shown}"
        );
        assert_eq!(
            format!("{:?}", HostAuth::OwnerKey(Secret::new("k"))),
            "OwnerKey"
        );
    }
}
