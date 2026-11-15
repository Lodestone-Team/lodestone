//! Uploaded `.lodeimg` files, kept for an hour so `POST /instance/create_from_image` can
//! make a server from one by its `upload_id`, as the modpack uploads are
//! (`modpack::store`). Only the user who uploaded a file may use or discard it.
//!
//! Each file can be tens of gigabytes, so they are deleted once their hour is up whether
//! or not anyone uploads again ([`Uploads::prune_periodically`]), at most
//! [`MAX_RECEIVING`] arrive at once ([`MAX_PER_USER`] of one user's), and each one arriving
//! reserves the size its request declared among the core's large writes, so it never
//! passes the free-space check counting on the same space as another upload, a server
//! being made from a file or a file written for download ([`Receiving`]). Each user keeps at most [`MAX_PER_USER`]
//! uploads, their own oldest going first: no user's uploads push out another's. Files are
//! read one at a time ([`Uploads::reading`]), servers made from them one at a time
//! ([`Uploads::placing`]) and at most one from each upload ([`Uploads::take`]), and a file
//! let go of is deleted off the lock and the async threads.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use color_eyre::eyre::Context;

use super::inspect::ImageSummary;
use crate::auth::user_id::UserId;
use crate::error::{Error, ErrorKind};
use crate::util::disk::{LargeWrites, Reserved};

/// How long an upload is kept.
pub const UPLOAD_TTL: Duration = Duration::from_secs(60 * 60);
/// At most this many uploads of one user are kept, and arrive at once (each can be
/// gigabytes); their oldest goes first.
pub const MAX_PER_USER: usize = 2;
/// The largest file taken: the export container's own limit on what a file unpacks to.
pub const MAX_IMAGE_UPLOAD_BYTES: u64 = 64 << 30;
/// At most this many uploads arrive at once, whoever sends them.
pub const MAX_RECEIVING: usize = 4;
/// How often uploads past their hour are deleted.
pub const PRUNE_EVERY: Duration = Duration::from_secs(5 * 60);

/// An uploaded file. It is deleted when the last clone is dropped, so a file a server is
/// being made from stays until that is done.
#[derive(Clone)]
pub struct StoredImage {
    pub file: Arc<tempfile::TempPath>,
    /// The name it was uploaded with.
    pub file_name: String,
    pub uploader: UserId,
    /// What it holds, for the uploader.
    pub summary: ImageSummary,
    created: Instant,
    seq: u64,
    /// Set while a server is being made from it ([`Uploads::take`]).
    in_use: Arc<AtomicBool>,
}

impl StoredImage {
    pub fn path(&self) -> PathBuf {
        self.file.to_path_buf()
    }
}

pub struct Uploads {
    dir: PathBuf,
    uploads: Mutex<HashMap<String, StoredImage>>,
    next_seq: std::sync::atomic::AtomicU64,
    /// The uploads arriving now, by a number of their own, with who sends each.
    receiving: Mutex<HashMap<u64, UserId>>,
    /// The core's large writes, which an arriving upload reserves its size among.
    large_writes: Arc<LargeWrites>,
    /// Held while an upload is read and checked ([`Uploads::reading`]).
    reading: Arc<tokio::sync::Semaphore>,
    /// Held while a server's files are placed from an upload ([`Uploads::placing`]).
    placing: tokio::sync::Semaphore,
}

/// An upload arriving ([`Uploads::start`]): counted against [`MAX_RECEIVING`] and holding
/// what is still to arrive of its declared size among the core's large writes until it is
/// dropped.
pub struct Receiving {
    uploads: Arc<Uploads>,
    id: u64,
    declared: u64,
    space: Reserved,
}

impl Receiving {
    /// Says `bytes` of it have arrived so far.
    pub fn wrote(&self, bytes: u64) {
        self.space
            .set_remaining(self.declared.saturating_sub(bytes));
    }

    /// What the other large writes (uploads arriving, servers being made from files,
    /// files written for download) are still to write: the free space this one may count
    /// on is the disk's less that.
    pub fn others_remaining(&self) -> u64 {
        self.space.others()
    }
}

impl Drop for Receiving {
    fn drop(&mut self) {
        lock(&self.uploads.receiving).remove(&self.id);
    }
}

/// A server being made from an upload ([`Uploads::take`]); the upload is free again once
/// this is dropped, however the creation ended.
pub struct InUse(Arc<AtomicBool>);

impl Drop for InUse {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Uploads {
    /// Uploads in `dir`, reserving their sizes among `large_writes`. Files an earlier run
    /// left there are removed in the background ([`super::clear_leftovers`]).
    pub fn new(dir: PathBuf, large_writes: Arc<LargeWrites>) -> Uploads {
        super::clear_leftovers(&dir);
        Uploads {
            dir,
            uploads: Mutex::new(HashMap::new()),
            next_seq: Default::default(),
            receiving: Mutex::new(HashMap::new()),
            large_writes,
            reading: Arc::new(tokio::sync::Semaphore::new(1)),
            placing: tokio::sync::Semaphore::new(1),
        }
    }

    /// Waits until no other upload is being read, and holds that until the permit is
    /// dropped. Reading a file holds its manifests in memory several times over (up to
    /// 512 MiB each, a million entries), so files are read one at a time, never four at
    /// once from users sending small files that unpack to huge manifests.
    ///
    /// The permit is owned so it can go into the blocking read with the file: a request
    /// dropped mid-read (the client hung up) drops its future, but the read goes on to its
    /// end, and the next file must wait for that, not for the request.
    pub async fn reading(&self) -> tokio::sync::OwnedSemaphorePermit {
        self.reading
            .clone()
            .acquire_owned()
            .await
            .expect("the semaphore is never closed")
    }

    /// Waits until no other server is being made from an upload, and holds that until the
    /// permit is dropped. Placing a file's files unpacks, downloads and writes gigabytes,
    /// each step checking the free space first; two at once would both count on the same
    /// space and fill the disk under the running servers.
    pub async fn placing(&self) -> tokio::sync::SemaphorePermit<'_> {
        self.placing
            .acquire()
            .await
            .expect("the semaphore is never closed")
    }

    /// Starts receiving `uploader`'s upload that says it is `declared` bytes (0 when it
    /// doesn't say). Refused while [`MAX_RECEIVING`] others arrive, or [`MAX_PER_USER`] of
    /// the uploader's.
    pub fn start(self: &Arc<Self>, uploader: &UserId, declared: u64) -> Result<Receiving, Error> {
        let mut receiving = lock(&self.receiving);
        let theirs = receiving.values().filter(|who| *who == uploader).count();
        if theirs >= MAX_PER_USER {
            return Err(Error {
                kind: ErrorKind::Conflict,
                source: color_eyre::eyre::eyre!(
                    "You are uploading {theirs} files already; try again when one is done"
                ),
            });
        }
        if receiving.len() >= MAX_RECEIVING {
            return Err(Error {
                kind: ErrorKind::Conflict,
                source: color_eyre::eyre::eyre!(
                    "{} files are being uploaded already; try again when one is done",
                    receiving.len()
                ),
            });
        }
        let id = self
            .next_seq
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        receiving.insert(id, uploader.clone());
        Ok(Receiving {
            uploads: self.clone(),
            id,
            declared,
            space: self.large_writes.reserve(declared),
        })
    }

    /// Forgets the uploads past their hour; their files go once no server is being made
    /// from them.
    pub fn prune_expired(&self) {
        let removed: Vec<StoredImage> = {
            let mut uploads = lock(&self.uploads);
            let expired: Vec<String> = uploads
                .iter()
                .filter(|(_, u)| u.created.elapsed() >= UPLOAD_TTL)
                .map(|(id, _)| id.clone())
                .collect();
            expired.iter().filter_map(|id| uploads.remove(id)).collect()
        };
        // the files are deleted here, outside the lock (the caller's thread may block)
        drop(removed);
    }

    /// Lets go of uploads taken out of the map, outside its lock and, in the core, on a
    /// thread that may block: dropping the last clone deletes a file of up to 64 GB, which
    /// can take seconds on some disks.
    fn discard(removed: Vec<StoredImage>) {
        if removed.is_empty() {
            return;
        }
        match tokio::runtime::Handle::try_current() {
            Ok(runtime) => {
                runtime.spawn_blocking(move || drop(removed));
            }
            Err(_) => drop(removed),
        }
    }

    /// Deletes uploads past their hour every [`PRUNE_EVERY`], so a file whose uploader
    /// went away doesn't keep its space until the next upload. Runs until the core stops.
    pub async fn prune_periodically(self: Arc<Self>) {
        let mut interval = tokio::time::interval(PRUNE_EVERY);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            interval.tick().await;
            let uploads = self.clone();
            let _ = tokio::task::spawn_blocking(move || uploads.prune_expired()).await;
        }
    }

    /// The folder uploads stream into: its disk's free space is checked as they do.
    pub fn dir(&self) -> &std::path::Path {
        &self.dir
    }

    /// A new empty file for an upload, deleted when dropped unless it is stored.
    pub fn new_file(&self) -> Result<tempfile::TempPath, Error> {
        std::fs::create_dir_all(&self.dir).context("Failed to create the upload folder")?;
        Ok(tempfile::Builder::new()
            .prefix("image-")
            .suffix(&format!(".{}", super::EXTENSION))
            .tempfile_in(&self.dir)
            .context("Failed to create a file for the upload")?
            .into_temp_path())
    }

    /// Takes the uploads past their hour out of `uploads` into `removed`.
    fn prune(uploads: &mut HashMap<String, StoredImage>, removed: &mut Vec<StoredImage>) {
        let expired: Vec<String> = uploads
            .iter()
            .filter(|(_, u)| u.created.elapsed() >= UPLOAD_TTL)
            .map(|(id, _)| id.clone())
            .collect();
        removed.extend(expired.iter().filter_map(|id| uploads.remove(id)));
    }

    /// Takes `uploader`'s oldest uploads past [`MAX_PER_USER`] out of `uploads` into
    /// `removed`.
    fn prune_users(
        uploads: &mut HashMap<String, StoredImage>,
        uploader: &UserId,
        removed: &mut Vec<StoredImage>,
    ) {
        loop {
            let theirs: Vec<(&String, u64)> = uploads
                .iter()
                .filter(|(_, u)| u.uploader == *uploader)
                .map(|(id, u)| (id, u.seq))
                .collect();
            if theirs.len() <= MAX_PER_USER {
                break;
            }
            let Some(oldest) = theirs
                .iter()
                .min_by_key(|(_, seq)| *seq)
                .map(|(id, _)| (*id).clone())
            else {
                break;
            };
            removed.extend(uploads.remove(&oldest));
        }
    }

    /// Makes the upload `id` as old as `age`, for tests of its expiry.
    #[cfg(test)]
    pub fn age_for_test(&self, id: &str, age: Duration) {
        if let Some(upload) = lock(&self.uploads).get_mut(id) {
            upload.created = Instant::now().checked_sub(age).unwrap_or_else(Instant::now);
        }
    }

    /// Keeps an upload and returns its `upload_id`.
    pub fn insert(
        &self,
        file: tempfile::TempPath,
        file_name: String,
        uploader: UserId,
        summary: ImageSummary,
    ) -> String {
        let id = crate::util::rand_alphanumeric(32);
        let mut removed = Vec::new();
        let mut uploads = self.uploads.lock().unwrap_or_else(|e| e.into_inner());
        let who = uploader.clone();
        uploads.insert(
            id.clone(),
            StoredImage {
                file: Arc::new(file),
                file_name,
                uploader,
                summary,
                created: Instant::now(),
                seq: self
                    .next_seq
                    .fetch_add(1, std::sync::atomic::Ordering::SeqCst),
                in_use: Default::default(),
            },
        );
        Self::prune(&mut uploads, &mut removed);
        Self::prune_users(&mut uploads, &who, &mut removed);
        drop(uploads);
        Self::discard(removed);
        id
    }

    /// The upload `id`, when `user` uploaded it.
    pub fn get(&self, id: &str, user: &UserId) -> Option<StoredImage> {
        let mut removed = Vec::new();
        let found = {
            let mut uploads = self.uploads.lock().unwrap_or_else(|e| e.into_inner());
            Self::prune(&mut uploads, &mut removed);
            uploads.get(id).filter(|u| u.uploader == *user).cloned()
        };
        Self::discard(removed);
        found
    }

    /// The upload `id`, when `user` uploaded it, for a server to be made from it: refused
    /// while one is being made from it already, until the returned [`InUse`] is dropped.
    /// Each creation from a file holds the file's manifests and plan in memory until its
    /// turn to place them comes, so a client sending the same upload again and again
    /// mustn't queue any number of them.
    pub fn take(&self, id: &str, user: &UserId) -> Result<(StoredImage, InUse), Error> {
        let upload = self.get(id, user).ok_or_else(|| Error {
            kind: ErrorKind::NotFound,
            source: color_eyre::eyre::eyre!(
                "The file was not found. Uploads are kept for one hour; open it again"
            ),
        })?;
        if upload.in_use.swap(true, Ordering::SeqCst) {
            return Err(Error {
                kind: ErrorKind::Conflict,
                source: color_eyre::eyre::eyre!("A server is already being made from this file"),
            });
        }
        let in_use = InUse(upload.in_use.clone());
        Ok((upload, in_use))
    }

    /// Forgets the upload `id` when `user` uploaded it (its file goes once no server is
    /// being made from it). Whether there was one.
    pub fn remove(&self, id: &str, user: &UserId) -> bool {
        let mut removed = Vec::new();
        let found = {
            let mut uploads = self.uploads.lock().unwrap_or_else(|e| e.into_inner());
            Self::prune(&mut uploads, &mut removed);
            match uploads.get(id) {
                Some(upload) if upload.uploader == *user => {
                    removed.extend(uploads.remove(id));
                    true
                }
                _ => false,
            }
        };
        Self::discard(removed);
        found
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(n: usize) -> UserId {
        UserId::from(format!("user{n}"))
    }

    #[test]
    fn uploads_arriving_are_counted_and_reserve_their_size() {
        let dir = tempfile::tempdir().unwrap();
        let uploads = Arc::new(Uploads::new(dir.path().join("uploads"), Default::default()));
        let first = uploads.start(&user(0), 1000).unwrap();
        let second = uploads.start(&user(1), 300).unwrap();
        assert_eq!(first.others_remaining(), 300);
        assert_eq!(second.others_remaining(), 1000);
        first.wrote(400);
        assert_eq!(second.others_remaining(), 600);
        let mut more: Vec<Receiving> = (2..MAX_RECEIVING)
            .map(|n| uploads.start(&user(n), 0).unwrap())
            .collect();
        let refused = uploads.start(&user(99), 0).err().unwrap();
        assert!(matches!(refused.kind, ErrorKind::Conflict));
        // one done: another may start, and its reservation is gone
        drop(first);
        assert_eq!(second.others_remaining(), 0);
        more.push(uploads.start(&user(98), 0).unwrap());
    }

    #[tokio::test]
    async fn a_read_holds_its_turn_after_its_request_is_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let uploads = Arc::new(Uploads::new(dir.path().join("uploads"), Default::default()));
        let (started_tx, started_rx) = std::sync::mpsc::channel::<()>();
        let (finish_tx, finish_rx) = std::sync::mpsc::channel::<()>();
        let request = {
            let uploads = uploads.clone();
            tokio::spawn(async move {
                let receiving = uploads.start(&user(1), 0).unwrap();
                let reading = uploads.reading().await;
                // as the handler does: the turn and the slot go into the blocking read
                crate::images::blocking(move || {
                    let _hold = (reading, receiving);
                    started_tx.send(()).unwrap();
                    finish_rx.recv().unwrap();
                    Ok(())
                })
                .await
            })
        };
        tokio::task::spawn_blocking(move || started_rx.recv().unwrap())
            .await
            .unwrap();
        // the client hangs up: the request's future is dropped, the read goes on
        request.abort();
        let _ = request.await;
        let waited = tokio::time::timeout(Duration::from_millis(100), uploads.reading()).await;
        assert!(
            waited.is_err(),
            "the next read waits for the one still running"
        );
        let second = uploads
            .start(&user(1), 0)
            .expect("one of the user's slots is free");
        assert!(
            uploads.start(&user(1), 0).is_err(),
            "the read still holds its upload's slot"
        );
        drop(second);
        finish_tx.send(()).unwrap();
        let turn = tokio::time::timeout(Duration::from_secs(10), uploads.reading())
            .await
            .expect("the turn is free once the read ended");
        drop(turn);
    }

    #[test]
    fn one_user_can_neither_hold_every_slot_nor_push_out_anothers_upload() {
        let dir = tempfile::tempdir().unwrap();
        let uploads = Arc::new(Uploads::new(dir.path().join("uploads"), Default::default()));
        let (a, b) = (user(1), user(2));
        let held: Vec<Receiving> = (0..MAX_PER_USER)
            .map(|_| uploads.start(&b, 0).unwrap())
            .collect();
        let refused = uploads.start(&b, 0).err().unwrap();
        assert!(matches!(refused.kind, ErrorKind::Conflict));
        // another user still gets a slot
        drop(uploads.start(&a, 0).unwrap());
        drop(held);

        let summary = || -> ImageSummary {
            serde_json::from_value(serde_json::json!({
                "image": "x", "file_name": "x", "label": null, "made_by": "host",
                "producer_version": "1", "created": 0, "minecraft": "1.21.1",
                "loader": "vanilla", "loader_version": null, "flavour": null, "server": null,
                "server_choices": [], "unsupported": null, "server_note": null, "base": null,
                "mods_from_modrinth": {"names": [], "count": 0},
                "mods_from_curseforge": {"names": [], "count": 0},
                "carried_mods": {"names": [], "count": 0},
                "client_only": {"names": [], "count": 0}, "server_mods": 0, "worlds": [],
                "default_world": null, "files": 0, "total_bytes": 0, "download_bytes": 0,
                "unpacked_bytes": 0, "needed_bytes": 0, "disk_free": null,
                "not_downloadable": {"names": [], "count": 0},
                "left_out": {"names": [], "count": 0}, "withheld": {"names": [], "count": 0},
                "blanked": {"names": [], "count": 0},
                "secrets_not_taken": {"names": [], "count": 0}, "not_taken": [], "operators": {"names": [], "count": 0},
                "offline_mode": false, "needs_global_files": false, "scripts_skipped": 0
            }))
            .unwrap()
        };
        let keep = |who: &UserId| {
            uploads.insert(
                uploads.new_file().unwrap(),
                "f".into(),
                who.clone(),
                summary(),
            )
        };
        let a_first = keep(&a);
        let b_ids: Vec<String> = (0..MAX_PER_USER + 3).map(|_| keep(&b)).collect();
        assert!(
            uploads.get(&a_first, &a).is_some(),
            "B's uploads never push out A's"
        );
        let kept: Vec<&String> = b_ids
            .iter()
            .filter(|id| uploads.get(id, &b).is_some())
            .collect();
        assert_eq!(
            kept,
            b_ids[b_ids.len() - MAX_PER_USER..]
                .iter()
                .collect::<Vec<_>>()
        );
        // one server at a time from an upload, and only by its uploader
        let (_, in_use) = uploads.take(&a_first, &a).unwrap();
        let busy = uploads.take(&a_first, &a).err().unwrap();
        assert!(matches!(busy.kind, ErrorKind::Conflict));
        let theirs = uploads.take(&a_first, &b).err().unwrap();
        assert!(matches!(theirs.kind, ErrorKind::NotFound));
        drop(in_use);
        assert!(uploads.take(&a_first, &a).is_ok());
    }
}
