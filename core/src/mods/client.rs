//! Which installed mods players need too, and the client modpack (`.mrpack`) that gives
//! them those mods.

use std::collections::{HashMap, VecDeque};
use std::io::Write;
use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::metadata::JarEnvironment;
use super::scan::{scan_folder, ScannedFile};
use super::{
    bad_request, check_mod_file_name, match_files, InstalledModKind, InstalledModLoader, ModFolder,
    ModrinthMatch,
};
use crate::error::{Error, ErrorKind};
use crate::modrinth::{ModrinthApi, ModrinthDependencyType, ModrinthVersion, CDN_HOST};

/// Whether players must install a mod that is on the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
#[serde(rename_all = "snake_case")]
pub enum ClientRequirement {
    /// Players cannot join without it.
    Required,
    /// Players can join without it.
    Optional,
    /// It only runs on the server.
    Unsupported,
    Unknown,
}

impl ClientRequirement {
    /// From a Modrinth `client_side` value.
    pub fn from_modrinth(client_side: &str) -> Self {
        match client_side {
            "required" => ClientRequirement::Required,
            "optional" => ClientRequirement::Optional,
            "unsupported" => ClientRequirement::Unsupported,
            _ => ClientRequirement::Unknown,
        }
    }
}

/// The requirement of one mod on its own, before dependencies: plugins never; then
/// Modrinth's `client_side`; then the jar's `environment`.
pub fn base_requirement(
    loader: InstalledModLoader,
    file: &ScannedFile,
    matched: Option<&ModrinthMatch>,
) -> ClientRequirement {
    if loader.kind() == InstalledModKind::Plugin {
        return ClientRequirement::Unsupported;
    }
    if let Some(m) = matched {
        return ClientRequirement::from_modrinth(&m.client_side);
    }
    match file.environment {
        Some(JarEnvironment::Server) => ClientRequirement::Unsupported,
        Some(JarEnvironment::Client) => ClientRequirement::Required,
        None => ClientRequirement::Unknown,
    }
}

/// A scanned file with its Modrinth match and version, if any.
pub struct MatchedFile {
    pub file: ScannedFile,
    pub matched: Option<(ModrinthMatch, ModrinthVersion)>,
}

impl MatchedFile {
    /// The name shown to users, as in [`super::InstalledMod::display_name`].
    pub fn display_name(&self) -> &str {
        self.matched
            .as_ref()
            .map(|(m, _)| m.title.as_str())
            .or(self.file.metadata.as_ref().and_then(|m| m.name.as_deref()))
            .unwrap_or(&self.file.file_name)
    }
}

/// Fabric dependencies that are not mods players install.
const IGNORED_FABRIC_DEPENDENCIES: [&str; 3] = ["minecraft", "java", "fabricloader"];

/// The client requirement and `required_by` names of each file, in the same order.
///
/// An enabled mod that players need makes every installed, enabled mod it requires needed
/// too (by Modrinth project id for the Modrinth dependencies of its version, and by mod id
/// for `fabric.mod.json` `depends`), repeated until nothing changes. Disabled mods keep
/// their own requirement and never make other mods needed.
pub fn client_requirements(
    loader: InstalledModLoader,
    files: &[MatchedFile],
) -> Vec<(ClientRequirement, Vec<String>)> {
    let mut result: Vec<(ClientRequirement, Vec<String>)> = files
        .iter()
        .map(|f| {
            (
                base_requirement(loader, &f.file, f.matched.as_ref().map(|(m, _)| m)),
                Vec::new(),
            )
        })
        .collect();
    if loader.kind() == InstalledModKind::Plugin {
        return result;
    }

    let enabled: Vec<usize> = (0..files.len())
        .filter(|&i| files[i].file.enabled)
        .collect();
    let mut by_project: HashMap<&str, Vec<usize>> = HashMap::new();
    let mut by_mod_id: HashMap<&str, Vec<usize>> = HashMap::new();
    for &i in &enabled {
        if let Some((m, _)) = &files[i].matched {
            by_project.entry(m.project_id.as_str()).or_default().push(i);
        }
        if let Some(id) = files[i]
            .file
            .metadata
            .as_ref()
            .and_then(|m| m.id.as_deref())
        {
            by_mod_id.entry(id).or_default().push(i);
        }
    }
    let dependencies_of = |i: usize| -> Vec<usize> {
        let mut out = Vec::new();
        if let Some((_, version)) = &files[i].matched {
            for dependency in &version.dependencies {
                if dependency.dependency_type != ModrinthDependencyType::Required {
                    continue;
                }
                if let Some(found) = dependency
                    .project_id
                    .as_deref()
                    .and_then(|p| by_project.get(p))
                {
                    out.extend(found);
                }
            }
        }
        for id in &files[i].file.depends {
            if IGNORED_FABRIC_DEPENDENCIES.contains(&id.as_str()) {
                continue;
            }
            if let Some(found) = by_mod_id.get(id.as_str()) {
                out.extend(found);
            }
        }
        out.sort_unstable();
        out.dedup();
        out.retain(|&j| j != i);
        out
    };

    let mut queue: VecDeque<usize> = enabled
        .iter()
        .copied()
        .filter(|&i| result[i].0 == ClientRequirement::Required)
        .collect();
    // each mod is expanded once, so cycles end
    let mut expanded = vec![false; files.len()];
    while let Some(i) = queue.pop_front() {
        if std::mem::replace(&mut expanded[i], true) {
            continue;
        }
        let name = files[i].display_name().to_string();
        for j in dependencies_of(i) {
            let (requirement, required_by) = &mut result[j];
            if !required_by.contains(&name) {
                required_by.push(name.clone());
            }
            if *requirement != ClientRequirement::Required {
                *requirement = ClientRequirement::Required;
                queue.push_back(j);
            }
        }
    }
    result
}

/// Scans the folder and looks the files up on Modrinth.
pub async fn matched_files(
    api: &dyn ModrinthApi,
    folder: &ModFolder,
) -> Result<Vec<MatchedFile>, Error> {
    let files = scan_folder(folder).await?;
    let mut matches = match_files(api, &files).await?;
    Ok(files
        .into_iter()
        .map(|file| {
            let matched = matches.remove(&file.sha512);
            MatchedFile { file, matched }
        })
        .collect())
}

/// The response of `GET /instance/{uuid}/mods/client_pack`: a download key for
/// `GET /file/{key}`, like the file API's download links.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ClientPackDownload {
    pub key: String,
    /// The name the file downloads as.
    pub file_name: String,
    /// How many mods the pack gives players.
    pub mod_count: u32,
    /// How many of them are not on Modrinth, so their jars are inside the pack.
    pub included_jars: u32,
}

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

/// The CDN URL of the version file with this SHA-512, if it may go into a pack.
fn cdn_url(version: &ModrinthVersion, sha512: &str) -> Option<String> {
    let file = version
        .files
        .iter()
        .find(|f| f.sha512.eq_ignore_ascii_case(sha512))?;
    let url = url::Url::parse(&file.url).ok()?;
    (url.scheme() == "https" && url.host_str() == Some(CDN_HOST)).then(|| file.url.clone())
}

fn sha1_of(path: &Path) -> std::io::Result<String> {
    use sha1::Digest;
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = sha1::Sha1::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Plans the client pack of a Fabric instance named `instance_name`: one index entry per
/// enabled, needed mod that is on Modrinth, and the jars of the others.
pub fn plan_client_pack(
    folder: &ModFolder,
    files: &[MatchedFile],
    instance_name: &str,
    loader_version: &str,
) -> Result<ClientPackPlan, Error> {
    if folder.loader != InstalledModLoader::Fabric {
        return Err(bad_request(
            "Client modpacks are only available for Fabric servers",
        ));
    }
    let requirements = client_requirements(folder.loader, files);
    let mut entries = Vec::new();
    let mut included = Vec::new();
    for (file, (requirement, _)) in files.iter().zip(&requirements) {
        if !file.file.enabled || *requirement != ClientRequirement::Required {
            continue;
        }
        let name = &file.file.file_name;
        if check_mod_file_name(name).is_err() {
            return Err(bad_request(format!(
                "{name} cannot be put into a modpack. Rename it to a plain .jar file name"
            )));
        }
        let path = folder.dir.join(name);
        let url = file
            .matched
            .as_ref()
            .and_then(|(_, version)| cdn_url(version, &file.file.sha512));
        match url {
            Some(url) => {
                let sha1 = sha1_of(&path).with_context(|| format!("Failed to read {name}"))?;
                entries.push(PackIndexFile {
                    path: format!("mods/{name}"),
                    hashes: PackFileHashes {
                        sha1,
                        sha512: file.file.sha512.clone(),
                    },
                    env: PackFileEnv {
                        client: "required".into(),
                        server: "unsupported".into(),
                    },
                    downloads: vec![url],
                    file_size: file.file.size,
                });
            }
            None => included.push((name.clone(), path)),
        }
    }
    if entries.is_empty() && included.is_empty() {
        return Err(bad_request(
            "Players don't need any mods to join this server",
        ));
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    included.sort();
    let dependencies = [
        ("minecraft".to_string(), folder.game_version.clone()),
        ("fabric-loader".to_string(), loader_version.to_string()),
    ]
    .into_iter()
    .collect();
    Ok(ClientPackPlan {
        index: PackIndex {
            format_version: 1,
            game: "minecraft".into(),
            version_id: format!("{instance_name} (Lodestone server)"),
            name: format!("{instance_name} client mods"),
            summary: format!("Mods needed to join {instance_name}"),
            files: entries,
            dependencies,
        },
        included,
    })
}

/// Writes the pack to `out`: the index, then the included jars under `overrides/mods/`.
/// Nothing else goes into it.
pub fn write_client_pack(plan: &ClientPackPlan, out: &Path) -> Result<(), Error> {
    let file = std::fs::File::create(out).context("Failed to create the modpack file")?;
    let mut writer = zip::ZipWriter::new(file);
    let deflated = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    // jars are already compressed
    let stored = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Stored)
        .large_file(true);
    let zip_error = |e: zip::result::ZipError| eyre!("Failed to write the modpack: {e}");
    writer
        .start_file("modrinth.index.json", deflated)
        .map_err(zip_error)?;
    let index = serde_json::to_vec_pretty(&plan.index).context("Failed to write the index")?;
    writer
        .write_all(&index)
        .context("Failed to write the modpack")?;
    for (name, path) in &plan.included {
        writer
            .start_file(format!("overrides/mods/{name}"), stored)
            .map_err(zip_error)?;
        let mut jar =
            std::fs::File::open(path).with_context(|| format!("Failed to read {name}"))?;
        std::io::copy(&mut jar, &mut writer).with_context(|| format!("Failed to read {name}"))?;
    }
    writer
        .finish()
        .map_err(zip_error)?
        .sync_all()
        .context("Failed to write the modpack")?;
    Ok(())
}

/// Plans and writes a client pack into `out` on a blocking thread. Returns the plan.
pub async fn build_client_pack(
    api: &dyn ModrinthApi,
    folder: &ModFolder,
    instance_name: &str,
    loader_version: Option<&str>,
    out: &Path,
) -> Result<ClientPackPlan, Error> {
    if folder.loader != InstalledModLoader::Fabric {
        return Err(bad_request(
            "Client modpacks are only available for Fabric servers",
        ));
    }
    let loader_version = loader_version
        .ok_or_else(|| Error {
            kind: ErrorKind::Internal,
            source: eyre!("The Fabric loader version of this instance is unknown"),
        })?
        .to_string();
    let files = matched_files(api, folder).await?;
    let folder = folder.clone();
    let instance_name = instance_name.to_string();
    let out = out.to_path_buf();
    tokio::task::spawn_blocking(move || {
        let plan = plan_client_pack(&folder, &files, &instance_name, &loader_version)?;
        write_client_pack(&plan, &out)?;
        Ok(plan)
    })
    .await
    .map_err(|e| eyre!("The modpack task failed: {e}"))?
}

#[cfg(test)]
mod tests;
