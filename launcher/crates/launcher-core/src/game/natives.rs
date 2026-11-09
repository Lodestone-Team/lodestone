//! Extracting old-style native jars into the natives directory.
//!
//! The jars come from Mojang and are checked by SHA-1, but extraction still refuses
//! anything that could write outside the natives directory: entries with absolute
//! paths, `..` components, backslashes or drive prefixes, and symbolic links.
//!
//! This module never calls zip's `ZipArchive::extract`: it writes each regular file
//! itself, with default permissions, so no symlink, SUID bit or file mode from the jar
//! reaches the disk. zip's `aes-crypto` feature is off.

use std::{
    fs::{self, File},
    io,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, thiserror::Error)]
pub enum NativesError {
    #[error("could not read {jar}: {reason}")]
    BadJar { jar: PathBuf, reason: String },
    #[error("{jar} has an unsafe entry {entry:?}")]
    UnsafeEntry { jar: PathBuf, entry: String },
    #[error("could not extract natives into {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// Extracts the files of `jar` into `dest`, skipping entries that start with one of
/// `exclude` (such as `META-INF/`) and directories. Returns the files written.
pub fn extract(jar: &Path, dest: &Path, exclude: &[String]) -> Result<Vec<PathBuf>, NativesError> {
    let bad_jar = |reason: String| NativesError::BadJar {
        jar: jar.to_owned(),
        reason,
    };
    let io_err = |path: &Path| {
        let path = path.to_owned();
        move |source| NativesError::Io { path, source }
    };
    let file = File::open(jar).map_err(|err| bad_jar(err.to_string()))?;
    let mut archive = zip::ZipArchive::new(std::io::BufReader::new(file))
        .map_err(|err| bad_jar(err.to_string()))?;
    fs::create_dir_all(dest).map_err(io_err(dest))?;

    // Check every entry before writing anything.
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|err| bad_jar(err.to_string()))?;
        let name = entry
            .name()
            .map_err(|err| bad_jar(err.to_string()))?
            .into_owned();
        if excluded(&name, exclude) {
            continue;
        }
        if is_symlink(entry.unix_mode()) || safe_entry_path(&name).is_none() {
            return Err(NativesError::UnsafeEntry {
                jar: jar.to_owned(),
                entry: name,
            });
        }
    }

    let mut written = Vec::new();
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| bad_jar(err.to_string()))?;
        let name = entry
            .name()
            .map_err(|err| bad_jar(err.to_string()))?
            .into_owned();
        if excluded(&name, exclude) || entry.is_dir() {
            continue;
        }
        // Checked again: the name is read anew, and must never be trusted unchecked.
        let Some(relative) = safe_entry_path(&name).filter(|_| !is_symlink(entry.unix_mode()))
        else {
            return Err(NativesError::UnsafeEntry {
                jar: jar.to_owned(),
                entry: name,
            });
        };
        let path = dest.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(io_err(parent))?;
        }
        let mut out = File::create(&path).map_err(io_err(&path))?;
        io::copy(&mut entry, &mut out).map_err(io_err(&path))?;
        written.push(path);
    }
    Ok(written)
}

fn excluded(name: &str, exclude: &[String]) -> bool {
    exclude
        .iter()
        .any(|prefix| name.starts_with(prefix.as_str()))
}

/// A zip entry's mode says symlink when its file type bits are `S_IFLNK`.
fn is_symlink(mode: Option<u32>) -> bool {
    mode.is_some_and(|mode| mode & 0o170000 == 0o120000)
}

/// The entry's path relative to the destination, or `None` if it is unsafe.
pub fn safe_entry_path(name: &str) -> Option<PathBuf> {
    if name.is_empty()
        || name.contains('\\')
        || name.contains('\0')
        || name.starts_with('/')
        || name.as_bytes().get(1) == Some(&b':')
    {
        return None;
    }
    let path = Path::new(name.trim_end_matches('/'));
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => clean.push(part),
            Component::CurDir => {}
            _ => return None,
        }
    }
    (!clean.as_os_str().is_empty()).then_some(clean)
}

/// Empties `dir` (or creates it), so a launch starts with only this version's natives.
pub fn reset_dir(dir: &Path) -> io::Result<()> {
    // Retried: on Windows a DLL can't be deleted while anything has it open, and
    // Defender scans the ones the last launch extracted; a game that just quit can
    // also take a moment to let go of them.
    crate::fsx::retry(|| match fs::remove_dir_all(dir) {
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        other => other,
    })?;
    fs::create_dir_all(dir)
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use zip::write::SimpleFileOptions as FileOptions;

    use super::*;

    /// Writes a jar with the given entries: (name, contents, is a symlink).
    fn jar(dir: &Path, entries: &[(&str, &[u8], bool)]) -> PathBuf {
        let path = dir.join("natives.jar");
        let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
        for (name, contents, symlink) in entries {
            if *symlink {
                writer
                    .add_symlink(
                        *name,
                        std::str::from_utf8(contents).unwrap(),
                        FileOptions::default(),
                    )
                    .unwrap();
            } else if name.ends_with('/') {
                writer.add_directory(*name, FileOptions::default()).unwrap();
            } else {
                writer.start_file(*name, FileOptions::default()).unwrap();
                writer.write_all(contents).unwrap();
            }
        }
        writer.finish().unwrap();
        path
    }

    #[test]
    fn extracts_files_and_skips_excluded_entries() {
        let dir = tempfile::tempdir().unwrap();
        let jar = jar(
            dir.path(),
            &[
                ("META-INF/", b"", false),
                ("META-INF/MANIFEST.MF", b"Manifest-Version: 1.0", false),
                ("liblwjgl.dylib", b"native code", false),
                ("macos/x64/org/lwjgl/libglfw.dylib", b"glfw", false),
            ],
        );
        let dest = dir.path().join("natives");
        let written = extract(&jar, &dest, &["META-INF/".to_string()]).unwrap();
        assert_eq!(written.len(), 2);
        assert_eq!(
            fs::read(dest.join("liblwjgl.dylib")).unwrap(),
            b"native code"
        );
        assert_eq!(
            fs::read(dest.join("macos/x64/org/lwjgl/libglfw.dylib")).unwrap(),
            b"glfw"
        );
        assert!(!dest.join("META-INF").exists());
    }

    #[test]
    fn rejects_unsafe_entries_before_writing_anything() {
        for bad in [
            "../escape.dylib",
            "a/../../escape.dylib",
            "/abs.dylib",
            "C:/win.dll",
            "a\\b.dll",
        ] {
            let dir = tempfile::tempdir().unwrap();
            let jar = jar(
                dir.path(),
                &[("ok.dylib", b"ok", false), (bad, b"evil", false)],
            );
            let dest = dir.path().join("natives");
            let err = extract(&jar, &dest, &[]).unwrap_err();
            assert!(
                matches!(err, NativesError::UnsafeEntry { .. }),
                "{bad}: {err}"
            );
            assert!(!dest.join("ok.dylib").exists(), "{bad}");
            assert!(!dir.path().join("escape.dylib").exists());
        }
    }

    #[test]
    fn rejects_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let jar = jar(dir.path(), &[("link.dylib", b"/etc/passwd", true)]);
        let err = extract(&jar, &dir.path().join("natives"), &[]).unwrap_err();
        assert!(matches!(err, NativesError::UnsafeEntry { .. }), "{err}");
        // A symlink under an excluded prefix is never extracted, so it is allowed.
        let jar2 = {
            let sub = dir.path().join("second");
            fs::create_dir_all(&sub).unwrap();
            self::jar(
                &sub,
                &[("META-INF/link", b"/etc", true), ("a.dylib", b"a", false)],
            )
        };
        extract(&jar2, &dir.path().join("n2"), &["META-INF/".into()]).unwrap();
    }

    #[test]
    fn rejects_a_file_that_is_not_a_jar() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("broken.jar");
        fs::write(&path, b"not a zip").unwrap();
        assert!(matches!(
            extract(&path, dir.path(), &[]),
            Err(NativesError::BadJar { .. })
        ));
    }

    #[test]
    fn entry_paths() {
        assert_eq!(
            safe_entry_path("a/b.dylib"),
            Some(PathBuf::from("a/b.dylib"))
        );
        assert_eq!(safe_entry_path("./a.dylib"), Some(PathBuf::from("a.dylib")));
        assert_eq!(safe_entry_path("dir/"), Some(PathBuf::from("dir")));
        for bad in ["", "/", "..", "../a", "a/..", "/a", "c:/a", "a\\..\\b"] {
            assert_eq!(safe_entry_path(bad), None, "{bad}");
        }
    }

    #[test]
    fn resets_the_natives_dir() {
        let dir = tempfile::tempdir().unwrap();
        let natives = dir.path().join("natives");
        fs::create_dir_all(natives.join("old")).unwrap();
        fs::write(natives.join("old/lib.dylib"), b"stale").unwrap();
        reset_dir(&natives).unwrap();
        assert!(natives.is_dir());
        assert_eq!(fs::read_dir(&natives).unwrap().count(), 0);
        reset_dir(&dir.path().join("fresh")).unwrap();
    }
}
