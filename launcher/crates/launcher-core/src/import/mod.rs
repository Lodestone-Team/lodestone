//! Importing instances from other launchers: the official launcher, Prism Launcher,
//! MultiMC, the Modrinth App, ATLauncher, the CurseForge app and GDLauncher.
//!
//! - [`detect`] finds each launcher's instances in its default place on macOS, or in a
//!   folder the user picks; each launcher's own module reads its files into a [`Found`]
//!   (name, Minecraft version, loader as the launcher wrote it, memory, JVM arguments,
//!   icon, game folder). Nothing here writes into another launcher's folders.
//! - [`mapping`] turns a launcher's version ids and loader versions into this launcher's
//!   ([`resolve`] picks the closest loader version the loader's list offers when the
//!   exact one isn't there, and says so).
//! - [`copy`] plans and copies the game folder (never moves it), without caches and logs,
//!   following symlinks only when they stay inside it.
//! - [`import`] builds the new instance in a hidden folder, matches its mods and packs to
//!   Modrinth by SHA-1, and renames it into place; a failure or cancel leaves nothing.

pub mod atlauncher;
pub mod copy;
pub mod curseforge;
pub mod detect;
pub mod gdlauncher;
pub mod jsonb;
pub mod mapping;
pub mod modrinth_app;
pub mod prism;
pub mod sqlite;
pub mod vanilla;

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU8, AtomicU64, Ordering},
};

use crate::{
    content::{ContentKind, InstanceDirs, Modrinth, listing},
    game::{
        download::CancelToken,
        install::{InstallError, Installer},
        loaders::{LoaderChoice, LoaderKind},
    },
    instances::{CUSTOM_ICON, ICON_FILE, Instance, InstanceError, InstanceStore, Loader, Memory},
    storage,
};

pub use copy::{CopyOptions, CopyPlan};
pub use detect::{Launcher, Locations};

/// The launchers instances can be imported from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Source {
    Vanilla,
    Prism,
    MultiMc,
    ModrinthApp,
    AtLauncher,
    CurseForge,
    GdLauncher,
    GdLauncherCarbon,
}

impl Source {
    pub const ALL: [Source; 8] = [
        Source::Vanilla,
        Source::Prism,
        Source::MultiMc,
        Source::ModrinthApp,
        Source::AtLauncher,
        Source::CurseForge,
        Source::GdLauncher,
        Source::GdLauncherCarbon,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Source::Vanilla => "Minecraft Launcher",
            Source::Prism => "Prism Launcher",
            Source::MultiMc => "MultiMC",
            Source::ModrinthApp => "Modrinth App",
            Source::AtLauncher => "ATLauncher",
            Source::CurseForge => "CurseForge",
            Source::GdLauncher => "GDLauncher",
            Source::GdLauncherCarbon => "GDLauncher Carbon",
        }
    }
}

/// The Minecraft version an instance asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WantedVersion {
    Exact(String),
    /// The official launcher's "Latest release" profile.
    LatestRelease,
    LatestSnapshot,
}

impl WantedVersion {
    pub fn label(&self) -> String {
        match self {
            WantedVersion::Exact(version) => version.clone(),
            WantedVersion::LatestRelease => "Latest release".into(),
            WantedVersion::LatestSnapshot => "Latest snapshot".into(),
        }
    }
}

/// The loader an instance uses, with its version as that launcher wrote it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WantedLoader {
    pub kind: LoaderKind,
    /// `None` when the launcher didn't say, or said "latest".
    pub version: Option<String>,
}

impl WantedLoader {
    pub fn new(kind: LoaderKind, version: Option<&str>) -> Self {
        let version = version
            .map(str::trim)
            .filter(|v| !v.is_empty() && !matches!(*v, "latest" | "stable"))
            .map(str::to_string);
        Self { kind, version }
    }
}

/// An instance's icon in the other launcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconSource {
    File(PathBuf),
    /// Image bytes (the official launcher's `data:` URLs).
    Bytes(Vec<u8>),
}

/// An instance found in another launcher.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub source: Source,
    /// Unique among everything detected: the source and its game folder (and profile).
    pub key: String,
    pub name: String,
    /// The folder the game runs in, whose files are copied.
    pub game_dir: PathBuf,
    pub game_version: WantedVersion,
    pub loader: Option<WantedLoader>,
    pub memory: Option<Memory>,
    pub jvm_args: Option<String>,
    pub icon: Option<IconSource>,
    /// What can't be carried over, in plain words.
    pub notes: Vec<String>,
}

impl Found {
    pub(crate) fn new(
        source: Source,
        name: &str,
        game_dir: PathBuf,
        version: WantedVersion,
    ) -> Self {
        Self {
            source,
            key: format!("{source:?}:{}", game_dir.display()),
            name: name.trim().to_string(),
            game_dir,
            game_version: version,
            loader: None,
            memory: None,
            jvm_args: None,
            icon: None,
            notes: Vec::new(),
        }
    }

    /// "1.20.1 · Fabric 0.16.14", as the list shows it.
    pub fn version_label(&self) -> String {
        match &self.loader {
            None => self.game_version.label(),
            Some(loader) => match &loader.version {
                Some(version) => format!(
                    "{} · {} {version}",
                    self.game_version.label(),
                    loader.kind.label()
                ),
                None => format!("{} · {}", self.game_version.label(), loader.kind.label()),
            },
        }
    }

    pub(crate) fn note(&mut self, note: impl Into<String>) {
        self.notes.push(note.into());
    }

    /// Memory in MiB, if the launcher overrode it and the values make sense.
    pub(crate) fn set_memory(&mut self, min: Option<u64>, max: Option<u64>) {
        let Some(max) = max.filter(|m| (256..=1024 * 1024).contains(m)) else {
            return;
        };
        let min = min.filter(|m| *m >= 128).unwrap_or(512).min(max);
        self.memory = Some(Memory {
            min: min as u32,
            max: max as u32,
        });
    }

    pub(crate) fn set_jvm_args(&mut self, args: Option<&str>) {
        self.jvm_args = args
            .map(|a| a.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|a| !a.is_empty());
    }
}

/// What the new instance will be: the version and loader resolved against Mojang's and
/// the loader's lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolved {
    pub game_version: String,
    pub loader: Option<LoaderChoice>,
    pub notes: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("cancelled")]
    Cancelled,
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Instance(#[from] InstanceError),
    #[error(transparent)]
    Install(#[from] InstallError),
}

pub(crate) fn io_error(path: &Path) -> impl FnOnce(io::Error) -> ImportError + '_ {
    move |source| ImportError::Io {
        path: path.to_owned(),
        source,
    }
}

/// Resolves `found`'s version and loader with `installer`'s cached lists: Mojang's for
/// "latest release"/"latest snapshot", the loader's for its version.
pub async fn resolve(installer: &Installer, found: &Found) -> Result<Resolved, ImportError> {
    let mut notes = Vec::new();
    let game_version = match &found.game_version {
        WantedVersion::Exact(version) => version.clone(),
        latest => {
            let manifest = installer.version_manifest().await?;
            let latest_ids = manifest.latest.ok_or_else(|| {
                ImportError::Invalid("Mojang's version list has no latest version".into())
            })?;
            match latest {
                WantedVersion::LatestSnapshot => latest_ids.snapshot,
                _ => latest_ids.release,
            }
        }
    };
    let loader = match &found.loader {
        None => None,
        Some(wanted) => {
            let list = installer
                .loader_versions(wanted.kind, &game_version)
                .await?;
            let resolution = mapping::resolve_loader(wanted, &game_version, &list);
            notes.extend(resolution.note);
            resolution.choice
        }
    };
    Ok(Resolved {
        game_version,
        loader,
        notes,
    })
}

/// The stages of an import, for its progress.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Waiting,
    Copying,
    Matching,
    Done,
}

/// An import's progress, shared with the UI.
#[derive(Debug, Default)]
pub struct ImportProgress {
    stage: AtomicU8,
    pub bytes_done: AtomicU64,
    pub bytes_total: AtomicU64,
    pub files_done: AtomicU64,
    pub files_total: AtomicU64,
}

impl ImportProgress {
    pub fn stage(&self) -> Stage {
        match self.stage.load(Ordering::Relaxed) {
            1 => Stage::Copying,
            2 => Stage::Matching,
            3 => Stage::Done,
            _ => Stage::Waiting,
        }
    }

    pub fn set_stage(&self, stage: Stage) {
        let value = match stage {
            Stage::Waiting => 0,
            Stage::Copying => 1,
            Stage::Matching => 2,
            Stage::Done => 3,
        };
        self.stage.store(value, Ordering::Relaxed);
    }

    /// Bytes copied out of the total, 0 to 1.
    pub fn fraction(&self) -> Option<f32> {
        let total = self.bytes_total.load(Ordering::Relaxed);
        (total > 0).then(|| self.bytes_done.load(Ordering::Relaxed) as f32 / total as f32)
    }
}

/// One instance to import.
#[derive(Clone, Debug)]
pub struct ImportRequest {
    pub found: Found,
    pub resolved: Resolved,
    pub options: CopyOptions,
}

/// The imported instance, and what to tell the user about it.
#[derive(Clone, Debug)]
pub struct Imported {
    pub instance: Instance,
    pub notes: Vec<String>,
    /// Files and folders left out, and why.
    pub skipped: Vec<(String, String)>,
}

/// Removes the staging folder unless the import got to the end.
struct Staging {
    dir: PathBuf,
    keep: bool,
}

impl Drop for Staging {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_dir_all(&self.dir);
        }
    }
}

/// Imports `request` into `store`: copies the game folder into a hidden folder, sets the
/// name, icon, memory and JVM arguments, matches the mods and packs to Modrinth (with
/// `api`; without it, or offline, they are matched the next time the Content tab lists
/// them), and renames the folder into place. On an error or cancel nothing is left.
pub async fn import(
    store: &InstanceStore,
    request: &ImportRequest,
    api: Option<&Modrinth>,
    progress: &ImportProgress,
    cancel: &CancelToken,
) -> Result<Imported, ImportError> {
    let found = &request.found;
    let mut notes: Vec<String> = found.notes.clone();
    notes.extend(request.resolved.notes.iter().cloned());
    let plan = copy::plan(&found.game_dir, &request.options)?;
    fs::create_dir_all(store.root()).map_err(io_error(store.root()))?;
    let mut staging = Staging {
        dir: store.new_staging()?,
        keep: false,
    };
    progress.set_stage(Stage::Copying);
    let game = staging.dir.join("minecraft");
    copy::copy(&plan, &game, progress, cancel)?;

    let mut instance = Instance {
        name: if found.name.is_empty() {
            format!("{} instance", found.source.label())
        } else {
            found.name.clone()
        },
        game_version: request.resolved.game_version.clone(),
        loader: Loader::from_kind(request.resolved.loader.as_ref().map(|l| l.kind)),
        loader_version: request.resolved.loader.as_ref().map(|l| l.version.clone()),
        created: crate::instances::now(),
        memory: found.memory,
        jvm_args: found.jvm_args.clone(),
        ..Instance::default()
    };
    if let Some(icon) = &found.icon {
        match icon_bytes(icon).and_then(|bytes| crate::instances::icon_png(&bytes)) {
            Ok(png) => {
                let path = staging.dir.join(ICON_FILE);
                storage::write_atomic(&path, &png).map_err(io_error(&path))?;
                instance.icon = Some(CUSTOM_ICON.to_string());
            }
            Err(err) => notes.push(format!("The icon couldn't be used: {err}.")),
        }
    }

    if cancel.is_cancelled() {
        return Err(ImportError::Cancelled);
    }
    progress.set_stage(Stage::Matching);
    let dirs = InstanceDirs::new(&staging.dir);
    for kind in ContentKind::ALL {
        if cancel.is_cancelled() {
            return Err(ImportError::Cancelled);
        }
        if !dirs.content_dir(kind).is_dir() {
            continue;
        }
        match listing::refresh(api, &dirs, kind).await {
            Ok((_, _, Some(error))) => {
                notes.push(format!(
                    "{} weren't matched to Modrinth ({error}); the Content tab tries again.",
                    kind.label()
                ));
            }
            Ok(_) => {}
            Err(err) => notes.push(format!("{} couldn't be read: {err}", kind.label())),
        }
    }
    if cancel.is_cancelled() {
        return Err(ImportError::Cancelled);
    }
    let instance = store.commit_staged(&staging.dir, instance)?;
    staging.keep = true;
    progress.set_stage(Stage::Done);
    Ok(Imported {
        instance,
        notes,
        skipped: plan.skipped,
    })
}

/// The largest icon read.
const MAX_ICON_BYTES: u64 = 16 * 1024 * 1024;

fn icon_bytes(icon: &IconSource) -> Result<Vec<u8>, String> {
    match icon {
        IconSource::Bytes(bytes) => Ok(bytes.clone()),
        IconSource::File(path) => {
            let metadata = fs::metadata(path).map_err(|err| err.to_string())?;
            if !metadata.is_file() || metadata.len() > MAX_ICON_BYTES {
                return Err("it isn't a small image file".into());
            }
            fs::read(path).map_err(|err| err.to_string())
        }
    }
}

/// Reads a JSON file of at most `limit` bytes.
pub(crate) fn read_json(path: &Path, limit: u64) -> Result<serde_json::Value, String> {
    let metadata = fs::metadata(path).map_err(|err| format!("{}: {err}", path.display()))?;
    if metadata.len() > limit {
        return Err(format!("{} is too large", path.display()));
    }
    let bytes = fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
    serde_json::from_slice(&bytes).map_err(|err| format!("{}: {err}", path.display()))
}

/// Reads a small text file.
pub(crate) fn read_text(path: &Path, limit: u64) -> Result<String, String> {
    let metadata = fs::metadata(path).map_err(|err| format!("{}: {err}", path.display()))?;
    if metadata.len() > limit {
        return Err(format!("{} is too large", path.display()));
    }
    let bytes = fs::read(path).map_err(|err| format!("{}: {err}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// The folders directly in `dir` (not hidden, not links), sorted by name.
pub(crate) fn subfolders(dir: &Path) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    dirs.sort();
    dirs
}

/// The folder's name, for instances named after their folder.
pub(crate) fn folder_name(dir: &Path) -> String {
    dir.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// `path` if it is a file inside `root` (after resolving links).
pub(crate) fn file_inside(root: &Path, path: &Path) -> Option<PathBuf> {
    let root = fs::canonicalize(root).ok()?;
    let path = fs::canonicalize(path).ok()?;
    (path.starts_with(&root) && path.is_file()).then_some(path)
}

/// `-Xms`/`-Xmx` in MiB taken out of JVM arguments: (min, max, the other arguments).
pub(crate) fn split_memory_args(args: &str) -> (Option<u64>, Option<u64>, String) {
    fn mib(value: &str) -> Option<u64> {
        let value = value.trim();
        let (number, unit) = value.split_at(value.find(|c: char| !c.is_ascii_digit())?.max(1));
        let number: u64 = number.parse().ok()?;
        match unit.to_ascii_lowercase().as_str() {
            "g" => Some(number * 1024),
            "m" => Some(number),
            "k" => Some(number / 1024),
            _ => None,
        }
        .or_else(|| unit.is_empty().then_some(number / (1024 * 1024)))
    }
    let (mut min, mut max) = (None, None);
    let mut rest = Vec::new();
    for word in args.split_whitespace() {
        if let Some(value) = word.strip_prefix("-Xmx") {
            max = mib(value).or(max);
        } else if let Some(value) = word.strip_prefix("-Xms") {
            min = mib(value).or(min);
        } else {
            rest.push(word);
        }
    }
    (min, max, rest.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_arguments() {
        assert_eq!(
            split_memory_args("-Xmx4G -XX:+UseG1GC -Xms512m"),
            (Some(512), Some(4096), "-XX:+UseG1GC".into())
        );
        assert_eq!(
            split_memory_args("-Xmx2048M"),
            (None, Some(2048), String::new())
        );
        assert_eq!(split_memory_args(""), (None, None, String::new()));
    }
}
