//! Released Lodestones that share the image store's folder never collect what images use.
//!
//! The garbage collectors below are copied from the released code, as close to the letter
//! as std allows (logging and error types aside): the core's `Store::gc` and `sweep_temp`
//! (`core/src/backups/store.rs`, unchanged from Lodestone Host 0.6.0-beta.2 to this
//! commit), and the Launcher's `collect_garbage_alone` with `manifest_objects`
//! (`launcher/crates/launcher-core/src/snapshots.rs` at this commit) and its first version
//! (Lodestone Launcher 0.1.0-beta.1). Each deletes every object in `objects/` that its own
//! manifests don't name, which is every object only images use. The store's downgrade
//! guard (`lodestone_image::store::guard`) makes each of them delete nothing.

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::time::{Duration, SystemTime};

use serde::Deserialize;

use lodestone_image::hex::sha256_hex;
use lodestone_image::manifest::{Entry, Game, Image, Kind, Loader, Source};
use lodestone_image::paths::CanonicalPath;
use lodestone_image::store::guard::{self, GUARD_FOLDER};
use lodestone_image::store::{Encoding, Store};

// ---------------------------------------------------------------------------------------
// The core's collector, Lodestone Host 0.6.0-beta.2.

const POLICY_FILE: &str = "policy.json";
const JOURNAL_FILE: &str = "restore-journal.json";
const MANIFEST_SCHEMA: u32 = 1;
const TEMP_MAX_AGE: Duration = Duration::from_secs(3600);

#[derive(Deserialize)]
struct GcManifest {
    schema: u32,
    #[serde(default)]
    files: Vec<GcFile>,
}

#[derive(Deserialize)]
struct GcFile {
    sha256: String,
}

fn valid_sha256(hash: &str) -> bool {
    hash.len() == 64
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn valid_server_id(id: &str) -> bool {
    (1..=100).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn remove_file_forced(path: &Path) -> std::io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(path) {
        let mut permissions = meta.permissions();
        if permissions.readonly() {
            #[allow(clippy::permissions_set_readonly_false)]
            permissions.set_readonly(false);
            let _ = fs::set_permissions(path, permissions);
        }
    }
    fs::remove_file(path)
}

/// `Store::gc`: objects removed, or why it refused.
fn core_gc(root: &Path) -> Result<u64, String> {
    let mut used: HashSet<String> = HashSet::new();
    let servers = match fs::read_dir(root.join("servers")) {
        Ok(entries) => entries.flatten().collect(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(format!("Failed to list the backups: {e}")),
    };
    for server in servers {
        if !server.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let entries = fs::read_dir(server.path()).map_err(|e| e.to_string())?;
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.ends_with(".json") || name == POLICY_FILE || name == JOURNAL_FILE {
                continue;
            }
            let refuse = |why: String| {
                format!(
                    "Not deleting unused backup files: {} {why}",
                    entry.path().display()
                )
            };
            let bytes =
                fs::read(entry.path()).map_err(|e| refuse(format!("can't be read ({e})")))?;
            let hashes: GcManifest = serde_json::from_slice(&bytes)
                .map_err(|e| refuse(format!("can't be parsed ({e})")))?;
            if hashes.schema > MANIFEST_SCHEMA {
                return Err(refuse("was made by a newer Lodestone".to_string()));
            }
            used.extend(hashes.files.into_iter().map(|f| f.sha256));
        }
    }

    let mut removed = 0;
    core_sweep_temp(root, TEMP_MAX_AGE);
    let Ok(dirs) = fs::read_dir(root.join("objects")) else {
        return Ok(removed);
    };
    for dir in dirs.flatten() {
        if !dir.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let Ok(files) = fs::read_dir(dir.path()) else {
            continue;
        };
        for file in files.flatten() {
            let name = file.file_name();
            let Some(name) = name.to_str() else { continue };
            if !valid_sha256(name) || used.contains(name) {
                continue;
            }
            if remove_file_forced(&file.path()).is_ok() {
                removed += 1;
            }
        }
    }
    Ok(removed)
}

/// `Store::sweep_temp`.
fn core_sweep_temp(root: &Path, max_age: Duration) -> u64 {
    let objects = root.join("objects");
    let mut dirs = vec![objects.clone()];
    if let Ok(entries) = fs::read_dir(&objects) {
        dirs.extend(
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .map(|e| e.path()),
        );
    }
    let now = SystemTime::now();
    let mut removed = 0;
    for dir in dirs {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let is_temp = entry
                .file_name()
                .to_str()
                .is_some_and(|n| n.ends_with(".tmp"));
            let old = entry
                .metadata()
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| now.duration_since(t).ok())
                .is_some_and(|age| age >= max_age);
            if is_temp && old && remove_file_forced(&entry.path()).is_ok() {
                removed += 1;
            }
        }
    }
    removed
}

// ---------------------------------------------------------------------------------------
// The Launcher's collectors.

const STALE_TEMP: Duration = Duration::from_secs(24 * 60 * 60);

/// `manifest_objects`.
fn manifest_objects(path: &Path) -> Option<Vec<String>> {
    #[derive(Deserialize)]
    struct Objects {
        #[serde(default)]
        files: Vec<Object>,
    }
    #[derive(Deserialize)]
    struct Object {
        sha256: String,
    }
    let text = fs::read_to_string(path).ok()?;
    let objects: Objects = serde_json::from_str(&text).ok()?;
    Some(objects.files.into_iter().map(|f| f.sha256).collect())
}

/// `collect_garbage_alone`: bytes freed.
fn launcher_gc(root: &Path) -> u64 {
    let mut used: HashSet<String> = HashSet::new();
    for dir in fs::read_dir(root.join("instances"))
        .into_iter()
        .flatten()
        .flatten()
    {
        for entry in fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.extension().is_some_and(|x| x == "json") {
                continue;
            }
            match manifest_objects(&path) {
                Some(objects) => used.extend(objects),
                None => return 0,
            }
        }
    }
    let mut freed = 0;
    for prefix in fs::read_dir(root.join("objects"))
        .into_iter()
        .flatten()
        .flatten()
    {
        for object in fs::read_dir(prefix.path()).into_iter().flatten().flatten() {
            let name = object.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                let age = object
                    .metadata()
                    .and_then(|m| m.modified())
                    .ok()
                    .and_then(|modified| modified.elapsed().ok());
                if age.is_some_and(|age| age > STALE_TEMP) {
                    let _ = remove_file_forced(&object.path());
                }
                continue;
            }
            if !used.contains(&name) {
                freed += object.metadata().map(|m| m.len()).unwrap_or(0);
                let _ = remove_file_forced(&object.path());
            }
        }
        let _ = fs::remove_dir(prefix.path());
    }
    freed
}

/// Lodestone Launcher 0.1.0-beta.1's `collect_garbage`: any `.json` that doesn't read as
/// a whole snapshot manifest keeps everything.
fn launcher_gc_beta_1(root: &Path) -> u64 {
    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct Snapshot {
        schema: u32,
        id: String,
        files: Vec<SnapshotFile>,
    }
    #[derive(Deserialize)]
    struct SnapshotFile {
        sha256: String,
    }
    let mut used: HashSet<String> = HashSet::new();
    for dir in fs::read_dir(root.join("instances"))
        .into_iter()
        .flatten()
        .flatten()
    {
        for entry in fs::read_dir(dir.path()).into_iter().flatten().flatten() {
            let read = fs::read_to_string(entry.path())
                .ok()
                .and_then(|text| serde_json::from_str::<Snapshot>(&text).ok());
            match read {
                Some(snapshot) => used.extend(snapshot.files.into_iter().map(|f| f.sha256)),
                None if entry.path().extension().is_some_and(|x| x == "json") => return 0,
                None => {}
            }
        }
    }
    let mut freed = 0;
    for prefix in fs::read_dir(root.join("objects"))
        .into_iter()
        .flatten()
        .flatten()
    {
        for object in fs::read_dir(prefix.path()).into_iter().flatten().flatten() {
            let name = object.file_name().to_string_lossy().to_string();
            if !used.contains(&name) {
                freed += object.metadata().map(|m| m.len()).unwrap_or(0);
                let _ = remove_file_forced(&object.path());
            }
        }
        let _ = fs::remove_dir(prefix.path());
    }
    freed
}

// ---------------------------------------------------------------------------------------
// The Launcher's snapshot writes and restores (`store_file` and `revert_shared` in
// `launcher/crates/launcher-core/src/snapshots.rs`, the same since 0.1.0-beta.1): any file
// at `objects/ab/<sha256>` is taken for that file's bytes.

/// `store_file`: stores `bytes` (a file being snapshotted) unless its object is there.
fn launcher_store_file(root: &Path, bytes: &[u8]) -> String {
    let sha256 = sha256_hex(bytes);
    let object = root.join("objects").join(&sha256[..2]).join(&sha256);
    if !object.is_file() {
        fs::create_dir_all(object.parent().unwrap()).unwrap();
        fs::write(&object, bytes).unwrap();
    }
    sha256
}

/// `revert_shared`: copies the object over the instance's file, unchecked.
fn launcher_revert(root: &Path, sha256: &str, to: &Path) {
    fs::copy(root.join("objects").join(&sha256[..2]).join(sha256), to).unwrap();
}

// ---------------------------------------------------------------------------------------

/// Bytes that don't repeat, so a large file is cut into many chunks.
fn noise(len: usize, mut seed: u64) -> Vec<u8> {
    (0..len)
        .map(|_| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed as u8
        })
        .collect()
}

/// A store at `root` shared with an old format: one image with a small file and a large,
/// chunked one, an object only an old manifest names, and one nothing names. Returns the
/// image and the old object's hash.
fn shared_store(root: &Path, old_manifest: (&str, String)) -> (Store, Image, String) {
    let store = Store::new(root, Encoding::Raw);
    let small = b"options".to_vec();
    let large = noise(3 << 20, 99);
    let mut image = Image::new(
        Kind::Data,
        Game {
            minecraft: "1.21.1".into(),
            loader: Loader::Fabric,
            loader_version: None,
        },
    );
    for (path, bytes) in [("config/a.toml", &small), ("mods/big.jar", &large)] {
        let stored = store.put_bytes(bytes).unwrap();
        image.entries.push(Entry::new(
            CanonicalPath::new(path).unwrap(),
            stored.size,
            stored.sha256,
            Source::Object {},
        ));
    }
    image.seal();
    store.save_manifest("instance", &image).unwrap();

    // What the old format has: an object of its own (written as it writes them: raw
    // bytes are enough for its collector, which reads only names) and a manifest.
    let old = b"an old backup's file".to_vec();
    let old_sha = sha256_hex(&old);
    let old_path = root.join("objects").join(&old_sha[..2]).join(&old_sha);
    fs::create_dir_all(old_path.parent().unwrap()).unwrap();
    fs::write(&old_path, &old).unwrap();
    let garbage = b"nothing names this".to_vec();
    let garbage_sha = sha256_hex(&garbage);
    let garbage_path = root
        .join("objects")
        .join(&garbage_sha[..2])
        .join(&garbage_sha);
    fs::create_dir_all(garbage_path.parent().unwrap()).unwrap();
    fs::write(&garbage_path, &garbage).unwrap();
    let (relative, manifest) = old_manifest;
    let manifest_path = root.join(relative);
    fs::create_dir_all(manifest_path.parent().unwrap()).unwrap();
    fs::write(manifest_path, manifest.replace("OLD_SHA", &old_sha)).unwrap();
    (store, image, old_sha)
}

fn objects(root: &Path) -> usize {
    walkdir::WalkDir::new(root.join("objects"))
        .min_depth(2)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .count()
}

fn intact(store: &Store, image: &Image) -> bool {
    image
        .entries
        .iter()
        .all(|e| store.verify_object(&e.sha256, e.size, &|| false).is_ok())
}

fn remove_guards(root: &Path) {
    for (path, _) in guard::guard_files(root) {
        fs::remove_file(path).unwrap();
    }
}

fn core_manifest() -> (&'static str, String) {
    (
        "servers/2d6c4a1e-7d8b-4b1a-9a43-5e2f3c1b0a99/20261114-101500-ab12.json",
        r#"{"schema":1,"id":"20261114-101500-ab12","files":[{"path":"a","size":20,"sha256":"OLD_SHA","modified_ns":0}]}"#.to_string(),
    )
}

fn launcher_manifest() -> (&'static str, String) {
    (
        "instances/survival/20261114-125244-a7d2.json",
        r#"{"schema":1,"id":"20261114-125244-a7d2","files":[{"path":"minecraft/a","category":"other","size":20,"sha256":"OLD_SHA","mode":420}]}"#.to_string(),
    )
}

#[test]
fn a_released_core_collects_nothing_while_images_share_its_folder() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("backups");
    let (store, image, _) = shared_store(&root, core_manifest());
    let before = objects(&root);

    let refused = core_gc(&root).unwrap_err();
    assert!(refused.contains("newer Lodestone"), "{refused}");
    assert_eq!(objects(&root), before);
    assert!(intact(&store, &image));
    // Nothing else in the core looks in the guard's folder: it isn't a server id.
    assert!(!valid_server_id(GUARD_FOLDER));

    // Without the guard, the same collector breaks the image: this is what it prevents.
    remove_guards(&root);
    assert!(core_gc(&root).unwrap() >= 2);
    assert!(!intact(&store, &image));
}

#[test]
fn a_released_launcher_collects_nothing_while_images_share_its_folder() {
    for collector in [launcher_gc, launcher_gc_beta_1] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("snapshots");
        let (store, image, old_sha) = shared_store(&root, launcher_manifest());
        let before = objects(&root);

        assert_eq!(collector(&root), 0);
        assert_eq!(objects(&root), before);
        assert!(intact(&store, &image));
        // The Launcher's listings skip it: it isn't a snapshot manifest.
        assert!(manifest_objects(&guard::guard_files(&root)[1].0).is_none());

        remove_guards(&root);
        assert!(collector(&root) > 0);
        assert!(!intact(&store, &image));
        // Its own snapshot's object it keeps either way.
        assert!(root
            .join("objects")
            .join(&old_sha[..2])
            .join(&old_sha)
            .exists());
    }
}

/// After going back to a released Launcher, its snapshots of a file images already stored
/// (chunked) store the file whole and restore it as it was: `objects/` never holds a chunk
/// list, which it would take for the file and skip storing, then write over the world.
#[test]
fn a_released_launcher_never_mistakes_an_image_object_for_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("snapshots");
    let (_store, image, _) = shared_store(&root, launcher_manifest());
    // Every file in `objects/` is the file its name says.
    for entry in walkdir::WalkDir::new(root.join("objects")).min_depth(2) {
        let entry = entry.unwrap();
        if entry.file_type().is_file() {
            let name = entry.file_name().to_str().unwrap().to_string();
            assert_eq!(sha256_hex(&fs::read(entry.path()).unwrap()), name);
        }
    }
    // The released Launcher snapshots the large file (unchanged), then restores it.
    let large = noise(3 << 20, 99);
    let large_sha = &image.entries[1].sha256;
    assert_eq!(launcher_store_file(&root, &large), *large_sha);
    let restored = dir.path().join("big.jar");
    launcher_revert(&root, large_sha, &restored);
    assert_eq!(fs::read(&restored).unwrap(), large);
}
