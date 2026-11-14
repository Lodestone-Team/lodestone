//! What the data directory takes on disk, for Settings > Storage, and removing what
//! can be removed safely.
//!
//! The scan walks the directory on a background thread and can be cancelled. It
//! splits the space into instances (worlds, mods and packs, logs, screenshots and the
//! rest), Java runtimes, game files (versions, libraries, assets), the cache, and
//! whatever else the directory holds.
//!
//! What can be removed, and why it is safe:
//!
//! - **Unused game files.** A version is in use when an instance plays it, directly
//!   or as the parent (`inheritsFrom`) of a loader profile an instance uses. Every
//!   other `versions/<id>` folder goes. Libraries go only when a removed version's
//!   install record lists them and no version in use lists or names them; files a
//!   loader's installer made (Forge's processors) are in no record, so they are never
//!   touched. Asset objects go when no asset index in use lists them, and only if
//!   every index in use is on disk to say so. All of it downloads again at the next
//!   launch that needs it.
//! - **Unused Java runtimes:** components that no version in use asks for.
//! - **The cache:** everything under `cache/`, which the launcher fills again.
//! - **Logs and crash reports** of the instances, all or those older than a cutoff.
//!
//! Instances themselves go to the Trash through `InstanceStore::delete`, not here.
//! Callers refuse these while a game runs; logs skip the instances that are running.

use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs, io,
    path::{Component, Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime},
};

use serde::Serialize;

use crate::{
    game::{
        assets::AssetIndex,
        libraries,
        loaders::{self, profile_id},
        platform::Platform,
        record::{InstallRecord, version_record_path},
        runtime,
        version::{DEFAULT_RUNTIME, VersionJson},
        zulu,
    },
    instances::Instance,
    paths::DataDir,
};

/// Stops a scan or a cleanup from another thread.
#[derive(Clone, Debug, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DiskError {
    #[error("cancelled")]
    Cancelled,
    #[error("couldn't remove {path}: {reason}")]
    Remove { path: String, reason: String },
}

/// An instance's folder, by what is in it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct InstanceUsage {
    pub id: String,
    pub name: String,
    /// `minecraft/saves`.
    pub worlds: u64,
    /// `minecraft/mods`, `resourcepacks`, `shaderpacks` and `datapacks`.
    pub content: u64,
    /// `minecraft/logs` and `crash-reports`.
    pub logs: u64,
    /// `minecraft/screenshots`.
    pub screenshots: u64,
    /// Everything else: configs, natives, the icon and background, and so on.
    pub other: u64,
}

impl InstanceUsage {
    pub fn total(&self) -> u64 {
        self.worlds + self.content + self.logs + self.screenshots + self.other
    }
}

/// A Java runtime component, such as `java-runtime-delta`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct RuntimeUsage {
    pub component: String,
    pub bytes: u64,
    /// The Java major version, from a version that uses it, if any does.
    pub java: Option<u32>,
    /// The versions in use that need it, as `1.20.1`.
    pub used_by: Vec<String>,
}

impl RuntimeUsage {
    pub fn unused(&self) -> bool {
        self.used_by.is_empty()
    }
}

/// Versions, libraries and assets.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct GameFiles {
    pub versions: u64,
    pub libraries: u64,
    pub assets: u64,
    /// What "Remove unused game files" would free.
    pub unused: Unused,
}

impl GameFiles {
    pub fn total(&self) -> u64 {
        self.versions + self.libraries + self.assets
    }
}

/// The game files no instance needs.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Unused {
    /// Version ids.
    pub versions: Vec<String>,
    pub versions_bytes: u64,
    pub libraries: u64,
    pub assets: u64,
}

impl Unused {
    pub fn total(&self) -> u64 {
        self.versions_bytes + self.libraries + self.assets
    }
}

/// Logs and crash reports across the instances.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Logs {
    pub bytes: u64,
    pub files: usize,
    /// Those older than [`OLD_LOGS`].
    pub old_bytes: u64,
    pub old_files: usize,
}

/// What "Delete old logs" means by old.
pub const OLD_LOGS: Duration = Duration::from_secs(30 * 24 * 60 * 60);

/// The files of downloads and imports that didn't finish: `instances/.lodestone-*`
/// (a modpack's download, a half-made instance) untouched for [`LEFTOVER_AGE`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Leftovers {
    pub bytes: u64,
    /// Their names under `instances/`.
    pub items: Vec<String>,
}

/// How long a leftover must have been untouched, so a running import is never one.
pub const LEFTOVER_AGE: Duration = Duration::from_secs(60 * 60);

/// The prefix of the temporary files and folders imports make in `instances/`.
const LEFTOVER_PREFIX: &str = ".lodestone-";

/// What the data directory takes.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Largest first.
    pub instances: Vec<InstanceUsage>,
    /// Largest first.
    pub runtimes: Vec<RuntimeUsage>,
    pub game_files: GameFiles,
    pub cache: u64,
    pub leftovers: Leftovers,
    pub logs: Logs,
    /// Snapshots of instances.
    pub snapshots: crate::snapshots::Usage,
    /// Settings, accounts and other small files, and anything else in the folder.
    pub other: u64,
}

impl Report {
    pub fn instances_total(&self) -> u64 {
        self.instances.iter().map(InstanceUsage::total).sum()
    }

    pub fn runtimes_total(&self) -> u64 {
        self.runtimes.iter().map(|r| r.bytes).sum()
    }

    pub fn unused_runtimes(&self) -> u64 {
        self.runtimes
            .iter()
            .filter(|r| r.unused())
            .map(|r| r.bytes)
            .sum()
    }

    pub fn total(&self) -> u64 {
        self.instances_total()
            + self.runtimes_total()
            + self.game_files.total()
            + self.cache
            + self.leftovers.bytes
            + self.snapshots.bytes
            + self.other
    }
}

/// What a cleanup removes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clean {
    UnusedGameFiles,
    UnusedRuntimes,
    Cache,
    /// [`Leftovers`].
    Leftovers,
    /// Logs and crash reports older than [`OLD_LOGS`].
    OldLogs,
    AllLogs,
    /// Automatic snapshots, those of deleted instances, and shared snapshot files.
    Snapshots,
}

impl Clean {
    /// The name the control API uses.
    pub fn name(self) -> &'static str {
        match self {
            Clean::UnusedGameFiles => "game-files",
            Clean::UnusedRuntimes => "runtimes",
            Clean::Cache => "cache",
            Clean::Leftovers => "leftovers",
            Clean::OldLogs => "old-logs",
            Clean::AllLogs => "logs",
            Clean::Snapshots => "snapshots",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        [
            Clean::UnusedGameFiles,
            Clean::UnusedRuntimes,
            Clean::Cache,
            Clean::Leftovers,
            Clean::OldLogs,
            Clean::AllLogs,
            Clean::Snapshots,
        ]
        .into_iter()
        .find(|clean| clean.name() == name)
    }
}

/// What a cleanup did.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Cleaned {
    pub bytes: u64,
    pub files: usize,
}

/// Counts what the data directory takes. `instances` are the launcher's instances,
/// whose folders are `instances/<id>`.
pub fn scan(data: &DataDir, instances: &[Instance], cancel: &Cancel) -> Result<Report, DiskError> {
    scan_on(data, instances, &Platform::current(), cancel)
}

/// [`scan`] on `machine`, whose processor decides which Java runtimes versions use.
fn scan_on(
    data: &DataDir,
    instances: &[Instance],
    machine: &Platform,
    cancel: &Cancel,
) -> Result<Report, DiskError> {
    let mut sizer = Sizer::new(cancel);
    let mut report = Report::default();

    // Instances.
    let mut instance_dirs = HashSet::new();
    for instance in instances {
        let dir = data.instances_dir().join(&instance.id);
        instance_dirs.insert(instance.id.clone());
        report
            .instances
            .push(instance_usage(&mut sizer, &dir, instance)?);
        let (bytes, files, old_bytes, old_files) =
            logs_in(&dir.join("minecraft"), SystemTime::now(), cancel)?;
        report.logs.bytes += bytes;
        report.logs.files += files;
        report.logs.old_bytes += old_bytes;
        report.logs.old_files += old_files;
    }
    // What else is under instances/: leftovers of imports, or "other".
    let mut stray = 0;
    let old = SystemTime::now() - LEFTOVER_AGE;
    for entry in read_dir(&data.instances_dir()) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if instance_dirs.contains(&name) {
            continue;
        }
        let bytes = sizer.size(&entry.path())?;
        if is_leftover(&name, &entry.path(), old) {
            report.leftovers.bytes += bytes;
            report.leftovers.items.push(name);
        } else {
            stray += bytes;
        }
    }
    report.leftovers.items.sort();
    report
        .instances
        .sort_by(|a, b| b.total().cmp(&a.total()).then(a.name.cmp(&b.name)));

    // What the instances use.
    let used = Used::find(data, instances, machine);

    // Java runtimes.
    // Files loose in the runtimes folder count as "other".
    let mut runtime_files = 0;
    for entry in read_dir(&data.runtimes_dir()) {
        if !entry.path().is_dir() {
            runtime_files += sizer.size(&entry.path())?;
            continue;
        }
        let component = entry.file_name().to_string_lossy().into_owned();
        let mut bytes = sizer.size(&entry.path())?;
        let users = used.runtimes.get(&component);
        // Zulu 8 keeps each build in a folder of its own and runs the newest: while it
        // is in use, the older builds are listed apart, unused.
        if users.is_some() && component == zulu::COMPONENT {
            for older in older_zulu_builds(&entry.path()) {
                let older_bytes = sizer.size(&older)?;
                bytes = bytes.saturating_sub(older_bytes);
                report.runtimes.push(RuntimeUsage {
                    component: format!(
                        "{component}/{}",
                        older.file_name().unwrap_or_default().to_string_lossy()
                    ),
                    bytes: older_bytes,
                    java: Some(8),
                    used_by: Vec::new(),
                });
            }
        }
        report.runtimes.push(RuntimeUsage {
            java: users.and_then(|(java, _)| *java),
            used_by: users
                .map(|(_, versions)| versions.iter().cloned().collect())
                .unwrap_or_default(),
            component,
            bytes,
        });
    }
    report
        .runtimes
        .sort_by(|a, b| b.bytes.cmp(&a.bytes).then(a.component.cmp(&b.component)));

    // Game files.
    report.game_files.versions = sizer.size(&data.versions_dir())?;
    report.game_files.libraries = sizer.size(&data.libraries_dir())?;
    report.game_files.assets = sizer.size(&data.assets_dir())?;
    report.game_files.unused = unused_game_files(data, &used, cancel)?;

    report.cache = sizer.size(&data.cache_dir())?;
    let store = crate::snapshots::Store::new(data);
    let ids: HashSet<&str> = instances.iter().map(|i| i.id.as_str()).collect();
    report.snapshots = store.usage(&ids);

    // Everything else at the top of the data directory.
    let known: HashSet<PathBuf> = [
        data.instances_dir(),
        data.runtimes_dir(),
        data.versions_dir(),
        data.libraries_dir(),
        data.assets_dir(),
        data.cache_dir(),
        store.root().to_path_buf(),
    ]
    .into_iter()
    .collect();
    let mut other = stray + runtime_files;
    for entry in read_dir(data.root()) {
        if !known.contains(&entry.path()) {
            other += sizer.size(&entry.path())?;
        }
    }
    report.other = other;
    Ok(report)
}

/// The Zulu 8 builds in `<runtimes>/zulu-8` that games no longer use: all but the newest
/// for each platform ([`zulu::older_builds`]).
fn older_zulu_builds(component_dir: &Path) -> Vec<PathBuf> {
    read_dir(component_dir)
        .into_iter()
        .filter(|entry| entry.path().is_dir())
        .flat_map(|entry| zulu::older_builds(&entry.path()))
        .collect()
}

/// Removes `what`. `instances` are all the instances, `running` the ids of those
/// whose game is starting or running: their logs are kept, and callers refuse the
/// other cleanups while any runs.
pub fn clean(
    data: &DataDir,
    instances: &[Instance],
    running: &[String],
    what: Clean,
    cancel: &Cancel,
) -> Result<Cleaned, DiskError> {
    clean_on(data, instances, running, what, &Platform::current(), cancel)
}

/// [`clean`] on `machine`, whose processor decides which Java runtimes versions use.
fn clean_on(
    data: &DataDir,
    instances: &[Instance],
    running: &[String],
    what: Clean,
    machine: &Platform,
    cancel: &Cancel,
) -> Result<Cleaned, DiskError> {
    let mut cleaned = Cleaned::default();
    match what {
        Clean::Snapshots => {
            let ids: HashSet<&str> = instances.iter().map(|i| i.id.as_str()).collect();
            cleaned.bytes = crate::snapshots::Store::new(data).clean_up(&ids);
        }
        Clean::Cache => {
            for entry in read_dir(&data.cache_dir()) {
                remove(&entry.path(), &mut cleaned, cancel)?;
            }
        }
        Clean::Leftovers => {
            let ids: HashSet<&str> = instances.iter().map(|i| i.id.as_str()).collect();
            let old = SystemTime::now() - LEFTOVER_AGE;
            for entry in read_dir(&data.instances_dir()) {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !ids.contains(name.as_str()) && is_leftover(&name, &entry.path(), old) {
                    remove(&entry.path(), &mut cleaned, cancel)?;
                }
            }
        }
        Clean::UnusedRuntimes => {
            let used = Used::find(data, instances, machine);
            for entry in read_dir(&data.runtimes_dir()) {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !entry.path().is_dir() {
                    continue;
                }
                if !used.runtimes.contains_key(&name) {
                    remove(&entry.path(), &mut cleaned, cancel)?;
                } else if name == zulu::COMPONENT {
                    // No game runs now (callers make sure), so no older build is open.
                    for older in older_zulu_builds(&entry.path()) {
                        remove(&older, &mut cleaned, cancel)?;
                    }
                }
            }
        }
        Clean::UnusedGameFiles => {
            let used = Used::find(data, instances, machine);
            let plan = UnusedPlan::find(data, &used, cancel)?;
            for id in &plan.versions {
                remove(&data.versions_dir().join(id), &mut cleaned, cancel)?;
            }
            for path in plan.library_files.iter().chain(&plan.asset_files) {
                remove(path, &mut cleaned, cancel)?;
            }
            prune_empty_dirs(&data.libraries_dir());
            prune_empty_dirs(&data.assets_dir().join("objects"));
        }
        Clean::OldLogs | Clean::AllLogs => {
            let cutoff = (what == Clean::OldLogs).then(|| SystemTime::now() - OLD_LOGS);
            for instance in instances {
                if running.contains(&instance.id) {
                    continue;
                }
                let game = data.instances_dir().join(&instance.id).join("minecraft");
                for folder in ["logs", "crash-reports"] {
                    for (file, meta) in files_under(&game.join(folder)) {
                        if cancel.is_cancelled() {
                            return Err(DiskError::Cancelled);
                        }
                        let old_enough = match cutoff {
                            None => true,
                            Some(cutoff) => meta.modified().is_ok_and(|modified| modified < cutoff),
                        };
                        if old_enough {
                            remove(&file, &mut cleaned, cancel)?;
                        }
                    }
                }
            }
        }
    }
    Ok(cleaned)
}

/// What the instances use: version ids, asset indexes and runtime components.
#[derive(Debug, Default)]
struct Used {
    versions: BTreeSet<String>,
    /// The parsed version files of `versions`; a version in use that isn't on disk or
    /// doesn't parse is missing here, and [`Used::complete`] is false.
    jsons: Vec<VersionJson>,
    asset_indexes: BTreeSet<String>,
    /// By runtimes folder (a component, or `zulu-8`): the Java major version and the
    /// versions that need it, as they run on this machine ([`runtime::folder_for`]).
    runtimes: BTreeMap<String, (Option<u32>, BTreeSet<String>)>,
    /// Every version in use was read, so libraries and assets can be judged.
    complete: bool,
}

impl Used {
    fn find(data: &DataDir, instances: &[Instance], machine: &Platform) -> Self {
        let mut used = Used {
            complete: true,
            ..Default::default()
        };
        let mut queue: Vec<String> = Vec::new();
        for instance in instances {
            if !instance.game_version.is_empty() {
                queue.push(instance.game_version.clone());
            }
            if let Some(choice) = instance.loader_choice() {
                match profile_id(&choice, &instance.game_version) {
                    Ok(id) => queue.push(id),
                    // A profile id that can't be made: the version can't be judged.
                    Err(_) => used.complete = false,
                }
            }
        }
        // The versions instances start: each one's runtime is judged with what it
        // inherits, below.
        let tops = queue.clone();
        let mut loaded: BTreeMap<String, usize> = BTreeMap::new();
        while let Some(id) = queue.pop() {
            if !safe_name(&id) || !used.versions.insert(id.clone()) {
                continue;
            }
            let path = data.versions_dir().join(&id).join(format!("{id}.json"));
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                // Not installed yet: it has nothing on disk to keep, and downloads
                // what it needs when it is first played.
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(_) => {
                    used.complete = false;
                    continue;
                }
            };
            let Ok(json) = VersionJson::parse(&bytes) else {
                // Installed but unreadable: its libraries and assets can't be told
                // apart from others', so only whole unused versions go.
                used.complete = false;
                continue;
            };
            if let Some(parent) = &json.inherits_from {
                queue.push(parent.clone());
            }
            // A loader profile's assets come from its parent.
            if json.inherits_from.is_none() {
                used.asset_indexes.insert(json.assets_id().to_string());
            }
            loaded.insert(id.clone(), used.jsons.len());
            used.jsons.push(json);
        }
        // The runtime each started version uses: its own component, or the one it
        // inherits, in the Java its libraries need on this machine. On Apple silicon
        // that is not always the component's own folder: Java 8 versions that run
        // natively use Zulu 8, Java 16 ones Mojang's arm64 Java 17.
        for top in &tops {
            let mut chain: Vec<(&str, &VersionJson)> = Vec::new();
            let mut next = Some(top.as_str());
            while let Some(id) = next {
                let Some(json) = loaded.get(id).map(|&i| &used.jsons[i]) else {
                    break;
                };
                if chain.len() > 8 || chain.iter().any(|(seen, _)| *seen == id) {
                    break;
                }
                chain.push((id, json));
                next = json.inherits_from.as_deref();
            }
            let Some(&(root_id, root)) = chain.last() else {
                continue;
            };
            let (owner, component, major) = chain
                .iter()
                .find_map(|(id, json)| {
                    let java = json.java_version.as_ref()?;
                    Some((*id, java.component.clone()?, java.major_version))
                })
                .unwrap_or((
                    root_id,
                    root.runtime_component().to_string(),
                    root.java_major(),
                ));
            let merged = chain[..chain.len() - 1]
                .iter()
                .rev()
                .fold(root.clone(), |parent, (_, child)| {
                    loaders::merge(child, &parent)
                });
            let java = machine.with_arch(libraries::java_arch(&merged, machine));
            let (folder, major) = runtime::folder_for(&component, Some(major), &java);
            let entry = used
                .runtimes
                .entry(folder.to_string())
                .or_insert((None, BTreeSet::new()));
            entry.0 = entry.0.or(major);
            entry.1.insert(owner.to_string());
        }
        // Old versions without a `javaVersion` use the default runtime.
        if used.runtimes.is_empty() && !used.versions.is_empty() {
            used.runtimes
                .insert(DEFAULT_RUNTIME.to_string(), (Some(8), BTreeSet::new()));
        }
        used
    }
}

/// The paths "Remove unused game files" would remove.
#[derive(Debug, Default)]
struct UnusedPlan {
    versions: Vec<String>,
    library_files: Vec<PathBuf>,
    asset_files: Vec<PathBuf>,
}

impl UnusedPlan {
    fn find(data: &DataDir, used: &Used, cancel: &Cancel) -> Result<Self, DiskError> {
        let mut plan = UnusedPlan::default();
        for entry in read_dir(&data.versions_dir()) {
            let id = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_dir() && !used.versions.contains(&id) {
                plan.versions.push(id);
            }
        }
        if !used.complete {
            // Without every version in use read, only whole unused versions go.
            return Ok(plan);
        }

        // Libraries: listed in a removed version's record, and not needed by any
        // version in use (its record, or its version file's libraries).
        let mut needed: HashSet<String> = HashSet::new();
        for id in &used.versions {
            for key in record_keys(&data.versions_dir(), id) {
                needed.insert(key);
            }
        }
        for json in &used.jsons {
            for path in library_paths(json) {
                needed.insert(format!("libraries/{path}"));
            }
        }
        let mut candidates = BTreeSet::new();
        for id in &plan.versions {
            for key in record_keys(&data.versions_dir(), id) {
                if key.starts_with("libraries/") && !needed.contains(&key) {
                    candidates.insert(key);
                }
            }
        }
        for key in candidates {
            if let Some(path) = inside(data.root(), &key)
                && path.is_file()
            {
                plan.library_files.push(path);
            }
        }

        // Assets: objects that no index in use lists, if every one of those is on disk.
        let mut keep: HashSet<String> = HashSet::new();
        for index_id in &used.asset_indexes {
            if !safe_name(index_id) {
                return Ok(plan);
            }
            let path = data
                .assets_dir()
                .join("indexes")
                .join(format!("{index_id}.json"));
            let Some(index) = fs::read(&path)
                .ok()
                .and_then(|bytes| AssetIndex::parse(&bytes).ok())
            else {
                return Ok(plan);
            };
            keep.extend(
                index
                    .objects
                    .into_values()
                    .map(|object| object.hash.to_ascii_lowercase()),
            );
        }
        let objects = data.assets_dir().join("objects");
        for prefix in read_dir(&objects) {
            for object in read_dir(&prefix.path()) {
                if cancel.is_cancelled() {
                    return Err(DiskError::Cancelled);
                }
                let name = object.file_name().to_string_lossy().to_ascii_lowercase();
                // The entry's own type: free, where `path().is_file()` opens each file
                // on Windows, and there are thousands.
                if object.file_type().is_ok_and(|kind| kind.is_file()) && !keep.contains(&name) {
                    plan.asset_files.push(object.path());
                }
            }
        }
        // Indexes no version in use names, with their records.
        for entry in read_dir(&data.assets_dir().join("indexes")) {
            let name = entry.file_name().to_string_lossy().into_owned();
            let id = name
                .strip_suffix(".install.json")
                .or_else(|| name.strip_suffix(".json"));
            if let Some(id) = id
                && !used.asset_indexes.contains(id)
            {
                plan.asset_files.push(entry.path());
            }
        }
        Ok(plan)
    }
}

/// How much "Remove unused game files" would free.
fn unused_game_files(data: &DataDir, used: &Used, cancel: &Cancel) -> Result<Unused, DiskError> {
    let plan = UnusedPlan::find(data, used, cancel)?;
    let mut sizer = Sizer::new(cancel);
    let mut unused = Unused::default();
    for id in &plan.versions {
        unused.versions_bytes += sizer.size(&data.versions_dir().join(id))?;
    }
    unused.versions = plan.versions;
    for path in &plan.library_files {
        unused.libraries += sizer.size(path)?;
    }
    for path in &plan.asset_files {
        unused.assets += sizer.size(path)?;
    }
    Ok(unused)
}

/// The files a version's install record lists, relative to the data directory.
fn record_keys(versions_dir: &Path, id: &str) -> Vec<String> {
    let path = version_record_path(versions_dir, id);
    match crate::storage::load::<InstallRecord>(&path) {
        Ok(loaded) => loaded.document.files.into_keys().collect(),
        Err(_) => Vec::new(),
    }
}

/// The library paths a version file names, relative to `libraries/`: from their
/// downloads, and from their Maven names.
fn library_paths(json: &VersionJson) -> Vec<String> {
    let mut paths = Vec::new();
    for library in &json.libraries {
        if let Ok(path) = crate::game::libraries::maven_path(&library.name, None) {
            paths.push(path);
        }
        let downloads = &library.downloads;
        if let Some(path) = downloads.artifact.as_ref().and_then(|a| a.path.clone()) {
            paths.push(path);
        }
        for classifier in downloads.classifiers.values() {
            if let Some(path) = &classifier.path {
                paths.push(path.clone());
            }
        }
        for classifier in library.natives.values() {
            for arch in ["64", "32"] {
                let classifier = classifier.replace("${arch}", arch);
                if let Ok(path) =
                    crate::game::libraries::maven_path(&library.name, Some(&classifier))
                {
                    paths.push(path);
                }
            }
        }
    }
    paths
}

/// `key` (relative, with `/`) under `root`, if it stays inside it.
fn inside(root: &Path, key: &str) -> Option<PathBuf> {
    let relative = Path::new(key);
    let safe = relative
        .components()
        .all(|c| matches!(c, Component::Normal(_)));
    (safe && !key.is_empty()).then(|| root.join(relative))
}

/// Whether `name` under `instances/` is a leftover: an import's temporary file or
/// folder, untouched since `old`.
fn is_leftover(name: &str, path: &Path, old: SystemTime) -> bool {
    name.starts_with(LEFTOVER_PREFIX) && newest_change(path).is_some_and(|newest| newest < old)
}

/// The latest modification time of `path` or anything in it.
fn newest_change(path: &Path) -> Option<SystemTime> {
    let meta = fs::symlink_metadata(path).ok()?;
    let mut newest = meta.modified().ok()?;
    if meta.is_dir() {
        for entry in read_dir(path) {
            if let Some(time) = newest_change(&entry.path()) {
                newest = newest.max(time);
            }
        }
    }
    Some(newest)
}

/// A version id or index id that is a plain folder or file name.
fn safe_name(name: &str) -> bool {
    !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains(['/', '\\'])
        && !name.contains('\0')
}

fn instance_usage(
    sizer: &mut Sizer,
    dir: &Path,
    instance: &Instance,
) -> Result<InstanceUsage, DiskError> {
    let mut usage = InstanceUsage {
        id: instance.id.clone(),
        name: instance.name.clone(),
        ..Default::default()
    };
    let game = dir.join("minecraft");
    for entry in read_dir(dir) {
        if entry.path() == game {
            continue;
        }
        usage.other += sizer.size(&entry.path())?;
    }
    for entry in read_dir(&game) {
        let bytes = sizer.size(&entry.path())?;
        match entry.file_name().to_string_lossy().as_ref() {
            "saves" => usage.worlds += bytes,
            "mods" | "resourcepacks" | "shaderpacks" | "datapacks" => usage.content += bytes,
            "logs" | "crash-reports" => usage.logs += bytes,
            "screenshots" => usage.screenshots += bytes,
            _ => usage.other += bytes,
        }
    }
    Ok(usage)
}

/// The size and count of the logs and crash reports under `game`, and of those
/// modified before `now - OLD_LOGS`.
fn logs_in(
    game: &Path,
    now: SystemTime,
    cancel: &Cancel,
) -> Result<(u64, usize, u64, usize), DiskError> {
    let cutoff = now - OLD_LOGS;
    let (mut bytes, mut files, mut old_bytes, mut old_files) = (0, 0, 0, 0);
    for folder in ["logs", "crash-reports"] {
        for (_, meta) in files_under(&game.join(folder)) {
            if cancel.is_cancelled() {
                return Err(DiskError::Cancelled);
            }
            bytes += meta.len();
            files += 1;
            if meta.modified().is_ok_and(|m| m < cutoff) {
                old_bytes += meta.len();
                old_files += 1;
            }
        }
    }
    Ok((bytes, files, old_bytes, old_files))
}

/// Sizes of files and folders, without following symbolic links and counting each
/// hard-linked file once (a loader's client jar is a hard link to the vanilla one).
struct Sizer<'a> {
    cancel: &'a Cancel,
    /// Hard-linked files already counted, by device and inode. Only Unix reports
    /// those (`first_time`); elsewhere every file counts.
    #[cfg(unix)]
    seen: HashSet<(u64, u64)>,
    checked: u32,
    /// Files sized so far, hard links included.
    files: usize,
}

impl<'a> Sizer<'a> {
    fn new(cancel: &'a Cancel) -> Self {
        Self {
            cancel,
            #[cfg(unix)]
            seen: HashSet::new(),
            checked: 0,
            files: 0,
        }
    }

    fn size(&mut self, path: &Path) -> Result<u64, DiskError> {
        let Ok(meta) = fs::symlink_metadata(path) else {
            return Ok(0);
        };
        self.size_of(path, &meta)
    }

    /// The size of `path`, whose `meta` is known. A folder's entries are sized from
    /// what reading the folder returned: on Windows that costs nothing more, where
    /// asking for each path again opens each file.
    fn size_of(&mut self, path: &Path, meta: &fs::Metadata) -> Result<u64, DiskError> {
        self.checked = self.checked.wrapping_add(1);
        if self.checked.is_multiple_of(256) && self.cancel.is_cancelled() {
            return Err(DiskError::Cancelled);
        }
        if meta.is_dir() {
            let mut total = 0;
            for entry in read_dir(path) {
                // Like `symlink_metadata`: a link is not followed.
                if let Ok(meta) = entry.metadata() {
                    total += self.size_of(&entry.path(), &meta)?;
                }
            }
            Ok(total)
        } else if meta.is_file() {
            self.files += 1;
            Ok(if self.first_time(meta) { meta.len() } else { 0 })
        } else {
            Ok(0)
        }
    }

    #[cfg(unix)]
    fn first_time(&mut self, meta: &fs::Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        meta.nlink() <= 1 || self.seen.insert((meta.dev(), meta.ino()))
    }

    #[cfg(not(unix))]
    fn first_time(&mut self, _meta: &fs::Metadata) -> bool {
        true
    }
}

/// A folder's entries; none if it is missing or unreadable.
fn read_dir(dir: &Path) -> Vec<fs::DirEntry> {
    fs::read_dir(dir)
        .map(|entries| entries.flatten().collect())
        .unwrap_or_default()
}

/// The files under `dir`, at any depth, without following symbolic links, with their
/// metadata as reading the folder returned it (free on Windows).
fn files_under(dir: &Path) -> Vec<(PathBuf, fs::Metadata)> {
    let mut files = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in read_dir(&dir) {
            match entry.metadata() {
                Ok(meta) if meta.is_dir() => stack.push(entry.path()),
                Ok(meta) if meta.is_file() => files.push((entry.path(), meta)),
                _ => {}
            }
        }
    }
    files
}

/// Removes a file or a folder, counting what it frees.
fn remove(path: &Path, cleaned: &mut Cleaned, cancel: &Cancel) -> Result<(), DiskError> {
    if cancel.is_cancelled() {
        return Err(DiskError::Cancelled);
    }
    let Ok(meta) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    let error = |err: io::Error| DiskError::Remove {
        path: path.display().to_string(),
        reason: err.to_string(),
    };
    if meta.is_dir() {
        let mut sizer = Sizer::new(cancel);
        let bytes = sizer.size_of(path, &meta)?;
        fs::remove_dir_all(path).map_err(error)?;
        cleaned.bytes += bytes;
        cleaned.files += sizer.files;
    } else {
        fs::remove_file(path).map_err(error)?;
        cleaned.bytes += meta.len();
        cleaned.files += 1;
    }
    Ok(())
}

/// Removes the empty folders under `dir`, keeping `dir`.
fn prune_empty_dirs(dir: &Path) {
    fn prune(dir: &Path) -> bool {
        let mut empty = true;
        for entry in read_dir(dir) {
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => {
                    if prune(&entry.path()) {
                        let _ = fs::remove_dir(entry.path());
                    } else {
                        empty = false;
                    }
                }
                _ => empty = false,
            }
        }
        empty
    }
    prune(dir);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        game::{
            platform::{Arch, Os},
            record::FileRecord,
        },
        instances::Loader,
    };

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn instance(id: &str, game: &str, loader: Option<(Loader, &str)>) -> Instance {
        Instance {
            id: id.into(),
            name: id.into(),
            game_version: game.into(),
            loader: loader.map_or(Loader::None, |(l, _)| l),
            loader_version: loader.map(|(_, v)| v.to_string()),
            ..Default::default()
        }
    }

    fn version(data: &DataDir, id: &str, json: serde_json::Value, record: &[(&str, &[u8])]) {
        let dir = data.versions_dir().join(id);
        write(
            &dir.join(format!("{id}.json")),
            serde_json::to_vec(&json).unwrap().as_slice(),
        );
        write(&dir.join(format!("{id}.jar")), b"jar");
        let mut install = InstallRecord::new("key");
        for (key, bytes) in record {
            write(&data.root().join(key), bytes);
            install.files.insert(
                key.to_string(),
                FileRecord {
                    size: bytes.len() as u64,
                    mtime_ns: 0,
                    sha1: "0".repeat(40),
                },
            );
        }
        install
            .save(&version_record_path(&data.versions_dir(), id))
            .unwrap();
    }

    fn vanilla(id: &str, index: &str, runtime: &str, major: u32) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "mainClass": "net.minecraft.client.main.Main",
            "assetIndex": { "id": index, "sha1": "0", "size": 1, "url": "https://x/" },
            "javaVersion": { "component": runtime, "majorVersion": major },
            "libraries": [],
        })
    }

    fn index(data: &DataDir, id: &str, hashes: &[&str]) {
        let objects: serde_json::Map<String, serde_json::Value> = hashes
            .iter()
            .enumerate()
            .map(|(i, h)| (format!("a{i}"), serde_json::json!({ "hash": h, "size": 1 })))
            .collect();
        write(
            &data.assets_dir().join("indexes").join(format!("{id}.json")),
            serde_json::to_vec(&serde_json::json!({ "objects": objects }))
                .unwrap()
                .as_slice(),
        );
        for hash in hashes {
            write(
                &data
                    .assets_dir()
                    .join("objects")
                    .join(&hash[..2])
                    .join(hash),
                b"x",
            );
        }
    }

    /// Two vanilla versions and a Fabric profile on one; only 1.20.1 + Fabric is used.
    fn fixture() -> (tempfile::TempDir, DataDir, Vec<Instance>) {
        let tmp = tempfile::tempdir().unwrap();
        let data = DataDir::at(tmp.path());
        let used_hash = "aa".to_string() + &"1".repeat(38);
        let unused_hash = "bb".to_string() + &"2".repeat(38);
        version(
            &data,
            "1.20.1",
            vanilla("1.20.1", "5", "java-runtime-gamma", 17),
            &[
                ("libraries/shared/lib.jar", b"shared"),
                ("libraries/only120/a.jar", b"a"),
            ],
        );
        version(
            &data,
            "fabric-loader-0.16.14-1.20.1",
            serde_json::json!({
                "id": "fabric-loader-0.16.14-1.20.1",
                "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                "inheritsFrom": "1.20.1",
                "libraries": [{ "name": "net.fabricmc:fabric-loader:0.16.14" }],
            }),
            &[],
        );
        write(
            &data
                .libraries_dir()
                .join("net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar"),
            b"fabric",
        );
        version(
            &data,
            "1.8.9",
            vanilla("1.8.9", "1.8", "jre-legacy", 8),
            &[
                ("libraries/shared/lib.jar", b"shared"),
                ("libraries/old/old.jar", b"old-lib"),
            ],
        );
        index(&data, "5", &[&used_hash]);
        index(&data, "1.8", &[&unused_hash]);
        // a processor output: in no record
        write(&data.libraries_dir().join("forge/patched.jar"), b"patched");
        write(
            &data.runtimes_dir().join("java-runtime-gamma/mac-os/java"),
            b"17",
        );
        write(&data.runtimes_dir().join("jre-legacy/mac-os/java"), b"8");
        write(&data.cache_dir().join("version_manifest_v2.json"), b"{}");
        let game = data.instances_dir().join("fab").join("minecraft");
        write(&game.join("saves/World/level.dat"), b"world");
        write(&game.join("mods/a.jar"), b"mod");
        write(&game.join("logs/latest.log"), b"log");
        write(&game.join("screenshots/s.png"), b"png");
        write(&game.join("options.txt"), b"opt");
        let instances = vec![instance("fab", "1.20.1", Some((Loader::Fabric, "0.16.14")))];
        (tmp, data, instances)
    }

    #[test]
    fn sizes_instances_and_categories() {
        let (_tmp, data, instances) = fixture();
        let report = scan(&data, &instances, &Cancel::new()).unwrap();
        let fab = &report.instances[0];
        assert_eq!(fab.worlds, 5);
        assert_eq!(fab.content, 3);
        assert_eq!(fab.logs, 3);
        assert_eq!(fab.screenshots, 3);
        assert_eq!(fab.other, 3);
        assert_eq!(report.logs.files, 1);
        assert_eq!(report.logs.old_files, 0);
        assert_eq!(report.cache, 2);
        let gamma = report
            .runtimes
            .iter()
            .find(|r| r.component == "java-runtime-gamma")
            .unwrap();
        assert_eq!(gamma.java, Some(17));
        assert_eq!(gamma.used_by, vec!["1.20.1".to_string()]);
        assert!(
            report
                .runtimes
                .iter()
                .find(|r| r.component == "jre-legacy")
                .unwrap()
                .unused()
        );
        assert_eq!(report.game_files.unused.versions, vec!["1.8.9".to_string()]);
        // old.jar (7) only; the shared library and the processor output stay
        assert_eq!(report.game_files.unused.libraries, 7);
        assert!(report.game_files.unused.assets > 0);
        let total_on_disk = Sizer::new(&Cancel::new()).size(data.root()).unwrap();
        assert_eq!(report.total(), total_on_disk);
    }

    #[test]
    fn removes_only_what_nothing_uses() {
        let (_tmp, data, instances) = fixture();
        let cleaned = clean(
            &data,
            &instances,
            &[],
            Clean::UnusedGameFiles,
            &Cancel::new(),
        )
        .unwrap();
        assert!(cleaned.bytes > 0);
        assert!(!data.versions_dir().join("1.8.9").exists());
        assert!(data.versions_dir().join("1.20.1/1.20.1.jar").exists());
        assert!(
            data.versions_dir()
                .join("fabric-loader-0.16.14-1.20.1")
                .exists()
        );
        assert!(data.libraries_dir().join("shared/lib.jar").exists());
        assert!(data.libraries_dir().join("only120/a.jar").exists());
        assert!(
            data.libraries_dir()
                .join("net/fabricmc/fabric-loader/0.16.14/fabric-loader-0.16.14.jar")
                .exists()
        );
        assert!(data.libraries_dir().join("forge/patched.jar").exists());
        assert!(!data.libraries_dir().join("old").exists());
        let used_hash = "aa".to_string() + &"1".repeat(38);
        assert!(
            data.assets_dir()
                .join("objects/aa")
                .join(&used_hash)
                .exists()
        );
        assert!(!data.assets_dir().join("objects/bb").exists());
        assert!(data.assets_dir().join("indexes/5.json").exists());
        assert!(!data.assets_dir().join("indexes/1.8.json").exists());
        // nothing more to remove
        let again = scan(&data, &instances, &Cancel::new()).unwrap();
        assert_eq!(again.game_files.unused.total(), 0);
    }

    #[test]
    fn a_version_not_installed_yet_doesnt_block_cleanup() {
        let (_tmp, data, mut instances) = fixture();
        instances.push(instance("new", "1.21.1", None));
        let report = scan(&data, &instances, &Cancel::new()).unwrap();
        assert_eq!(report.game_files.unused.libraries, 7);
        assert!(report.game_files.unused.assets > 0);
    }

    #[test]
    fn an_unreadable_version_keeps_libraries_and_assets() {
        let (_tmp, data, instances) = fixture();
        // the version in use is on disk but doesn't parse
        write(
            &data.versions_dir().join("1.20.1/1.20.1.json"),
            b"{ not json",
        );
        let report = scan(&data, &instances, &Cancel::new()).unwrap();
        assert_eq!(report.game_files.unused.versions, vec!["1.8.9".to_string()]);
        assert_eq!(report.game_files.unused.libraries, 0);
        assert_eq!(report.game_files.unused.assets, 0);
        clean(
            &data,
            &instances,
            &[],
            Clean::UnusedGameFiles,
            &Cancel::new(),
        )
        .unwrap();
        assert!(data.libraries_dir().join("old/old.jar").exists());
        assert!(!data.versions_dir().join("1.8.9").exists());
    }

    #[test]
    fn old_import_leftovers_go_and_new_ones_stay() {
        let (_tmp, data, instances) = fixture();
        let old_pack = data.instances_dir().join(".lodestone-pack-abc.mrpack");
        let staging = data.instances_dir().join(".lodestone-staging-1");
        let fresh = data.instances_dir().join(".lodestone-staging-2");
        write(&old_pack, b"pack");
        write(&staging.join("minecraft/mods/a.jar"), b"jar");
        write(&fresh.join("minecraft/mods/b.jar"), b"jar");
        let two_hours_ago = SystemTime::now() - Duration::from_secs(2 * 60 * 60);
        for path in [
            old_pack.clone(),
            staging.join("minecraft/mods/a.jar"),
            staging.join("minecraft/mods"),
            staging.join("minecraft"),
            staging.clone(),
        ] {
            let mut options = fs::File::options();
            options.write(path.is_file()).read(true);
            // Windows opens a folder only with FILE_FLAG_BACKUP_SEMANTICS, and changing
            // its time needs write access.
            #[cfg(windows)]
            {
                use std::os::windows::fs::OpenOptionsExt;
                options.custom_flags(0x0200_0000).write(true);
            }
            options
                .open(&path)
                .unwrap()
                .set_modified(two_hours_ago)
                .unwrap();
        }
        let report = scan(&data, &instances, &Cancel::new()).unwrap();
        assert_eq!(
            report.leftovers.items,
            vec![
                ".lodestone-pack-abc.mrpack".to_string(),
                ".lodestone-staging-1".to_string()
            ]
        );
        assert_eq!(report.leftovers.bytes, 7);
        clean(&data, &instances, &[], Clean::Leftovers, &Cancel::new()).unwrap();
        assert!(!old_pack.exists());
        assert!(!staging.exists());
        assert!(fresh.exists());
        assert!(data.instances_dir().join("fab").exists());
    }

    #[test]
    fn removes_unused_runtimes_cache_and_logs() {
        let (_tmp, data, instances) = fixture();
        let cleaned = clean(
            &data,
            &instances,
            &[],
            Clean::UnusedRuntimes,
            &Cancel::new(),
        )
        .unwrap();
        assert_eq!(cleaned.bytes, 1);
        assert!(!data.runtimes_dir().join("jre-legacy").exists());
        assert!(data.runtimes_dir().join("java-runtime-gamma").exists());

        clean(&data, &instances, &[], Clean::Cache, &Cancel::new()).unwrap();
        assert!(data.cache_dir().exists());
        assert_eq!(read_dir(&data.cache_dir()).len(), 0);

        // recent logs stay for "old", and a running instance's stay for "all"
        let log = data.instances_dir().join("fab/minecraft/logs/latest.log");
        clean(&data, &instances, &[], Clean::OldLogs, &Cancel::new()).unwrap();
        assert!(log.exists());
        clean(
            &data,
            &instances,
            &["fab".into()],
            Clean::AllLogs,
            &Cancel::new(),
        )
        .unwrap();
        assert!(log.exists());
        clean(&data, &instances, &[], Clean::AllLogs, &Cancel::new()).unwrap();
        assert!(!log.exists());
        assert!(
            data.instances_dir()
                .join("fab/minecraft/saves/World/level.dat")
                .exists()
        );
    }

    /// On Apple silicon, 1.16.5 runs in Zulu 8 and 1.17.1 in Mojang's arm64 Java 17
    /// (`java-runtime-gamma`), not in the runtimes their files name; 1.12.2 runs in the
    /// Intel Java 8 those name. Those are kept, and Zulu's older build goes. On an Intel
    /// Mac the same versions use the runtimes their files name.
    #[test]
    fn apple_silicon_keeps_the_java_versions_really_run_in() {
        let tmp = tempfile::tempdir().unwrap();
        let data = DataDir::at(tmp.path());
        let mut instances = Vec::new();
        for id in ["1.12.2", "1.16.5", "1.17.1"] {
            let path = format!("{}/testdata/versions/{id}.json", env!("CARGO_MANIFEST_DIR"));
            let json: serde_json::Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
            version(&data, id, json, &[]);
            instances.push(instance(id, id, None));
        }
        let runtimes = data.runtimes_dir();
        for folder in [
            "jre-legacy/mac-os",
            "java-runtime-alpha/mac-os",
            "java-runtime-beta/mac-os",
            "java-runtime-gamma/mac-os-arm64",
        ] {
            write(&runtimes.join(folder).join("java"), b"j");
        }
        let zulu = runtimes.join("zulu-8/mac-os-arm64");
        for (build, size) in [
            ("zulu8.94.0.17-ca-jre8.0.492-macosx_aarch64", 3),
            ("zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64", 5),
        ] {
            let java = zulu.join(build).join("zulu-8.jre/Contents/Home/bin/java");
            write(&java, &vec![b'z'; size]);
            crate::game::runtime::mark_installed(&zulu.join(build), &"0".repeat(64)).unwrap();
        }
        let arm64 = Platform::new(Os::MacOs, Arch::Arm64, "14.6.1");

        let report = scan_on(&data, &instances, &arm64, &Cancel::new()).unwrap();
        let row = |component: &str| {
            report
                .runtimes
                .iter()
                .find(|r| r.component == component)
                .unwrap_or_else(|| panic!("{component}: {:#?}", report.runtimes))
                .clone()
        };
        assert_eq!(row("zulu-8").used_by, ["1.16.5"]);
        assert_eq!(row("zulu-8").java, Some(8));
        assert_eq!(row("java-runtime-gamma").used_by, ["1.17.1"]);
        assert_eq!(row("java-runtime-gamma").java, Some(17));
        assert_eq!(row("jre-legacy").used_by, ["1.12.2"]);
        assert!(row("java-runtime-alpha").unused());
        assert!(row("java-runtime-beta").unused());
        let older = row("zulu-8/zulu8.94.0.17-ca-jre8.0.492-macosx_aarch64");
        assert!(older.unused());
        assert_eq!(older.java, Some(8));

        clean_on(
            &data,
            &instances,
            &[],
            Clean::UnusedRuntimes,
            &arm64,
            &Cancel::new(),
        )
        .unwrap();
        assert!(
            zulu.join("zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64")
                .exists()
        );
        assert!(
            !zulu
                .join("zulu8.94.0.17-ca-jre8.0.492-macosx_aarch64")
                .exists()
        );
        assert!(runtimes.join("java-runtime-gamma").exists());
        assert!(runtimes.join("jre-legacy").exists());
        assert!(!runtimes.join("java-runtime-alpha").exists());
        assert!(!runtimes.join("java-runtime-beta").exists());

        // An Intel Mac runs them in the runtimes their files name: Zulu goes.
        let intel = Platform::new(Os::MacOs, Arch::X86_64, "14.6.1");
        let report = scan_on(&data, &instances, &intel, &Cancel::new()).unwrap();
        let zulu_row = report
            .runtimes
            .iter()
            .find(|r| r.component == "zulu-8")
            .unwrap();
        assert!(zulu_row.unused());
        assert!(
            report
                .runtimes
                .iter()
                .any(|r| r.component == "java-runtime-gamma" && r.unused())
        );
    }

    #[test]
    fn a_cancelled_scan_stops() {
        let (_tmp, data, instances) = fixture();
        let cancel = Cancel::new();
        cancel.cancel();
        assert!(matches!(
            clean(&data, &instances, &[], Clean::Cache, &cancel),
            Err(DiskError::Cancelled)
        ));
    }

    #[test]
    fn keys_outside_the_folder_are_refused() {
        let root = Path::new("/data");
        assert!(inside(root, "libraries/a/b.jar").is_some());
        assert!(inside(root, "../etc/passwd").is_none());
        assert!(inside(root, "/etc/passwd").is_none());
        assert!(inside(root, "").is_none());
        assert!(!safe_name(".."));
        assert!(!safe_name("a/b"));
        assert!(safe_name("1.20.1"));
    }

    #[test]
    fn clean_names_round_trip() {
        for clean in [
            Clean::UnusedGameFiles,
            Clean::UnusedRuntimes,
            Clean::Cache,
            Clean::Leftovers,
            Clean::OldLogs,
            Clean::AllLogs,
        ] {
            assert_eq!(Clean::from_name(clean.name()), Some(clean));
        }
    }
}
