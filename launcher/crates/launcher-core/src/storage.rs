//! Versioned JSON documents, written atomically.
//!
//! Each file is a JSON object with a `"version"` number next to the document's own
//! fields. A file without one is version 0. Loading applies the document's
//! migrations in order, from the file's version to the current one, before it is
//! deserialized. A file from a newer launcher (a higher version) is refused and left
//! untouched, so an older launcher never overwrites data it does not understand.
//!
//! Writes go to a temporary file in the same directory, which is flushed to disk and
//! then renamed over the target. A crash or a failed write leaves the old file as it
//! was.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Map, Value};

/// Migrates a document's fields from one version to the next.
pub type Migration = fn(Map<String, Value>) -> anyhow::Result<Map<String, Value>>;

/// A document stored as a versioned JSON file.
pub trait Document: Serialize + DeserializeOwned + Default {
    /// The version this build writes.
    const VERSION: u32;
    /// `MIGRATIONS[n]` turns a version `n` document into version `n + 1`. It has
    /// exactly [`Document::VERSION`] entries.
    const MIGRATIONS: &'static [Migration];
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("could not read or write {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{path} is not a valid launcher file: {reason}")]
    Corrupt { path: PathBuf, reason: String },
    #[error(
        "{path} was written by a newer Lodestone Launcher (version {found}, this one reads up to {supported})"
    )]
    TooNew {
        path: PathBuf,
        found: u64,
        supported: u32,
    },
}

/// A document loaded from disk.
#[derive(Debug)]
pub struct Loaded<T> {
    pub document: T,
    /// The version in the file, or `None` if there was no file.
    pub file_version: Option<u64>,
}

impl<T> Loaded<T> {
    /// Whether the file was at an older version and should be written back.
    pub fn migrated(&self, current: u32) -> bool {
        self.file_version
            .is_some_and(|version| version < current as u64)
    }
}

/// Loads a document, returning the default if the file does not exist.
pub fn load<T: Document>(path: &Path) -> Result<Loaded<T>, StorageError> {
    let bytes = match fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(Loaded {
                document: T::default(),
                file_version: None,
            });
        }
        Err(source) => {
            return Err(StorageError::Io {
                path: path.to_owned(),
                source,
            });
        }
    };
    let corrupt = |reason: String| StorageError::Corrupt {
        path: path.to_owned(),
        reason,
    };
    let value: Value = serde_json::from_slice(&bytes).map_err(|err| corrupt(err.to_string()))?;
    let Value::Object(mut fields) = value else {
        return Err(corrupt("expected a JSON object".into()));
    };
    let version = match fields.remove("version") {
        None => 0,
        Some(Value::Number(number)) => number
            .as_u64()
            .ok_or_else(|| corrupt("\"version\" is not a whole number".into()))?,
        Some(_) => return Err(corrupt("\"version\" is not a number".into())),
    };
    if version > T::VERSION as u64 {
        return Err(StorageError::TooNew {
            path: path.to_owned(),
            found: version,
            supported: T::VERSION,
        });
    }
    let fields = migrate(fields, version as usize, T::MIGRATIONS)
        .map_err(|err| corrupt(format!("{err:#}")))?;
    let document =
        serde_json::from_value(Value::Object(fields)).map_err(|err| corrupt(err.to_string()))?;
    Ok(Loaded {
        document,
        file_version: Some(version),
    })
}

/// Applies `migrations[from..]` in order.
pub fn migrate(
    mut fields: Map<String, Value>,
    from: usize,
    migrations: &[Migration],
) -> anyhow::Result<Map<String, Value>> {
    for migration in migrations.iter().skip(from) {
        fields = migration(fields)?;
    }
    Ok(fields)
}

/// Writes a document at the current version, atomically.
pub fn save<T: Document>(path: &Path, document: &T) -> Result<(), StorageError> {
    let io_err = |source| StorageError::Io {
        path: path.to_owned(),
        source,
    };
    let Value::Object(fields) = serde_json::to_value(document).map_err(|err| io_err(err.into()))?
    else {
        return Err(io_err(io::Error::other("document is not a JSON object")));
    };
    let mut out = Map::new();
    out.insert("version".into(), T::VERSION.into());
    out.extend(fields);
    let mut bytes =
        serde_json::to_vec_pretty(&Value::Object(out)).map_err(|err| io_err(err.into()))?;
    bytes.push(b'\n');
    write_atomic(path, &bytes).map_err(io_err)
}

/// Replaces `path` with `contents` atomically, creating the parent directory.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    write_atomic_with(path, |file| file.write_all(contents))
}

/// Like [`write_atomic`], with `write` filling the temporary file. If `write`
/// fails, the temporary file is removed and `path` is left as it was.
pub fn write_atomic_with(
    path: &Path,
    write: impl FnOnce(&mut File) -> io::Result<()>,
) -> io::Result<()> {
    let dir = path
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;

    let (tmp_path, mut file) = create_temp_file(dir, name)?;
    let result = write(&mut file)
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            drop(file);
            fs::rename(&tmp_path, path)
        });
    if let Err(err) = result {
        let _ = fs::remove_file(&tmp_path);
        return Err(err);
    }
    // Flush the rename itself. Not every platform can open a directory for this.
    if let Ok(dir) = File::open(dir) {
        let _ = dir.sync_all();
    }
    Ok(())
}

fn create_temp_file(dir: &Path, name: &std::ffi::OsStr) -> io::Result<(PathBuf, File)> {
    for _ in 0..16 {
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(format!(
            ".{:016x}.tmp",
            u64::from_le_bytes(crate::random_bytes())
        ));
        let tmp_path = dir.join(tmp_name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            // The files hold account names, so keep them private to the user.
            options.mode(0o600);
        }
        match options.open(&tmp_path) {
            Ok(file) => return Ok((tmp_path, file)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "could not create a temporary file",
    ))
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;
    use serde_json::json;

    use super::*;

    fn files_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn atomic_write_replaces_the_file_and_leaves_no_temp_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested/data.json");
        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second, longer than the first").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second, longer than the first");
        assert_eq!(files_in(path.parent().unwrap()), ["data.json"]);
    }

    #[cfg(unix)]
    #[test]
    fn atomic_write_makes_the_file_private() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        write_atomic(&path, b"{}").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_failed_write_keeps_the_old_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data.json");
        write_atomic(&path, b"original").unwrap();

        let err = write_atomic_with(&path, |file| {
            file.write_all(b"half of the new con")?;
            Err(io::Error::other("disk full"))
        })
        .unwrap_err();
        assert_eq!(err.to_string(), "disk full");
        assert_eq!(fs::read(&path).unwrap(), b"original");
        assert_eq!(files_in(dir.path()), ["data.json"]);
    }

    /// A test document with two migrations: version 0 had `name`, version 1
    /// renamed it to `title`, and version 2 added `tags`.
    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Doc {
        title: String,
        tags: Vec<String>,
    }

    impl Document for Doc {
        const VERSION: u32 = 2;
        const MIGRATIONS: &'static [Migration] = &[
            |mut fields| {
                let name = fields.remove("name").unwrap_or_default();
                fields.insert("title".into(), name);
                Ok(fields)
            },
            |mut fields| {
                fields.insert("tags".into(), json!(["migrated"]));
                Ok(fields)
            },
        ];
    }

    #[test]
    fn missing_file_loads_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = load::<Doc>(&dir.path().join("none.json")).unwrap();
        assert_eq!(loaded.document, Doc::default());
        assert_eq!(loaded.file_version, None);
        assert!(!loaded.migrated(Doc::VERSION));
    }

    #[test]
    fn migrates_each_version_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.json");

        // Version 0: no "version" key.
        fs::write(&path, r#"{"name": "zero"}"#).unwrap();
        let loaded = load::<Doc>(&path).unwrap();
        assert!(loaded.migrated(Doc::VERSION));
        assert_eq!(
            loaded.document,
            Doc {
                title: "zero".into(),
                tags: vec!["migrated".into()],
            }
        );

        // Version 1 runs only the second migration.
        fs::write(&path, r#"{"version": 1, "title": "one"}"#).unwrap();
        let loaded = load::<Doc>(&path).unwrap();
        assert_eq!(loaded.file_version, Some(1));
        assert_eq!(loaded.document.title, "one");
        assert_eq!(loaded.document.tags, ["migrated"]);

        // The current version runs none.
        fs::write(&path, r#"{"version": 2, "title": "two", "tags": []}"#).unwrap();
        let loaded = load::<Doc>(&path).unwrap();
        assert!(!loaded.migrated(Doc::VERSION));
        assert!(loaded.document.tags.is_empty());
    }

    #[test]
    fn saves_with_the_current_version_and_reloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.json");
        let doc = Doc {
            title: "saved".into(),
            tags: vec!["a".into()],
        };
        save(&path, &doc).unwrap();
        let raw: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw, json!({"version": 2, "title": "saved", "tags": ["a"]}));
        assert_eq!(load::<Doc>(&path).unwrap().document, doc);
    }

    #[test]
    fn refuses_a_newer_version_and_leaves_the_file_alone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.json");
        let newer = r#"{"version": 3, "title": "from the future"}"#;
        fs::write(&path, newer).unwrap();
        let err = load::<Doc>(&path).unwrap_err();
        assert!(
            matches!(
                err,
                StorageError::TooNew {
                    found: 3,
                    supported: 2,
                    ..
                }
            ),
            "{err}"
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), newer);
    }

    #[test]
    fn reports_corrupt_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("doc.json");
        for contents in [
            "not json",
            "[1, 2]",
            r#"{"version": "2"}"#,
            r#"{"version": -1}"#,
            r#"{"version": 2, "title": 5, "tags": []}"#,
        ] {
            fs::write(&path, contents).unwrap();
            let err = load::<Doc>(&path).unwrap_err();
            assert!(
                matches!(err, StorageError::Corrupt { .. }),
                "{contents}: {err}"
            );
        }
    }
}
