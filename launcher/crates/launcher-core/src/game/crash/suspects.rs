//! Which mods a crash points at.
//!
//! "Here is the log" is not much help to someone with two hundred mods, yet a crash
//! usually says which ones were involved:
//!
//! - a mod that couldn't patch the game is named in the error (`... from mod sodium`);
//! - when the game crashes in a mod's own code, the stack trace's first frames are in a
//!   class from that mod's jar;
//! - code a mod injected into the game shows up in the trace as a method named
//!   `redirect$bakh000$zenith$...`, with the mod's id in it;
//! - the report's "Mixins in Stacktrace" block lists the mods that patched each class in
//!   the crashing stack.
//!
//! A mod that patched the crashing code is a lead, not proof: Sodium and its kind patch
//! nearly every render class. So the evidence is ranked, a mod that patched most of the
//! stack counts for little, and [`likely`] names one mod only when the evidence points at
//! one. Only the report's first "Mixins in Stacktrace" block counts (the one under
//! `-- Head --`): the later ones are for other stacks, such as the code that wrote the
//! report.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
};

use super::super::log4j::LogLine;
use crate::content::{ContentKind, InstanceDirs, scan};

/// A mod a crash points at, strongest evidence first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Suspect {
    /// The jar in `mods/`.
    pub file_name: String,
    /// The mod's name, or the file name without `.jar`.
    pub name: String,
    pub evidence: Evidence,
}

/// Why a mod is a suspect, strongest first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Evidence {
    /// The error says the mod couldn't patch the game.
    FailedToPatch,
    /// The crash happened in the mod's own code.
    CrashedInItsCode,
    /// The mod changed the code that crashed.
    PatchedCrashingCode,
    /// The mod's code was running, further from the crash.
    InStack,
}

impl Evidence {
    fn weight(self) -> u32 {
        match self {
            Evidence::FailedToPatch => 100,
            Evidence::CrashedInItsCode => 60,
            Evidence::PatchedCrashingCode => 20,
            Evidence::InStack => 5,
        }
    }
}

/// Whether the evidence points at a single mod: the first suspect crashed in its own
/// code or failed to patch the game, and no other has evidence that strong.
pub fn likely(suspects: &[Suspect]) -> bool {
    let strong = |s: &Suspect| s.evidence <= Evidence::CrashedInItsCode;
    suspects.first().is_some_and(strong) && !suspects.get(1).is_some_and(strong)
}

/// The most suspects worth showing.
pub const MAX_SUSPECTS: usize = 5;

/// Finds the enabled mods in the instance's `mods/` that the crash report (if one was
/// written) and the end of the log point at. Opens the mods' jars, so call it off the
/// window's thread.
pub fn find(dirs: &InstanceDirs, crash_report: Option<&str>, log: &[LogLine]) -> Vec<Suspect> {
    let clues = clues(crash_report, log);
    if clues.is_empty() {
        return Vec::new();
    }
    let Ok(files) = scan::scan(dirs, ContentKind::Mod) else {
        return Vec::new();
    };
    let mods: Vec<_> = files
        .into_iter()
        .filter(|f| f.enabled_by_name() && !f.is_dir)
        .collect();
    let mods_dir = dirs.content_dir(ContentKind::Mod);
    let paths: Vec<_> = mods.iter().map(|m| mods_dir.join(&m.file_name)).collect();
    let owners = owners(&paths, &clues.classes(), &clues.configs());
    let by_id: HashMap<String, usize> = mods
        .iter()
        .enumerate()
        .filter_map(|(i, m)| Some((m.meta.id.clone()?.to_ascii_lowercase(), i)))
        .collect();

    // The evidence for each jar, by its index in `mods`.
    let mut scores: BTreeMap<usize, (Evidence, u32)> = BTreeMap::new();
    let mut add = |index: usize, evidence: Evidence| {
        let entry = scores.entry(index).or_insert((evidence, 0));
        entry.0 = entry.0.min(evidence);
        entry.1 += evidence.weight();
    };
    for clue in &clues.list {
        let index = match &clue.target {
            Target::ModId(id) => by_id.get(id).copied(),
            Target::Class(class) => owners.get(&class_path(class)).copied(),
            Target::Config(config) => owners.get(config).copied(),
        };
        if let Some(index) = index {
            add(index, clue.evidence);
        }
    }

    let mut ranked: Vec<_> = scores.into_iter().collect();
    ranked.sort_by(|(a, (ea, sa)), (b, (eb, sb))| {
        ea.cmp(eb)
            .then(sb.cmp(sa))
            .then_with(|| mods[*a].display_name().cmp(&mods[*b].display_name()))
    });
    ranked
        .into_iter()
        .take(MAX_SUSPECTS)
        .map(|(index, (evidence, _))| Suspect {
            file_name: mods[index].file_name.clone(),
            name: mods[index].display_name(),
            evidence,
        })
        .collect()
}

/// Something in the crash that points at a mod.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Clue {
    target: Target,
    evidence: Evidence,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Target {
    /// A mod id, lowercase.
    ModId(String),
    /// A class, such as `com.example.mod.Thing`.
    Class(String),
    /// A mixin config file, such as `sodium.mixins.json`.
    Config(String),
}

#[derive(Debug, Default)]
struct Clues {
    list: Vec<Clue>,
}

impl Clues {
    fn is_empty(&self) -> bool {
        self.list.is_empty()
    }

    fn push(&mut self, target: Target, evidence: Evidence) {
        self.list.push(Clue { target, evidence });
    }

    /// The `.class` paths to look for in the jars.
    fn classes(&self) -> HashSet<String> {
        self.list
            .iter()
            .filter_map(|c| match &c.target {
                Target::Class(class) => Some(class_path(class)),
                _ => None,
            })
            .collect()
    }

    fn configs(&self) -> HashSet<String> {
        self.list
            .iter()
            .filter_map(|c| match &c.target {
                Target::Config(config) => Some(config.clone()),
                _ => None,
            })
            .collect()
    }
}

/// `com.example.Thing` → `com/example/Thing.class`.
fn class_path(class: &str) -> String {
    format!("{}.class", class.replace('.', "/"))
}

/// Everything in the crash report and the log that points at a mod.
fn clues(crash_report: Option<&str>, log: &[LogLine]) -> Clues {
    let mut clues = Clues::default();

    let mut log_text = String::new();
    for line in log.iter().rev().take(300).rev() {
        log_text.push_str(&line.message);
        log_text.push('\n');
        if let Some(throwable) = &line.throwable {
            log_text.push_str(throwable);
            log_text.push('\n');
        }
    }

    for text in crash_report.into_iter().chain([log_text.as_str()]) {
        failed_patches(text, &mut clues);
    }

    // The stack trace: the report's, else the last one in the log.
    let trace = match crash_report {
        Some(report) => first_trace(report),
        None => last_trace(&log_text),
    };
    trace_clues(&trace, &mut clues);

    if let Some(report) = crash_report {
        head_mixins(report, &mut clues);
    }
    clues
}

/// Words that mark a line as a mixin failure.
const PATCH_FAILURES: &[&str] = &[
    "Mixin apply for mod",
    "Mixin apply failed",
    "FAILED during APPLY",
    "FAILED during PREPARE",
    "InvalidInjectionException",
    "InvalidMixinException",
    "MixinApplyError",
    "MixinTransformerError",
    "Critical injection failure",
    "could not find any targets matching",
];

/// Mods named in mixin failures: `Mixin apply for mod sodium failed`, `... from mod
/// sodium`, or the mixin config, `[sodium.mixins.json:core.SomeMixin ...]`.
fn failed_patches(text: &str, clues: &mut Clues) {
    for line in text.lines() {
        if !PATCH_FAILURES.iter().any(|sign| line.contains(sign)) {
            continue;
        }
        for marker in ["for mod ", "from mod "] {
            for (index, _) in line.match_indices(marker) {
                let id = mod_id_at(&line[index + marker.len()..]);
                if !id.is_empty() && !is_platform_mod(&id) {
                    clues.push(Target::ModId(id), Evidence::FailedToPatch);
                }
            }
        }
        for word in line.split(|c: char| c.is_whitespace() || "[](){}:,'\"".contains(c)) {
            if word.ends_with(".json") && word.contains("mixin") && !is_platform_config(word) {
                clues.push(Target::Config(word.to_string()), Evidence::FailedToPatch);
            }
        }
    }
}

/// The mod id at the start of `text`, lowercase.
fn mod_id_at(text: &str) -> String {
    text.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// One `at ...` line of a stack trace.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Frame {
    class: String,
    method: String,
}

/// A stack trace, split at its `Caused by:` lines; the root cause is last.
type Trace = Vec<Vec<Frame>>;

/// Parses `at knot//MC//net.minecraft.Foo.bar(Foo.java:12) ~[x.jar:?]`: the class and
/// method, without the class loader (`knot//`, `app//`) or module (`java.base/`).
fn frame(line: &str) -> Option<Frame> {
    let rest = line.trim_start().strip_prefix("at ")?;
    let call = rest.split('(').next()?.trim();
    let call = call.rsplit('/').next()?;
    let (class, method) = call.rsplit_once('.')?;
    if class.is_empty() || method.is_empty() || class.contains(' ') {
        return None;
    }
    Some(Frame {
        class: class.to_string(),
        method: method.to_string(),
    })
}

/// Reads frames from `lines` until the trace ends, starting new causes at `Caused by:`.
fn read_trace<'a>(lines: impl Iterator<Item = &'a str>) -> Trace {
    let mut trace: Trace = vec![Vec::new()];
    let mut started = false;
    for line in lines {
        let trimmed = line.trim();
        if let Some(frame) = frame(trimmed) {
            started = true;
            trace.last_mut().unwrap().push(frame);
        } else if trimmed.starts_with("Caused by:") {
            trace.push(Vec::new());
        } else if trimmed.starts_with("...") && trimmed.ends_with("more") {
            continue;
        } else if started && (trimmed.is_empty() || trimmed.starts_with("A detailed walkthrough")) {
            break;
        }
    }
    trace.retain(|cause| !cause.is_empty());
    trace
}

/// The crash report's own trace, from the top of the report.
fn first_trace(report: &str) -> Trace {
    let start = report
        .lines()
        .position(|l| l.starts_with("Description:"))
        .unwrap_or(0);
    read_trace(report.lines().skip(start))
}

/// The last trace in the log.
fn last_trace(log: &str) -> Trace {
    let lines: Vec<&str> = log.lines().collect();
    let Some(last_frame) = lines.iter().rposition(|l| frame(l).is_some()) else {
        return Vec::new();
    };
    // Back to the exception line before the first frame of this trace, past any
    // `Caused by:` sections.
    let mut start = last_frame;
    while start > 0 {
        let line = lines[start - 1].trim();
        if frame(line).is_some()
            || line.starts_with("Caused by:")
            || (line.starts_with("...") && line.ends_with("more"))
        {
            start -= 1;
        } else {
            break;
        }
    }
    read_trace(lines[start..].iter().copied())
}

/// Clues from a stack trace. The root cause's first frame outside Java and common
/// libraries is where the crash happened: in a mod's class, or in a method a mod
/// injected. Mods further down were running, which is weaker evidence.
fn trace_clues(trace: &Trace, clues: &mut Clues) {
    let Some(root) = trace.last() else {
        return;
    };
    let mut seen_game_frame = false;
    for (position, frame) in root.iter().enumerate() {
        if is_library_class(&frame.class) {
            continue;
        }
        let at_crash = !seen_game_frame;
        seen_game_frame = true;
        if let Some(id) = injected_by(&frame.method) {
            let evidence = if at_crash {
                Evidence::CrashedInItsCode
            } else if position < 12 {
                Evidence::PatchedCrashingCode
            } else {
                Evidence::InStack
            };
            clues.push(Target::ModId(id), evidence);
        }
        if !is_platform_class(&frame.class) {
            let evidence = if at_crash {
                Evidence::CrashedInItsCode
            } else {
                Evidence::InStack
            };
            clues.push(Target::Class(frame.class.clone()), evidence);
        }
    }
    // The outer exceptions' frames: the code that caught the crash and rethrew it.
    for cause in &trace[..trace.len() - 1] {
        for frame in cause {
            if !is_library_class(&frame.class) && !is_platform_class(&frame.class) {
                clues.push(Target::Class(frame.class.clone()), Evidence::InStack);
            }
        }
    }
}

/// The mod id in a method Mixin merged into a game class:
/// `redirect$bakh000$zenith$enchTooltipRewrite` → `zenith`. Mixin without a mod id
/// in its names (`handler$zza000$onRender`) gives nothing.
fn injected_by(method: &str) -> Option<String> {
    let parts: Vec<&str> = method.split('$').collect();
    if parts.len() < 4 {
        return None;
    }
    let tag = parts[1];
    let looks_like_tag = tag.len() >= 4
        && tag.chars().all(|c| c.is_ascii_alphanumeric())
        && tag.chars().rev().take(3).all(|c| c.is_ascii_digit());
    let id = mod_id_at(parts[2]);
    (looks_like_tag && id.len() == parts[2].len() && !id.is_empty() && !is_platform_mod(&id))
        .then_some(id)
}

/// The first "Mixins in Stacktrace" block: the mixins on each class in the crashing
/// stack, the class that crashed first. A mixin on that class is a lead, unless the
/// mod patched most of the stack (which says nothing about this crash).
fn head_mixins(report: &str, clues: &mut Clues) {
    let mut lines = report.lines();
    let Some(first) = lines.find(|l| l.trim_start().starts_with("Mixins in Stacktrace:")) else {
        return;
    };
    if !first.trim().trim_end_matches(':').ends_with("Stacktrace") {
        // "Mixins in Stacktrace: None found"
        return;
    }
    // The mixins, by the class they patch.
    let mut classes: Vec<Vec<(String, Option<String>)>> = Vec::new();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("-- ") {
            break;
        }
        let depth = line.len() - line.trim_start().len();
        if trimmed.ends_with(':') && depth <= 1 {
            classes.push(Vec::new());
            continue;
        }
        let Some(current) = classes.last_mut() else {
            continue;
        };
        let (class, config) = match trimmed.split_once(" (") {
            Some((class, config)) => (class, Some(config.trim_end_matches(')').to_string())),
            None => (trimmed, None),
        };
        if is_platform_class(class) || config.as_deref().is_some_and(is_platform_config) {
            continue;
        }
        current.push((class.to_string(), config));
    }
    if classes.is_empty() {
        return;
    }

    // How many of the classes each mixin config patched.
    let mut breadth: HashMap<String, HashSet<usize>> = HashMap::new();
    for (index, mixins) in classes.iter().enumerate() {
        for (class, config) in mixins {
            let key = config.clone().unwrap_or_else(|| package_root(class));
            breadth.entry(key).or_default().insert(index);
        }
    }
    let total = classes.len();
    for (index, mixins) in classes.iter().enumerate() {
        for (class, config) in mixins {
            let key = config.clone().unwrap_or_else(|| package_root(class));
            let spread = breadth.get(&key).map_or(0, HashSet::len);
            let broad = total >= 3 && spread * 2 > total;
            let evidence = if index == 0 && !broad {
                Evidence::PatchedCrashingCode
            } else {
                Evidence::InStack
            };
            clues.push(Target::Class(class.clone()), evidence);
            if let Some(config) = config {
                clues.push(Target::Config(config.clone()), evidence);
            }
        }
    }
}

/// `me.jellysquid.mods.sodium.mixin.Foo` → `me.jellysquid.mods`, to group mixins
/// without a config by mod.
fn package_root(class: &str) -> String {
    class.split('.').take(3).collect::<Vec<_>>().join(".")
}

/// Java and common libraries: never the mod that crashed, even when a mod bundles them.
fn is_library_class(class: &str) -> bool {
    const LIBRARIES: &[&str] = &[
        "java.",
        "javax.",
        "jdk.",
        "sun.",
        "com.sun.",
        "kotlin.",
        "kotlinx.",
        "scala.",
        "com.google.",
        "it.unimi.dsi.",
        "org.apache.",
        "io.netty.",
        "org.slf4j.",
        "org.objectweb.asm.",
        "org.lwjgl.",
        "org.joml.",
        "com.electronwill.",
        "org.jetbrains.",
    ];
    LIBRARIES.iter().any(|prefix| class.starts_with(prefix))
}

/// Minecraft, the mod loaders and Mixin: the game's own code, which mods change but
/// which is never itself the mod to turn off.
fn is_platform_class(class: &str) -> bool {
    const PLATFORM: &[&str] = &[
        "net.minecraft.",
        "com.mojang.",
        "net.fabricmc.",
        "org.quiltmc.",
        "net.minecraftforge.",
        "net.neoforged.",
        "cpw.mods.",
        "org.spongepowered.",
        "com.llamalad7.mixinextras.",
        "MC.",
        // Launchers' own entry points.
        "org.prismlauncher.",
        "org.multimc.",
    ];
    PLATFORM.iter().any(|prefix| class.starts_with(prefix)) || !class.contains('.')
}

/// Mod ids of the game, the loaders and their APIs.
fn is_platform_mod(id: &str) -> bool {
    matches!(
        id,
        "minecraft"
            | "java"
            | "fabricloader"
            | "fabric"
            | "fabric-api"
            | "quilt_loader"
            | "forge"
            | "neoforge"
            | "mixinextras"
    ) || id.starts_with("fabric-")
        || id.starts_with("quilted_fabric")
}

fn is_platform_config(config: &str) -> bool {
    config.starts_with("fabric-")
        || config.starts_with("fabricloader")
        || config.starts_with("mixinextras")
        || config.starts_with("forge.")
        || config.starts_with("neoforge.")
}

/// Which jar has each of the `classes` (`a/b/C.class`) and mixin `configs`, by their
/// index in `jars`. Looks in the jars' own files first, then in the jars they bundle
/// (`META-INF/jars/`, `META-INF/jarjar/`) for what is still missing.
fn owners(
    jars: &[std::path::PathBuf],
    classes: &HashSet<String>,
    configs: &HashSet<String>,
) -> HashMap<String, usize> {
    // A big modpack has hundreds of jars, so they are read on several threads; each
    // returns what it found, and where two jars have the same file the first wins.
    let results = in_parallel(jars.len(), |index| {
        let mut found = Vec::new();
        let mut nested = Vec::new();
        if let Some(archive) = open(&jars[index]) {
            for name in archive.file_names() {
                if let Some(key) = wanted(name, classes, configs) {
                    found.push(key);
                } else if is_nested_jar(name) {
                    nested.push(name.to_string());
                }
            }
        }
        (found, nested)
    });
    let mut found: HashMap<String, usize> = HashMap::new();
    for (index, (keys, _)) in results.iter().enumerate() {
        for key in keys {
            found.entry(key.clone()).or_insert(index);
        }
    }
    let missing: HashSet<String> = classes
        .iter()
        .chain(configs)
        .filter(|k| !found.contains_key(*k))
        .cloned()
        .collect();
    if missing.is_empty() {
        return found;
    }

    // What's still missing may be in a jar a mod bundles. Reading those means
    // unpacking each one, so only for what's missing.
    let (classes, configs): (HashSet<_>, HashSet<_>) =
        missing.into_iter().partition(|k| k.ends_with(".class"));
    let in_nested = in_parallel(jars.len(), |index| {
        let inner = &results[index].1;
        let mut found = Vec::new();
        if inner.is_empty() {
            return found;
        }
        let Some(mut archive) = open(&jars[index]) else {
            return found;
        };
        for name in inner {
            let Some(bytes) = read_zip_entry(&mut archive, name, MAX_NESTED) else {
                continue;
            };
            let Ok(nested) = zip::ZipArchive::new(Cursor::new(bytes)) else {
                continue;
            };
            found.extend(
                nested
                    .file_names()
                    .filter_map(|name| wanted(name, &classes, &configs)),
            );
        }
        found
    });
    for (index, keys) in in_nested.into_iter().enumerate() {
        for key in keys {
            found.entry(key).or_insert(index);
        }
    }
    found
}

/// The biggest bundled jar worth unpacking.
const MAX_NESTED: u64 = 64 * 1024 * 1024;

fn open(path: &std::path::Path) -> Option<zip::ZipArchive<BufReader<File>>> {
    // Buffered: reading a jar's index makes a few small reads per entry.
    zip::ZipArchive::new(BufReader::new(File::open(path).ok()?)).ok()
}

/// The key `name` (a path in a jar) is found under, if it's one of the `classes` or a
/// mixin config in `configs` (wherever it is in the jar).
fn wanted(name: &str, classes: &HashSet<String>, configs: &HashSet<String>) -> Option<String> {
    if classes.contains(name) {
        return Some(name.to_string());
    }
    let file_name = name.rsplit('/').next().unwrap_or(name);
    configs.contains(file_name).then(|| file_name.to_string())
}

/// `f(0..count)` on a few threads, the results in order (the default for any a
/// panicking thread lost).
fn in_parallel<T: Send + Default>(count: usize, f: impl Fn(usize) -> T + Sync) -> Vec<T> {
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 8)
        .min(count.max(1));
    let next = std::sync::atomic::AtomicUsize::new(0);
    let done: Vec<(usize, T)> = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if index >= count {
                            break done;
                        }
                        done.push((index, f(index)));
                    }
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap_or_default())
            .collect()
    });
    let mut results: Vec<T> = (0..count).map(|_| T::default()).collect();
    for (index, result) in done {
        results[index] = result;
    }
    results
}

fn is_nested_jar(name: &str) -> bool {
    (name.starts_with("META-INF/jars/") || name.starts_with("META-INF/jarjar/"))
        && name.ends_with(".jar")
}

fn read_zip_entry<R: Read + Seek>(
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

#[cfg(test)]
mod tests;
