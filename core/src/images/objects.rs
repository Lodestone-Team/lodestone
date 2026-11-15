//! Streaming one object out of the store to a player (`GET …/objects/{sha256}`, see
//! [`super::serve`] for which objects a player may fetch).
//!
//! A player always gets the file itself: a large file (over 1 MiB, a chunk list in the
//! store) is put together from its chunks as it is sent, so chunk lists never leave the
//! server. The stream reads at most [`UNIT`] at a time, each read on a blocking thread of
//! its own under its own shared hold on the store's collection lock
//! (`Store::hold_off_collection`): a slow player holds no thread while it reads, and a
//! collection waits for at most one read, never for a whole download. Every byte is checked
//! before it is sent: a whole object as it is read, a chunk against its own hash, and on a
//! full download of a large file the whole file's SHA-256 at the end. A check that fails
//! ends the stream with an error, which aborts the response (the player sees a body cut
//! short and fetches it again), and the damaged object or chunk is deleted, so the next
//! record of the server's content stores the file again from its disk.

use std::sync::Arc;

use axum::body::Bytes;
use futures::Stream;
use lodestone_image::store::chunklist::{ChunkList, CHUNKED_ABOVE};
use lodestone_image::store::{ObjectForm, Store, StoreError};
use sha2::{Digest, Sha256};
use tokio::sync::OwnedSemaphorePermit;
use tracing::warn;

/// The most object downloads that run at once, over every server and route.
pub const MAX_OBJECT_STREAMS: usize = 64;
/// The most bytes one step of a stream reads (and holds collections off for).
const UNIT: usize = 1 << 20;

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
    hasher: Sha256,
    done: bool,
    _permit: OwnedSemaphorePermit,
}

/// The bytes of the object `sha256` (a file of `size` bytes), or of `range` of it, as a
/// response body stream (see the module). `permit` is one of [`MAX_OBJECT_STREAMS`],
/// released when the stream ends or is dropped.
pub(crate) fn object_stream(
    store: Arc<Store>,
    sha256: String,
    size: u64,
    range: Option<ByteRange>,
    permit: OwnedSemaphorePermit,
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
        hasher: Sha256::new(),
        done: false,
        _permit: permit,
    };
    futures::stream::try_unfold(progress, |mut progress| async move {
        if progress.done {
            return Ok(None);
        }
        let (bytes, progress) = tokio::task::spawn_blocking(move || {
            let bytes = progress.step();
            (bytes, progress)
        })
        .await
        .map_err(|e| std::io::Error::other(format!("A background task failed: {e}")))?;
        let bytes = bytes?;
        if bytes.is_empty() && progress.done {
            return Ok(None);
        }
        Ok(Some((Bytes::from(bytes), progress)))
    })
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
                .inspect_err(|e| remove_damaged_chunk(&self.store, chunk, e))?;
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
                let digest = std::mem::take(&mut self.hasher).finalize();
                if format!("{digest:x}") != self.sha256 {
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

/// Deletes a chunk found damaged (a missing one is left to the next record).
fn remove_damaged_chunk(
    store: &Store,
    chunk: &lodestone_image::store::chunklist::ChunkRef,
    e: &StoreError,
) {
    if matches!(e, StoreError::Damaged(_)) {
        warn!("The stored chunk {} is damaged; deleting it", chunk.sha256);
        if let Err(e) = store.remove_damaged_chunk(chunk) {
            warn!("Failed to delete the damaged chunk {}: {e}", chunk.sha256);
        }
    }
}

#[cfg(test)]
#[path = "objects_tests.rs"]
mod tests;
