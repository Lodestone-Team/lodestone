//! Streaming one object out of the store to a player (`GET …/objects/{sha256}`, see
//! [`super::serve`] for which objects a player may fetch).
//!
//! A player always gets the file itself: a large file (over 1 MiB, a chunk list in the
//! store) is put together from its chunks as it is sent, so chunk lists never leave the
//! server. A download is a task of its own that reads at most [`UNIT`] at a time, each read
//! on a blocking thread of its own under its own shared hold on the store's collection lock
//! (`Store::hold_off_collection`), and hands the bytes to the connection in pieces of at
//! most [`PIECE`]: a slow player holds no thread while it reads, and a collection waits for
//! at most one read, never for a whole download. Every byte is checked before it is sent:
//! a whole object as it is read, a chunk against its own hash, and on a full download of a
//! large file the whole file's SHA-256 at the end. A check that fails ends the stream with
//! an error, which aborts the response (the player sees a body cut short and fetches it
//! again), and the damaged object is deleted (for a large file, the damaged or missing
//! chunk and the file's chunk list), so the next request for it gets a 503 and starts a
//! record of the server's content, which stores the file again from its disk.
//!
//! Downloads are limited ([`Downloads`]): at most [`MAX_OBJECT_STREAMS`] at once over every
//! server and route, at most [`MAX_OBJECT_STREAMS_PER_SERVER`] of one server, at most
//! [`MAX_OBJECT_STREAMS_PER_CLIENT`] of one client where the route can tell clients apart
//! ([`ClientKey`]), and a download whose player takes no piece for [`IDLE_TIMEOUT`] ends
//! (cut short, as a failed check does), giving its place back. So a client that opens
//! downloads and stops reading holds them for a minute at most, and one client can't take
//! a server's every place, let alone every server's.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv6Addr};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::Duration;

use axum::body::Bytes;
use futures::Stream;
use lodestone_image::hex::Sha256Hasher;
use lodestone_image::store::chunklist::{ChunkList, CHUNKED_ABOVE};
use lodestone_image::store::{ObjectForm, Store, StoreError};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_stream::wrappers::ReceiverStream;
use tracing::{debug, warn};

use crate::types::InstanceUuid;

/// The most object downloads that run at once, over every server and route.
pub const MAX_OBJECT_STREAMS: usize = 64;
/// The most object downloads of one server that run at once, so clients that hold
/// downloads open can only crowd out that server's players.
pub const MAX_OBJECT_STREAMS_PER_SERVER: usize = 16;
/// The most object downloads one client runs at once, over every server ([`ClientKey`]). A
/// player runs at most 4; this leaves room for a few behind one address (a household's
/// router), while one client alone can't hold a server's every place.
pub const MAX_OBJECT_STREAMS_PER_CLIENT: usize = 8;
/// How long a download waits for its player to take the next piece before it ends. With
/// pieces of [`PIECE`], a player must read at least about 4 KiB/s.
pub const IDLE_TIMEOUT: Duration = Duration::from_secs(60);
/// The most bytes one step of a stream reads (and holds collections off for).
const UNIT: usize = 1 << 20;
/// The most bytes handed to the connection at once.
const PIECE: usize = 256 << 10;

/// Who a download is for, as far as its route can tell (see [`Downloads`]). The main
/// port's routes know the peer's address; a friend tunnel knows the friend. A Connect
/// invite's info port knows neither (every player of an invite comes through the same
/// streams), so its downloads have only the per-server and overall limits.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum ClientKey {
    /// An IPv4 address, or an IPv6 address's /64 (one household or host is given a whole
    /// /64, so its addresses count as one).
    Address(IpAddr),
    /// A friend, by their key.
    Friend([u8; 32]),
}

impl ClientKey {
    pub(crate) fn of_address(address: IpAddr) -> ClientKey {
        match address.to_canonical() {
            IpAddr::V6(v6) => {
                let prefix = u128::from(v6) & !((1u128 << 64) - 1);
                ClientKey::Address(IpAddr::V6(Ipv6Addr::from(prefix)))
            }
            v4 => ClientKey::Address(v4),
        }
    }
}

/// Which downloads may start ([`MAX_OBJECT_STREAMS`], [`MAX_OBJECT_STREAMS_PER_SERVER`],
/// [`MAX_OBJECT_STREAMS_PER_CLIENT`]).
pub(crate) struct Downloads {
    all: Arc<Semaphore>,
    servers: StdMutex<HashMap<InstanceUuid, Arc<Semaphore>>>,
    /// How many downloads each client runs; a client with none has no entry, so the map
    /// only ever holds clients that are downloading.
    clients: Arc<StdMutex<HashMap<ClientKey, usize>>>,
}

impl Default for Downloads {
    fn default() -> Downloads {
        Downloads {
            all: Arc::new(Semaphore::new(MAX_OBJECT_STREAMS)),
            servers: Default::default(),
            clients: Default::default(),
        }
    }
}

/// A running download's places: one of its client's (when the route knows the client), one
/// of its server's and one of all.
pub(crate) struct DownloadPermit {
    _client: Option<ClientPlace>,
    _server: OwnedSemaphorePermit,
    _all: OwnedSemaphorePermit,
}

/// One of a client's places, given back when dropped.
struct ClientPlace {
    clients: Arc<StdMutex<HashMap<ClientKey, usize>>>,
    key: ClientKey,
}

impl Drop for ClientPlace {
    fn drop(&mut self) {
        let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(running) = clients.get_mut(&self.key) {
            *running -= 1;
            if *running == 0 {
                clients.remove(&self.key);
            }
        }
    }
}

impl Downloads {
    /// A place for one more download of `uuid`'s objects for `client` (`None`: a route that
    /// can't tell clients apart), or `None` while any limit is reached.
    pub(crate) fn try_start(
        &self,
        uuid: &InstanceUuid,
        client: Option<&ClientKey>,
    ) -> Option<DownloadPermit> {
        let client = match client {
            Some(key) => {
                let mut clients = self.clients.lock().unwrap_or_else(|e| e.into_inner());
                let running = clients.entry(key.clone()).or_insert(0);
                if *running >= MAX_OBJECT_STREAMS_PER_CLIENT {
                    return None;
                }
                *running += 1;
                Some(ClientPlace {
                    clients: self.clients.clone(),
                    key: key.clone(),
                })
            }
            None => None,
        };
        let server = self
            .servers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(uuid.clone())
            .or_insert_with(|| Arc::new(Semaphore::new(MAX_OBJECT_STREAMS_PER_SERVER)))
            .clone();
        let server = server.try_acquire_owned().ok()?;
        let all = self.all.clone().try_acquire_owned().ok()?;
        Some(DownloadPermit {
            _client: client,
            _server: server,
            _all: all,
        })
    }

    /// Forgets a deleted server's limit (its running downloads keep their places).
    pub(crate) fn forget(&self, uuid: &InstanceUuid) {
        self.servers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(uuid);
    }

    /// How many downloads run, over every server.
    #[cfg(test)]
    pub(crate) fn running(&self) -> usize {
        MAX_OBJECT_STREAMS - self.all.available_permits()
    }
}

/// A byte range of a file, both ends included, within its size.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ByteRange {
    pub start: u64,
    pub end: u64,
}

/// Where a stream is.
struct Progress {
    store: Arc<Store>,
    sha256: String,
    size: u64,
    /// The bytes to send, both ends included; `None` for a file of 0 bytes.
    range: Option<ByteRange>,
    /// The whole file is sent, so its SHA-256 is checked at the end.
    full: bool,
    /// The large file's chunks, once read.
    list: Option<ChunkList>,
    /// The next chunk to read, and where in the file it starts.
    next: usize,
    offset: u64,
    hasher: Sha256Hasher,
    done: bool,
}

/// The bytes of the object `sha256` (a file of `size` bytes), or of `range` of it, as a
/// response body stream (see the module). `permit` is held until the download ends: sent,
/// failed, given up after `idle` without the player taking a piece, or dropped.
pub(crate) fn object_stream(
    store: Arc<Store>,
    sha256: String,
    size: u64,
    range: Option<ByteRange>,
    permit: DownloadPermit,
    idle: Duration,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Send {
    let full = range.is_none();
    let range = range.or_else(|| {
        (size > 0).then(|| ByteRange {
            start: 0,
            end: size - 1,
        })
    });
    let progress = Progress {
        store,
        sha256,
        size,
        range,
        full,
        list: None,
        next: 0,
        offset: 0,
        hasher: Sha256Hasher::new(),
        done: false,
    };
    // Room for one piece: the task reads ahead at most one unit.
    let (tx, rx) = tokio::sync::mpsc::channel(1);
    tokio::spawn(async move {
        let _permit = permit;
        let mut progress = progress;
        loop {
            let stepped = tokio::task::spawn_blocking(move || {
                let bytes = progress.step();
                (bytes, progress)
            })
            .await;
            let (bytes, back) = match stepped {
                Ok(stepped) => stepped,
                Err(e) => {
                    let failed = std::io::Error::other(format!("A background task failed: {e}"));
                    let _ = tokio::time::timeout(idle, tx.send(Err(failed))).await;
                    return;
                }
            };
            progress = back;
            let bytes = match bytes {
                Ok(bytes) => bytes,
                Err(e) => {
                    let _ = tokio::time::timeout(idle, tx.send(Err(e))).await;
                    return;
                }
            };
            for piece in bytes.chunks(PIECE) {
                // A copy of its own, so a connection that stalls holds one piece, not the
                // whole unit it was cut from.
                let piece = Bytes::copy_from_slice(piece);
                match tokio::time::timeout(idle, tx.send(Ok(piece))).await {
                    Ok(Ok(())) => {}
                    // The player went away.
                    Ok(Err(_)) => return,
                    Err(_) => {
                        debug!(
                            "A player took nothing of {} for {idle:?}; ending the download",
                            progress.sha256
                        );
                        return;
                    }
                }
            }
            if progress.done {
                return;
            }
        }
    });
    ReceiverStream::new(rx)
}

impl Progress {
    /// Reads the next unit, holding collections off while it does. Blocking.
    fn step(&mut self) -> Result<Vec<u8>, std::io::Error> {
        let result = (|| {
            let _hold = self.store.hold_off_collection()?;
            if self.list.is_none() {
                match self.store.object_form(&self.sha256, self.size)? {
                    ObjectForm::Whole => return self.whole(),
                    ObjectForm::Chunked(list) => self.list = Some(list),
                }
            }
            self.chunks()
        })();
        result.map_err(|e| {
            self.done = true;
            self.failed(e)
        })
    }

    /// A small object, read whole (checked), and the part of it asked for.
    fn whole(&mut self) -> Result<Vec<u8>, StoreError> {
        self.done = true;
        if self.size > CHUNKED_ABOVE {
            // The store keeps every larger file as a chunk list: this can't be read in
            // pieces, and reading it whole could take any amount of memory.
            return Err(StoreError::Damaged(self.sha256.clone()));
        }
        let bytes = self.store.read_object_bytes(&self.sha256, self.size)?;
        Ok(match self.range {
            Some(range) => bytes[range.start as usize..=range.end as usize].to_vec(),
            None => Vec::new(),
        })
    }

    /// The next chunks of a large file, up to [`UNIT`] bytes of the range.
    fn chunks(&mut self) -> Result<Vec<u8>, StoreError> {
        let (Some(list), Some(range)) = (&self.list, self.range) else {
            self.done = true;
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        while self.next < list.chunks.len() && self.offset <= range.end && out.len() < UNIT {
            let chunk = &list.chunks[self.next];
            let len = u64::from(chunk.size);
            let chunk_end = self.offset + len;
            if chunk_end <= range.start {
                // Before the range: skipped unread (only a range skips any).
                self.offset = chunk_end;
                self.next += 1;
                continue;
            }
            let bytes = self
                .store
                .read_chunk(chunk)
                .inspect_err(|e| lost_chunk(&self.store, &self.sha256, self.size, chunk, e))?;
            if self.full {
                self.hasher.update(&bytes);
            }
            let from = range.start.saturating_sub(self.offset) as usize;
            let to = (range.end.min(chunk_end - 1) - self.offset) as usize;
            out.extend_from_slice(&bytes[from..=to]);
            self.offset = chunk_end;
            self.next += 1;
        }
        if self.next >= list.chunks.len() || self.offset > range.end {
            self.done = true;
            if self.full {
                let digest = std::mem::take(&mut self.hasher).finish_hex();
                if digest != self.sha256 {
                    return Err(StoreError::Damaged(self.sha256.clone()));
                }
            }
        }
        Ok(out)
    }

    /// The error a stream ends with, after deleting what was found damaged.
    fn failed(&self, e: StoreError) -> std::io::Error {
        match &e {
            StoreError::Damaged(name) if *name == self.sha256 => {
                warn!("The stored object {} is damaged; deleting it", self.sha256);
                if let Err(e) = self.store.remove_damaged(&self.sha256) {
                    warn!("Failed to delete the damaged object {}: {e}", self.sha256);
                }
            }
            StoreError::Missing(_) => {}
            _ => warn!("Failed to send the object {}: {e}", self.sha256),
        }
        std::io::Error::other(e.to_string())
    }
}

/// Whether the store holds the object `sha256` for a file of `size` bytes, whole or as a
/// sound chunk list: what a download needs before it promises a body. Reads at most the
/// chunk list, not its chunks: a damaged whole object, or a large file that lost a chunk,
/// is only found as it is sent, which then deletes it ([`lost_chunk`]), so the next request
/// gets here `false`. Checking every chunk here would cost an open per chunk on every
/// request (tens of thousands for a file of a few GiB). A damaged one is deleted, so a
/// record stores it again. Blocking.
pub(crate) fn object_there(store: &Store, sha256: &str, size: u64) -> Result<bool, StoreError> {
    let _hold = store.hold_off_collection()?;
    match store.object_form(sha256, size) {
        Ok(_) => Ok(true),
        Err(StoreError::Missing(_) | StoreError::WrongSize { .. }) => Ok(false),
        Err(StoreError::Damaged(_)) => {
            warn!("The stored object {sha256} is damaged; deleting it");
            if let Err(e) = store.remove_damaged(sha256) {
                warn!("Failed to delete the damaged object {sha256}: {e}");
            }
            Ok(false)
        }
        Err(e) => Err(e),
    }
}

/// After a chunk of the large file `sha256` (of `size` bytes) couldn't be read: deletes it
/// when it was damaged, and then the file's chunk list, which can no longer give the file
/// whole (`Store::remove_incomplete`). So the object counts as missing: the next request
/// for it gets the 503 that starts a record ([`object_there`]), and that record, which
/// can't claim it, stores the file again from the server's disk with the lost chunk. A
/// chunk lost some other way (missing) goes the same way.
fn lost_chunk(
    store: &Store,
    sha256: &str,
    size: u64,
    chunk: &lodestone_image::store::chunklist::ChunkRef,
    e: &StoreError,
) {
    match e {
        StoreError::Damaged(_) => {
            warn!("The stored chunk {} is damaged; deleting it", chunk.sha256);
            if let Err(e) = store.remove_damaged_chunk(chunk) {
                warn!("Failed to delete the damaged chunk {}: {e}", chunk.sha256);
            }
        }
        StoreError::Missing(_) => {}
        _ => return,
    }
    match store.remove_incomplete(sha256, size) {
        Ok(true) => warn!("The stored object {sha256} lost a chunk; it will be stored again"),
        Ok(false) => {}
        Err(e) => warn!("Failed to delete the incomplete object {sha256}: {e}"),
    }
}

#[cfg(test)]
#[path = "objects_tests.rs"]
mod tests;
