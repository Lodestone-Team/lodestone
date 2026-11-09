//! Automatic setup: a new instance's memory, Java arguments and game options, chosen for
//! this computer and the instance's mods, so a first modpack doesn't start with too
//! little memory or a render distance the computer can't draw.
//!
//! It runs at launch rather than when the instance is made, so mods added before the
//! first game count too, and in two steps:
//!
//! - [`configure_launch`], before the launch command is built: the memory (from the mods
//!   folder and the computer's memory) and, for a modded instance, garbage-collector
//!   tuning in the JVM arguments. Only for an instance that has never been played and
//!   whose settings follow the global defaults, with no global memory or arguments set;
//!   afterwards, a setting that still has the value it chose follows the mods (more mods,
//!   more memory), and one the user changed is theirs.
//! - [`configure_options`], before the game starts, once its client jar is there: a new
//!   `options.txt` with video settings for the computer, and the controls (mouse
//!   sensitivity, key bindings) and language from the main instance, the last instance
//!   played or the official launcher, when shared game settings don't cover them. Only
//!   when the instance has no `options.txt` (a modpack's own is kept), and only once.
//!
//! Settings > Instance defaults > "Set up new instances automatically" turns it off.

pub mod hardware;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

pub use hardware::{Gpu, GpuKind, Hardware, Tier};

use crate::{
    content::options::{self, OptionsFile},
    instances::{Instance, Loader, Memory},
    paths::DataDir,
    settings::InstanceDefaults,
    shared_options::{self, Category, InstanceSharing, Profile},
};

/// What's in an instance's `mods` folder, as far as memory and drawing go.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ModLoad {
    /// Enabled mod jars.
    pub count: u32,
    /// Their total size, bytes.
    pub bytes: u64,
    /// A renderer that draws far faster than the game's own (Sodium and its ports,
    /// OptiFine).
    pub renderer: bool,
    /// Distant Horizons, which draws far terrain itself and needs memory for it.
    pub distant_horizons: bool,
    /// A shader loader (Iris, Oculus, OptiFine).
    pub shaders: bool,
}

/// Mods known by their file names, which start with the mod's name.
const RENDERERS: &[&str] = &[
    "sodium",
    "embeddium",
    "rubidium",
    "celeritas",
    "nvidium",
    "optifine",
    "vulkanmod",
];
const SHADER_LOADERS: &[&str] = &["iris", "oculus", "optifine"];
const DISTANT_HORIZONS: &[&str] = &["distanthorizons"];

/// Reads the `mods` folder of `game_dir`: the enabled jars, by name and size.
pub fn scan_mods(game_dir: &Path) -> ModLoad {
    let mut load = ModLoad::default();
    let Ok(entries) = std::fs::read_dir(game_dir.join("mods")) else {
        return load;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.ends_with(".jar") {
            continue;
        }
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if !meta.is_file() {
            continue;
        }
        load.count += 1;
        load.bytes += meta.len();
        let plain: String = name.chars().filter(char::is_ascii_alphanumeric).collect();
        let is = |names: &[&str]| names.iter().any(|n| plain.starts_with(n));
        load.renderer |= is(RENDERERS);
        load.shaders |= is(SHADER_LOADERS);
        load.distant_horizons |= is(DISTANT_HORIZONS);
    }
    load
}

/// The memory recommended for an instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryPlan {
    pub memory: Memory,
    /// What the mods want, MiB; more than `memory.max` when the computer can't spare it.
    pub wanted: u32,
}

impl MemoryPlan {
    /// Whether the computer's memory held the recommendation back.
    pub fn limited(&self) -> bool {
        self.wanted > self.memory.max
    }
}

/// The most memory recommended: past this, longer garbage collections cost more than
/// the room gains.
pub const MAX_RECOMMENDED_MB: u32 = 12 * 1024;

fn round_up(mb: u32, step: u32) -> u32 {
    mb.div_ceil(step) * step
}

/// The memory for an instance with `mods` (`modded` when it has a loader) of a `modern`
/// version (1.18 and later, whose worlds are taller) on `hardware`, at least `pack` MiB
/// when its modpack recommends that much.
///
/// Mods are measured by count and size: each loaded mod costs memory for its classes
/// and registries, and big jars carry big assets. What the pack's author recommends
/// wins over the estimate when it is more, past [`MAX_RECOMMENDED_MB`] too, since they
/// know their pack. The computer keeps at least 2 GiB, or a quarter of its memory, for
/// the system and other apps.
pub fn recommend_memory(
    hardware: &Hardware,
    mods: &ModLoad,
    modded: bool,
    modern: bool,
    pack: Option<u32>,
) -> MemoryPlan {
    let tier = Tier::of(hardware);
    let wanted = if !modded || mods.count == 0 {
        if modern && tier >= Tier::High {
            3072
        } else {
            2048
        }
    } else {
        let jar_mb = (mods.bytes / (1024 * 1024)).min(u64::from(u32::MAX / 8)) as u32;
        let mut mb = 2048 + 10 * mods.count + 3 * jar_mb;
        if mods.distant_horizons {
            mb += 1024;
        }
        if mods.shaders {
            mb += 512;
        }
        mb.max(3072)
    };
    let wanted = round_up(wanted, 512).min(MAX_RECOMMENDED_MB);
    let wanted = wanted.max(pack.map_or(0, |mb| round_up(mb, 512)));
    let cap = match hardware.total_mb() {
        Some(total) => {
            let reserve = (total / 4).max(2048);
            (total.saturating_sub(reserve) / 512 * 512).max(1024)
        }
        // Without the computer's memory, what the launcher gives by default.
        None => 4096,
    };
    let max = wanted.min(cap);
    // Half the maximum to start with: the heap grows as the game needs it, without
    // taking all of it from other apps at once.
    let min = (max / 2 / 512 * 512).max(512);
    MemoryPlan {
        memory: Memory { min, max },
        wanted,
    }
}

/// JVM arguments for a modded instance with `max_mb` of memory: the G1 garbage
/// collector tuned for short pauses with a large young generation (what most modpacks
/// recommend), which Java 8, where it isn't the default, needs most. Every flag exists
/// from Java 8 to the current releases.
pub fn recommend_jvm_args(max_mb: u32) -> String {
    let large = max_mb >= 12 * 1024;
    let mut args = vec![
        "-XX:+UseG1GC",
        "-XX:+ParallelRefProcEnabled",
        "-XX:MaxGCPauseMillis=200",
        "-XX:+UnlockExperimentalVMOptions",
        "-XX:+DisableExplicitGC",
    ];
    if large {
        args.extend([
            "-XX:G1NewSizePercent=40",
            "-XX:G1MaxNewSizePercent=50",
            "-XX:G1HeapRegionSize=16M",
            "-XX:G1ReservePercent=15",
            "-XX:InitiatingHeapOccupancyPercent=20",
        ]);
    } else {
        args.extend(["-XX:G1NewSizePercent=30", "-XX:G1MaxNewSizePercent=40"]);
        if max_mb >= 4096 {
            args.push("-XX:G1HeapRegionSize=8M");
        }
        args.extend([
            "-XX:G1ReservePercent=20",
            "-XX:InitiatingHeapOccupancyPercent=15",
        ]);
    }
    args.extend([
        "-XX:G1HeapWastePercent=5",
        "-XX:G1MixedGCCountTarget=4",
        "-XX:G1MixedGCLiveThresholdPercent=90",
        "-XX:SurvivorRatio=32",
        "-XX:MaxTenuringThreshold=1",
        "-XX:+PerfDisableSharedMem",
    ]);
    args.join(" ")
}

/// The video settings for `tier`, in the shared settings' canonical format. A modded
/// instance without a faster renderer draws a little closer; Distant Horizons draws the
/// far terrain itself, so the game's own distance can stay moderate.
pub fn recommend_video(tier: Tier, mods: &ModLoad, modded: bool) -> BTreeMap<String, String> {
    let (mut render, mut simulation, graphics, particles, biome, mipmaps, entities, shadows) =
        match tier {
            Tier::Low => (8, 6, "fast", 1, 1, 2, "0.75", false),
            Tier::Medium => (12, 8, "fancy", 0, 2, 4, "1.0", true),
            Tier::High => (16, 10, "fancy", 0, 2, 4, "1.0", true),
            Tier::Ultra => (20, 12, "fancy", 0, 3, 4, "1.25", true),
        };
    if modded && mods.count > 0 {
        if mods.renderer {
            if tier >= Tier::Medium {
                render += 4;
            }
        } else if mods.count > 50 {
            render -= 2;
            simulation -= 2;
        }
        if mods.distant_horizons {
            render = render.min(12);
        }
        // A big pack ticks many more blocks and entities.
        if mods.count > 150 {
            simulation = (simulation - 2).max(5);
        }
    }
    let render = render.clamp(6, 32);
    let simulation = simulation.clamp(5, render);
    [
        ("renderDistance", render.to_string()),
        ("simulationDistance", simulation.to_string()),
        ("graphicsPreset", graphics.to_string()),
        ("particles", particles.to_string()),
        ("biomeBlendRadius", biome.to_string()),
        ("mipmapLevels", mipmaps.to_string()),
        ("entityDistanceScaling", entities.to_string()),
        ("entityShadows", shadows.to_string()),
    ]
    .into_iter()
    .map(|(key, value)| (key.to_string(), value))
    .collect()
}

/// Whether `game_version` is 1.18 or later (or a snapshot, or the year-numbered
/// releases), whose worlds are taller and need more memory.
pub fn is_modern(game_version: &str) -> bool {
    if let Some(rest) = game_version.strip_prefix("1.") {
        let minor: String = rest.chars().take_while(char::is_ascii_digit).collect();
        return minor.parse::<u32>().is_ok_and(|m| m >= 18);
    }
    // a1.2.6, b1.7.3, rd-132211, inf-…: the oldest versions.
    !(game_version.starts_with(['a', 'b', 'c'])
        || game_version.starts_with("rd-")
        || game_version.starts_with("inf-"))
}

/// What [`configure_launch`] changed or found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LaunchOutcome {
    /// The memory it gave the instance, when it changed.
    pub memory: Option<Memory>,
    /// It set the JVM arguments.
    pub jvm_args: bool,
    /// The mods want more than the computer can spare: (given, wanted), MiB.
    pub limited: Option<(u32, u32)>,
    /// The memory the user chose is well below what the mods want: (have, recommended).
    pub low_memory: Option<(u32, u32)>,
    /// The instance changed and should be saved.
    pub changed: bool,
}

/// Chooses the memory and JVM arguments of `instance` before a launch (see the module's
/// documentation). `game_dir` is its game folder.
pub fn configure_launch(
    instance: &mut Instance,
    defaults: &InstanceDefaults,
    hardware: &Hardware,
    game_dir: &Path,
) -> LaunchOutcome {
    let mut outcome = LaunchOutcome::default();
    let first = instance.auto_config.is_none() && instance.last_played.is_none();
    if !first && instance.auto_config.is_none() {
        return outcome;
    }
    let modded = instance.loader != Loader::None;
    let mods = if modded {
        scan_mods(game_dir)
    } else {
        ModLoad::default()
    };
    let plan = recommend_memory(
        hardware,
        &mods,
        modded,
        is_modern(&instance.game_version),
        instance.recommended_memory,
    );
    let mut state = instance.auto_config.clone().unwrap_or_default();

    let follows_memory = instance.memory.is_some() && instance.memory == state.memory;
    let free_memory = first
        && instance.memory.is_none()
        && defaults.min_ram.is_none()
        && defaults.max_ram.is_none();
    if follows_memory || free_memory {
        if instance.memory != Some(plan.memory) {
            instance.memory = Some(plan.memory);
            outcome.memory = Some(plan.memory);
        }
        state.memory = Some(plan.memory);
        if plan.limited() {
            outcome.limited = Some((plan.memory.max, plan.wanted));
        }
    } else if modded {
        let (_, have) = instance.memory(defaults, hardware.total_memory);
        if have + 1024 <= plan.memory.max {
            outcome.low_memory = Some((have, plan.memory.max));
        }
    }

    if modded {
        let follows_args = instance.jvm_args.is_some() && instance.jvm_args == state.jvm_args;
        let free_args = first && instance.jvm_args.is_none() && defaults.jvm_args.trim().is_empty();
        if follows_args || free_args {
            let (_, max) = instance.memory(defaults, hardware.total_memory);
            let args = recommend_jvm_args(max);
            if instance.jvm_args.as_deref() != Some(args.as_str()) {
                instance.jvm_args = Some(args.clone());
                outcome.jvm_args = true;
            }
            state.jvm_args = Some(args);
        }
    }

    if instance.auto_config.as_ref() != Some(&state) {
        instance.auto_config = Some(state);
        outcome.changed = true;
    }
    outcome.changed |= outcome.memory.is_some() || outcome.jvm_args;
    outcome
}

/// Where controls and language were copied from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ControlsSource {
    /// Another instance, by name.
    Instance(String),
    /// The official Minecraft Launcher's folder.
    MinecraftLauncher,
}

/// A place to copy controls from: an `options.txt` and the game version it is for.
#[derive(Clone, Debug)]
pub struct Candidate {
    pub source: ControlsSource,
    /// The game folder with `options.txt`.
    pub game_dir: PathBuf,
    /// The game version, for a file without a `version` line; empty when unknown.
    pub game_version: String,
}

/// What [`configure_options`] wrote.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OptionsOutcome {
    /// The tier the video settings are for, when it wrote any.
    pub video: Option<Tier>,
    /// The render distance it chose.
    pub render_distance: Option<u32>,
    /// Where the controls and language came from, when it copied any.
    pub controls_from: Option<ControlsSource>,
    /// Lines written.
    pub written: usize,
    /// The instance changed and should be saved.
    pub changed: bool,
}

/// The categories copied from another game folder when sharing doesn't cover them.
const COPIED: [Category; 2] = [Category::Controls, Category::Language];

/// Sets up a new instance's `options.txt` before its first game (see the module's
/// documentation). `data` is the data directory (for the shared settings), `instance_dir`
/// the instance's folder, `jar` its client jar (for the format),
/// `candidates` the places to copy controls from, best first.
pub fn configure_options(
    data: &DataDir,
    instance: &mut Instance,
    instance_dir: &Path,
    jar: Option<&Path>,
    hardware: &Hardware,
    candidates: &[Candidate],
) -> Result<OptionsOutcome, shared_options::SharedError> {
    let mut outcome = OptionsOutcome::default();
    let Some(state) = instance.auto_config.as_mut() else {
        return Ok(outcome);
    };
    if state.options {
        return Ok(outcome);
    }
    state.options = true;
    outcome.changed = true;
    let game_dir = instance_dir.join("minecraft");
    let path = game_dir.join(options::FILE);
    // A modpack's own options, or a game played before: theirs.
    if path.exists() {
        return Ok(outcome);
    }
    let mut file = OptionsFile::parse("");
    let Some(target) = shared_options::target(&file, &instance.game_version, jar) else {
        return Ok(outcome);
    };
    // What shared game settings will write anyway is left to them.
    let profile = shared_options::load_profile(data).unwrap_or_default();
    let sharing = shared_options::load_instance(instance_dir).unwrap_or_default();
    let shared = |key: &str| shared_options::will_share(&profile, &sharing, key);

    let modded = instance.loader != Loader::None;
    let mods = if modded {
        scan_mods(&game_dir)
    } else {
        ModLoad::default()
    };
    let tier = Tier::of(hardware);
    let mut values: BTreeMap<String, String> = recommend_video(tier, &mods, modded)
        .into_iter()
        .filter(|(key, _)| !shared(key))
        .collect();
    let video_keys: Vec<String> = values.keys().cloned().collect();

    let copy_controls = !covered(&profile, &sharing, &COPIED);
    if copy_controls {
        for candidate in candidates {
            let copied: BTreeMap<String, String> = read_copyable(candidate)
                .into_iter()
                .filter(|(key, _)| !shared(key))
                .collect();
            if !copied.is_empty() {
                values.extend(copied);
                outcome.controls_from = Some(candidate.source.clone());
                break;
            }
        }
    }

    let written = shared_options::write_values(&mut file, target.dv, &values);
    if written.is_empty() {
        outcome.controls_from = None;
        return Ok(outcome);
    }
    if written.iter().any(|key| video_keys.contains(key)) {
        outcome.video = Some(tier);
        outcome.render_distance = values
            .get("renderDistance")
            .filter(|_| written.iter().any(|k| k == "renderDistance"))
            .and_then(|v| v.parse().ok());
    }
    if outcome.controls_from.is_some() && written.iter().all(|key| video_keys.contains(key)) {
        outcome.controls_from = None;
    }
    if let Some(version) = target.add_version_line {
        file.set_first("version", &version.to_string());
    }
    std::fs::create_dir_all(&game_dir).map_err(|source| crate::storage::StorageError::Io {
        path: game_dir.clone(),
        source,
    })?;
    file.write(&path)?;
    outcome.written = written.len();
    Ok(outcome)
}

/// Whether shared game settings cover `categories` for this instance: it uses sharing,
/// and the global set has a value in each of them that it shares.
fn covered(profile: &Profile, sharing: &InstanceSharing, categories: &[Category]) -> bool {
    categories.iter().all(|category| {
        profile.values.keys().any(|key| {
            shared_options::category_of(key) == Some(*category)
                && shared_options::will_share(profile, sharing, key)
        })
    })
}

/// The controls and language in a candidate's `options.txt`, canonical.
fn read_copyable(candidate: &Candidate) -> BTreeMap<String, String> {
    let Ok(file) = OptionsFile::read(&candidate.game_dir.join(options::FILE)) else {
        return BTreeMap::new();
    };
    if file.is_empty() {
        return BTreeMap::new();
    }
    let Some(target) = shared_options::target(&file, &candidate.game_version, None) else {
        return BTreeMap::new();
    };
    shared_options::instance_values(&file, target.dv)
        .into_iter()
        .filter(|(key, _)| {
            shared_options::category_of(key).is_some_and(|c| COPIED.contains(&c))
                // Mods' bindings only mean something with the mod.
                && shared_options::keys::find(key).is_some()
        })
        .collect()
}

/// The places to copy controls from, best first: the main instance for shared settings,
/// then the other instances by when they were last played, then the official launcher.
/// `instances` is every instance with its folder; `this` is the one being set up.
pub fn candidates(
    profile_preferred: Option<&str>,
    instances: &[(Instance, PathBuf)],
    this: &str,
) -> Vec<Candidate> {
    let mut played: Vec<&(Instance, PathBuf)> = instances
        .iter()
        .filter(|(i, _)| i.id != this && i.last_played.is_some())
        .collect();
    played.sort_by_key(|(i, _)| std::cmp::Reverse(i.last_played));
    if let Some(main) = profile_preferred
        && let Some(index) = played.iter().position(|(i, _)| i.id == main)
    {
        let main = played.remove(index);
        played.insert(0, main);
    }
    let mut out: Vec<Candidate> = played
        .into_iter()
        .map(|(instance, dir)| Candidate {
            source: ControlsSource::Instance(instance.name.clone()),
            game_dir: dir.join("minecraft"),
            game_version: instance.game_version.clone(),
        })
        .collect();
    if let Some(locations) = crate::import::detect::Locations::current() {
        for (source, dir) in locations.candidates() {
            if source == crate::import::Source::Vanilla && dir.join(options::FILE).is_file() {
                out.push(Candidate {
                    source: ControlsSource::MinecraftLauncher,
                    game_dir: dir,
                    game_version: String::new(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
