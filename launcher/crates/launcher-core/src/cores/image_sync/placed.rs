//! Every file a server's view places on a player's machine ("What a view places"): its
//! base pack's client share (the index files a client gets, then `overrides/` with
//! `client-overrides/` over them, as any Modrinth launcher installs a pack), less the
//! view's `removed`, with the view's entries over it by path.
//!
//! The set is worked out in full for each view, so moving from one view to the next is the
//! difference between two such sets ("From one view to the next on disk"): a base update
//! changes the pack's client-only files and `client-overrides/`, a path leaving `removed`
//! brings the pack's file back, and `base` entries follow the base, none of which a delta
//! says.
//!
//! The client view's rules let through paths a server must never write on a player's
//! machine (`options.txt`, `servers.dat`, worlds), so an entry must be in a content folder
//! (`mods/`, `config/`, `defaultconfigs/`, `kubejs/`, `resourcepacks/`, `shaderpacks/`) or
//! at a path the base pack itself places; anything else refuses the whole view.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use lodestone_image::{
    CanonicalPath, Image, Source, Withheld, paths::case_collisions, paths::is_config,
    view::needed_withheld,
};
use lodestone_minecraft::mrpack::PackDownload;

use super::SyncError;
use crate::{
    images::scope::{self, is_content},
    modpack::{
        FileSource, OverrideFile, PackFile, PackInfo,
        merge::{Digest, Fetch, Wanted},
        slash_path,
    },
};

/// The most a view may place, in bytes: the bound imports have.
pub const MAX_PLACED_BYTES: u64 = 64 * 1_000_000_000;

/// A CurseForge file a view names that can be downloaded: where from, and its SHA-1.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurseForgeFile {
    pub url: url::Url,
    pub sha1: String,
}

/// What a view places.
#[derive(Clone, Debug, Default)]
pub struct Placed {
    /// Every file, by path in the game folder.
    pub files: BTreeMap<String, Wanted>,
    /// Files the server has that the view can't give and a player would need, with why.
    pub missing: Vec<Withheld>,
    /// CurseForge files whose authors don't allow other apps to download them: the player
    /// gets them by hand. (path, project, file)
    pub curseforge_listed: Vec<(String, u64, u64)>,
    pub total_bytes: u64,
    /// The paths of the view's entries that are the base pack's own file there (a `base`
    /// entry, or a Modrinth file with the hash of the one the pack places): not mods the
    /// server added.
    pub from_base: BTreeSet<String>,
}

/// A view's base pack, as [`placed`] needs it: its client share, and the files of its
/// server share's `overrides/` with `server-overrides/` over them, which the core takes a
/// `base` entry from when the client share has another copy at its path.
#[derive(Clone, Copy)]
pub struct BaseShare<'a> {
    pub pack: &'a PackInfo,
    pub server_overrides: &'a [OverrideFile],
}

/// What `view` places on top of its base pack `base`'s client share (`None` for a view
/// without a base). `curseforge` resolves the CurseForge files the view names that may be
/// downloaded (by project and file); one it doesn't resolve is listed for the player.
pub fn placed(
    view: &Image,
    base: Option<BaseShare<'_>>,
    curseforge: &HashMap<(u64, u64), CurseForgeFile>,
) -> Result<Placed, SyncError> {
    // 1. The base's client share.
    let mut share: BTreeMap<String, Wanted> = BTreeMap::new();
    let mut server_overrides: HashMap<&str, &OverrideFile> = HashMap::new();
    if let Some(BaseShare {
        pack,
        server_overrides: overrides,
    }) = base
    {
        server_overrides.extend(overrides.iter().map(|file| (file.path.as_str(), file)));
        for download in &pack.downloads {
            share.insert(slash_path(&download.path), Wanted::download(download));
        }
        for file in &pack.overrides {
            share.insert(file.path.clone(), Wanted::override_file(file));
        }
    }
    let mut files = share.clone();
    // 2. Less what the view removed.
    for path in &view.removed {
        files.remove(path.as_str());
    }
    // 3. The view's entries over it.
    let mut curseforge_listed = Vec::new();
    let mut from_base = BTreeSet::new();
    for entry in &view.entries {
        let path = entry.path.as_str();
        // the pack's own file: a `base` entry, or a Modrinth file with the hash of the
        // pack's (which its download checks); a server's own file or a CurseForge one
        // only claims its hashes, so it isn't taken for the pack's
        let pack_file = match entry.source {
            Source::Base {} => true,
            Source::Modrinth { .. } => share.get(path).is_some_and(|copy| same_file(entry, copy)),
            Source::Object {} | Source::Curseforge { .. } => false,
        };
        if pack_file {
            from_base.insert(path.to_string());
        }
        let pack_places = share.contains_key(path) && {
            let parts: Vec<&str> = entry.path.components().collect();
            !scope::never_pack_content(&parts)
        };
        if !is_content(&entry.path) && !pack_places {
            return Err(SyncError::Refused(format!(
                "it would place {}, which a server can't change on players' machines",
                lodestone_image::display_text(path)
            )));
        }
        let source = if is_config(&entry.path) {
            FileSource::Override
        } else {
            FileSource::Index
        };
        let record = |sha1: Option<&str>, project: Option<String>| PackFile {
            sha1: sha1.unwrap_or_default().to_string(),
            source,
            project,
            sha256: Some(entry.sha256.clone()),
        };
        let wanted = match &entry.source {
            Source::Modrinth {
                project,
                version,
                url,
            } => {
                // only from Modrinth's CDN, and from the project and version it names
                let url = url::Url::parse(url)
                    .ok()
                    .filter(|url| {
                        crate::modpack::cdn_ids(url)
                            .is_some_and(|ids| ids == (project.clone(), version.clone()))
                    })
                    .ok_or_else(|| {
                        SyncError::Refused(format!(
                            "{} isn't a file on Modrinth's CDN of the project it names",
                            lodestone_image::display_text(path)
                        ))
                    })?;
                Wanted {
                    digest: Digest::Sha256(entry.sha256.clone()),
                    record: record(entry.sha1.as_deref(), Some(project.clone())),
                    fetch: Fetch::Download {
                        download: PackDownload {
                            path: path.into(),
                            urls: vec![url],
                            sha1: entry.sha1.clone().unwrap_or_default(),
                            sha512: entry.sha512.clone().unwrap_or_default(),
                            size: entry.size,
                        },
                        expect_sha256: Some(entry.sha256.clone()),
                    },
                }
            }
            Source::Object {} => Wanted {
                digest: Digest::Sha256(entry.sha256.clone()),
                record: record(entry.sha1.as_deref(), None),
                fetch: Fetch::Object {
                    sha256: entry.sha256.clone(),
                    size: entry.size,
                },
            },
            Source::Base {} => {
                // the client share's copy, else the server share's, which is where the
                // core took it from when the pack has a client-only copy at the path
                let candidates: Vec<Wanted> = share
                    .get(path)
                    .cloned()
                    .into_iter()
                    .chain(server_overrides.get(path).map(|f| Wanted::override_file(f)))
                    .collect();
                if candidates.is_empty() {
                    return Err(SyncError::BaseFileMissing(
                        lodestone_image::display_text(path).into_owned(),
                    ));
                }
                // placed as the pack places it (an override never over the player's own
                // file, as a modpack update keeps it), a config as a config
                candidates
                    .iter()
                    .find_map(|copy| {
                        let source = if is_config(&entry.path) {
                            FileSource::Override
                        } else {
                            copy.record.source
                        };
                        base_file(entry, copy, source)
                    })
                    .ok_or_else(|| {
                        SyncError::Refused(format!(
                            "{} isn't the modpack's file, though it says it is",
                            lodestone_image::display_text(path)
                        ))
                    })?
            }
            Source::Curseforge {
                project,
                file,
                downloadable,
            } => match curseforge.get(&(*project, *file)).filter(|_| *downloadable) {
                Some(found) => Wanted {
                    digest: Digest::Sha256(entry.sha256.clone()),
                    record: record(Some(&found.sha1), None),
                    fetch: Fetch::Download {
                        download: PackDownload {
                            path: path.into(),
                            urls: vec![found.url.clone()],
                            sha1: found.sha1.clone(),
                            sha512: String::new(),
                            size: entry.size,
                        },
                        expect_sha256: Some(entry.sha256.clone()),
                    },
                },
                None => {
                    // the player gets it by hand; nothing of the base's goes there
                    curseforge_listed.push((path.to_string(), *project, *file));
                    files.remove(path);
                    continue;
                }
            },
        };
        files.insert(path.to_string(), wanted);
    }

    // Names one disk folder or file in two spellings can't be placed on most disks.
    let canonical: Vec<CanonicalPath> = files
        .keys()
        .filter_map(|path| CanonicalPath::new(path.as_str()).ok())
        .collect();
    if let Some((a, b)) = case_collisions(canonical.iter()).into_iter().next() {
        return Err(SyncError::Refused(format!(
            "{} and {} differ only in letter case",
            lodestone_image::display_text(a.as_str()),
            lodestone_image::display_text(b.as_str())
        )));
    }
    let total_bytes = files
        .values()
        .fold(0u64, |sum, wanted| sum.saturating_add(wanted.fetch.size()));
    if total_bytes > MAX_PLACED_BYTES {
        return Err(SyncError::Refused(
            "its files add up to more than 64 GB".into(),
        ));
    }
    Ok(Placed {
        files,
        missing: needed_withheld(view).into_iter().cloned().collect(),
        curseforge_listed,
        total_bytes,
        from_base,
    })
}

/// The paths of `view`'s `base` entries that the client share of `pack` has no matching
/// copy of: those are looked up in the pack's server share ([`BaseShare`]).
pub fn needs_server_copy(view: &Image, pack: &PackInfo) -> BTreeSet<String> {
    let mut share: HashMap<String, Wanted> = HashMap::new();
    for download in &pack.downloads {
        share.insert(slash_path(&download.path), Wanted::download(download));
    }
    for file in &pack.overrides {
        share.insert(file.path.clone(), Wanted::override_file(file));
    }
    view.entries
        .iter()
        .filter(|entry| matches!(entry.source, Source::Base {}))
        .filter(|entry| {
            share
                .get(entry.path.as_str())
                .is_none_or(|copy| base_file(entry, copy, FileSource::Index).is_none())
        })
        .map(|entry| entry.path.as_str().to_string())
        .collect()
}

/// Whether `entry` is the file `copy` (the base's at its path) by a hash both give.
fn same_file(entry: &lodestone_image::Entry, copy: &Wanted) -> bool {
    let sha1 = !copy.record.sha1.is_empty()
        && entry
            .sha1
            .as_deref()
            .is_some_and(|sha1| sha1.eq_ignore_ascii_case(&copy.record.sha1));
    let sha512 = match &copy.fetch {
        Fetch::Download { download, .. } => {
            !download.sha512.is_empty()
                && entry
                    .sha512
                    .as_deref()
                    .is_some_and(|sha512| sha512.eq_ignore_ascii_case(&download.sha512))
        }
        _ => false,
    };
    sha1 || sha512
}

/// A `base` entry from the pack's copy `from_pack` at its path, when that copy is the
/// file the entry names (by every hash and size both give); `None` when it isn't.
fn base_file(
    entry: &lodestone_image::Entry,
    from_pack: &Wanted,
    source: FileSource,
) -> Option<Wanted> {
    let pack_sha1 = &from_pack.record.sha1;
    if entry
        .sha1
        .as_deref()
        .is_some_and(|sha1| !sha1.eq_ignore_ascii_case(pack_sha1))
    {
        return None;
    }
    let fetch = match &from_pack.fetch {
        Fetch::Download { download, .. } => {
            if let (Some(theirs), false) = (entry.sha512.as_deref(), download.sha512.is_empty())
                && !theirs.eq_ignore_ascii_case(&download.sha512)
            {
                return None;
            }
            if download.size > 0 && download.size != entry.size {
                return None;
            }
            Fetch::Download {
                download: download.clone(),
                expect_sha256: Some(entry.sha256.clone()),
            }
        }
        Fetch::Override { file, .. } => {
            if file.size != entry.size {
                return None;
            }
            Fetch::Override {
                file: file.clone(),
                expect_sha256: Some(entry.sha256.clone()),
            }
        }
        Fetch::Object { .. } => return None,
    };
    Some(Wanted {
        digest: Digest::Sha256(entry.sha256.clone()),
        record: PackFile {
            sha1: pack_sha1.clone(),
            source,
            project: from_pack.record.project.clone(),
            sha256: Some(entry.sha256.clone()),
        },
        fetch,
    })
}

/// The Modrinth projects and paths a view's files come from, to find the added packs'
/// copies of the same mods ([`crate::modpack::addon::superseded_by`]).
pub fn projects_and_paths(placed: &Placed) -> (BTreeSet<String>, BTreeSet<String>) {
    let projects = placed
        .files
        .values()
        .filter_map(|w| w.record.project.clone())
        .collect();
    let paths = placed.files.keys().cloned().collect();
    (projects, paths)
}

#[cfg(test)]
mod tests {
    use lodestone_image::{
        Base, Entry, Game, Kind, Loader, RULES, Side, Withheld, WithheldReason, hex::sha256_hex,
        view,
    };

    use super::*;

    fn sha1(bytes: &[u8]) -> String {
        crate::game::download::sha1_hex(bytes)
    }

    /// A pack with a mod in its index, a mod and a config in `overrides/`, and a file
    /// outside the content folders (`scripts/`).
    fn pack() -> PackInfo {
        let download = |path: &str, bytes: &[u8]| PackDownload {
            path: path.into(),
            urls: vec![
                url::Url::parse("https://cdn.modrinth.com/data/P/versions/V/x.jar").unwrap(),
            ],
            sha1: sha1(bytes),
            sha512: "a".repeat(128),
            size: bytes.len() as u64,
        };
        let file = |path: &str, bytes: &[u8], index| OverrideFile {
            path: path.into(),
            index,
            size: bytes.len() as u64,
            sha1: sha1(bytes),
        };
        PackInfo {
            name: "Pack".into(),
            version_number: Some("1.0".into()),
            summary: None,
            game_version: "1.21.1".into(),
            loader: None,
            recommended_memory: None,
            downloads: vec![download("mods/sodium.jar", b"sodium")],
            overrides: vec![
                file("mods/bundled.jar", b"bundled", 1),
                file("config/pack.toml", b"pack config", 2),
                file("scripts/recipes.zs", b"recipes", 3),
            ],
            skipped: Vec::new(),
        }
    }

    fn entry(path: &str, bytes: &[u8], source: Source) -> Entry {
        let mut entry = Entry::new(
            CanonicalPath::new(path).unwrap(),
            bytes.len() as u64,
            sha256_hex(bytes),
            source,
        );
        if matches!(entry.source, Source::Base {}) {
            entry.sha1 = Some(sha1(bytes));
        }
        entry
    }

    fn client_view(change: impl FnOnce(&mut Image)) -> Image {
        let mut image = Image::new(
            Kind::Content,
            Game {
                minecraft: "1.21.1".into(),
                loader: Loader::Fabric,
                loader_version: Some("0.16.10".into()),
            },
        );
        image.base = Some(Base::ModrinthPack {
            project: "P".into(),
            version: "V".into(),
            sha512: "b".repeat(128),
        });
        change(&mut image);
        image.seal();
        view(&image, Side::Client, RULES).unwrap()
    }

    fn base(pack: &PackInfo) -> Option<BaseShare<'_>> {
        Some(BaseShare {
            pack,
            server_overrides: &[],
        })
    }

    fn paths(placed: &Placed) -> Vec<&str> {
        placed.files.keys().map(String::as_str).collect()
    }

    #[test]
    fn the_base_share_less_removed_with_entries_over_it() {
        let view = client_view(|image| {
            image
                .removed
                .push(CanonicalPath::new("mods/bundled.jar").unwrap());
            // configs never leave a client view's base share
            image
                .removed
                .push(CanonicalPath::new("config/pack.toml").unwrap());
            image
                .entries
                .push(entry("mods/own.jar", b"own", Source::Object {}));
            image
                .entries
                .push(entry("mods/sodium.jar", b"sodium", Source::Base {}));
            image.entries.push(entry(
                "scripts/recipes.zs",
                b"server recipes",
                Source::Object {},
            ));
        });
        let placed = placed(&view, base(&pack()), &HashMap::new()).unwrap();
        assert_eq!(
            paths(&placed),
            [
                "config/pack.toml",
                "mods/own.jar",
                "mods/sodium.jar",
                "scripts/recipes.zs"
            ]
        );
        // the base's sodium, checked against the server's SHA-256 too
        let sodium = &placed.files["mods/sodium.jar"];
        assert_eq!(sodium.digest, Digest::Sha256(sha256_hex(b"sodium")));
        assert!(matches!(
            &sodium.fetch,
            Fetch::Download { expect_sha256: Some(sha), .. } if *sha == sha256_hex(b"sodium")
        ));
        // the server's own file, from the server
        assert!(matches!(
            placed.files["mods/own.jar"].fetch,
            Fetch::Object { size: 3, .. }
        ));
        // the pack's config is an override: a later view dropping it leaves it
        assert_eq!(
            placed.files["config/pack.toml"].record.source,
            FileSource::Override
        );
        assert_eq!(placed.total_bytes, 6 + 3 + 11 + 14);
    }

    #[test]
    fn a_base_entry_the_pack_lacks_or_has_otherwise_is_refused() {
        let view = client_view(|image| {
            image
                .entries
                .push(entry("mods/gone.jar", b"gone", Source::Base {}));
        });
        assert!(matches!(
            placed(&view, base(&pack()), &HashMap::new()),
            Err(SyncError::BaseFileMissing(path)) if path == "mods/gone.jar"
        ));
        let view = client_view(|image| {
            let mut changed = entry("mods/sodium.jar", b"sodium changed", Source::Base {});
            changed.sha1 = Some(sha1(b"sodium changed"));
            image.entries.push(changed);
        });
        assert!(matches!(
            placed(&view, base(&pack()), &HashMap::new()),
            Err(SyncError::Refused(_))
        ));
        // no base pack at all
        let view = client_view(|image| {
            image
                .entries
                .push(entry("mods/sodium.jar", b"sodium", Source::Base {}));
        });
        assert!(matches!(
            placed(&view, None, &HashMap::new()),
            Err(SyncError::BaseFileMissing(_))
        ));
    }

    /// The client view's rules let these through; a server must still never write them.
    #[test]
    fn files_outside_the_content_folders_are_refused() {
        for path in [
            "options.txt",
            "servers.dat",
            "optionsof.txt",
            "screenshots/a.png",
            "logs2/x.txt",
            "scripts/new.zs",
        ] {
            let view = client_view(|image| {
                image.entries.push(entry(path, b"x", Source::Object {}));
            });
            assert!(
                matches!(
                    placed(&view, base(&pack()), &HashMap::new()),
                    Err(SyncError::Refused(_))
                ),
                "{path}"
            );
        }
        // a path the pack itself places is fine (scripts/recipes.zs above), and so are
        // the content folders
        for path in [
            "resourcepacks/r.zip",
            "shaderpacks/s.zip",
            "kubejs/startup_scripts/a.js",
        ] {
            let view = client_view(|image| {
                image.entries.push(entry(path, b"x", Source::Object {}));
            });
            placed(&view, base(&pack()), &HashMap::new()).unwrap();
        }
    }

    #[test]
    fn names_that_clash_by_case_and_huge_views_are_refused() {
        let view = client_view(|image| {
            image
                .entries
                .push(entry("mods/JEI.jar", b"a", Source::Object {}));
            image
                .entries
                .push(entry("mods/jei.jar", b"b", Source::Object {}));
        });
        assert!(matches!(
            placed(&view, None, &HashMap::new()),
            Err(SyncError::Refused(message)) if message.contains("letter case")
        ));
        let view = client_view(|image| {
            let mut big = entry("mods/big.jar", b"x", Source::Object {});
            big.size = MAX_PLACED_BYTES + 1;
            image.entries.push(big);
        });
        assert!(matches!(
            placed(&view, None, &HashMap::new()),
            Err(SyncError::Refused(message)) if message.contains("64 GB")
        ));
    }

    #[test]
    fn missing_files_and_curseforge_files_are_listed() {
        let view = client_view(|image| {
            image.withheld.push(Withheld {
                path: "mods/linked.jar".into(),
                reason: WithheldReason::Symlink,
            });
            image.withheld.push(Withheld {
                path: "config/secret.toml".into(),
                reason: WithheldReason::Unreadable,
            });
            image.entries.push(entry(
                "mods/blocked.jar",
                b"blocked",
                Source::Curseforge {
                    project: 1,
                    file: 2,
                    downloadable: false,
                },
            ));
            image.entries.push(entry(
                "mods/allowed.jar",
                b"allowed",
                Source::Curseforge {
                    project: 3,
                    file: 4,
                    downloadable: true,
                },
            ));
        });
        let mut curseforge = HashMap::new();
        curseforge.insert(
            (3, 4),
            CurseForgeFile {
                url: url::Url::parse("https://edge.forgecdn.net/files/4/allowed.jar").unwrap(),
                sha1: sha1(b"allowed"),
            },
        );
        let placed = placed(&view, None, &curseforge).unwrap();
        assert_eq!(placed.missing.len(), 1);
        assert_eq!(placed.missing[0].path, "mods/linked.jar");
        assert_eq!(
            placed.curseforge_listed,
            [("mods/blocked.jar".to_string(), 1, 2)]
        );
        assert_eq!(paths(&placed), ["mods/allowed.jar"]);
    }

    fn modrinth(project: &str, version: &str, url: &str) -> Source {
        Source::Modrinth {
            project: project.into(),
            version: version.into(),
            url: url.into(),
        }
    }

    /// A Modrinth entry is downloaded only from Modrinth's CDN, and only from the project
    /// and version it names: never from CurseForge's CDN (which would get around a
    /// CurseForge author's choice) or any other host a pack may use.
    #[test]
    fn modrinth_files_only_come_from_modrinths_cdn() {
        for (project, url) in [
            (
                "AANobbMI",
                "https://edge.forgecdn.net/files/1234/567/blocked.jar",
            ),
            (
                "AANobbMI",
                "https://github.com/someone/mod/releases/download/1/x.jar",
            ),
            (
                "AANobbMI",
                "https://cdn.modrinth.com/data/OTHER/versions/V1/x.jar",
            ),
        ] {
            // the view check refuses some of these on its own; whatever it lets through,
            // this refuses too
            let mut view = client_view(|_| {});
            let mut jar = entry("mods/x.jar", b"x", modrinth(project, "V1", url));
            jar.sha1 = Some(sha1(b"x"));
            jar.sha512 = Some("c".repeat(128));
            view.entries.push(jar);
            assert!(
                matches!(
                    placed(&view, None, &HashMap::new()),
                    Err(SyncError::Refused(_))
                ),
                "{url}"
            );
        }
        let url = "https://cdn.modrinth.com/data/AANobbMI/versions/V1/x.jar";
        let view = client_view(|image| {
            let mut jar = entry("mods/x.jar", b"x", modrinth("AANobbMI", "V1", url));
            jar.sha512 = Some("c".repeat(128));
            image.entries.push(jar);
        });
        let placed = placed(&view, None, &HashMap::new()).unwrap();
        assert_eq!(
            placed.files["mods/x.jar"].record.project.as_deref(),
            Some("AANobbMI")
        );
    }

    /// A pack with a client-only copy of a mod (in `client-overrides/`) and another in
    /// `overrides/`: the core takes a `base` entry from its server share, so the entry is
    /// the `overrides/` copy, which the pack's zip has too. (A config marked for players
    /// is the same.)
    #[test]
    fn a_base_entry_can_be_the_server_shares_copy() {
        let mut pack = pack();
        let client_copy = OverrideFile {
            path: "mods/both.jar".into(),
            index: 7,
            size: 6,
            sha1: sha1(b"client"),
        };
        pack.overrides.push(client_copy);
        let server_copy = OverrideFile {
            path: "mods/both.jar".into(),
            index: 4,
            size: 11,
            sha1: sha1(b"both sides!"),
        };
        let view = client_view(|image| {
            image
                .entries
                .push(entry("mods/both.jar", b"both sides!", Source::Base {}));
        });
        let share = BaseShare {
            pack: &pack,
            server_overrides: std::slice::from_ref(&server_copy),
        };
        let found = placed(&view, Some(share), &HashMap::new()).unwrap();
        let foo = &found.files["mods/both.jar"];
        assert!(matches!(
            &foo.fetch,
            Fetch::Override { file, expect_sha256: Some(sha) }
                if file.index == 4 && *sha == sha256_hex(b"both sides!")
        ));
        assert!(found.from_base.contains("mods/both.jar"));
        // neither copy: refused
        let view = client_view(|image| {
            image
                .entries
                .push(entry("mods/both.jar", b"another one", Source::Base {}));
        });
        assert!(matches!(
            placed(&view, Some(share), &HashMap::new()),
            Err(SyncError::Refused(_))
        ));
    }

    /// A mod the base pack ships, at the same version, isn't one the server added, even
    /// when the core names it by its Modrinth URL rather than as `base`.
    #[test]
    fn the_bases_own_files_are_told_apart() {
        let url = "https://cdn.modrinth.com/data/P/versions/V/x.jar";
        let view = client_view(|image| {
            let mut sodium = entry("mods/sodium.jar", b"sodium", modrinth("P", "V", url));
            sodium.sha1 = Some(sha1(b"sodium"));
            sodium.sha512 = Some("a".repeat(128));
            image.entries.push(sodium);
            let mut newer = entry("mods/bundled.jar", b"newer", modrinth("P", "V", url));
            newer.sha1 = Some(sha1(b"newer"));
            newer.sha512 = Some("d".repeat(128));
            image.entries.push(newer);
            image
                .entries
                .push(entry("mods/own.jar", b"own", Source::Object {}));
        });
        let placed = placed(&view, base(&pack()), &HashMap::new()).unwrap();
        assert_eq!(
            placed
                .from_base
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["mods/sodium.jar"]
        );
    }
}
