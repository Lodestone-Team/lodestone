//! Image exports: one zip file carrying an image to another computer, and the strict
//! reader that takes one in.
//!
//! ```text
//! image.json            the image, sealed
//! images/<id>.json      the manifests it links to: a data image's content image
//! objects/<sha256>      the plain bytes of every object only this store has (entries with
//!                       source `object`, and the settings' icon and background); for a
//!                       file over 1 MiB, its chunk list
//! chunks/<sha256>       the chunks those lists name
//! ```
//!
//! A file over 1 MiB always travels as its chunk list, even when the store keeps it whole
//! (the core's large files from before chunking): it is cut on the way out exactly as a
//! store cuts it, so the same image gives the same export from any store.
//!
//! Files with any other source travel as references (a Modrinth or CurseForge version, the
//! base pack), so an export of a modded instance is mostly its worlds and configs.
//!
//! An export comes from another computer, so reading one trusts nothing, with the same rules
//! as the Launcher's `.lodesnap` reader: only the names above, no name twice, local headers
//! that match the central directory, at most [`Limits::max_entries`] entries and
//! [`Limits::max_total_bytes`] unpacked, no image whose files add up to more than
//! [`Limits::max_total_bytes`] either (entries can share one object, and a list can repeat
//! one chunk, so a small export could otherwise name an image that fills the disk when it
//! is placed), manifests whose contents hash to their ids, every
//! object an image needs present and no file it doesn't, every declared size what the
//! manifest says, and every byte read checked against its SHA-256 before the store keeps
//! it. Names are only ever looked up, never joined onto a folder, so no entry can reach
//! outside the store. A caller's hook checks free space before anything is unpacked.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use crate::sha256::Sha256;

use crate::hex::{is_sha256, sha256_hex};
use crate::identity::IdentityError;
use crate::manifest::{Image, ManifestError, Source};
use crate::store::cdc::{Chunker, MAX_CHUNK};
use crate::store::chunklist::{ChunkList, ChunkRef, CHUNKED_ABOVE};
use crate::store::{
    hex_digest, manifest_id, Cancel, ObjectForm, Store, StoreError, MAX_MANIFEST_BYTES,
};

/// The image's manifest in an export.
pub const IMAGE_FILE: &str = "image.json";

/// What an export may hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub max_entries: usize,
    /// The sum of the zip entries' declared sizes, and of each image's files' sizes.
    pub max_total_bytes: u64,
    /// Each manifest's size.
    pub max_manifest_bytes: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Limits {
            max_entries: 1_000_000,
            max_total_bytes: 64 << 30,
            max_manifest_bytes: MAX_MANIFEST_BYTES,
        }
    }
}

/// Why an export can't be written or read.
#[derive(Debug, thiserror::Error)]
pub enum ContainerError {
    /// The export breaks a rule; the message says which, for the user.
    #[error("the export isn't valid: {0}")]
    Invalid(String),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("there isn't room to import it: {0}")]
    NoRoom(String),
    #[error("{what}: {source}")]
    Io {
        what: String,
        #[source]
        source: io::Error,
    },
    #[error("cancelled")]
    Cancelled,
}

fn invalid(message: impl Into<String>) -> ContainerError {
    ContainerError::Invalid(message.into())
}

/// The objects `images` carry in an export, by SHA-256 with their sizes: entries whose
/// source is `object`, and the settings' stored files. Errors when a hash isn't one or
/// one object is given two sizes.
pub fn carried_objects(images: &[&Image]) -> Result<BTreeMap<String, u64>, String> {
    let mut carried: BTreeMap<String, u64> = BTreeMap::new();
    let mut add = |sha256: &str, size: u64| -> Result<(), String> {
        if !is_sha256(sha256) {
            return Err(format!("{sha256:?} isn't a SHA-256"));
        }
        match carried.insert(sha256.to_string(), size) {
            Some(other) if other != size => Err(format!(
                "the object {sha256} is listed with two sizes, {other} and {size}"
            )),
            _ => Ok(()),
        }
    };
    for image in images {
        for entry in &image.entries {
            if entry.source == (Source::Object {}) {
                add(&entry.sha256, entry.size)?;
            }
        }
        let client = image.settings.client.as_ref();
        let stored_files = client
            .into_iter()
            .flat_map(|c| c.icon.iter().chain(c.background.iter()));
        for stored in stored_files {
            add(&stored.sha256, stored.size)?;
        }
    }
    Ok(carried)
}

/// What [`export`] wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportReport {
    pub objects: usize,
    pub chunks: usize,
    /// Plain bytes of objects and chunks written.
    pub bytes: u64,
}

/// Writes `image` (sealed), the manifests it links to (`linked`: only its content image),
/// and the objects they carry from `store` as an export at `to`. The zip is written to
/// `<to>.part` and renamed when whole, so `to` is never half written; every object is
/// checked as it is read.
pub fn export(
    store: &Store,
    image: &Image,
    linked: &[Image],
    to: &Path,
    cancel: Cancel,
) -> Result<ExportReport, ContainerError> {
    image.validate().map_err(ManifestError::Invalid)?;
    image.verify()?;
    for other in linked {
        other.validate().map_err(ManifestError::Invalid)?;
        other.verify()?;
        if image.content.as_deref() != Some(other.id.as_str()) {
            return Err(invalid(format!(
                "the image doesn't link to the image {}",
                other.id
            )));
        }
    }
    let mut images = vec![image];
    images.extend(linked.iter());
    let carried = carried_objects(&images).map_err(invalid)?;

    let part = part_path(to);
    let result = write_zip(store, image, linked, &carried, &part, cancel).and_then(|report| {
        fs::rename(&part, to).map_err(|source| ContainerError::Io {
            what: format!("Failed to write {}", to.display()),
            source,
        })?;
        if let Some(dir) = to.parent() {
            let _ = sync_parent(dir);
        }
        Ok(report)
    });
    if result.is_err() {
        let _ = fs::remove_file(&part);
    }
    result
}

/// `<to>.part`.
fn part_path(to: &Path) -> PathBuf {
    let mut name = to.as_os_str().to_owned();
    name.push(".part");
    PathBuf::from(name)
}

fn sync_parent(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    File::open(dir)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

fn write_zip(
    store: &Store,
    image: &Image,
    linked: &[Image],
    carried: &BTreeMap<String, u64>,
    part: &Path,
    cancel: Cancel,
) -> Result<ExportReport, ContainerError> {
    let io = |source: io::Error| ContainerError::Io {
        what: format!("Failed to write {}", part.display()),
        source,
    };
    let zip_err = |e: zip::result::ZipError| ContainerError::Io {
        what: format!("Failed to write {}", part.display()),
        source: io::Error::other(e.to_string()),
    };
    // A fixed time, so the same image and objects give the same bytes.
    let options = |size: u64| {
        zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated)
            .last_modified_time(zip::DateTime::default())
            .large_file(size >= 1 << 31)
    };
    let file = File::create(part).map_err(io)?;
    let mut zip = zip::ZipWriter::new(BufWriter::new(file));
    let mut report = ExportReport::default();

    let json = image.to_json_pretty();
    zip.start_file(IMAGE_FILE, options(json.len() as u64))
        .map_err(zip_err)?;
    zip.write_all(&json).map_err(io)?;
    for other in linked {
        let json = other.to_json_pretty();
        zip.start_file(
            format!("images/{}.json", other.id),
            options(json.len() as u64),
        )
        .map_err(zip_err)?;
        zip.write_all(&json).map_err(io)?;
    }

    let mut written_chunks: HashSet<String> = HashSet::new();
    for (sha256, &size) in carried {
        if cancel() {
            return Err(ContainerError::Cancelled);
        }
        match store.object_form(sha256, size)? {
            ObjectForm::Whole if size > CHUNKED_ABOVE => {
                // Stored whole (the core's, from before chunking): cut as a store would,
                // listing the chunks first and then writing them, reading the object
                // twice (and checking its hash each time) rather than holding it.
                let mut chunks = Vec::new();
                cut_whole(store, sha256, size, cancel, &mut |chunk| {
                    chunks.push(ChunkRef {
                        sha256: sha256_hex(chunk),
                        // Chunks are at most 256 KiB.
                        size: u32::try_from(chunk.len()).unwrap_or(u32::MAX),
                    });
                    Ok(())
                })?;
                let encoded = ChunkList { size, chunks }.encode();
                zip.start_file(format!("objects/{sha256}"), options(encoded.len() as u64))
                    .map_err(zip_err)?;
                zip.write_all(&encoded).map_err(io)?;
                cut_whole(store, sha256, size, cancel, &mut |chunk| {
                    let chunk_sha = sha256_hex(chunk);
                    if written_chunks.insert(chunk_sha.clone()) {
                        zip.start_file(format!("chunks/{chunk_sha}"), options(chunk.len() as u64))
                            .map_err(zip_err)?;
                        zip.write_all(chunk).map_err(io)?;
                        report.chunks += 1;
                        report.bytes = report.bytes.saturating_add(chunk.len() as u64);
                    }
                    Ok(())
                })?;
            }
            ObjectForm::Whole => {
                zip.start_file(format!("objects/{sha256}"), options(size))
                    .map_err(zip_err)?;
                store.read_object(sha256, size, &mut zip, cancel)?;
                report.bytes = report.bytes.saturating_add(size);
            }
            ObjectForm::Chunked(list) => {
                let encoded = list.encode();
                zip.start_file(format!("objects/{sha256}"), options(encoded.len() as u64))
                    .map_err(zip_err)?;
                zip.write_all(&encoded).map_err(io)?;
                // Every chunk is read (and checked) to check the whole file's hash too;
                // each is written once.
                let mut hasher = Sha256::new();
                for chunk in &list.chunks {
                    if cancel() {
                        return Err(ContainerError::Cancelled);
                    }
                    let bytes = store.read_chunk(chunk)?;
                    hasher.update(&bytes);
                    if written_chunks.insert(chunk.sha256.clone()) {
                        zip.start_file(
                            format!("chunks/{}", chunk.sha256),
                            options(bytes.len() as u64),
                        )
                        .map_err(zip_err)?;
                        zip.write_all(&bytes).map_err(io)?;
                        report.chunks += 1;
                        report.bytes += bytes.len() as u64;
                    }
                }
                if hex_digest(hasher) != *sha256 {
                    return Err(StoreError::Damaged(sha256.clone()).into());
                }
            }
        }
        report.objects += 1;
    }
    let file = zip
        .finish()
        .map_err(zip_err)?
        .into_inner()
        .map_err(|e| io(e.into_error()))?;
    file.sync_all().map_err(io)?;
    Ok(report)
}

/// Reads the whole object `sha256` (checking its hash) and cuts it into chunks as the store
/// cuts a large file, giving each chunk to `emit` in order.
fn cut_whole(
    store: &Store,
    sha256: &str,
    size: u64,
    cancel: Cancel,
    emit: &mut dyn FnMut(&[u8]) -> Result<(), ContainerError>,
) -> Result<(), ContainerError> {
    /// A writer that cuts what it is given, keeping the first error `emit` gave.
    struct Cutter<'e> {
        chunker: Chunker,
        emit: &'e mut dyn FnMut(&[u8]) -> Result<(), ContainerError>,
        failed: Option<ContainerError>,
    }
    impl Write for Cutter<'_> {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            let Cutter {
                chunker,
                emit,
                failed,
            } = self;
            if failed.is_some() {
                return Err(io::Error::other("an earlier chunk failed"));
            }
            match chunker.push(bytes, &mut **emit) {
                Ok(()) => Ok(bytes.len()),
                Err(e) => {
                    *failed = Some(e);
                    Err(io::Error::other("a chunk failed"))
                }
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut cutter = Cutter {
        chunker: Chunker::new(),
        emit,
        failed: None,
    };
    let read = store.read_object(sha256, size, &mut cutter, cancel);
    if let Some(failed) = cutter.failed.take() {
        return Err(failed);
    }
    read?;
    let Cutter { chunker, emit, .. } = cutter;
    chunker.finish(emit)
}

/// How an export holds one object. A chunk list is only checked by [`open`], not kept: a
/// list can name one small chunk millions of times, so lists are read again, one at a
/// time, when they are imported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Carried {
    Whole {
        size: u64,
    },
    /// The file's size, and the list's in the export.
    Chunked {
        size: u64,
        list_bytes: u64,
    },
}

/// An export that passed every check [`open`] makes. Its objects go into a store with
/// [`Container::import_objects`]; its manifests are the caller's to save, under a new
/// instance.
pub struct Container {
    archive: zip::ZipArchive<BufReader<File>>,
    image: Image,
    linked: Vec<Image>,
    objects: BTreeMap<String, Carried>,
    unpacked: u64,
}

impl std::fmt::Debug for Container {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Container")
            .field("image", &self.image.id)
            .field("objects", &self.objects.len())
            .field("unpacked", &self.unpacked)
            .finish()
    }
}

/// What [`Container::import_objects`] did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    /// Objects added to the store (the rest were there).
    pub objects: usize,
    /// Bytes the store grew by (encoded).
    pub added_bytes: u64,
}

/// What a name in an export is.
enum Name {
    Image,
    Linked(String),
    Object(String),
    Chunk(String),
    Folder,
}

fn classify(name: &str) -> Option<Name> {
    match name {
        IMAGE_FILE => Some(Name::Image),
        "objects/" | "chunks/" | "images/" => Some(Name::Folder),
        _ => {
            if let Some(file) = name.strip_prefix("images/") {
                return manifest_id(file).map(|id| Name::Linked(id.to_string()));
            }
            if let Some(hash) = name.strip_prefix("objects/") {
                return is_sha256(hash).then(|| Name::Object(hash.to_string()));
            }
            if let Some(hash) = name.strip_prefix("chunks/") {
                return is_sha256(hash).then(|| Name::Chunk(hash.to_string()));
            }
            None
        }
    }
}

/// Opens the export at `path` and checks everything but the objects' bytes, which
/// [`Container::import_objects`] checks as it reads them.
pub fn open(path: &Path, limits: &Limits) -> Result<Container, ContainerError> {
    let file = File::open(path).map_err(|source| ContainerError::Io {
        what: format!("Failed to open {}", path.display()),
        source,
    })?;
    let mut archive =
        zip::ZipArchive::new(BufReader::new(file)).map_err(|_| invalid("it isn't a zip file"))?;
    if archive.len() > limits.max_entries {
        return Err(invalid("it has too many files"));
    }
    // Duplicate names, and local headers that say something other than the directory (a
    // zip read differently by different tools), are refused outright.
    archive
        .validate_header_consistency()
        .map_err(|e| invalid(format!("its zip structure is inconsistent ({e})")))?;

    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut has_image = false;
    let mut linked_declared: BTreeMap<String, u64> = BTreeMap::new();
    let mut image_declared = 0;
    let mut objects_declared: BTreeMap<String, u64> = BTreeMap::new();
    let mut chunks_declared: BTreeMap<String, u64> = BTreeMap::new();
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let entry = archive
            .by_index_raw(i)
            .map_err(|e| invalid(format!("an entry can't be read ({e})")))?;
        let name = entry
            .name()
            .map_err(|_| invalid("an entry's name isn't readable"))?
            .into_owned();
        if !seen.insert(name.clone()) {
            return Err(invalid(format!("{name:?} is in it twice")));
        }
        if entry.encrypted() {
            return Err(invalid(format!("{name:?} is encrypted")));
        }
        let size = entry.size();
        total = total.saturating_add(size);
        let is_dir = entry.is_dir();
        let kind = classify(&name).ok_or_else(|| invalid(format!("unexpected file {name:?}")))?;
        match (kind, is_dir) {
            (Name::Folder, true) => {}
            (Name::Image, false) => {
                has_image = true;
                image_declared = size;
            }
            (Name::Linked(id), false) => {
                linked_declared.insert(id, size);
            }
            (Name::Object(hash), false) => {
                objects_declared.insert(hash, size);
            }
            (Name::Chunk(hash), false) => {
                if size == 0 || size > MAX_CHUNK as u64 {
                    return Err(invalid(format!("the chunk {hash} has an impossible size")));
                }
                chunks_declared.insert(hash, size);
            }
            _ => return Err(invalid(format!("unexpected file {name:?}"))),
        }
        if matches!(classify(&name), Some(Name::Image | Name::Linked(_)))
            && size > limits.max_manifest_bytes
        {
            return Err(invalid(format!("{name} is too big")));
        }
    }
    if total > limits.max_total_bytes {
        return Err(invalid(format!(
            "it is larger than {} GB unpacked",
            limits.max_total_bytes / 1_000_000_000
        )));
    }
    if !has_image {
        return Err(invalid("it has no image.json"));
    }

    let image = Image::from_json(&read_entry(&mut archive, IMAGE_FILE, image_declared)?)?;
    // The cheap bounds before hashing the whole manifest.
    check_files_size(&image, limits)?;
    image.verify()?;
    let mut linked = Vec::new();
    for (id, declared) in &linked_declared {
        if image.content.as_deref() != Some(id.as_str()) {
            return Err(invalid(format!(
                "it carries the image {id}, which its image doesn't link to"
            )));
        }
        let other = Image::from_json(&read_entry(
            &mut archive,
            &format!("images/{id}.json"),
            *declared,
        )?)?;
        check_files_size(&other, limits)?;
        other.verify()?;
        if other.id != *id {
            return Err(invalid(format!("images/{id}.json holds another image")));
        }
        linked.push(other);
    }

    let mut images = vec![&image];
    images.extend(linked.iter());
    let needed = carried_objects(&images).map_err(invalid)?;
    let mut objects = BTreeMap::new();
    let mut used_chunks: HashSet<String> = HashSet::new();
    let mut unpacked: u64 = 0;
    for (sha256, &size) in &needed {
        let name = format!("objects/{sha256}");
        let Some(&declared) = objects_declared.get(sha256) else {
            return Err(invalid(format!("{name} is missing from it")));
        };
        if declared == size {
            objects.insert(sha256.clone(), Carried::Whole { size });
            unpacked = unpacked.saturating_add(size);
            continue;
        }
        if size <= CHUNKED_ABOVE || declared > ChunkList::max_encoded_len(size) {
            return Err(invalid(format!("{name} isn't the size its image says")));
        }
        // Streamed: a line in memory at a time, and only the chunks the export has kept
        // (each once), however many times the list names them.
        let entry = archive
            .by_name(&name)
            .map_err(|_| invalid(format!("{name} can't be read")))?;
        let mut reader = BufReader::new(entry.take(declared));
        let mut problem: Option<ContainerError> = None;
        let visited = ChunkList::visit(&mut reader, &mut |hash, chunk_size| {
            match chunks_declared.get(hash) {
                Some(&declared) if declared == u64::from(chunk_size) => {}
                Some(_) => {
                    problem = Some(invalid(format!(
                        "chunks/{hash} isn't the size its list says"
                    )));
                    return Err(String::new());
                }
                None => {
                    problem = Some(invalid(format!("chunks/{hash} is missing from it")));
                    return Err(String::new());
                }
            }
            if !used_chunks.contains(hash) {
                used_chunks.insert(hash.to_string());
                unpacked = unpacked.saturating_add(u64::from(chunk_size));
            }
            Ok(())
        });
        let read = reader.into_inner().limit();
        if let Some(problem) = problem {
            return Err(problem);
        }
        let list_size = visited.map_err(|why| invalid(format!("{name}: {why}")))?;
        if read != 0 {
            return Err(invalid(format!("{name} isn't the size it says")));
        }
        if list_size != size {
            return Err(invalid(format!("{name} is a chunk list for another size")));
        }
        unpacked = unpacked.saturating_add(declared);
        objects.insert(
            sha256.clone(),
            Carried::Chunked {
                size,
                list_bytes: declared,
            },
        );
    }
    if let Some(extra) = objects_declared.keys().find(|h| !needed.contains_key(*h)) {
        return Err(invalid(format!("objects/{extra} isn't used by its image")));
    }
    if let Some(extra) = chunks_declared.keys().find(|h| !used_chunks.contains(*h)) {
        return Err(invalid(format!(
            "chunks/{extra} isn't used by any chunk list"
        )));
    }
    Ok(Container {
        archive,
        image,
        linked,
        objects,
        unpacked,
    })
}

/// The sum of `image`'s files' sizes, whatever their source: what placing it all writes.
pub fn files_size(image: &Image) -> u64 {
    image
        .entries
        .iter()
        .map(|e| e.size)
        .fold(0, u64::saturating_add)
}

/// Refuses an image whose files add up to more than the limit.
fn check_files_size(image: &Image, limits: &Limits) -> Result<(), ContainerError> {
    if files_size(image) > limits.max_total_bytes {
        return Err(invalid(format!(
            "its image {} has files adding up to more than {} GB",
            image.id,
            limits.max_total_bytes / 1_000_000_000
        )));
    }
    Ok(())
}

/// Reads an entry of exactly `declared` bytes (the zip checks its CRC at the end).
fn read_entry(
    archive: &mut zip::ZipArchive<BufReader<File>>,
    name: &str,
    declared: u64,
) -> Result<Vec<u8>, ContainerError> {
    let entry = archive
        .by_name(name)
        .map_err(|_| invalid(format!("{name} can't be read")))?;
    let mut bytes = Vec::with_capacity(usize::try_from(declared.min(64 << 20)).unwrap_or(0));
    entry
        .take(declared.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| invalid(format!("{name} can't be read ({e})")))?;
    if bytes.len() as u64 != declared {
        return Err(invalid(format!("{name} isn't the size it says")));
    }
    Ok(bytes)
}

impl Container {
    /// The image, valid and verified.
    pub fn image(&self) -> &Image {
        &self.image
    }

    /// The manifests the image links to (its content image), valid and verified.
    pub fn linked(&self) -> &[Image] {
        &self.linked
    }

    /// The bytes importing may unpack (objects, lists and chunks), for a free-space check.
    pub fn unpacked_bytes(&self) -> u64 {
        self.unpacked
    }

    /// What placing the image's files writes ([`files_size`]), for a free-space check
    /// before materializing it; a linked image's is `files_size` of it.
    pub fn image_bytes(&self) -> u64 {
        files_size(&self.image)
    }

    /// Adds the export's objects to `store`, after `check_room` agreed to
    /// [`Container::unpacked_bytes`]. Each object's bytes (each chunk's, then the whole
    /// file's) are checked against their SHA-256 before the store names anything by it;
    /// objects the store already has complete ([`Store::claim_object`]) are skipped, so
    /// importing an export again is how a store repairs a chunk list that lost a chunk.
    pub fn import_objects(
        &mut self,
        store: &Store,
        check_room: &dyn Fn(u64) -> Result<(), String>,
        cancel: Cancel,
    ) -> Result<ImportReport, ContainerError> {
        check_room(self.unpacked).map_err(ContainerError::NoRoom)?;
        let mut report = ImportReport::default();
        for (sha256, &carried) in &self.objects {
            if cancel() {
                return Err(ContainerError::Cancelled);
            }
            let size = match carried {
                Carried::Whole { size } | Carried::Chunked { size, .. } => size,
            };
            // Complete, every chunk of a list included (a list that lost a damaged chunk
            // gets it back from the export), and touched against a collection running now.
            if store.claim_object(sha256, size) {
                continue;
            }
            let name = format!("objects/{sha256}");
            let bad = |e: StoreError| match e {
                StoreError::HashMismatch { .. } | StoreError::SizeMismatch { .. } => {
                    invalid(format!("{name} doesn't match its hash or size"))
                }
                StoreError::Source(e) => invalid(format!("{name} can't be read ({e})")),
                other => ContainerError::Store(other),
            };
            let stored = match carried {
                Carried::Whole { size } => {
                    let entry = self
                        .archive
                        .by_name(&name)
                        .map_err(|_| invalid(format!("{name} can't be read")))?;
                    let mut limited = entry.take(size.saturating_add(1));
                    store
                        .put_reader(&mut limited, size, Some(sha256), cancel)
                        .map_err(bad)?
                }
                Carried::Chunked { size, list_bytes } => {
                    // This one list in memory (its chunks are read from the archive while
                    // it is walked), never every list at once.
                    let list = read_entry(&mut self.archive, &name, list_bytes)?;
                    let mut writer = store.writer(size, Some(sha256))?;
                    let mut problem: Option<ContainerError> = None;
                    let archive = &mut self.archive;
                    let walked = ChunkList::visit(&mut &list[..], &mut |hash, chunk_size| {
                        let pushed = (|| {
                            if cancel() {
                                return Err(ContainerError::Cancelled);
                            }
                            let chunk_name = format!("chunks/{hash}");
                            let bytes = read_entry(archive, &chunk_name, u64::from(chunk_size))?;
                            if sha256_hex(&bytes) != hash {
                                return Err(invalid(format!(
                                    "{chunk_name} doesn't match its hash"
                                )));
                            }
                            writer.push(&bytes).map_err(bad)
                        })();
                        pushed.map_err(|e| {
                            problem = Some(e);
                            String::new()
                        })
                    });
                    if let Some(problem) = problem {
                        return Err(problem);
                    }
                    walked.map_err(|why| invalid(format!("{name}: {why}")))?;
                    writer.finish().map_err(bad)?
                }
            };
            report.objects += 1;
            report.added_bytes += stored.added_bytes;
        }
        Ok(report)
    }
}

#[cfg(test)]
mod tests;
