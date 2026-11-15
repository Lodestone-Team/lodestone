//! "Download this world": a server's backup comes to the Launcher as a `.lodeimg`
//! (`docs/instance-images.md`, "Sharing", Host to Launcher, and "M6b decisions"), which
//! the snapshot import makes into a new instance (`images::export::import_file`, through
//! `images::from_server`: the client view of the server's content and its worlds).
//!
//! The core exports one of the server's backups (the newest, one the user picks, or one it
//! takes now) with its secrets taken out and without plugins' settings
//! (`AdminClient::export_backup_to`), the Launcher downloads it with progress and Cancel,
//! and the core's copy is let go of however the download ends. It needs permission to
//! read the server's files (the export's own, as for a backup's zip), so a signed-in user
//! with it or the owner; a Launcher's own key can't.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};
use std::time::Duration;

use super::CoreError;
use super::admin::AdminClient;
use super::admin::backups::BackupInfo;
use super::admin::events::{Awaited, ProgressionEndValue, WaitError, Waited};
use super::admin::images::ExportedBackup;
use crate::game::download::{CancelToken, Progress, Sleep};

/// The temporary files' folder, in the Launcher's cache folder: emptied at start.
pub const DOWNLOADS_FOLDER: &str = "world-downloads";
/// How long a new backup may take before the download gives up waiting for it.
const BACKUP_WAIT: Duration = Duration::from_secs(60 * 60);

/// Which backup is downloaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WhichBackup {
    /// The newest backup.
    Newest,
    /// The backup with this id.
    Id(String),
    /// A backup the core takes now (the server may be running: a live backup).
    New,
}

/// Why a download stopped.
#[derive(Debug, thiserror::Error)]
pub enum WorldDownloadError {
    /// The core is older than milestone 6: it can't export a backup as a file.
    #[error(
        "This core is too old to send a world. Update Lodestone on that computer, then try again."
    )]
    NeedsUpdate,
    #[error("You can't read this server's files, so its world can't be downloaded. Ask its owner.")]
    Forbidden,
    /// The user may read the server's files but not back it up.
    #[error("You can't back this server up. Pick one of its backups instead.")]
    BackupForbidden,
    /// The file the core wrote won't fit on this computer's disk with room to spare.
    #[error(
        "There isn't enough free space for the world: it needs {needed} bytes, and {free} are free."
    )]
    NoRoom { needed: u64, free: u64 },
    #[error("The server has no backups yet. Back it up first.")]
    NoBackups,
    /// The backup the core took failed.
    #[error("The backup failed: {0}")]
    BackupFailed(String),
    #[error("Cancelled")]
    Cancelled,
    #[error(transparent)]
    Core(CoreError),
}

impl From<CoreError> for WorldDownloadError {
    fn from(err: CoreError) -> Self {
        match err {
            CoreError::Forbidden(_) => WorldDownloadError::Forbidden,
            other => WorldDownloadError::Core(other),
        }
    }
}

/// The server's backups, newest first.
pub async fn backups(
    admin: &AdminClient,
    server: &str,
) -> Result<Vec<BackupInfo>, WorldDownloadError> {
    let mut list = admin.list_backups(server).await?.backups;
    list.sort_by_key(|b| std::cmp::Reverse(b.created));
    Ok(list)
}

/// A stage of [`download`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DownloadStage {
    /// The backup is found and the core writes the file: where every download starts, so
    /// a download of a listed backup never says the server is being backed up.
    Preparing = 0,
    /// The core backs the server up ([`WhichBackup::New`]).
    BackingUp = 1,
    /// The file comes.
    Downloading = 2,
}

/// Where a download has got to, for a dialog to show.
#[derive(Debug, Default)]
pub struct DownloadProgress {
    stage: AtomicU8,
    /// The file's bytes once it comes.
    pub bytes: Arc<Progress>,
    /// The backup the core took for it ([`WhichBackup::New`]), once taken.
    made: std::sync::Mutex<Option<String>>,
}

impl DownloadProgress {
    pub fn stage(&self) -> DownloadStage {
        match self.stage.load(Ordering::Relaxed) {
            1 => DownloadStage::BackingUp,
            2 => DownloadStage::Downloading,
            // the bytes count once the core has written the file
            _ if self.bytes.snapshot().bytes_total == 0 => DownloadStage::Preparing,
            _ => DownloadStage::Downloading,
        }
    }

    /// A download whose file's bytes are counted in `bytes` (a job's progress, say).
    pub fn with_bytes(bytes: Arc<Progress>) -> DownloadProgress {
        DownloadProgress {
            stage: AtomicU8::new(DownloadStage::Preparing as u8),
            bytes,
            made: Default::default(),
        }
    }

    /// The backup the core took for the download, once taken: a download that stopped
    /// after it can download it next instead of taking another.
    pub fn backup_made(&self) -> Option<String> {
        self.made.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn set(&self, stage: DownloadStage) {
        self.stage.store(stage as u8, Ordering::Relaxed);
    }

    /// Half of a 180 MB world downloaded: for the offscreen screenshots.
    pub fn preview_downloading() -> DownloadProgress {
        let progress = DownloadProgress::default();
        progress.set(DownloadStage::Downloading);
        progress.bytes.set(crate::game::download::ProgressSnapshot {
            files_total: 1,
            files_done: 0,
            bytes_total: 180_000_000,
            bytes_done: 90_000_000,
        });
        progress
    }
}

/// A downloaded world: the file, deleted when this is dropped, and what the core said it
/// left out.
#[derive(Debug)]
pub struct Downloaded {
    file: PathBuf,
    pub exported: ExportedBackup,
    /// The backup downloaded.
    pub backup: String,
}

impl Downloaded {
    /// The `.lodeimg` file, until this is dropped.
    pub fn path(&self) -> &Path {
        &self.file
    }

    /// The file at `file` as if downloaded, with what the core said it left out (deleted
    /// when dropped like any): for the offscreen screenshots.
    pub fn preview(file: PathBuf, exported: ExportedBackup) -> Downloaded {
        Downloaded {
            file,
            exported,
            backup: String::new(),
        }
    }
}

impl Drop for Downloaded {
    /// Deletes the file from a small thread: the last holder may be a window's thread (a
    /// dialog closed just as the download ended), and unlinking a world-sized file can
    /// take a while on some disks.
    fn drop(&mut self) {
        let file = std::mem::take(&mut self.file);
        if std::thread::Builder::new()
            .name("lodestone-world-download-delete".into())
            .spawn({
                let file = file.clone();
                move || {
                    let _ = std::fs::remove_file(file);
                }
            })
            .is_err()
        {
            let _ = std::fs::remove_file(file);
        }
    }
}

/// Downloads the server `server`'s backup `which` as a `.lodeimg` into `dir` (see the
/// module). A new backup is waited for by reading the core's event buffer (with `sleep`).
/// Cancel stops the backup's wait, the core's writing or the download; the core's file is
/// let go of however the download ends, and nothing is left in `dir`.
pub async fn download(
    admin: &AdminClient,
    server: &str,
    which: WhichBackup,
    dir: &Path,
    progress: &DownloadProgress,
    cancel: &CancelToken,
    sleep: &Sleep,
) -> Result<Downloaded, WorldDownloadError> {
    let backup = match which {
        WhichBackup::Id(id) => id,
        WhichBackup::Newest => backups(admin, server)
            .await?
            .into_iter()
            .next()
            .map(|b| b.id)
            .ok_or(WorldDownloadError::NoBackups)?,
        WhichBackup::New => {
            progress.set(DownloadStage::BackingUp);
            let made = new_backup(admin, server, cancel, sleep).await?;
            *progress.made.lock().unwrap_or_else(|e| e.into_inner()) = Some(made.clone());
            made
        }
    };
    if cancel.is_cancelled() {
        return Err(WorldDownloadError::Cancelled);
    }
    progress.set(DownloadStage::Preparing);
    std::fs::create_dir_all(dir).map_err(|e| {
        WorldDownloadError::Core(CoreError::Invalid(format!("{}: {e}", dir.display())))
    })?;
    let ready = admin
        .export_backup_ready(server, &backup, false, cancel)
        .await
        .map_err(|err| match err {
            _ if cancel.is_cancelled() => WorldDownloadError::Cancelled,
            err => export_error(err),
        })?;
    // The core says the size first: a world that won't fit never starts filling the disk
    // (a game saving in another instance would fail then), and the core's file goes. The
    // import that follows needs at least as much again for the files it places while the
    // file is still here, so a disk with room for the file alone is refused now, not after
    // the whole download (the import checks the exact room again). The download is then
    // held to the stated size (`download_export`).
    let needed = ready
        .size
        .saturating_mul(2)
        .saturating_add(crate::images::recorder::SPARE_BYTES);
    if let Some(free) = crate::disk::free_space(dir)
        && needed > free
    {
        let _ = admin.release_download(&ready.key).await;
        return Err(WorldDownloadError::NoRoom { needed, free });
    }
    let random = u64::from_le_bytes(crate::random_bytes());
    let file = dir.join(format!("{random:016x}.lodeimg"));
    let exported = admin
        .download_export(ready, &file, &progress.bytes, cancel)
        .await;
    match exported {
        Ok(exported) => {
            progress.set(DownloadStage::Downloading);
            Ok(Downloaded {
                file,
                exported,
                backup,
            })
        }
        Err(_) if cancel.is_cancelled() => {
            let _ = std::fs::remove_file(&file);
            Err(WorldDownloadError::Cancelled)
        }
        Err(err) => {
            let _ = std::fs::remove_file(&file);
            Err(err.into())
        }
    }
}

/// The export route's refusal: a 404 with no message of the core's own (no such route) or
/// a 405 is a core from before milestone 6; a 404 the core explains ("Backup not found",
/// a server gone) is its own message.
fn export_error(err: CoreError) -> WorldDownloadError {
    match err {
        CoreError::NotFound(message) if message == super::client::NOT_FOUND => {
            WorldDownloadError::NeedsUpdate
        }
        CoreError::Server { status: 405, .. } => WorldDownloadError::NeedsUpdate,
        other => other.into(),
    }
}

/// Backs the server up now and waits for the backup to end; returns its id.
async fn new_backup(
    admin: &AdminClient,
    server: &str,
    cancel: &CancelToken,
    sleep: &Sleep,
) -> Result<String, WorldDownloadError> {
    let task = admin
        .create_backup(server, None)
        .await
        .map_err(|err| match err {
            CoreError::Forbidden(_) => WorldDownloadError::BackupForbidden,
            other => other.into(),
        })?;
    let awaited = Awaited {
        id: Some(task),
        start: &|_| false,
        end: &|_| false,
    };
    match admin
        .wait_progression(&awaited, &|_| {}, cancel, sleep, BACKUP_WAIT)
        .await
    {
        Ok(Waited::Ended(ended)) => match ended.value {
            Some(ProgressionEndValue::BackupCompleted {
                backup_id: Some(id),
                success: true,
                ..
            }) => Ok(id),
            Some(ProgressionEndValue::BackupCompleted { message, .. }) => {
                Err(WorldDownloadError::BackupFailed(message))
            }
            _ => Err(WorldDownloadError::BackupFailed(
                ended.message.unwrap_or_default(),
            )),
        },
        // the job's id is known, so it is never unseen; the newest backup is then it
        Ok(Waited::Unseen) => backups(admin, server)
            .await?
            .into_iter()
            .next()
            .map(|b| b.id)
            .ok_or(WorldDownloadError::NoBackups),
        Err(WaitError::Cancelled) => Err(WorldDownloadError::Cancelled),
        Err(WaitError::TimedOut(_)) => Err(WorldDownloadError::BackupFailed(
            "it is still running".into(),
        )),
        Err(WaitError::Core(err)) => Err(err.into()),
    }
}
