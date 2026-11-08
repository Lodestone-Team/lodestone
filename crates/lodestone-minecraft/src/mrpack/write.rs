//! Making `.mrpack` files: the index Lodestone writes for a client pack, and the writer
//! (with the `zip` feature).

#[cfg(feature = "zip")]
use std::io::Write;
#[cfg(feature = "zip")]
use std::path::Path;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[cfg(feature = "zip")]
use crate::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFileHashes {
    pub sha1: String,
    pub sha512: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackFileEnv {
    pub client: String,
    pub server: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIndexFile {
    pub path: String,
    pub hashes: PackFileHashes,
    pub env: PackFileEnv,
    pub downloads: Vec<String>,
    pub file_size: u64,
}

/// `modrinth.index.json` of a client pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackIndex {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    pub summary: String,
    pub files: Vec<PackIndexFile>,
    pub dependencies: std::collections::BTreeMap<String, String>,
}

/// What a client pack holds.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientPackPlan {
    pub index: PackIndex,
    /// Jars that are not on Modrinth: their name and their path on disk. They go into
    /// `overrides/mods/`.
    pub included: Vec<(String, PathBuf)>,
}

/// Writes the pack to `out`: the index, then the included jars under `overrides/mods/`.
/// Nothing else goes into it.
#[cfg(feature = "zip")]
pub fn write_client_pack(plan: &ClientPackPlan, out: &Path) -> Result<(), Error> {
    let file =
        std::fs::File::create(out).map_err(Error::io("Failed to create the modpack file"))?;
    let mut writer = zip::ZipWriter::new(file);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    // jars are already compressed
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    let zip_error =
        |e: zip::result::ZipError| Error::Internal(format!("Failed to write the modpack: {e}"));
    writer
        .start_file(super::INDEX_FILE, deflated)
        .map_err(zip_error)?;
    let index = serde_json::to_vec_pretty(&plan.index).map_err(|source| Error::Json {
        context: "Failed to write the index".to_string(),
        source,
    })?;
    writer
        .write_all(&index)
        .map_err(Error::io("Failed to write the modpack"))?;
    for (name, path) in &plan.included {
        writer
            .start_file(format!("overrides/mods/{name}"), stored)
            .map_err(zip_error)?;
        let mut jar =
            std::fs::File::open(path).map_err(Error::io(format!("Failed to read {name}")))?;
        std::io::copy(&mut jar, &mut writer)
            .map_err(Error::io(format!("Failed to read {name}")))?;
    }
    writer
        .finish()
        .map_err(zip_error)?
        .sync_all()
        .map_err(Error::io("Failed to write the modpack"))?;
    Ok(())
}
