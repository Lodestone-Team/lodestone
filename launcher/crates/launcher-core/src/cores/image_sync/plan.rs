//! What installing a server's view does to an instance, worked out before anything
//! changes and shown to the player: the three-way merge between what the last applied
//! view placed (`modpack.json`), what this one places ([`super::placed`]) and the disk
//! ([`crate::modpack::merge`]), with the server's `mods/` authoritative (a mod of the
//! server's that the player changed is replaced, so they can join) and everything else
//! the player changed kept: their own mods, configs they edited, a pack config the server
//! dropped.
//!
//! The instance's Minecraft version and loader are fixed: a view for another one is
//! refused for an existing instance ([`SyncError::GameChanged`]). The loader version
//! follows the server's.

use std::collections::{BTreeSet, HashMap};

use lodestone_image::{Image, Loader, Source, WithheldReason, display_text};

use super::{
    SyncError,
    base::BasePack,
    placed::{Placed, projects_and_paths},
};
use crate::{
    content::{InstanceDirs, disabled_name},
    cores::trust::{FileRole, PulledFile, file_role},
    game::loaders::{LoaderChoice, LoaderKind},
    instances::{self, Instance},
    modpack::{
        ModpackRecord, addon,
        merge::{self, Fetch},
        update::{self, UpdatePlan},
    },
};

/// The folders whose files must match the server's view.
pub const AUTHORITATIVE: [&str; 1] = ["mods"];

/// The plan for one view, and what to tell the player about it.
#[derive(Clone, Debug)]
pub struct ServerPlan {
    /// The checked view this installs.
    pub view: Image,
    pub update: UpdatePlan,
    pub summary: Summary,
    /// The loader the instance runs: the view's.
    pub loader: Option<LoaderChoice>,
}

/// What the player sees before a server's files are installed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    /// The base pack's name and version, when the server is built on one.
    pub base: Option<(String, String)>,
    /// The Minecraft version and loader ("Fabric 0.16.10"), the server's.
    pub minecraft: String,
    pub loader: String,
    /// Mods the server has that its base pack doesn't (file names).
    pub added_mods: Vec<String>,
    /// Files only the server can give (it uploaded them, or changed them): paths.
    pub server_only: Vec<String>,
    /// Files the server has that its view can't give and the player would lack: shown
    /// path and why.
    pub missing: Vec<(String, WithheldReason)>,
    /// CurseForge files the player must download by hand: paths.
    pub curseforge_listed: Vec<String>,
    /// What changes on disk.
    pub changes: PackChanges,
    /// The bytes downloaded (from the server, Modrinth and CurseForge; not the base
    /// pack's own files, copied out of the pack), shown before anything is installed.
    pub download_bytes: u64,
}

/// The loader a view's game needs, as the instance names it. Forge's versions are full
/// Maven versions there (`1.20.1-47.4.28`).
pub fn loader_of(view: &Image) -> Result<Option<LoaderChoice>, SyncError> {
    let game = &view.game;
    let kind = match &game.loader {
        Loader::Vanilla => return Ok(None),
        Loader::Fabric => LoaderKind::Fabric,
        Loader::Quilt => LoaderKind::Quilt,
        Loader::Forge => LoaderKind::Forge,
        Loader::NeoForge => LoaderKind::NeoForge,
        Loader::Other(other) => {
            return Err(SyncError::Refused(format!(
                "it runs the mod loader {}, which {} doesn't know",
                display_text(other),
                crate::wording::app_name()
            )));
        }
    };
    let Some(version) = game
        .loader_version
        .as_deref()
        .map(str::trim)
        .filter(|v| !v.is_empty())
    else {
        return Err(SyncError::Refused(format!(
            "it doesn't say which {} version it runs",
            kind.label()
        )));
    };
    if !lodestone_minecraft::modrinth::search::is_valid_filter_value(version)
        || !lodestone_minecraft::modrinth::search::is_valid_filter_value(&game.minecraft)
    {
        return Err(SyncError::Refused("its game version isn't valid".into()));
    }
    let version = match kind {
        LoaderKind::Forge if !version.starts_with(&format!("{}-", game.minecraft)) => {
            format!("{}-{version}", game.minecraft)
        }
        _ => version.to_string(),
    };
    Ok(Some(LoaderChoice { kind, version }))
}

/// Refuses `view` for `instance` when it names another Minecraft version or loader: the
/// instance's game is fixed ([`SyncError::GameChanged`]). Checked as soon as the view is,
/// before its base pack is downloaded or CurseForge asked, so a view that will be refused
/// costs nothing.
pub fn check_game(instance: &Instance, view: &Image) -> Result<(), SyncError> {
    let loader = loader_of(view)?;
    let kind = loader.as_ref().map(|l| l.kind);
    if instance.game_version == view.game.minecraft
        && instance.loader == instances::Loader::from_kind(kind)
    {
        return Ok(());
    }
    Err(SyncError::GameChanged {
        server_game: view.game.minecraft.clone(),
        server_loader: loader_label(loader.as_ref()),
        instance_game: instance.game_version.clone(),
        instance_loader: instance.loader_label(),
    })
}

/// "Fabric 0.16.10", or "Vanilla".
fn loader_label(loader: Option<&LoaderChoice>) -> String {
    match loader {
        Some(loader) => format!("{} {}", loader.kind.label(), loader.version),
        None => "Vanilla".into(),
    }
}

/// The plan to bring an instance to `view`, which places `placed` over its base pack
/// `base`: the instance at `existing` (its folders, and its `instance.json`), whose
/// `modpack.json` is `current`, or a new one (`None`). `name` names the server, for a
/// server without a base pack. A file whose recorded hash is the view's and whose size is
/// right isn't read (`merge::Rules::trust_recorded`), but those at `untrusted` (mods the
/// check on a 204 found missing or changed) are.
pub fn plan(
    existing: Option<(&InstanceDirs, &Instance)>,
    current: &ModpackRecord,
    view: Image,
    placed: Placed,
    base: Option<&BasePack>,
    name: &str,
    untrusted: &BTreeSet<String>,
) -> Result<ServerPlan, SyncError> {
    let loader = loader_of(&view)?;
    if let Some((_, instance)) = existing {
        check_game(instance, &view)?;
    }
    // The base pack's name and version come from its index, which the server chose: shown
    // (the summary, the progress, the instance's page) without characters that could span
    // lines or reverse text.
    let shown = |text: &str| display_text(text).into_owned();
    let base_name = base.map(|b| {
        (
            shown(&b.pack.name),
            shown(b.pack.version_number.as_deref().unwrap_or_default()),
        )
    });
    let record = ModpackRecord {
        title: base_name
            .as_ref()
            .map_or_else(|| name.to_string(), |(pack, _)| pack.clone()),
        version_number: base_name
            .as_ref()
            .map(|(_, version)| version.clone())
            .unwrap_or_default(),
        project_id: base.map(|b| b.project.clone()).unwrap_or_default(),
        version_id: base.map(|b| b.version.clone()).unwrap_or_default(),
        pack_sha512: base.map(|b| b.sha512.clone()),
        from_server: true,
        ..ModpackRecord::default()
    };
    let dirs = existing.map(|(dirs, _)| dirs);
    let mut update = update::plan_files(
        dirs.map(|d| d.game.as_path()),
        current,
        &placed.files,
        record,
        merge::Rules {
            authoritative: &AUTHORITATIVE,
            trust_recorded: true,
            untrusted: Some(untrusted),
            dropped_overrides_go: true,
        },
    );
    update.game_version = view.game.minecraft.clone();
    update.loader = loader.clone();
    // A CurseForge file to get by hand that the player has, as the view names it, isn't
    // missing.
    let has = |path: &str, sha256: &str| {
        current
            .files
            .get(path)
            .and_then(|file| file.sha256.as_deref())
            .is_some_and(|have| have.eq_ignore_ascii_case(sha256))
    };
    // each entry's SHA-256 by path (paths are unique in a checked view), looked up once
    // per listed file rather than by a walk of every entry
    let sha256_of: HashMap<&str, &str> = view
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e.sha256.as_str()))
        .collect();
    let curseforge_listed: Vec<String> = placed
        .curseforge_listed
        .iter()
        .map(|(path, _, _)| path.clone())
        .filter(|path| {
            !sha256_of
                .get(path.as_str())
                .is_some_and(|sha256| has(path, sha256))
        })
        .collect();
    // Files the view can't give (withheld, or CurseForge's to get by hand) stay on disk
    // when the player has them, still recorded: removing them would leave the player
    // without a file the server still has.
    let cant_give: BTreeSet<String> = placed
        .missing
        .iter()
        .map(|w| w.path.clone())
        .chain(
            placed
                .curseforge_listed
                .iter()
                .map(|(path, _, _)| path.clone()),
        )
        .collect();
    let game = dirs.map(|d| d.game.as_path());
    update.keep_files(&cant_give, current, |path| {
        game.is_some_and(|game| {
            [path.to_string(), disabled_name(path)]
                .iter()
                .any(|p| std::fs::symlink_metadata(game.join(p)).is_ok_and(|m| m.is_file()))
        })
    });
    // A mod a pack the player added has, which the server's view has too, goes, so the
    // game doesn't load two versions of it.
    if let Some(dirs) = dirs {
        let (projects, paths) = projects_and_paths(&placed);
        update.remove_also(addon::superseded_by(dirs, &paths, &projects));
    }

    let summary = Summary {
        base: base_name,
        minecraft: view.game.minecraft.clone(),
        loader: loader_label(loader.as_ref()),
        added_mods: view
            .entries
            .iter()
            .filter(|e| e.path.first().eq_ignore_ascii_case("mods"))
            .filter(|e| !placed.from_base.contains(e.path.as_str()))
            .filter(|e| placed.files.contains_key(e.path.as_str()))
            .map(|e| e.path.file_name().to_string())
            .collect(),
        server_only: view
            .entries
            .iter()
            .filter(|e| matches!(e.source, Source::Object {}))
            .map(|e| e.path.as_str().to_string())
            .collect(),
        missing: placed
            .missing
            .iter()
            .map(|w| (w.display_path().into_owned(), w.reason.clone()))
            .collect(),
        curseforge_listed,
        changes: PackChanges::from_plan(&update),
        download_bytes: update.download_size(),
    };
    Ok(ServerPlan {
        view,
        update,
        summary,
        loader,
    })
}

/// The files `plan` writes, for the trust advisory. A server's own files have no SHA-1
/// (Modrinth can't be asked about them), unless they are Modrinth's or CurseForge's.
pub fn pulls(plan: &UpdatePlan) -> Vec<PulledFile> {
    plan.writes()
        .map(|(fetch, path)| match fetch {
            Fetch::Download { download, .. } => PulledFile {
                path: path.to_string(),
                sha1: (!download.sha1.is_empty()).then(|| download.sha1.clone()),
                sha512: (!download.sha512.is_empty()).then(|| download.sha512.clone()),
                modrinth: download.urls.iter().find_map(crate::modpack::cdn_ids),
            },
            Fetch::Override { file, .. } => PulledFile {
                path: path.to_string(),
                sha1: Some(file.sha1.clone()),
                sha512: None,
                modrinth: None,
            },
            Fetch::Object { .. } => PulledFile {
                path: path.to_string(),
                sha1: None,
                sha512: None,
                modrinth: None,
            },
        })
        .collect()
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
