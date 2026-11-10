//! Finding the mod that crashes the game, by turning mods off in halves.
//!
//! "Turn mods off one at a time until the crash stops" is what people are told to do,
//! and with two hundred mods it takes all evening. Halving does it in about eight
//! tries: turn half of the mods that could be the cause off and play. If the game
//! still crashes, the cause is among the mods left on; if not, among those turned off.
//! Each try halves the mods that could be the cause, until one is left.
//!
//! A mod can't load without the mods it requires, so turning one off also turns off
//! every mod that needs it ([`Mods::closure`]); otherwise the loader would refuse to
//! start and the try would say nothing. Halves are picked from the mods nothing else
//! needs first, so a library everything needs doesn't take everything with it.
//!
//! The mods the crash pointed at ([`crate::game::crash::suspects`]) are tried first:
//! when the crash stops with just those off, the search has jumped most of the way.
//!
//! A search outlives the launcher: it is saved in the instance's `mod-finder.json`
//! before any mod is renamed, with every mod it may have turned off, so a search that
//! is interrupted can still put them all back ([`restore`]).

use std::{
    collections::{BTreeSet, HashMap},
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::PathBuf,
};

use lodestone_minecraft::jar;
use serde::{Deserialize, Serialize};

use super::{
    ContentError, ContentKind, InstanceDirs, disabled_name, enabled_name, files, io_error, scan,
};

/// The file in the instance's folder that a search is saved in.
pub const FILE_NAME: &str = "mod-finder.json";

/// One mod jar, and what it gives and needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModNode {
    /// The file name, without `.disabled`.
    pub file_name: String,
    pub name: String,
    /// The mod ids it gives: its own, those it `provides`, and those of the jars it
    /// bundles.
    pub provides: Vec<String>,
    /// The mod ids it can't load without.
    pub requires: Vec<String>,
}

/// The mods of a search, and which needs which.
#[derive(Clone, Debug, Default)]
pub struct Mods {
    nodes: Vec<ModNode>,
}

impl Mods {
    pub fn new(nodes: Vec<ModNode>) -> Self {
        Self { nodes }
    }

    /// Reads `file_names` (without `.disabled`; each may be on or off now) from the
    /// instance's `mods/`. Opens every jar, so call it off the window's thread.
    pub fn read(dirs: &InstanceDirs, file_names: &[String]) -> Self {
        let dir = dirs.content_dir(ContentKind::Mod);
        let names: HashMap<String, String> = scan::scan(dirs, ContentKind::Mod)
            .unwrap_or_default()
            .into_iter()
            .map(|file| (file.enabled_name(), file.display_name()))
            .collect();
        let nodes = file_names
            .iter()
            .map(|file_name| {
                let on = dir.join(file_name);
                let path = if on.exists() {
                    on
                } else {
                    dir.join(disabled_name(file_name))
                };
                let (provides, requires) = read_ids(&path);
                ModNode {
                    file_name: file_name.clone(),
                    name: names
                        .get(file_name)
                        .cloned()
                        .unwrap_or_else(|| file_name.trim_end_matches(".jar").to_string()),
                    provides,
                    requires,
                }
            })
            .collect();
        Self { nodes }
    }

    /// The mod's name, or its file name.
    pub fn name(&self, file_name: &str) -> String {
        self.nodes
            .iter()
            .find(|n| n.file_name == file_name)
            .map_or_else(|| file_name.to_string(), |n| n.name.clone())
    }

    /// `off`, and every mod that requires an id that only mods in it give (after
    /// those, the mods that need them, and so on). Ids no mod here gives, such as
    /// `minecraft` or the loader's, don't count.
    pub fn closure(&self, off: &BTreeSet<String>) -> BTreeSet<String> {
        let mut givers: HashMap<&str, Vec<&str>> = HashMap::new();
        for node in &self.nodes {
            for id in &node.provides {
                givers
                    .entry(id.as_str())
                    .or_default()
                    .push(node.file_name.as_str());
            }
        }
        let mut off = off.clone();
        loop {
            let mut changed = false;
            for node in &self.nodes {
                if off.contains(&node.file_name) {
                    continue;
                }
                let stranded = node.requires.iter().any(|id| {
                    givers
                        .get(id.as_str())
                        .is_some_and(|g| g.iter().all(|file| off.contains(*file)))
                });
                if stranded {
                    off.insert(node.file_name.clone());
                    changed = true;
                }
            }
            if !changed {
                return off;
            }
        }
    }
}

/// A mod named in a prompt about the mods another needs, or that need it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkedMod {
    /// Its file name as it is now (with `.disabled` for a mod that is off).
    pub file_name: String,
    pub name: String,
}

fn sorted(mut mods: Vec<LinkedMod>) -> Vec<LinkedMod> {
    mods.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.file_name.cmp(&b.file_name))
    });
    mods
}

/// The enabled mods that couldn't load once `file_name` (an enabled mod's file name)
/// is turned off: those that require an id only it gives, then those that need them,
/// and so on, sorted by name. A mod that another enabled mod can stand in for (both
/// give the id) is no one's only source, and ids no mod gives (`minecraft`, the
/// loader's) don't count. Opens every enabled jar, so call it off the window's thread.
pub fn dependents(dirs: &InstanceDirs, file_name: &str) -> Vec<LinkedMod> {
    let file_name = enabled_name(file_name);
    let enabled: Vec<String> = scan::scan(dirs, ContentKind::Mod)
        .unwrap_or_default()
        .into_iter()
        .filter(|file| file.enabled_by_name() && !file.is_dir)
        .map(|file| file.file_name)
        .collect();
    if !enabled.contains(&file_name) {
        return Vec::new();
    }
    let mods = Mods::read(dirs, &enabled);
    let off = mods.closure(&BTreeSet::from([file_name.clone()]));
    sorted(
        off.into_iter()
            .filter(|file| *file != file_name)
            .map(|file| LinkedMod {
                name: mods.name(&file),
                file_name: file,
            })
            .collect(),
    )
}

/// The mods that are off and must be turned on too for `file_name` (a mod that is
/// off, with or without `.disabled`) to load: for each id it requires that no enabled
/// mod gives, a mod that is off and gives it, then what those require, and so on,
/// sorted by name. Only what `file_name` leads to counts, not other enabled mods'
/// missing ones. Ids no mod in the folder gives (`minecraft`, the loader's, a mod
/// that isn't there) don't count; where several mods that are off give an id, the one
/// first by file name is picked. Opens every jar, so call it off the window's thread.
pub fn requirements(dirs: &InstanceDirs, file_name: &str) -> Vec<LinkedMod> {
    let target = enabled_name(file_name);
    let files: Vec<(String, bool)> = scan::scan(dirs, ContentKind::Mod)
        .unwrap_or_default()
        .into_iter()
        .filter(|file| !file.is_dir)
        .map(|file| (file.enabled_name(), file.enabled_by_name()))
        .collect();
    if !files.iter().any(|(name, on)| *name == target && !on) {
        return Vec::new();
    }
    let mut names: Vec<String> = files.iter().map(|(name, _)| name.clone()).collect();
    names.sort();
    names.dedup();
    let mods = Mods::read(dirs, &names);
    let node = |file: &str| mods.nodes.iter().find(|n| n.file_name == file);
    // The mods that will be on: those on now, the target, and what is added.
    let mut on: BTreeSet<String> = files
        .iter()
        .filter(|(_, on)| *on)
        .map(|(name, _)| name.clone())
        .collect();
    on.insert(target.clone());
    let gives =
        |file: &str, id: &str| node(file).is_some_and(|n| n.provides.iter().any(|p| p == id));
    let mut added = Vec::new();
    let mut queue = vec![target.clone()];
    while let Some(file) = queue.pop() {
        let Some(needs) = node(&file).map(|n| n.requires.clone()) else {
            continue;
        };
        for id in needs {
            if on.iter().any(|f| gives(f, &id)) {
                continue;
            }
            // `names` is in file name order, so the first giver is picked.
            if let Some(giver) = names.iter().find(|f| !on.contains(*f) && gives(f, &id)) {
                on.insert(giver.clone());
                added.push(giver.clone());
                queue.push(giver.clone());
            }
        }
    }
    sorted(
        added
            .into_iter()
            .map(|file| LinkedMod {
                name: mods.name(&file),
                file_name: disabled_name(&file),
            })
            .collect(),
    )
}

/// The mod ids a jar gives and requires, from its metadata and its bundled jars'.
fn read_ids(path: &std::path::Path) -> (Vec<String>, Vec<String>) {
    let Some(mut archive) = File::open(path)
        .ok()
        .and_then(|file| zip::ZipArchive::new(BufReader::new(file)).ok())
    else {
        return (Vec::new(), Vec::new());
    };
    let (mut provides, requires) = metadata_ids(&mut archive);
    let nested: Vec<String> = archive
        .file_names()
        .filter_map(Result::ok)
        .filter(|name| {
            (name.starts_with("META-INF/jars/") || name.starts_with("META-INF/jarjar/"))
                && name.ends_with(".jar")
        })
        .map(|name| name.into_owned())
        .collect();
    for name in nested {
        let Some(bytes) = read_entry(&mut archive, &name, 64 * 1024 * 1024) else {
            continue;
        };
        if let Ok(mut inner) = zip::ZipArchive::new(Cursor::new(bytes)) {
            provides.extend(metadata_ids(&mut inner).0);
        }
    }
    provides.sort();
    provides.dedup();
    (provides, requires)
}

/// The ids a jar's own metadata gives and requires.
fn metadata_ids<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> (Vec<String>, Vec<String>) {
    let mut provides = Vec::new();
    let mut requires = Vec::new();
    if let Some(text) = read_text(archive, "fabric.mod.json") {
        provides.extend(jar::fabric_ids(&text));
        requires.extend(jar::fabric_environment_and_depends(&text).1);
    }
    if let Some(text) = read_text(archive, "quilt.mod.json") {
        provides.extend(jar::quilt_ids(&text));
        requires.extend(jar::quilt_environment_and_depends(&text).1);
    }
    for file in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some(text) = read_text(archive, file) {
            let (ids, needed) = jar::mods_toml_ids_and_required(&text);
            provides.extend(ids);
            requires.extend(needed);
        }
    }
    (provides, requires)
}

fn read_text<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, name: &str) -> Option<String> {
    let bytes = read_entry(archive, name, 1024 * 1024)?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

fn read_entry<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    max: u64,
) -> Option<Vec<u8>> {
    let entry = archive.by_name(name).ok()?;
    if entry.size() > max {
        return None;
    }
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry.take(max).read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// A search, as saved between tries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Search {
    /// The mods that were on when it started (file names without `.disabled`).
    pub mods: Vec<String>,
    /// The mods one of which is still the cause.
    pub candidates: Vec<String>,
    /// The mods the crash pointed at, tried first.
    #[serde(default)]
    pub suspects: Vec<String>,
    /// The mods the search may have turned off, to turn back on.
    #[serde(default)]
    pub turned_off: Vec<String>,
    /// The mods off in the try being played.
    #[serde(default)]
    pub testing: Option<Vec<String>>,
    #[serde(default)]
    pub rounds: Vec<Round>,
    /// Whether the crash came while the game was starting, so a try that reaches the
    /// main menu has worked.
    #[serde(default)]
    pub at_startup: bool,
}

/// A try that has been played.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Round {
    pub off: Vec<String>,
    pub crashed: bool,
}

/// What to do next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    /// Play with these mods off.
    Test(BTreeSet<String>),
    /// These mods are the cause: one, or a few that need each other and so can only
    /// be turned off together.
    Found(Vec<String>),
    /// The game crashed even with all the mods that could be the cause turned off, so
    /// it isn't one of them (or the crash doesn't happen every time).
    NotFound,
}

impl Search {
    /// A search through the instance's enabled mods, starting with `suspects`.
    pub fn start(dirs: &InstanceDirs, suspects: &[String], at_startup: bool) -> Self {
        let mods: Vec<String> = scan::scan(dirs, ContentKind::Mod)
            .unwrap_or_default()
            .into_iter()
            .filter(|file| file.enabled_by_name() && !file.is_dir)
            .map(|file| file.file_name)
            .collect();
        Self::new(mods, suspects, at_startup)
    }

    pub fn new(mods: Vec<String>, suspects: &[String], at_startup: bool) -> Self {
        let suspects = suspects
            .iter()
            .map(|s| enabled_name(s))
            .filter(|s| mods.contains(s))
            .collect();
        Self {
            candidates: mods.clone(),
            mods,
            suspects,
            turned_off: Vec::new(),
            testing: None,
            rounds: Vec::new(),
            at_startup,
        }
    }

    /// Picks the next try (and remembers it), or says the search is over.
    pub fn next(&mut self, mods: &Mods) -> Step {
        let candidates: BTreeSet<String> = self.candidates.iter().cloned().collect();
        let within = |off: &BTreeSet<String>| off.intersection(&candidates).count();
        // Every try so far crashed, so nothing yet shows a mod is the cause: the last
        // ones left are only blamed once the crash stops with them off.
        let confirmed = self.rounds.iter().any(|r| !r.crashed);
        let found = |this: &mut Self| {
            if confirmed {
                Step::Found(this.candidates.clone())
            } else {
                let off = mods.closure(&candidates);
                this.testing = Some(off.iter().cloned().collect());
                Step::Test(off)
            }
        };
        match candidates.len() {
            0 => return Step::NotFound,
            1 => return found(self),
            _ => {}
        }

        // The mods the crash pointed at, on the first try.
        if self.rounds.is_empty() {
            let suspects: BTreeSet<String> = self
                .suspects
                .iter()
                .filter(|s| candidates.contains(*s))
                .cloned()
                .collect();
            if !suspects.is_empty() {
                let off = mods.closure(&suspects);
                if within(&off) < candidates.len() {
                    self.testing = Some(off.iter().cloned().collect());
                    return Step::Test(off);
                }
            }
        }

        // Half of them, the mods nothing needs first.
        let mut order: Vec<(usize, &String)> = candidates
            .iter()
            .map(|c| (within(&mods.closure(&BTreeSet::from([c.clone()]))), c))
            .collect();
        order.sort();
        let half = candidates.len() / 2;
        let mut chosen = BTreeSet::new();
        let mut off = BTreeSet::new();
        for (_, candidate) in order {
            if within(&off) >= half {
                break;
            }
            let mut more = chosen.clone();
            more.insert(candidate.clone());
            let more_off = mods.closure(&more);
            if within(&more_off) == candidates.len() {
                // That would turn every candidate off, which says nothing.
                continue;
            }
            chosen = more;
            off = more_off;
        }
        if off.is_empty() {
            // They all need each other: they can only be turned off together.
            return found(self);
        }
        self.testing = Some(off.iter().cloned().collect());
        Step::Test(off)
    }

    /// Records how the try went: whether the game still crashed.
    pub fn record(&mut self, crashed: bool) {
        let Some(off) = self.testing.take() else {
            return;
        };
        self.candidates.retain(|c| off.contains(c) != crashed);
        self.rounds.push(Round { off, crashed });
    }

    /// About how many more tries the search takes.
    pub fn tries_left(&self) -> usize {
        let n = self.candidates.len();
        if n <= 1 {
            0
        } else {
            (usize::BITS - (n - 1).leading_zeros()) as usize
        }
    }
}

fn path(dirs: &InstanceDirs) -> PathBuf {
    dirs.instance.join(FILE_NAME)
}

/// The instance's interrupted search, if it has one.
pub fn load(dirs: &InstanceDirs) -> Option<Search> {
    let bytes = std::fs::read(path(dirs)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub fn save(dirs: &InstanceDirs, search: &Search) -> Result<(), ContentError> {
    let path = path(dirs);
    let json =
        serde_json::to_vec_pretty(search).map_err(|err| ContentError::Invalid(err.to_string()))?;
    crate::storage::write_atomic(&path, &json).map_err(io_error(&path))
}

/// Turns off exactly `off` of the search's mods, and back on the others it turned off.
/// Saves the search first, with every mod that may end up off, so an interrupted
/// search can still turn them all back on.
pub fn apply(
    dirs: &InstanceDirs,
    search: &mut Search,
    off: &BTreeSet<String>,
) -> Result<(), ContentError> {
    let before: BTreeSet<String> = search.turned_off.iter().cloned().collect();
    search.turned_off = before.union(off).cloned().collect();
    save(dirs, search)?;
    for file_name in before.difference(off) {
        set_enabled(dirs, file_name, true)?;
    }
    for file_name in off {
        set_enabled(dirs, file_name, false)?;
    }
    search.turned_off = off.iter().cloned().collect();
    save(dirs, search)
}

/// Ends the search: turns back on every mod it turned off except `keep_off`, turns
/// `keep_off` off, and forgets the search.
pub fn restore(
    dirs: &InstanceDirs,
    search: &Search,
    keep_off: &BTreeSet<String>,
) -> Result<(), ContentError> {
    for file_name in search.turned_off.iter().filter(|f| !keep_off.contains(*f)) {
        set_enabled(dirs, file_name, true)?;
    }
    for file_name in keep_off {
        set_enabled(dirs, file_name, false)?;
    }
    forget(dirs)
}

/// Forgets the search without touching any mod.
pub fn forget(dirs: &InstanceDirs) -> Result<(), ContentError> {
    let path = path(dirs);
    match crate::fsx::remove_file(&path) {
        Err(err) if err.kind() != std::io::ErrorKind::NotFound => Err(io_error(&path)(err)),
        _ => Ok(()),
    }
}

/// Turns a mod on or off, whichever way it is now. A mod that is gone (the user
/// deleted it) is fine: it is off.
fn set_enabled(dirs: &InstanceDirs, file_name: &str, enabled: bool) -> Result<(), ContentError> {
    let current = if enabled {
        disabled_name(file_name)
    } else {
        file_name.to_string()
    };
    match files::set_mod_enabled(dirs, &current, enabled) {
        Ok(_) | Err(ContentError::NotFound(_)) => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests;
