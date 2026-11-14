//! The one collector of the backup folder: the image store's GC, which also keeps what the
//! core's older backup manifests name.
//!
//! The backup folder is shared: schema-1 backup manifests (`servers/<uuid>/<id>.json`) and
//! images (`images/<uuid>/<id>.json`) name objects in the same `objects/`. Once the image
//! store has written there, it has put its downgrade guard in `servers/.images/`, which the
//! core's old collector refuses to run past, so from then on this is the only collector.
//! [`ImageService::collect_blocking`] runs the image store's mark and sweep with a reader of
//! the older manifests ([`older_objects`]), which keeps:
//!
//! - every object of a schema-1 backup manifest, as the old collector did;
//! - the objects a schema-2 backup record names besides its image: the files the image
//!   can't hold (`unmapped`) and those carried over from an earlier backup (`carried`).
//!   Neither is ever claimed by saving the image, so only this reader keeps them.
//!
//! Anything else in a server's folder that ends in `.json` (other than the policy and a
//! restore's journal), unreadable or of another schema, refuses the whole collection:
//! its objects can't be told apart from garbage.
//!
//! Locks: the caller holds the backups' store lock exclusively around this (never inside
//! the reader); the store's collection lock is taken by the store itself, after it. The
//! reader takes neither, as `Store::gc_with_older_formats` requires.

use std::collections::HashSet;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use lodestone_image::store::gc::{GcError, GcOptions, GcReport};
use lodestone_image::store::guard::GUARD_FOLDER;
use serde::Deserialize;

use crate::error::Error;

use super::ImageService;

/// The schema of the backups' own records that sit beside their images.
pub const RECORD_SCHEMA: u32 = 2;

/// Files in a server's backup folder that are no backup.
const NOT_BACKUPS: [&str; 2] = ["policy.json", "restore-journal.json"];
/// An old manifest or record is at most this large (a million files fit).
const MAX_MANIFEST_BYTES: u64 = 512 << 20;

impl ImageService {
    /// Deletes what neither an image nor an older backup manifest uses (see the module).
    /// Blocking. The caller holds the backups' store lock exclusively around this call.
    pub fn collect_blocking(&self) -> Result<GcReport, Error> {
        self.collect_with(&GcOptions::default())
    }

    pub(crate) fn collect_with(&self, options: &GcOptions) -> Result<GcReport, Error> {
        let root = self.root().to_path_buf();
        self.store()
            .gc_with_older_formats(options, || older_objects(&root))
            .map_err(|e| Error::from(color_eyre::eyre::eyre!("Backup cleanup: {e}")))
    }
}

/// Only what keeping a schema-1 manifest's objects needs, read as leniently as the old
/// collector read it, so every manifest it kept is kept here too.
#[derive(Deserialize)]
struct SchemaOne {
    #[serde(default)]
    files: Vec<Named>,
}

/// Only the objects a schema-2 backup record names besides its image.
#[derive(Deserialize)]
struct RecordObjects {
    #[serde(default)]
    unmapped: Option<Vec<Named>>,
    #[serde(default)]
    carried: Option<Vec<Named>>,
}

#[derive(Deserialize)]
struct Named {
    sha256: String,
}

#[derive(Deserialize)]
struct SchemaProbe {
    #[serde(default)]
    schema: Option<u64>,
}

/// The objects the older manifests and the backup records under `<backups_root>/servers`
/// name (see the module), or why none can be deleted. Takes no lock.
pub fn older_objects(backups_root: &Path) -> Result<HashSet<String>, GcError> {
    let mut objects = HashSet::new();
    let servers = backups_root.join("servers");
    let entries = match fs::read_dir(&servers) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(objects),
        Err(e) => {
            return Err(GcError::Io {
                what: "Failed to list the backups".into(),
                source: e,
            })
        }
    };
    for server in entries {
        let server = server.map_err(|e| GcError::Io {
            what: "Failed to list the backups".into(),
            source: e,
        })?;
        if server.file_name() == GUARD_FOLDER || !server.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let files = fs::read_dir(server.path()).map_err(|e| refused(&server.path(), &e))?;
        for file in files {
            let file = file.map_err(|e| refused(&server.path(), &e))?;
            let name = file.file_name();
            let Some(name) = name.to_str() else { continue };
            if !name.ends_with(".json") || NOT_BACKUPS.contains(&name) {
                continue;
            }
            let path = file.path();
            let bytes = read_capped(&path).map_err(|why| GcError::Refused {
                path: path.display().to_string(),
                why,
            })?;
            named_objects(&bytes, &mut objects).map_err(|why| GcError::Refused {
                path: path.display().to_string(),
                why,
            })?;
        }
    }
    Ok(objects)
}

fn refused(path: &Path, e: &io::Error) -> GcError {
    GcError::Refused {
        path: path.display().to_string(),
        why: format!("can't be listed ({e})"),
    }
}

fn read_capped(path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| format!("can't be read ({e})"))?;
    let mut bytes = Vec::new();
    file.take(MAX_MANIFEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("can't be read ({e})"))?;
    if bytes.len() as u64 > MAX_MANIFEST_BYTES {
        return Err("is too large to be a backup".into());
    }
    Ok(bytes)
}

/// Adds the objects one old manifest or record names to `objects`.
fn named_objects(bytes: &[u8], objects: &mut HashSet<String>) -> Result<(), String> {
    // A JSON object, not anything serde would take for a struct (a list).
    if bytes.iter().find(|b| !b.is_ascii_whitespace()) != Some(&b'{') {
        return Err("isn't a backup manifest".into());
    }
    let probe: SchemaProbe =
        serde_json::from_slice(bytes).map_err(|e| format!("can't be parsed ({e})"))?;
    match probe.schema {
        // The old collector read a missing schema as 0 and kept its files.
        None | Some(0) | Some(1) => {
            let manifest: SchemaOne =
                serde_json::from_slice(bytes).map_err(|e| format!("can't be parsed ({e})"))?;
            objects.extend(manifest.files.into_iter().map(|f| f.sha256));
        }
        Some(schema) if schema == u64::from(RECORD_SCHEMA) => {
            let record: RecordObjects =
                serde_json::from_slice(bytes).map_err(|e| format!("can't be parsed ({e})"))?;
            let named = record.unmapped.into_iter().chain(record.carried).flatten();
            objects.extend(named.map(|f| f.sha256));
        }
        Some(schema) => {
            return Err(format!(
                "has schema {schema}, which this Lodestone doesn't know (made by a newer one?)"
            ))
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn older_objects_reads_v1_manifests_and_records() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(older_objects(root).unwrap().is_empty());
        let servers = root.join("servers");
        write(
            &servers.join("INSTANCE_a/20260101-000000-abcd.json"),
            br#"{"schema":1,"id":"x","files":[{"path":"a","sha256":"aa","size":1}],"skipped":[]}"#,
        );
        write(
            &servers.join("INSTANCE_a/20260102-000000-abcd.json"),
            br#"{"schema":2,"image":"i","unmapped":[{"disk":"world/DIM-1/r","sha256":"bb","size":2}],"carried":[{"path":"c","sha256":"cc"}]}"#,
        );
        write(&servers.join("INSTANCE_a/policy.json"), br#"{"schema":9}"#);
        write(&servers.join("INSTANCE_a/restore-journal.json"), b"garbage");
        write(&servers.join("INSTANCE_a/full-scan.count"), b"3");
        // The guard is schema 2 with no record fields: skipped as a folder anyway.
        write(
            &servers.join(".images/images.json"),
            lodestone_image::store::guard::CORE_GUARD.as_bytes(),
        );
        let objects = older_objects(root).unwrap();
        let mut sorted: Vec<&str> = objects.iter().map(String::as_str).collect();
        sorted.sort();
        assert_eq!(sorted, vec!["aa", "bb", "cc"]);

        // Anything unreadable, or another schema, refuses.
        for bad in [&b"{oops"[..], br#"{"schema":3,"files":[]}"#, b"[]", b""] {
            write(&servers.join("INSTANCE_b/x.json"), bad);
            assert!(
                matches!(older_objects(root), Err(GcError::Refused { .. })),
                "{}",
                String::from_utf8_lossy(bad)
            );
        }
    }
}
