//! The Launcher's world backup zips (`<instance>/backups/<world>/<time>.zip`) as data
//! images, read without extracting them.
//!
//! A world backup holds one world's files under `<world folder>/` (`session.lock` left
//! out), written by `launcher-core/src/worlds.rs` (`write_zip`). A zip put there by hand
//! may hold them elsewhere, and the Launcher restores it all the same, so the world is
//! found the way its restore finds it (`inspect_zip`): `level.dat` at the zip's root wins,
//! then the world folder's own name, then the one folder that has a `level.dat`; without
//! any, the world folder's name. Mac metadata (`__MACOSX/`, `.DS_Store`), which the
//! restore skips, is withheld as excluded. Its image lists the files as
//! `saves/<world>/…`, each with the SHA-256 and size of its bytes, read and hashed straight
//! from the zip: nothing is written to disk. The entries' source is `object`, since the
//! bytes are only in the zip until a store imports them.
//!
//! The zip may come from anywhere (a user can drop any file there), so reading is strict
//! and never panics: at most [`MAX_ZIP_ENTRIES`] entries and [`MAX_ZIP_BYTES`] unpacked,
//! every name checked as a canonical path, links never followed, and each entry's bytes
//! checked against the size its header gives. An entry that fails a check is withheld with
//! the reason, by its name in the zip; a zip that can't be read at all, or that other tools
//! would read differently (a name twice, local headers that disagree with the directory),
//! is an error.
//!
//! There is no way back: a zip can't be rewritten byte for byte, and images replace these
//! backups. Zip times are local times without a zone, so entries have no `modified`.
//! The game isn't in the zip; the caller passes the instance's, and sets the image's
//! `created` and `label` (neither is part of the id) from the file's time and name.

use std::collections::HashSet;
use std::io::{Read, Seek};

use crate::sha256::Sha256;

use super::{finish, LegacyError, Note};
use crate::manifest::{Entry, Game, Image, Kind, Source, WithheldReason, World};
use crate::paths::{always_excluded, validate_component, CanonicalPath};

/// A world zip may have at most this many entries, and this many bytes unpacked (the
/// Launcher's limits).
pub const MAX_ZIP_ENTRIES: usize = 500_000;
pub const MAX_ZIP_BYTES: u64 = 64 * 1024 * 1024 * 1024;

/// A converted world backup.
#[derive(Debug, Clone, PartialEq)]
pub struct Converted {
    pub image: Image,
    pub notes: Vec<Note>,
}

/// The data image of a backup of the world `folder` (its folder in `saves/`), read from
/// `reader`; see the module.
pub fn read<R: Read + Seek>(reader: R, folder: &str, game: Game) -> Result<Converted, LegacyError> {
    validate_component(folder)
        .map_err(|e| LegacyError::Invalid(format!("the world folder {folder:?}: {e}")))?;
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| LegacyError::Invalid(format!("it isn't a zip that can be read: {e}")))?;
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(LegacyError::Invalid("it has too many entries".into()));
    }
    // A name given twice (zip readers keep one copy, not all the same one), or local
    // headers that say something other than the directory, is refused, as image exports
    // refuse it.
    archive
        .validate_header_consistency()
        .map_err(|e| LegacyError::Invalid(format!("its zip structure is inconsistent ({e})")))?;
    let prefix = world_prefix(&mut archive, folder)?;
    let mut image = Image::new(Kind::Data, game);
    let mut notes = Vec::new();
    let mut seen: HashSet<CanonicalPath> = HashSet::new();
    let mut declared = 0u64;
    let mut world_size = 0u64;
    let mut files: Vec<(String, Entry)> = Vec::new();
    for index in 0..archive.len() {
        let (name, outcome) = read_entry(&mut archive, index, &prefix, folder, &mut declared)?;
        let entry = match outcome {
            Outcome::Skip => continue,
            Outcome::Withhold(reason) => {
                withhold(&mut image, &mut notes, name, reason);
                continue;
            }
            Outcome::File(entry) => entry,
        };
        if !seen.insert(entry.path.clone()) {
            withhold(&mut image, &mut notes, name, WithheldReason::BadName);
            continue;
        }
        files.push((name, *entry));
    }
    // A file that is also a folder of another (`w/a` and `w/a/b`): withheld, not fatal.
    let clashes = super::file_folder_clashes(files.iter().map(|(_, e)| e.path.as_str()));
    for (index, (name, entry)) in files.into_iter().enumerate() {
        if clashes.contains(&index) {
            withhold(&mut image, &mut notes, name, WithheldReason::BadName);
            continue;
        }
        world_size = world_size.saturating_add(entry.size);
        image.entries.push(entry);
    }
    image.worlds.push(World {
        name: folder.to_string(),
        level_name: None,
        data_version: None,
        game_version: None,
        size: Some(world_size),
        last_played: None,
        quiesced: Some(true),
    });
    Ok(Converted {
        image: finish(image)?,
        notes,
    })
}

/// Where the world's files are in the zip (`""` or `<folder>/`), as the Launcher's restore
/// picks it (see the module). Several folders with a `level.dat`, none of them `folder`,
/// is several worlds: the world folder's own name is used, and the rest is withheld.
fn world_prefix<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    folder: &str,
) -> Result<String, LegacyError> {
    let own = format!("{folder}/");
    // Only whether `own` holds a `level.dat`, and whether one other folder does or several,
    // matter, so each entry costs the same however many folders the zip has.
    let mut has_own = false;
    let mut first: Option<String> = None;
    let mut several = false;
    for index in 0..archive.len() {
        let raw = archive
            .by_index_raw(index)
            .map_err(|e| LegacyError::Invalid(format!("its entries: {e}")))?;
        let name = raw
            .name()
            .map_err(|e| LegacyError::Invalid(format!("its entries: {e}")))?;
        if raw.is_dir() || is_mac_metadata(&name) {
            continue;
        }
        match name.split('/').collect::<Vec<_>>().as_slice() {
            ["level.dat"] => return Ok(String::new()),
            [top, "level.dat"] if !top.is_empty() => {
                let top = format!("{top}/");
                if top == own {
                    has_own = true;
                } else if first.is_none() {
                    first = Some(top);
                } else if first.as_ref() != Some(&top) {
                    several = true;
                }
            }
            _ => {}
        }
    }
    Ok(match first {
        Some(one) if !has_own && !several => one,
        _ => own,
    })
}

/// Whether a zip entry is Mac metadata, which the Launcher's restore skips.
fn is_mac_metadata(name: &str) -> bool {
    name.starts_with("__MACOSX/") || name.split('/').any(|part| part == ".DS_Store")
}

/// What became of one zip entry.
enum Outcome {
    /// A folder, or the world's own folder entry.
    Skip,
    Withhold(WithheldReason),
    File(Box<Entry>),
}

/// Reads entry `index`: its name, and the file it is or why it isn't one. Errors only for
/// what makes the whole zip unreadable.
fn read_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    index: usize,
    prefix: &str,
    folder: &str,
    declared: &mut u64,
) -> Result<(String, Outcome), LegacyError> {
    let unreadable = |e: zip::result::ZipError| LegacyError::Invalid(format!("its entries: {e}"));
    let (name, size, mode, is_dir, is_link, encrypted) = {
        let raw = archive.by_index_raw(index).map_err(unreadable)?;
        let name = raw.name().map_err(unreadable)?.into_owned();
        (
            name,
            raw.size(),
            raw.unix_mode(),
            raw.is_dir(),
            raw.is_symlink(),
            raw.encrypted(),
        )
    };
    *declared = declared.saturating_add(size);
    if *declared > MAX_ZIP_BYTES {
        return Err(LegacyError::Invalid(
            "it is larger than 64 GB unpacked".into(),
        ));
    }
    if is_dir {
        return Ok((name, Outcome::Skip));
    }
    if is_mac_metadata(&name) {
        return Ok((name, Outcome::Withhold(WithheldReason::Excluded)));
    }
    if is_link {
        return Ok((name, Outcome::Withhold(WithheldReason::Symlink)));
    }
    let Some(inner) = name.strip_prefix(prefix).filter(|rest| !rest.is_empty()) else {
        return Ok((name, Outcome::Withhold(WithheldReason::BadName)));
    };
    let Ok(path) = CanonicalPath::new(format!("saves/{folder}/{inner}")) else {
        return Ok((name, Outcome::Withhold(WithheldReason::BadName)));
    };
    if always_excluded(&path) {
        return Ok((name, Outcome::Withhold(WithheldReason::Excluded)));
    }
    if encrypted {
        return Ok((name, Outcome::Withhold(WithheldReason::Unreadable)));
    }
    let file = match archive.by_index(index) {
        Ok(file) => file,
        // A compression this build can't read (see the crate's zip features) says nothing
        // about the entry: no entry of the zip could be read.
        Err(zip::result::ZipError::UnsupportedArchive(what)) => {
            return Err(LegacyError::Invalid(format!(
                "it can't be read here: {what}"
            )))
        }
        Err(_) => return Ok((name, Outcome::Withhold(WithheldReason::Unreadable))),
    };
    // Read one byte more than the header says, so a longer entry is caught.
    let mut limited = file.take(size.saturating_add(1));
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    let mut read = 0u64;
    loop {
        match limited.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                read += n as u64;
                hasher.update(&buffer[..n]);
            }
            Err(_) => return Ok((name, Outcome::Withhold(WithheldReason::Unreadable))),
        }
    }
    if read != size {
        return Ok((name, Outcome::Withhold(WithheldReason::Unreadable)));
    }
    let sha256 = super::lower_hex(&hasher.finalize());
    let mut entry = Entry::new(path, size, sha256, Source::Object {});
    entry.mode = match mode.map(|m| m & 0o777) {
        Some(bits) if bits != 0 => bits,
        _ => 0o644,
    };
    Ok((name, Outcome::File(Box::new(entry))))
}

fn withhold(image: &mut Image, notes: &mut Vec<Note>, path: String, reason: WithheldReason) {
    super::withhold(image, notes, &path, reason);
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::*;
    use crate::hex::sha256_hex;
    use crate::legacy::TestRng;
    use crate::manifest::Loader;

    fn game() -> Game {
        Game {
            minecraft: "1.21.1".into(),
            loader: Loader::Vanilla,
            loader_version: None,
        }
    }

    enum Item<'a> {
        File(&'a str, &'a [u8]),
        Dir(&'a str),
        Link(&'a str, &'a str),
    }

    fn zip_of(items: &[Item]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut out);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored)
                .unix_permissions(0o755);
            for item in items {
                match item {
                    Item::File(name, bytes) => {
                        zip.start_file(*name, options).unwrap();
                        zip.write_all(bytes).unwrap();
                    }
                    Item::Dir(name) => zip.add_directory(*name, options).unwrap(),
                    Item::Link(name, target) => zip.add_symlink(*name, *target, options).unwrap(),
                }
            }
            zip.finish().unwrap();
        }
        out.into_inner()
    }

    #[test]
    fn a_file_that_is_also_a_folder_is_withheld_not_fatal() {
        let bytes = zip_of(&[
            Item::File("w/level.dat", b"level"),
            Item::File("w/a", b"a file"),
            Item::File("w/a/b", b"and a folder"),
        ]);
        let converted = read(Cursor::new(bytes), "w", game()).unwrap();
        let paths: Vec<&str> = converted
            .image
            .entries
            .iter()
            .map(|e| e.path.as_str())
            .collect();
        assert_eq!(paths, ["saves/w/a/b", "saves/w/level.dat"]);
        assert_eq!(converted.image.withheld.len(), 1);
        assert_eq!(converted.image.withheld[0].path, "w/a");
        assert_eq!(converted.image.withheld[0].reason, WithheldReason::BadName);
    }

    #[test]
    fn checks_every_entry() {
        let bytes = zip_of(&[
            Item::Dir("W/"),
            Item::File("W/level.dat", b"level"),
            Item::File("W/session.lock", b"lock"),
            Item::File("W/../escape.txt", b"x"),
            Item::File("Other/level.dat", b"other"),
            Item::File("W/CON", b"x"),
            Item::Link("W/link", "/etc/passwd"),
            Item::File("W/region/r.0.0.mca", b"region"),
        ]);
        let converted = read(Cursor::new(bytes), "W", game()).unwrap();
        let image = &converted.image;
        let listed: Vec<(&str, u32)> = image
            .entries
            .iter()
            .map(|e| (e.path.as_str(), e.mode))
            .collect();
        assert_eq!(
            listed,
            [
                ("saves/W/level.dat", 0o755),
                ("saves/W/region/r.0.0.mca", 0o755)
            ]
        );
        assert_eq!(image.entries[0].sha256, sha256_hex(b"level"));
        let withheld: Vec<(&str, &str)> = image
            .withheld
            .iter()
            .map(|w| (w.path.as_str(), w.reason.as_str()))
            .collect();
        assert_eq!(
            withheld,
            [
                ("Other/level.dat", "bad_name"),
                ("W/../escape.txt", "bad_name"),
                ("W/CON", "bad_name"),
                ("W/link", "symlink"),
                ("W/session.lock", "excluded"),
            ]
        );
        assert_eq!(image.worlds[0].size, Some(11));
        assert_eq!(converted.notes.len(), 5);
    }

    #[test]
    fn the_world_is_found_where_the_launchers_restore_finds_it() {
        let paths = |items: &[Item]| {
            let converted = read(Cursor::new(zip_of(items)), "survival", game()).unwrap();
            let entries: Vec<String> = converted
                .image
                .entries
                .iter()
                .map(|e| e.path.as_str().to_string())
                .collect();
            let withheld: Vec<(String, String)> = converted
                .image
                .withheld
                .iter()
                .map(|w| (w.path.clone(), w.reason.as_str().to_string()))
                .collect();
            (entries, withheld)
        };
        // At the root (a zip copied in by hand).
        let (entries, withheld) = paths(&[
            Item::File("level.dat", b"level"),
            Item::File("region/r.0.0.mca", b"region"),
            Item::File("__MACOSX/._level.dat", b"meta"),
        ]);
        assert_eq!(
            entries,
            [
                "saves/survival/level.dat",
                "saves/survival/region/r.0.0.mca"
            ]
        );
        assert_eq!(
            withheld,
            [("__MACOSX/._level.dat".to_string(), "excluded".to_string())]
        );
        // In one folder named otherwise.
        let (entries, withheld) = paths(&[
            Item::File("Old Name/level.dat", b"level"),
            Item::File("Old Name/data/x.dat", b"x"),
        ]);
        assert_eq!(
            entries,
            ["saves/survival/data/x.dat", "saves/survival/level.dat"]
        );
        assert!(withheld.is_empty());
        // The world folder's own name wins over another world beside it.
        let (entries, withheld) = paths(&[
            Item::File("other/level.dat", b"other"),
            Item::File("survival/level.dat", b"level"),
        ]);
        assert_eq!(entries, ["saves/survival/level.dat"]);
        assert_eq!(
            withheld,
            [("other/level.dat".to_string(), "bad_name".to_string())]
        );
        // Several other worlds: the world folder's name, and everything else withheld.
        let (entries, withheld) = paths(&[
            Item::File("a/level.dat", b"a"),
            Item::File("b/level.dat", b"b"),
        ]);
        assert!(entries.is_empty());
        assert_eq!(withheld.len(), 2);
    }

    #[test]
    fn many_worlds_in_one_zip_are_read_quickly() {
        // A zip of many `<n>/level.dat`: finding the world mustn't compare every folder
        // with every other (minutes for the half million entries a zip may have).
        let names: Vec<String> = (0..40_000).map(|i| format!("d{i:06}/level.dat")).collect();
        let items: Vec<Item> = names.iter().map(|n| Item::File(n, b"")).collect();
        let zip = zip_of(&items);
        let started = std::time::Instant::now();
        let mut archive = zip::ZipArchive::new(Cursor::new(&zip)).unwrap();
        assert_eq!(world_prefix(&mut archive, "survival").unwrap(), "survival/");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn unreadable_zips_and_folders_are_errors() {
        assert!(read(Cursor::new(b"PK\x03\x04 nope".to_vec()), "W", game()).is_err());
        let ok = zip_of(&[Item::File("W/level.dat", b"level")]);
        assert!(read(Cursor::new(ok.clone()), "../W", game()).is_err());
        assert!(read(Cursor::new(ok), "", game()).is_err());
    }

    #[test]
    fn a_name_twice_is_refused() {
        // The zip writer refuses duplicate names, so rename one in the bytes.
        let mut bytes = zip_of(&[Item::File("W/a.dat", b"1"), Item::File("W/b.dat", b"2")]);
        let mut at = 0;
        while let Some(i) = bytes[at..].windows(7).position(|w| w == b"W/b.dat") {
            bytes[at + i + 2] = b'a';
            at += i + 7;
        }
        // Zip readers keep one copy of a name given twice, not always the same one: such
        // a zip reads differently in other tools, so it is refused outright.
        let error = read(Cursor::new(bytes), "W", game()).unwrap_err();
        assert!(error.to_string().contains("inconsistent"), "{error}");
    }

    /// Damaged zips never panic: each byte flip gives an image or an error.
    #[test]
    fn damaged_zips_never_panic() {
        let good = zip_of(&[
            Item::File("W/level.dat", b"level data"),
            Item::File("W/region/r.0.0.mca", &[7u8; 300]),
            Item::Dir("W/data/"),
        ]);
        let mut rng = TestRng::new(42);
        for _ in 0..2000 {
            let mut bytes = good.clone();
            for _ in 0..1 + rng.below(4) {
                let i = rng.below(bytes.len());
                bytes[i] = rng.next() as u8;
            }
            if rng.below(8) == 0 {
                bytes.truncate(rng.below(bytes.len()));
            }
            if let Ok(converted) = read(Cursor::new(bytes), "W", game()) {
                converted.image.verify().unwrap();
            }
        }
    }
}
