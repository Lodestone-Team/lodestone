//! Which sides a content image's files are for, from what the core already knows: never
//! from the network while recording.
//!
//! The inputs, in the spec's order of precedence (`lodestone_image::env::resolve` applies
//! them):
//!
//! 1. `owner_mark`: the "For players" marks (`mods/.lodestone_client_mods.json`, see
//!    `mods::client`): marked for players is client `required`, left out is `unsupported`.
//! 2. `base_pack`: the pack index's `env`, from `pack.json` ([`super::base`]), for files the
//!    pack wrote and that still have the pack's bytes (the recorder decides that).
//! 3. `modrinth_environment` and `modrinth_project`: what Modrinth said about the file's
//!    version, keyed by the file's SHA-512, as Lodestone learned it while installing or
//!    listing mods ([`ModrinthHints`]) and kept with the instance's content state.
//! 4. `jar_environment`: what the jar itself says (`fabric.mod.json` `environment`,
//!    `quilt.mod.json` `minecraft.environment`, a Forge or NeoForge `mods.toml`'s
//!    `clientSideOnly` and `displayTest`: `lodestone_minecraft::jar::mods_toml_environment`),
//!    scanned once per jar contents and kept in memory ([`JarScans`]).
//!
//! An input that isn't known stays unknown. Asking Modrinth here would hold up a record and
//! make image ids depend on the network's timing, so nothing here does.
//!
//! So a file's sides are settled when its image is recorded, and the client view a server
//! serves its players (`super::serve`) is a pure function of the image: nothing is looked up
//! while answering a player. What Modrinth says about a jar later (a Mods-tab listing) takes
//! effect at the next record. A side nothing knows stays `unknown`, and the client view
//! carries everything not `unsupported` on clients: a mod nobody can place goes to players
//! (they can always join), and the owner's "Not for players" mark takes it out.

use std::collections::HashMap;
use std::sync::Mutex as StdMutex;

use lodestone_image::env::{modrinth_environment, JarEnvironment, OwnerMark, SideValues};
use lodestone_image::EnvInputs;
use serde::{Deserialize, Serialize};

use crate::implementations::minecraft::Flavour;
use crate::mods::client::{ClientMarks, MARKS_FILE};
use crate::mods::InstalledModLoader;

use super::layout::InstanceContext;

/// What Modrinth said about one file's version: kept by SHA-512.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModrinthHint {
    /// The file's CDN URL, for the entry's `modrinth` source.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The version's `environment` (`client_and_server`, `server_only`…).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<String>,
    /// The project's `client_side` and `server_side`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_side: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_side: Option<String>,
}

impl ModrinthHint {
    /// The two Modrinth inputs this hint gives.
    pub fn inputs(&self) -> (Option<SideValues>, Option<SideValues>) {
        let environment = self.environment.as_deref().and_then(modrinth_environment);
        let project = (self.client_side.is_some() || self.server_side.is_some())
            .then(|| SideValues::parse(self.client_side.as_deref(), self.server_side.as_deref()));
        (environment, project)
    }
}

/// The most hints kept in memory; past this the oldest half is forgotten.
const MAX_HINTS: usize = 8192;

/// What Lodestone learned from Modrinth about files, by SHA-512, in memory. The Mods tab
/// and the modpack code note what they already fetched; a record reads it.
#[derive(Default)]
pub struct ModrinthHints {
    hints: StdMutex<(u64, HashMap<String, (u64, ModrinthHint)>)>,
}

impl ModrinthHints {
    pub fn note(&self, sha512: &str, hint: ModrinthHint) {
        if !is_sha512(sha512) {
            return;
        }
        let mut guard = self.hints.lock().unwrap_or_else(|e| e.into_inner());
        let (clock, hints) = &mut *guard;
        *clock += 1;
        let now = *clock;
        hints.insert(sha512.to_string(), (now, hint));
        if hints.len() > MAX_HINTS {
            let mut ages: Vec<u64> = hints.values().map(|(at, _)| *at).collect();
            ages.sort_unstable();
            let cut = ages[ages.len() / 2];
            hints.retain(|_, (at, _)| *at > cut);
        }
    }

    pub fn get(&self, sha512: &str) -> Option<ModrinthHint> {
        let guard = self.hints.lock().unwrap_or_else(|e| e.into_inner());
        guard.1.get(sha512).map(|(_, hint)| hint.clone())
    }
}

pub(crate) fn is_sha512(hash: &str) -> bool {
    hash.len() == 128 && hash.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// The mod or plugin loader that reads jars in the server's mods or plugins folder.
pub fn mod_loader(flavour: &Flavour) -> Option<InstalledModLoader> {
    match flavour {
        Flavour::Fabric { .. } => Some(InstalledModLoader::Fabric),
        Flavour::Forge { .. } => Some(InstalledModLoader::Forge),
        Flavour::NeoForge { .. } => Some(InstalledModLoader::NeoForge),
        Flavour::Paper { .. } => Some(InstalledModLoader::Paper),
        Flavour::Spigot => Some(InstalledModLoader::Spigot),
        Flavour::Vanilla => None,
    }
}

/// What the core knows about one jar of the mods or plugins folder.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JarFacts {
    pub sha512: Option<String>,
    pub mark: Option<OwnerMark>,
    pub jar: Option<JarEnvironment>,
}

/// What a scan of a jar found that doesn't change while its bytes don't: its SHA-512 and
/// what its own metadata says about its sides. Kept by the jar's SHA-256 ([`JarScans`]).
#[derive(Debug, Clone, Default, PartialEq)]
struct Scanned {
    sha512: Option<String>,
    jar: Option<JarEnvironment>,
}

/// Jars scanned before, by SHA-256, so a record reads only new or changed jars (a pack of
/// hundreds of mods is otherwise read whole at every Mods-tab action). Bounded: past
/// [`MAX_SCANS`] it starts again.
#[derive(Default)]
pub struct JarScans(StdMutex<HashMap<String, Scanned>>);

/// The most jars [`JarScans`] keeps.
const MAX_SCANS: usize = 16_384;

impl JarScans {
    fn get(&self, sha256: &str) -> Option<Scanned> {
        self.0
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(sha256)
            .cloned()
    }

    fn put(&self, sha256: &str, scanned: Scanned) {
        let mut scans = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if scans.len() >= MAX_SCANS {
            scans.clear();
        }
        scans.insert(sha256.to_string(), scanned);
    }
}

/// The jars of the server's mods or plugins folder among `paths` (canonical paths, which
/// are their disk paths on a server): their SHA-512, owner mark and jar environment, by
/// path. What a jar's scan found is kept in memory by its SHA-256 ([`JarScans`]), so a jar
/// is read once per contents while the core runs; the first record after a start reads
/// every jar (not persisted: a cost of a few seconds for a large pack, once).
pub async fn jar_facts(
    ctx: &InstanceContext,
    files: &[(String, String)],
    scans: &JarScans,
) -> HashMap<String, JarFacts> {
    let mut facts = HashMap::new();
    let Some(loader) = mod_loader(&ctx.flavour) else {
        return facts;
    };
    let folder = match loader.kind() {
        crate::mods::InstalledModKind::Mod => "mods",
        crate::mods::InstalledModKind::Plugin => "plugins",
    };
    let marks = if folder == "mods" {
        read_marks(&ctx.root.join(folder).join(MARKS_FILE)).await
    } else {
        ClientMarks::default()
    };
    for (path, sha256) in files {
        let Some(name) = path
            .strip_prefix(folder)
            .and_then(|rest| rest.strip_prefix('/'))
            .filter(|name| !name.contains('/'))
        else {
            continue;
        };
        if !crate::mods::is_mod_file_name(name) {
            continue;
        }
        let scanned = match scans.get(sha256) {
            Some(scanned) => scanned,
            None => {
                let on_disk = ctx.root.join(folder).join(name);
                let scan = crate::mods::scan::scan_file(&on_disk, loader).await.ok();
                let scanned = Scanned {
                    sha512: scan.as_ref().map(|s| s.sha512.clone()),
                    jar: scan.and_then(|s| s.environment).map(|env| match env {
                        crate::mods::metadata::JarEnvironment::Client => JarEnvironment::Client,
                        crate::mods::metadata::JarEnvironment::Server => JarEnvironment::Server,
                    }),
                };
                // only a scan that worked: a jar being written could read as damaged
                if scanned.sha512.is_some() {
                    scans.put(sha256, scanned.clone());
                }
                scanned
            }
        };
        let marked = marks.of(name);
        let mark = if marked.required {
            Some(OwnerMark::ForPlayers)
        } else if marked.excluded {
            Some(OwnerMark::NotForPlayers)
        } else {
            None
        };
        facts.insert(
            path.clone(),
            JarFacts {
                sha512: scanned.sha512,
                mark,
                jar: scanned.jar,
            },
        );
    }
    facts
}

/// The marks file, or no marks when it is missing or damaged.
async fn read_marks(path: &std::path::Path) -> ClientMarks {
    match tokio::fs::read(path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).unwrap_or_default(),
        Err(_) => ClientMarks::default(),
    }
}

/// A file's inputs from what is known of it.
pub fn inputs(
    facts: Option<&JarFacts>,
    base_pack: Option<lodestone_image::Env>,
    hint: Option<&ModrinthHint>,
) -> EnvInputs {
    let (modrinth_environment, modrinth_project) =
        hint.map(ModrinthHint::inputs).unwrap_or((None, None));
    EnvInputs {
        owner_mark: facts.and_then(|f| f.mark),
        base_pack: base_pack.map(SideValues::from),
        modrinth_environment,
        modrinth_project,
        jar: facts.and_then(|f| f.jar),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lodestone_image::env::resolve;
    use lodestone_image::{Env, EnvFrom, EnvValue};

    #[test]
    fn inputs_follow_the_spec_order() {
        let hint = ModrinthHint {
            url: None,
            environment: Some("server_only".into()),
            client_side: Some("required".into()),
            server_side: Some("required".into()),
        };
        // Modrinth's version environment beats its project sides.
        let (env, from) = resolve(&inputs(None, None, Some(&hint)));
        assert_eq!(env, Env::new(EnvValue::Unsupported, EnvValue::Required));
        assert_eq!(from, vec![EnvFrom::ModrinthEnvironment]);
        // The owner's mark decides the client side; the pack beats Modrinth on the server.
        let facts = JarFacts {
            sha512: None,
            mark: Some(OwnerMark::ForPlayers),
            jar: Some(JarEnvironment::Server),
        };
        let base = Env::new(EnvValue::Optional, EnvValue::Optional);
        let (env, from) = resolve(&inputs(Some(&facts), Some(base), Some(&hint)));
        assert_eq!(env, Env::new(EnvValue::Required, EnvValue::Optional));
        assert_eq!(from, vec![EnvFrom::BasePack, EnvFrom::OwnerMark]);
        // Nothing known is unknown.
        assert_eq!(resolve(&inputs(None, None, None)).0, Env::UNKNOWN);
    }

    #[test]
    fn hints_are_bounded_and_keyed_by_sha512() {
        let hints = ModrinthHints::default();
        hints.note("not a hash", ModrinthHint::default());
        assert_eq!(hints.get("not a hash"), None);
        for n in 0..(MAX_HINTS + 10) {
            hints.note(&format!("{n:0128x}"), ModrinthHint::default());
        }
        assert!(hints.hints.lock().unwrap().1.len() <= MAX_HINTS);
        // The newest are kept.
        assert!(hints.get(&format!("{:0128x}", MAX_HINTS + 9)).is_some());
    }
}
