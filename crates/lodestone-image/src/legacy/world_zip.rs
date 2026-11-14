//! The Launcher's world backup zips (`<instance>/backups/<world>/<time>.zip`) as data
//! images, read without extracting them.
//!
//! A world backup holds one world's files under `<world folder>/` (`session.lock` left
//! out), written by `launcher-core/src/worlds.rs` (`write_zip`). Its image lists them as
//! `saves/<world>/…`, each with the SHA-256 and size of its bytes, read and hashed straight
//! from the zip: nothing is written to disk. The entries' source is `object`, since the
//! bytes are only in the zip until a store imports them.
//!
//! The zip may come from anywhere (a user can drop any file there), so reading is strict
//! and never panics: at most [`MAX_ZIP_ENTRIES`] entries and [`MAX_ZIP_BYTES`] unpacked,
//! every name checked as a canonical path, links never followed, and each entry's bytes
//! checked against the size its header gives. An entry that fails a check is withheld with
//! the reason, by its name in the zip; a zip that can't be read at all is an error.
//!
//! There is no way back: a zip can't be rewritten byte for byte, and images replace these
//! backups. Zip times are local times without a zone, so entries have no `modified`.
//! The game isn't in the zip; the caller passes the instance's, and sets the image's
//! `created` and `label` (neither is part of the id) from the file's time and name.

use std::collections::HashSet;
use std::io::{Read, Seek};

use sha2::{Digest, Sha256};

use super::{finish, LegacyError, Note};
use crate::manifest::{Entry, Game, Image, Kind, Source, Withheld, WithheldReason, World};
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
    let prefix = format!("{folder}/");
    let mut image = Image::new(Kind::Data, game);
    let mut notes = Vec::new();
    let mut seen: HashSet<CanonicalPath> = HashSet::new();
    let mut declared = 0u64;
    let mut world_size = 0u64;
    for index in 0..archive.len() {
        let (name, outcome) = read_entry(&mut archive, index, &prefix, &mut declared)?;
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
        world_size = world_size.saturating_add(entry.size);
        image.entries.push(*entry);
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
    if is_link {
        return Ok((name, Outcome::Withhold(WithheldReason::Symlink)));
    }
    let Some(inner) = name.strip_prefix(prefix).filter(|rest| !rest.is_empty()) else {
        return Ok((name, Outcome::Withhold(WithheldReason::BadName)));
    };
    let Ok(path) = CanonicalPath::new(format!("saves/{prefix}{inner}")) else {
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
    entry.mode = match mode.map(|m| m & 0o7777) {
        Some(bits) if bits != 0 => bits,
        _ => 0o644,
    };
    Ok((name, Outcome::File(Box::new(entry))))
}

fn withhold(image: &mut Image, notes: &mut Vec<Note>, path: String, reason: WithheldReason) {
    notes.push(Note::NotListed {
        path: path.clone(),
        reason: reason.clone(),
    });
    image.withheld.push(Withheld { path, reason });
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
    fn unreadable_zips_and_folders_are_errors() {
        assert!(read(Cursor::new(b"PK\x03\x04 nope".to_vec()), "W", game()).is_err());
        let ok = zip_of(&[Item::File("W/level.dat", b"level")]);
        assert!(read(Cursor::new(ok.clone()), "../W", game()).is_err());
        assert!(read(Cursor::new(ok), "", game()).is_err());
        // A file and a folder of the same name can't both be in an image.
        let clash = zip_of(&[Item::File("W/a", b"1"), Item::File("W/a/b", b"2")]);
        assert!(read(Cursor::new(clash), "W", game()).is_err());
    }

    #[test]
    fn a_name_twice_is_listed_once() {
        // The zip writer refuses duplicate names, so rename one in the bytes.
        let mut bytes = zip_of(&[Item::File("W/a.dat", b"1"), Item::File("W/b.dat", b"2")]);
        let mut at = 0;
        while let Some(i) = bytes[at..].windows(7).position(|w| w == b"W/b.dat") {
            bytes[at + i + 2] = b'a';
            at += i + 7;
        }
        // The zip crate may already keep one of the two; either way the image lists the
        // name once, and the other is withheld if it got this far.
        let converted = read(Cursor::new(bytes), "W", game()).unwrap();
        assert_eq!(converted.image.entries.len(), 1);
        assert!(converted.image.withheld.iter().all(|w| w.path == "W/a.dat"));
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
