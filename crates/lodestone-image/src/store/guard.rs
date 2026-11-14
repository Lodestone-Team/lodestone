//! The downgrade guard: what keeps a released Lodestone from collecting what images use.
//!
//! The image store shares its folder with the formats it replaces, so that their objects
//! are read in place: the core's backup folder (`objects/`, manifests in
//! `servers/<uuid>/*.json`) and the Launcher's snapshot folder (`objects/`, manifests in
//! `instances/<id>/*.json`). Their garbage collectors, in every release that has them
//! (Lodestone Host 0.6.0-beta.2, Lodestone Launcher 0.1.0-beta.1 and later), know only
//! their own manifests and delete every other object in `objects/`: whole files and chunk
//! lists that only images name. After someone goes back to such a release, its first backup
//! cleanup would break every image.
//!
//! Neither collector can be taught about images, but both refuse to delete anything when
//! they find a manifest they can't account for. So before a store writes anything, it puts
//! one such file where each looks ([`ensure`]), and both then keep everything:
//!
//! - `servers/.images/images.json`, a manifest of schema 2: the core's `Store::gc` reads
//!   every `.json` in every folder under `servers/` and refuses, deleting nothing, when one
//!   "was made by a newer Lodestone". `.images` isn't a valid server id, so nothing else
//!   in the core looks there (backup listings, restore journals and save-off markers go
//!   through valid ids only).
//! - `instances/.images/images.json`, which doesn't parse as a snapshot manifest (its
//!   `files` is text): the Launcher's `collect_garbage` keeps everything when a `.json`
//!   there isn't one, and its listings skip files that don't parse.
//!
//! This is safer than the alternatives: a store of its own beside the old one would have to
//! copy or link every old object an image reuses, and a schema-1 list of everything images
//! use would have to be rewritten with every image, and lose objects if it fell behind. The
//! cost is that a released version, while it runs, frees no space in that folder; it warns
//! that its cleanup was refused, and nothing is lost. The guard stays for as long as images
//! use the folder; the image store itself never reads or deletes it, and the core and the
//! Launcher, when they collect their own old manifests' objects, skip the `.images`
//! folders.

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use super::fsx;

/// The folder each guard file is in, under the old manifests' folders.
pub const GUARD_FOLDER: &str = ".images";
/// The guard file's name.
pub const GUARD_FILE: &str = "images.json";

/// What the core's backup collector reads: schema 2, newer than any it knows.
pub const CORE_GUARD: &str = r#"{
  "schema": 2,
  "files": [],
  "about": "Lodestone keeps instance images in this folder (images/, objects/, chunks/). This file stops older Lodestone versions from deleting the objects images use: their backup cleanup refuses to run while it is here. Don't delete it."
}
"#;
/// What the Launcher's snapshot collector reads: not a snapshot manifest at all.
pub const LAUNCHER_GUARD: &str = r#"{
  "schema": 2,
  "files": "Lodestone keeps instance images in this folder (images/, objects/, chunks/). This file stops older Lodestone versions from deleting the objects images use: their snapshot cleanup keeps everything while it is here. Don't delete it."
}
"#;

/// The guard files under `root`, with what each must hold.
pub fn guard_files(root: &Path) -> [(PathBuf, &'static str); 2] {
    [
        (
            root.join("servers").join(GUARD_FOLDER).join(GUARD_FILE),
            CORE_GUARD,
        ),
        (
            root.join("instances").join(GUARD_FOLDER).join(GUARD_FILE),
            LAUNCHER_GUARD,
        ),
    ]
}

/// Puts the guard files under `root` where they are missing or don't hold what they must,
/// durably (flushed, with their folders), before the store writes anything else.
pub fn ensure(root: &Path) -> io::Result<()> {
    for (path, contents) in guard_files(root) {
        if fs::read(&path).is_ok_and(|bytes| bytes == contents.as_bytes()) {
            continue;
        }
        let dir = path.parent().unwrap_or(root);
        let created = !dir.exists();
        fs::create_dir_all(dir)?;
        let (mut file, temp) = fsx::create_temp(dir)?;
        let mut guard = fsx::TempGuard::new(temp.clone());
        file.write_all(contents.as_bytes())?;
        file.sync_all()?;
        drop(file);
        fsx::rename_with_retry(&temp, &path)?;
        guard.disarm();
        fsx::sync_dir(dir)?;
        if created {
            // The new folders' entries too, up to the store's folder.
            if let Some(parent) = dir.parent() {
                fsx::sync_dir(parent)?;
            }
            fsx::sync_dir(root)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guards_are_written_once_and_repaired() {
        let dir = tempfile::tempdir().unwrap();
        ensure(dir.path()).unwrap();
        for (path, contents) in guard_files(dir.path()) {
            assert_eq!(fs::read_to_string(&path).unwrap(), contents);
        }
        // Both are JSON objects with schema 2.
        for contents in [CORE_GUARD, LAUNCHER_GUARD] {
            let value: serde_json::Value = serde_json::from_str(contents).unwrap();
            assert_eq!(value["schema"], 2);
        }
        let [(core, _), _] = guard_files(dir.path());
        fs::write(&core, b"{}").unwrap();
        ensure(dir.path()).unwrap();
        assert_eq!(fs::read_to_string(&core).unwrap(), CORE_GUARD);
        // No temporary file is left behind.
        let leftovers = fs::read_dir(core.parent().unwrap()).unwrap().count();
        assert_eq!(leftovers, 1);
    }
}
