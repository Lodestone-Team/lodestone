//! A server's client pack in a launcher instance, through the modpack import and update
//! ([`crate::modpack`]): "Create matching instance" imports the pack as a new instance
//! (mods, overrides and configs, `modpack.json` and `content.json`, staged and renamed into
//! place), and "keep mods in step" plans an update from the pack the instance has to the
//! server's current one.
//!
//! The server is the authority on mods: a mod of the pack the user changed is replaced
//! (see [`update::plan_with`]), so the instance can join. Their own mods, a disabled mod
//! of the pack, configs they changed, worlds and options stay as they are.

use std::{
    fs,
    path::{Path, PathBuf},
};

use super::{
    matching_loader,
    models::ServerInstance,
    trust::{FileRole, PulledFile, file_role},
};
use crate::{
    content::{InstanceDirs, disabled_name, is_disabled_name},
    game::loaders::LoaderChoice,
    modpack::{
        self, ModpackError, ModpackRecord, PackInfo, io_error,
        update::{self, UpdatePlan},
    },
    storage,
};

/// The folders whose files must match the server's pack.
pub const AUTHORITATIVE: [&str; 1] = ["mods"];

/// Writes a downloaded pack to a hidden file in `dir`, for [`modpack::read_pack`]. The
/// caller removes it when done.
pub fn write_pack(dir: &Path, bytes: &[u8]) -> Result<PathBuf, ModpackError> {
    fs::create_dir_all(dir).map_err(io_error(dir))?;
    let path = dir.join(format!(
        ".lodestone-server-pack-{:016x}.mrpack",
        u64::from_le_bytes(crate::random_bytes())
    ));
    storage::write_atomic(&path, bytes).map_err(io_error(&path))?;
    Ok(path)
}

/// The record of a server's pack in `modpack.json`: not a Modrinth project, so the
/// modpack page offers no Modrinth update; the server link does that.
pub fn record(pack: &PackInfo) -> ModpackRecord {
    ModpackRecord {
        title: pack.name.clone(),
        version_number: pack.version_number.clone().unwrap_or_default(),
        ..ModpackRecord::default()
    }
}

/// The loader the instance needs: the pack's, mapped as the modpack import maps it,
/// else the server's (a server without a client pack).
pub fn loader_for(
    pack: Option<&PackInfo>,
    server: &ServerInstance,
) -> Result<Option<LoaderChoice>, String> {
    match pack.and_then(|p| p.loader.as_ref().map(|l| (p, l))) {
        Some((pack, (kind, version))) => {
            modpack::loader_choice(*kind, version, &pack.game_version, None).map(Some)
        }
        None => matching_loader(server),
    }
}

/// The plan to bring the instance at `dirs` in step with the server's `pack`. A mod a
/// pack the user added has, which the server's pack now has too, is removed, so the
/// game doesn't load two versions of it (see [`modpack::addon`]).
pub fn plan(dirs: &InstanceDirs, pack: &PackInfo, loader: Option<LoaderChoice>) -> UpdatePlan {
    let current = ModpackRecord::load(&dirs.instance).unwrap_or_default();
    let mut plan = update::plan_with(dirs, &current, pack, record(pack), loader, &AUTHORITATIVE);
    plan.remove_also(modpack::addon::superseded(dirs, pack));
    plan
}

fn sha1_at(path: &Path) -> Option<String> {
    fs::symlink_metadata(path)
        .ok()
        .filter(|m| m.is_file())
        .and_then(|_| lodestone_minecraft::hash::sha1_of(path).ok())
}

/// Whether the mods `modpack.json` records are on disk as the pack wrote them (a mod the
/// user disabled counts). `false` without a record.
pub fn mods_on_disk(dirs: &InstanceDirs) -> bool {
    let Some(record) = ModpackRecord::load(&dirs.instance) else {
        return false;
    };
    record
        .files
        .iter()
        .filter(|(path, _)| {
            AUTHORITATIVE
                .iter()
                .any(|d| path.starts_with(&format!("{d}/")))
        })
        .all(|(path, file)| {
            let mut candidates = vec![path.clone()];
            if !is_disabled_name(path) {
                candidates.push(disabled_name(path));
            }
            candidates.iter().any(|p| {
                sha1_at(&dirs.game.join(p)).is_some_and(|h| h.eq_ignore_ascii_case(&file.sha1))
            })
        })
}

/// Every file importing `pack` installs, for the advisory.
pub fn pack_pulls(pack: &PackInfo) -> Vec<PulledFile> {
    let mut files: Vec<PulledFile> = pack
        .downloads
        .iter()
        .map(|d| PulledFile {
            path: modpack::slash_path(&d.path),
            sha1: d.sha1.clone(),
            sha512: Some(d.sha512.clone()),
        })
        .collect();
    files.extend(pack.overrides.iter().map(|o| PulledFile {
        path: o.path.clone(),
        sha1: o.sha1.clone(),
        sha512: None,
    }));
    files
}

/// The files `plan` writes, for the advisory.
pub fn plan_pulls(plan: &UpdatePlan) -> Vec<PulledFile> {
    let mut files: Vec<PulledFile> = plan
        .downloads()
        .map(|(d, path)| PulledFile {
            path: path.to_string(),
            sha1: d.sha1.clone(),
            sha512: Some(d.sha512.clone()),
        })
        .collect();
    files.extend(plan.override_writes().map(|(o, path)| PulledFile {
        path: path.to_string(),
        sha1: o.sha1.clone(),
        sha512: None,
    }));
    files
}

/// What bringing an instance in step changes, by path in the game folder.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackChanges {
    pub added: Vec<String>,
    pub updated: Vec<String>,
    pub removed: Vec<String>,
    /// Configs and other files of the pack written.
    pub other: Vec<String>,
}

impl PackChanges {
    pub fn from_plan(plan: &UpdatePlan) -> Self {
        let is_mod = |p: &String| file_role(p) == FileRole::Code;
        let mut changes = PackChanges {
            added: plan.added.clone(),
            updated: plan
                .updated
                .iter()
                .map(|(_, new)| new.clone())
                .chain(plan.replaced_own.iter().cloned())
                .collect(),
            removed: plan.removed.clone(),
            other: Vec::new(),
        };
        for path in &plan.overrides_written {
            if is_mod(path) {
                changes.updated.push(path.clone());
            } else {
                changes.other.push(path.clone());
            }
        }
        changes.updated.sort();
        changes.updated.dedup();
        changes
    }

    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.updated.is_empty()
            && self.removed.is_empty()
            && self.other.is_empty()
    }

    /// "2 added, 1 updated, 1 removed, 3 other files".
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        for (count, word) in [
            (self.added.len(), "added"),
            (self.updated.len(), "updated"),
            (self.removed.len(), "removed"),
        ] {
            if count > 0 {
                parts.push(format!("{count} {word}"));
            }
        }
        match self.other.len() {
            0 => {}
            1 => parts.push("1 other file".into()),
            n => parts.push(format!("{n} other files")),
        }
        if parts.is_empty() {
            "no changes".into()
        } else {
            parts.join(", ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries() {
        let changes = PackChanges {
            added: vec!["mods/b.jar".into()],
            updated: vec!["mods/a.jar".into()],
            removed: vec!["mods/old.jar".into()],
            other: vec!["config/a.toml".into(), "config/b.toml".into()],
        };
        assert_eq!(
            changes.summary(),
            "1 added, 1 updated, 1 removed, 2 other files"
        );
        assert_eq!(PackChanges::default().summary(), "no changes");
        assert!(PackChanges::default().is_empty());
    }
}
