//! Exporting an instance as a Modrinth modpack (`.mrpack`).
//!
//! The pack is `modrinth.index.json` (format 1: the Minecraft version and the loader
//! as `dependencies`) and an `overrides/` folder with the instance's mods, configs,
//! resource packs, shader packs and options. A mod that is known to be on Modrinth
//! (its SHA-512 is in the map the caller passes, with its CDN URL) is listed in the
//! index instead, so the pack downloads it. Content management, which knows where
//! mods came from, is a later milestone; until then the map is empty and every mod
//! goes into the overrides.
//!
//! See <https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack>.

use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, File},
    io::{self, Write as _},
    path::{Path, PathBuf},
};

use lodestone_minecraft::mrpack::{
    self, PathCheck, WriteAll,
    write::{PackFileEnv, PackFileHashes, PackIndex, PackIndexFile},
};

use crate::{
    game::loaders::{LoaderKind, forge::forge_display},
    instances::Instance,
};

/// What goes into `overrides/`, relative to the game folder.
pub const EXPORTED: &[&str] = &[
    "mods",
    "config",
    "defaultconfigs",
    "kubejs",
    "resourcepacks",
    "shaderpacks",
    "options.txt",
];

/// A pack ready to write.
#[derive(Clone, Debug, PartialEq)]
pub struct ExportPlan {
    pub index: PackIndex,
    /// The path in the zip (`overrides/...`) and the file on disk.
    pub overrides: Vec<(String, PathBuf)>,
    /// Files left out, and why.
    pub skipped: Vec<(String, String)>,
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("couldn't write the modpack: {0}")]
    Zip(#[from] zip::result::ZipError),
}

fn io_error(path: &Path) -> impl FnOnce(io::Error) -> ExportError + '_ {
    move |source| ExportError::Io {
        path: path.to_owned(),
        source,
    }
}

/// The version of the loader, as `.mrpack` dependencies write it (Forge without the
/// Minecraft version: `47.4.28`).
pub fn dependency_version(kind: LoaderKind, version: &str, game_version: &str) -> String {
    match kind {
        LoaderKind::Forge => forge_display(version, game_version),
        _ => version.to_string(),
    }
}

/// Plans the pack of `instance`, whose game folder is `game_dir`. `on_modrinth` maps
/// a mod's SHA-512 to its Modrinth CDN URL.
pub fn plan(
    instance: &Instance,
    game_dir: &Path,
    on_modrinth: &HashMap<String, String>,
) -> Result<ExportPlan, ExportError> {
    let mut files = Vec::new();
    let mut overrides = Vec::new();
    let mut skipped = Vec::new();
    for top in EXPORTED {
        let path = game_dir.join(top);
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        let mut found = Vec::new();
        if meta.is_dir() {
            collect(&path, top, &mut found, &mut skipped)?;
        } else if meta.is_file() {
            found.push((top.to_string(), path));
        }
        for (relative, path) in found {
            match mrpack::check_pack_path(&relative, false, &WriteAll) {
                Ok(PathCheck::Write(_)) => {}
                _ => {
                    skipped.push((relative, "the name can't go into a modpack".into()));
                    continue;
                }
            }
            let is_mod = relative.starts_with("mods/") && relative.ends_with(".jar");
            if is_mod {
                let sha512 =
                    lodestone_minecraft::hash::sha512_of(&path).map_err(io_error(&path))?;
                if let Some(url) = on_modrinth.get(&sha512) {
                    let sha1 =
                        lodestone_minecraft::hash::sha1_of(&path).map_err(io_error(&path))?;
                    let size = fs::metadata(&path).map_err(io_error(&path))?.len();
                    files.push(PackIndexFile {
                        path: relative,
                        hashes: PackFileHashes { sha1, sha512 },
                        env: PackFileEnv {
                            client: "required".into(),
                            server: "required".into(),
                        },
                        downloads: vec![url.clone()],
                        file_size: size,
                    });
                    continue;
                }
            }
            overrides.push((format!("overrides/{relative}"), path));
        }
    }
    files.sort_by(|a, b| a.path.cmp(&b.path));
    overrides.sort();
    let mut dependencies = BTreeMap::new();
    dependencies.insert("minecraft".to_string(), instance.game_version.clone());
    if let Some(choice) = instance.loader_choice() {
        dependencies.insert(
            choice.kind.mrpack_key().to_string(),
            dependency_version(choice.kind, &choice.version, &instance.game_version),
        );
    }
    Ok(ExportPlan {
        index: PackIndex {
            format_version: 1,
            game: "minecraft".into(),
            version_id: "1.0.0".into(),
            name: instance.name.clone(),
            summary: format!(
                "Exported from Lodestone Launcher ({})",
                instance.loader_label()
            ),
            files,
            dependencies,
        },
        overrides,
        skipped,
    })
}

/// The files under `dir` (whose path in the pack is `prefix`), without symbolic links.
fn collect(
    dir: &Path,
    prefix: &str,
    out: &mut Vec<(String, PathBuf)>,
    skipped: &mut Vec<(String, String)>,
) -> Result<(), ExportError> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(io_error(dir))?
        .collect::<Result<_, _>>()
        .map_err(io_error(dir))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            skipped.push((
                path.display().to_string(),
                "the name isn't valid UTF-8".into(),
            ));
            continue;
        };
        let relative = format!("{prefix}/{name}");
        let kind = entry.file_type().map_err(io_error(&path))?;
        if kind.is_dir() {
            collect(&path, &relative, out, skipped)?;
        } else if kind.is_file() {
            out.push((relative, path));
        } else {
            skipped.push((relative, "links are not exported".into()));
        }
    }
    Ok(())
}

/// Writes the pack to `out`: the index, then the overrides. It is written to a
/// temporary file next to `out` and renamed into place.
pub fn write(plan: &ExportPlan, out: &Path) -> Result<(), ExportError> {
    let tmp = out.with_extension("mrpack.part");
    let result = (|| {
        let file = File::create(&tmp).map_err(io_error(&tmp))?;
        let mut zip = zip::ZipWriter::new(file);
        let deflated =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        zip.start_file(mrpack::INDEX_FILE, deflated)?;
        let index = serde_json::to_vec_pretty(&plan.index).map_err(|err| ExportError::Io {
            path: out.to_owned(),
            source: err.into(),
        })?;
        zip.write_all(&index).map_err(io_error(out))?;
        for (name, path) in &plan.overrides {
            // Jars are compressed already.
            let options = if name.ends_with(".jar") || name.ends_with(".zip") {
                zip::write::FileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored)
                    .large_file(true)
            } else {
                deflated.large_file(true)
            };
            zip.start_file(name.as_str(), options)?;
            let mut source = File::open(path).map_err(io_error(path))?;
            io::copy(&mut source, &mut zip).map_err(io_error(path))?;
        }
        zip.finish()?.sync_all().map_err(io_error(&tmp))?;
        fs::rename(&tmp, out).map_err(io_error(out))
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use super::*;
    use crate::instances::Loader;

    fn instance() -> Instance {
        Instance {
            name: "Pack".into(),
            game_version: "1.20.1".into(),
            loader: Loader::Forge,
            loader_version: Some("1.20.1-47.4.28".into()),
            ..Instance::default()
        }
    }

    #[test]
    fn plans_and_writes_a_pack() {
        let dir = tempfile::tempdir().unwrap();
        let game = dir.path().join("minecraft");
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::create_dir_all(game.join("config/sub")).unwrap();
        fs::create_dir_all(game.join("saves/World")).unwrap();
        fs::write(game.join("mods/known.jar"), b"known mod").unwrap();
        fs::write(game.join("mods/local.jar"), b"local mod").unwrap();
        fs::write(game.join("config/sub/a.toml"), b"a = 1").unwrap();
        fs::write(game.join("options.txt"), b"fov:90").unwrap();
        fs::write(game.join("saves/World/level.dat"), b"no").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc/hosts", game.join("config/link")).unwrap();

        let known_sha512 =
            lodestone_minecraft::hash::sha512_of(&game.join("mods/known.jar")).unwrap();
        let on_modrinth = HashMap::from([(
            known_sha512.clone(),
            "https://cdn.modrinth.com/data/abc/versions/def/known.jar".to_string(),
        )]);
        let plan = plan(&instance(), &game, &on_modrinth).unwrap();
        assert_eq!(plan.index.dependencies["minecraft"], "1.20.1");
        assert_eq!(plan.index.dependencies["forge"], "47.4.28");
        assert_eq!(plan.index.files.len(), 1);
        assert_eq!(plan.index.files[0].path, "mods/known.jar");
        assert_eq!(plan.index.files[0].hashes.sha512, known_sha512);
        assert_eq!(plan.index.files[0].file_size, 9);
        let names: Vec<&str> = plan.overrides.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            [
                "overrides/config/sub/a.toml",
                "overrides/mods/local.jar",
                "overrides/options.txt"
            ]
        );
        assert!(plan.skipped.iter().any(|(path, _)| path == "config/link"));

        let out = dir.path().join("Pack.mrpack");
        write(&plan, &out).unwrap();
        assert!(!out.with_extension("mrpack.part").exists());
        let mut zip = zip::ZipArchive::new(File::open(&out).unwrap()).unwrap();
        let mut index = String::new();
        zip.by_name("modrinth.index.json")
            .unwrap()
            .read_to_string(&mut index)
            .unwrap();
        // The pack reads back with the shared crate's reader.
        let parsed: mrpack::Index = serde_json::from_str(&index).unwrap();
        assert_eq!(parsed.format_version, 1);
        assert_eq!(parsed.name, "Pack");
        let (downloads, _) = mrpack::check_files(&parsed, mrpack::Side::Client, &WriteAll).unwrap();
        assert_eq!(downloads.len(), 1);
        let mut local = Vec::new();
        zip.by_name("overrides/mods/local.jar")
            .unwrap()
            .read_to_end(&mut local)
            .unwrap();
        assert_eq!(local, b"local mod");
        assert!(zip.by_name("overrides/saves/World/level.dat").is_err());
    }

    #[test]
    fn vanilla_and_other_loaders() {
        let dir = tempfile::tempdir().unwrap();
        let vanilla = Instance {
            game_version: "1.21.1".into(),
            ..Instance::default()
        };
        let plan = plan(&vanilla, dir.path(), &HashMap::new()).unwrap();
        assert_eq!(plan.index.dependencies.len(), 1);
        assert!(plan.overrides.is_empty());
        assert_eq!(
            dependency_version(LoaderKind::NeoForge, "21.1.209", "1.21.1"),
            "21.1.209"
        );
        assert_eq!(LoaderKind::Fabric.mrpack_key(), "fabric-loader");
        assert_eq!(LoaderKind::Quilt.mrpack_key(), "quilt-loader");
    }
}
