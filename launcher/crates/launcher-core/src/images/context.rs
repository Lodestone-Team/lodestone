//! What a content image says about an instance besides its files: its game, the pack it is
//! built on, the client settings that travel, and where each file came from. All read from
//! the instance's own files when a record runs; nothing is fetched.
//!
//! - The game is `instance.json`'s Minecraft version, loader (none is `vanilla`) and
//!   loader version.
//! - The base is `modpack.json`'s pack ([`ModpackRecord`]): a Modrinth pack with the
//!   SHA-512 of its `.mrpack` ([`ModpackRecord::pack_sha512`]), a CurseForge pack, or
//!   `loader` (a plain instance) when there is no `modpack.json`. A pack the image can't
//!   name (a local `.mrpack`, a server's pack, a Modrinth pack whose `.mrpack` hash isn't
//!   known) is no base.
//! - The settings are the client's recommended memory (`instance.json`) and resource pack
//!   order (`options.txt`'s `resourcePacks`). Not the shader: Iris and Oculus keep it in
//!   `config/*.properties`, which is content already. Not the icon or background: they are
//!   the instance's look in the Launcher, not what it plays.
//! - Every file is listed as an entry, the pack's too (no `base` sources, like the core).
//!   A file in `mods/`, `resourcepacks/` or `shaderpacks/` that `content.json`
//!   ([`Provenance`]) describes at the same size and modification time gets its hashes,
//!   its source (a Modrinth CDN file, or a CurseForge file) and its sides from Modrinth's
//!   project; any other file is an `object` with unknown sides.
//!
//! Everything here may come from a pack or a server, so it is checked before it is used:
//! hashes must be lowercase hex of the right length, a Modrinth source needs a CDN URL and
//! a SHA-512, and lists are bounded.

use std::path::PathBuf;

use lodestone_image::env::{EnvInputs, SideValues, resolve};
use lodestone_image::hex::{is_sha1, is_sha512};
use lodestone_image::settings::ClientSettings;
use lodestone_image::{Base, Entry, Game, Loader, Settings, Source};

use crate::content::options::PackOptions;
use crate::content::provenance::{self, Provenance};
use crate::content::{ContentKind, enabled_name};
use crate::instances::{self, Instance, InstanceError, InstanceStore};
use crate::modpack::ModpackRecord;
use crate::modpack::addon::AddedPacks;

use super::ImageError;
use super::scope::ContentScope;

/// The most resource packs an image's settings take from `options.txt`: far more than any
/// game lists, and well under what an image may hold.
const MAX_RESOURCE_PACKS: usize = 1000;

/// An instance as a record sees it.
#[derive(Debug, Clone)]
pub struct InstanceContext {
    pub id: String,
    /// The instance's folder (`instance.json`, `minecraft/`).
    pub dir: PathBuf,
    pub instance: Instance,
    pub record: Option<ModpackRecord>,
    pub provenance: Provenance,
    /// `options.txt`'s resource packs, in the game's order.
    pub resource_packs: Vec<String>,
    /// Which of its files are content: the content folders, and the files its pack and its
    /// added packs wrote elsewhere.
    pub scope: ContentScope,
}

impl InstanceContext {
    /// Reads the instance `id`. [`ImageError::Gone`] when there is no such instance, so its
    /// queued labels can be dropped; any other failure keeps them for the next try.
    pub fn load(instances: &InstanceStore, id: &str) -> Result<Self, ImageError> {
        let instance = instances.get(id).map_err(|e| match e {
            InstanceError::NotFound(id) => ImageError::Gone(id),
            other => ImageError::Instance(other.to_string()),
        })?;
        let dir = instances.dir(id);
        let mut resource_packs = PackOptions::read(&dir.join("minecraft"))
            .map(|options| options.enabled)
            .unwrap_or_default();
        resource_packs.truncate(MAX_RESOURCE_PACKS);
        let record = ModpackRecord::load(&dir);
        let added = AddedPacks::load(&dir);
        let scope = scope(record.as_ref(), &added);
        Ok(InstanceContext {
            id: id.to_string(),
            record,
            scope,
            provenance: Provenance::load(&dir),
            resource_packs,
            instance,
            dir,
        })
    }

    /// The game the instance runs.
    pub fn game(&self) -> Game {
        game_of(&self.instance)
    }

    /// The pack the instance is built on (see the module).
    pub fn base(&self) -> Option<Base> {
        let Some(record) = &self.record else {
            return Some(Base::Loader {});
        };
        if let Some(pack) = &record.curseforge {
            return Some(Base::CurseforgePack {
                project: pack.project_id,
                file: pack.file_id,
            });
        }
        let sha512 = record.pack_sha512.as_deref().filter(|h| is_sha512(h))?;
        // a server's instance too: its files are its server's base and what it placed
        let (project, version) = record.modrinth_ids()?;
        Some(Base::ModrinthPack {
            project: project.to_string(),
            version: version.to_string(),
            sha512: sha512.to_string(),
        })
    }

    /// The client settings an image carries (see the module).
    pub fn settings(&self) -> Settings {
        let mut settings = Settings {
            server: None,
            client: Some(ClientSettings {
                recommended_memory_mb: self.instance.recommended_memory,
                resource_packs: self.resource_packs.clone(),
                ..ClientSettings::default()
            }),
        };
        settings.normalize();
        settings
    }

    /// `entries` (a snapshot's, all `object`) with what `content.json` knows of them.
    pub fn describe(&self, entries: Vec<Entry>) -> Vec<Entry> {
        entries
            .into_iter()
            .map(|entry| self.describe_one(entry))
            .collect()
    }

    fn describe_one(&self, mut entry: Entry) -> Entry {
        let Some(known) = self.provenance_of(&entry) else {
            return entry;
        };
        entry.sha1 = Some(known.sha1.to_ascii_lowercase()).filter(|h| is_sha1(h));
        entry.sha512 = Some(known.sha512.to_ascii_lowercase()).filter(|h| is_sha512(h));
        let modrinth = known.modrinth.as_ref().and_then(|source| {
            let found = Source::modrinth_from_url(source.url.as_deref()?)?;
            entry.sha512.is_some().then_some(found)
        });
        entry.source = if let Some(source) = modrinth {
            source
        } else if let Some(cf) = &known.curseforge {
            Source::Curseforge {
                project: cf.project_id,
                file: cf.file_id,
                downloadable: cf.downloadable,
            }
        } else {
            Source::Object {}
        };
        let inputs = EnvInputs {
            modrinth_project: known.modrinth.as_ref().map(|source| {
                SideValues::parse(Some(&source.client_side), Some(&source.server_side))
            }),
            ..EnvInputs::default()
        };
        let (env, from) = resolve(&inputs);
        entry.env = env;
        entry.env_from = from;
        entry
    }

    /// The `content.json` entry of a file directly in `mods/`, `resourcepacks/` or
    /// `shaderpacks/` (under its enabled name), when it still has the file's size and
    /// modification time.
    fn provenance_of(&self, entry: &Entry) -> Option<&provenance::Entry> {
        let mut parts = entry.path.components();
        let (folder, name) = (parts.next()?, parts.next()?);
        if parts.next().is_some() {
            return None;
        }
        let kind = ContentKind::ALL.into_iter().find(|k| k.dir() == folder)?;
        let known = self
            .provenance
            .files
            .get(&format!("{}/{}", kind.dir(), enabled_name(name)))?;
        let modified = entry.modified.and_then(|ns| u64::try_from(ns).ok());
        (known.size == entry.size && known.modified.is_some() && known.modified == modified)
            .then_some(known)
    }
}

/// The game of `instance`, as images write it.
/// The content scope of an instance whose pack record is `record` and whose added packs
/// are `added`: the content folders and every file those packs wrote.
fn scope(record: Option<&ModpackRecord>, added: &AddedPacks) -> ContentScope {
    ContentScope::new(
        record
            .into_iter()
            .chain(added.packs.iter().map(|pack| &pack.record))
            .flat_map(|record| record.files.keys().map(String::as_str)),
    )
}

/// The content scope of the instance at `dir` now: what a content image recorded now
/// holds ([`ContentScope::holds_file`]).
pub fn scope_of(dir: &std::path::Path) -> ContentScope {
    scope(ModpackRecord::load(dir).as_ref(), &AddedPacks::load(dir))
}

pub fn game_of(instance: &Instance) -> Game {
    Game {
        minecraft: instance.game_version.clone(),
        loader: match instance.loader {
            instances::Loader::None => Loader::Vanilla,
            instances::Loader::Fabric => Loader::Fabric,
            instances::Loader::Quilt => Loader::Quilt,
            instances::Loader::Forge => Loader::Forge,
            instances::Loader::NeoForge => Loader::NeoForge,
        },
        loader_version: match instance.loader {
            instances::Loader::None => None,
            _ => instance.loader_version.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::provenance::{CurseForgeSource, ModrinthSource};
    use crate::modpack::CurseForgePack;
    use lodestone_image::{CanonicalPath, EnvFrom, EnvValue};

    fn context(record: Option<ModpackRecord>) -> InstanceContext {
        InstanceContext {
            id: "pack".into(),
            dir: PathBuf::from("/nowhere"),
            instance: Instance {
                id: "pack".into(),
                name: "Pack".into(),
                game_version: "1.21.1".into(),
                loader: instances::Loader::Fabric,
                loader_version: Some("0.16.10".into()),
                recommended_memory: Some(6144),
                ..Instance::default()
            },
            record,
            provenance: Provenance::default(),
            resource_packs: vec!["vanilla".into(), "file/Faithful.zip".into()],
            scope: ContentScope::default(),
        }
    }

    #[test]
    fn game_base_and_settings() {
        let plain = context(None);
        assert_eq!(
            plain.game(),
            Game {
                minecraft: "1.21.1".into(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.10".into()),
            }
        );
        assert_eq!(plain.base(), Some(Base::Loader {}));
        let client = plain.settings().client.unwrap();
        assert_eq!(client.recommended_memory_mb, Some(6144));
        assert_eq!(client.resource_packs, ["vanilla", "file/Faithful.zip"]);
        assert_eq!(client.shader, None);
        assert_eq!(client.icon, None);

        // Vanilla has no loader version, whatever instance.json says.
        let mut vanilla = context(None);
        vanilla.instance.loader = instances::Loader::None;
        assert_eq!(vanilla.game().loader, Loader::Vanilla);
        assert_eq!(vanilla.game().loader_version, None);
        // No settings at all is no client settings.
        vanilla.instance.recommended_memory = None;
        vanilla.resource_packs.clear();
        assert_eq!(vanilla.settings(), Settings::default());

        let sha512 = "ab".repeat(64);
        let modrinth = ModpackRecord {
            project_id: "P".into(),
            version_id: "V".into(),
            pack_sha512: Some(sha512.clone()),
            ..ModpackRecord::default()
        };
        assert_eq!(
            context(Some(modrinth.clone())).base(),
            Some(Base::ModrinthPack {
                project: "P".into(),
                version: "V".into(),
                sha512,
            })
        );
        // Without the .mrpack's hash, or not checkable as one: no base.
        let unknown = ModpackRecord {
            pack_sha512: None,
            ..modrinth.clone()
        };
        assert_eq!(context(Some(unknown)).base(), None);
        let odd = ModpackRecord {
            pack_sha512: Some("AB".repeat(64)),
            ..modrinth
        };
        assert_eq!(context(Some(odd)).base(), None);
        // A local .mrpack or a server's pack.
        assert_eq!(context(Some(ModpackRecord::default())).base(), None);

        let curseforge = ModpackRecord {
            curseforge: Some(CurseForgePack {
                project_id: 3,
                file_id: 4,
                ..CurseForgePack::default()
            }),
            ..ModpackRecord::default()
        };
        assert_eq!(
            context(Some(curseforge)).base(),
            Some(Base::CurseforgePack {
                project: 3,
                file: 4
            })
        );
    }

    fn entry(path: &str, size: u64, modified: i64) -> Entry {
        let mut entry = Entry::new(
            CanonicalPath::new(path).unwrap(),
            size,
            "0".repeat(64),
            Source::Object {},
        );
        entry.modified = Some(modified);
        entry
    }

    fn known(size: u64, modified: u64) -> provenance::Entry {
        provenance::Entry {
            size,
            modified: Some(modified),
            sha1: "1".repeat(40),
            sha512: "5".repeat(128),
            ..provenance::Entry::default()
        }
    }

    #[test]
    fn describe_takes_sources_and_sides_from_content_json() {
        let mut ctx = context(None);
        let url = "https://cdn.modrinth.com/data/AANobbMI/versions/abc/sodium.jar";
        ctx.provenance.files.insert(
            "mods/sodium.jar".into(),
            provenance::Entry {
                modrinth: Some(ModrinthSource {
                    url: Some(url.into()),
                    client_side: "required".into(),
                    server_side: "unsupported".into(),
                    ..ModrinthSource::default()
                }),
                ..known(10, 99)
            },
        );
        ctx.provenance.files.insert(
            "mods/jei.jar".into(),
            provenance::Entry {
                curseforge: Some(CurseForgeSource {
                    project_id: 238222,
                    file_id: 5,
                    downloadable: false,
                    ..CurseForgeSource::default()
                }),
                ..known(20, 99)
            },
        );
        ctx.provenance.files.insert(
            "mods/elsewhere.jar".into(),
            provenance::Entry {
                modrinth: Some(ModrinthSource {
                    url: Some("https://example.com/data/x/versions/y/z.jar".into()),
                    ..ModrinthSource::default()
                }),
                ..known(30, 99)
            },
        );
        ctx.provenance
            .files
            .insert("resourcepacks/pack.zip".into(), known(40, 99));

        let described = ctx.describe(vec![
            // Disabled: matched under its enabled name.
            entry("mods/sodium.jar.disabled", 10, 99),
            entry("mods/jei.jar", 20, 99),
            entry("mods/elsewhere.jar", 30, 99),
            entry("resourcepacks/pack.zip", 40, 99),
            // Changed since content.json saw it.
            entry("mods/jei.jar.disabled", 21, 99),
            entry("config/sodium.json", 10, 99),
        ]);
        let sodium = &described[0];
        assert_eq!(
            sodium.source,
            Source::Modrinth {
                project: "AANobbMI".into(),
                version: "abc".into(),
                url: url.into(),
            }
        );
        assert_eq!(sodium.sha512.as_deref(), Some("5".repeat(128).as_str()));
        assert_eq!(sodium.sha1.as_deref(), Some("1".repeat(40).as_str()));
        assert_eq!(sodium.env.client, EnvValue::Required);
        assert_eq!(sodium.env.server, EnvValue::Unsupported);
        assert_eq!(sodium.env_from, [EnvFrom::ModrinthProject]);

        assert_eq!(
            described[1].source,
            Source::Curseforge {
                project: 238222,
                file: 5,
                downloadable: false
            }
        );
        // Not a Modrinth CDN file: bytes only this store has.
        assert_eq!(described[2].source, Source::Object {});
        assert_eq!(described[3].source, Source::Object {});
        assert!(described[3].sha512.is_some());
        for unknown in &described[4..] {
            assert_eq!(unknown.source, Source::Object {});
            assert_eq!(unknown.sha512, None);
            assert_eq!(unknown.env_from, Vec::<EnvFrom>::new());
        }
    }

    #[test]
    fn hostile_content_json_claims_nothing() {
        let mut ctx = context(None);
        let url = "https://cdn.modrinth.com/data/A/versions/B/x.jar";
        ctx.provenance.files.insert(
            "mods/x.jar".into(),
            provenance::Entry {
                sha512: "not hex".into(),
                sha1: "zz".into(),
                modrinth: Some(ModrinthSource {
                    url: Some(url.into()),
                    ..ModrinthSource::default()
                }),
                ..known(1, 2)
            },
        );
        // A file deeper down is never matched to a top-level entry.
        ctx.provenance.files.insert("mods/a".into(), known(1, 2));
        let described = ctx.describe(vec![entry("mods/x.jar", 1, 2), entry("mods/a/b", 1, 2)]);
        // No SHA-512 to check a download against: not a Modrinth source.
        assert_eq!(described[0].source, Source::Object {});
        assert_eq!(described[0].sha512, None);
        assert_eq!(described[0].sha1, None);
        assert_eq!(described[1].sha1, None);
        // A file with no modification time never matches.
        let mut none = entry("mods/x.jar", 1, 2);
        none.modified = None;
        assert_eq!(ctx.describe(vec![none])[0].sha1, None);
    }
}
