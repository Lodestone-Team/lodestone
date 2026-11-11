//! Which installed mods players need too, and the client modpack (`.mrpack`) that gives
//! them those mods.
//!
//! The owner can mark a mod as needed by players ([`set_marked`]), for jars whose need
//! nothing says: uploaded jars that aren't on Modrinth and don't name an `environment`,
//! or Modrinth mods that are optional for players. The marks are kept in the mods folder
//! ([`MARKS_FILE`]) by the mod's enabled file name, so disabling, enabling and uploading
//! a new jar under the same name keep them.

use std::collections::{BTreeSet, HashMap, VecDeque};
use std::path::Path;

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::metadata::JarEnvironment;
use super::scan::{scan_folder, ScannedFile};
use super::{
    bad_request, check_mod_file_name, enabled_name, existing_mod_path, match_files,
    InstalledModKind, InstalledModLoader, ModFolder, ModrinthMatch,
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

/// The file in the mods folder with the mods the owner marked as needed by players.
/// Loaders only load `.jar` files, so it's never taken for a mod.
pub const MARKS_FILE: &str = ".lodestone_client_mods.json";

/// [`MARKS_FILE`]'s contents.
#[derive(Debug, Default, Serialize, Deserialize)]
struct ClientMarks {
    /// Enabled file names (without `.disabled`).
    #[serde(default)]
    required: BTreeSet<String>,
}

/// The enabled file names of the mods marked as needed by players. A missing file is
/// none; a damaged one is none too, with a warning.
pub async fn read_marks(folder: &ModFolder) -> BTreeSet<String> {
    if folder.kind() != InstalledModKind::Mod {
        return BTreeSet::new();
    }
    let path = folder.dir.join(MARKS_FILE);
    match tokio::fs::read_to_string(&path).await {
        Ok(text) => match serde_json::from_str::<ClientMarks>(&text) {
            Ok(marks) => marks.required,
            Err(e) => {
                tracing::warn!(
                    "{} is damaged, so no mods are marked as needed by players: {e}",
                    path.display()
                );
                BTreeSet::new()
            }
        },
        Err(_) => BTreeSet::new(),
    }
}

async fn write_marks(folder: &ModFolder, required: BTreeSet<String>) -> Result<(), Error> {
    let path = folder.dir.join(MARKS_FILE);
    if required.is_empty() {
        return match tokio::fs::remove_file(&path).await {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
                Err(Error::from(eyre!("Failed to update {MARKS_FILE}: {e}")))
            }
            _ => Ok(()),
        };
    }
    let text = serde_json::to_string_pretty(&ClientMarks { required }).expect("serializable");
    crate::persist::write_atomic(&path, text, None).await
}

/// Marks the mod `file_name` as needed by players, or takes the mark off (then its need
/// comes from Modrinth, the jar and other mods again). Only mods, not plugins.
pub async fn set_marked(folder: &ModFolder, file_name: &str, required: bool) -> Result<(), Error> {
    if folder.kind() != InstalledModKind::Mod {
        return Err(bad_request("Plugins only run on the server"));
    }
    existing_mod_path(folder, file_name).await?;
    let mut marks = read_marks(folder).await;
    let name = enabled_name(file_name);
    let changed = if required {
        marks.insert(name)
    } else {
        marks.remove(&name)
    };
    if changed {
        write_marks(folder, marks).await?;
    }
    Ok(())
}

/// Takes the mark off a mod that was deleted, so a later jar under its name isn't
/// marked by surprise.
pub async fn forget_mark(folder: &ModFolder, file_name: &str) -> Result<(), Error> {
    let mut marks = read_marks(folder).await;
    if marks.remove(&enabled_name(file_name)) {
        write_marks(folder, marks).await?;
    }
    Ok(())
}

/// The requirement of one mod on its own, before dependencies: plugins never; then the
/// owner's mark; then Modrinth's `client_side`; then the jar's `environment`.
pub fn base_requirement(
    loader: InstalledModLoader,
    file: &ScannedFile,
    matched: Option<&ModrinthMatch>,
    marked: bool,
) -> ClientRequirement {
    if loader.kind() == InstalledModKind::Plugin {
        return ClientRequirement::Unsupported;
    }
    if marked {
        return ClientRequirement::Required;
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
    /// The owner marked it as needed by players ([`set_marked`]).
    pub marked: bool,
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
                base_requirement(
                    loader,
                    &f.file,
                    f.matched.as_ref().map(|(m, _)| m),
                    f.marked,
                ),
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
    let marks = read_marks(folder).await;
    Ok(files
        .into_iter()
        .map(|file| {
            let matched = matches.remove(&file.sha512);
            let marked = marks.contains(&enabled_name(&file.file_name));
            MatchedFile {
                file,
                matched,
                marked,
            }
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

pub use lodestone_minecraft::mrpack::write::{
    write_client_pack, ClientPackPlan, PackFileEnv, PackFileHashes, PackIndex, PackIndexFile,
};

/// The CDN URL of the version file with this SHA-512, if it may go into a pack.
fn cdn_url(version: &ModrinthVersion, sha512: &str) -> Option<String> {
    let file = version
        .files
        .iter()
        .find(|f| f.sha512.eq_ignore_ascii_case(sha512))?;
    let url = url::Url::parse(&file.url).ok()?;
    (url.scheme() == "https" && url.host_str() == Some(CDN_HOST)).then(|| file.url.clone())
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
                let sha1 = lodestone_minecraft::hash::sha1_of(&path)
                    .with_context(|| format!("Failed to read {name}"))?;
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
            source: eyre!("The Fabric loader version of this server is unknown"),
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
