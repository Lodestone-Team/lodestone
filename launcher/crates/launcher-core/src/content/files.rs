//! Changing content by hand: adding local files ("Add file…" and drag and drop),
//! enabling and disabling mods (`.disabled`), reordering resource packs, picking the
//! shader pack, and deleting (to the Trash).

use std::{
    fs::{self, File},
    io,
    path::{Path, PathBuf},
};

use super::{
    ContentError, ContentKind, InstanceDirs, MAX_FILE_BYTES, Target, disabled_name, enabled_name,
    io_error, is_content_name, loader_change,
    options::{self, PackChange, PackOptions},
    packs::Compatibility,
    provenance::Provenance,
    scan::{self, ModFormat},
    shaders::{self, ShaderLoader},
};
use crate::{game::libraries::safe_relative_path, instances::Loader, storage};

/// A folder pack may hold at most this many files, and this many bytes.
const MAX_FOLDER_FILES: usize = 50_000;
const MAX_FOLDER_BYTES: u64 = 4 * 1024 * 1024 * 1024;

fn format_label(format: ModFormat) -> &'static str {
    match format {
        ModFormat::Fabric => "Fabric",
        ModFormat::Quilt => "Quilt",
        ModFormat::Forge => "Forge",
        ModFormat::NeoForge => "NeoForge",
    }
}

/// Checks that `path` can be added as `kind` to an instance with `target`, and returns
/// the name it gets.
pub fn check_addition(kind: ContentKind, path: &Path, target: &Target) -> Result<String, String> {
    let metadata = fs::symlink_metadata(path).map_err(|err| err.to_string())?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("The name isn't valid text.")?
        .to_string();
    let is_dir = metadata.is_dir();
    if metadata.file_type().is_symlink() {
        return Err("Links can't be added; add the file itself.".into());
    }
    if is_dir && !kind.allows_folders() {
        return Err("Mods are .jar files, not folders.".into());
    }
    if !is_content_name(kind, &name, is_dir)
        || (kind == ContentKind::Mod && name.to_ascii_lowercase().ends_with(".disabled"))
    {
        return Err(match kind {
            ContentKind::Mod => "Mods are .jar files.".into(),
            _ => format!("{} are .zip files or folders.", kind.label()),
        });
    }
    if !is_dir && metadata.len() > MAX_FILE_BYTES {
        return Err(format!(
            "It is larger than {} MB.",
            MAX_FILE_BYTES / (1024 * 1024)
        ));
    }
    let meta = scan::read_meta(kind, path, is_dir);
    match kind {
        ContentKind::Mod => {
            if let Some(problem) = meta.problem {
                return Err(problem);
            }
            if target.loader == Loader::None {
                return Err("This instance is vanilla; mods need a mod loader.".into());
            }
            // OptiFine runs on Forge, or on Fabric through OptiFabric.
            let fits = meta.optifine
                || loader_change::loads_on(&meta.mod_formats, target.loader, &target.game_version)
                    == Some(true);
            if !fits {
                let made_for: Vec<&str> =
                    meta.mod_formats.iter().map(|f| format_label(*f)).collect();
                return Err(format!(
                    "It's a {} mod, and this instance uses {}.",
                    made_for.join("/"),
                    target.loader.label()
                ));
            }
        }
        ContentKind::ResourcePack | ContentKind::ShaderPack => {
            if let Some(problem) = meta.problem {
                return Err(problem);
            }
        }
    }
    Ok(name)
}

/// Copies `path` into the content folder of `kind`, under its own name. Fails if that
/// name is taken. The copy is written under a temporary name and renamed into place.
pub fn add(
    dirs: &InstanceDirs,
    target: &Target,
    kind: ContentKind,
    path: &Path,
) -> Result<String, ContentError> {
    let name = check_addition(kind, path, target).map_err(ContentError::Invalid)?;
    let dir = dirs.content_dir(kind);
    fs::create_dir_all(&dir).map_err(io_error(&dir))?;
    let dest = dir.join(&name);
    let taken = [name.clone(), disabled_name(&name)]
        .iter()
        .take(if kind == ContentKind::Mod { 2 } else { 1 })
        .any(|n| fs::symlink_metadata(dir.join(n)).is_ok());
    if taken {
        return Err(ContentError::Conflict(format!(
            "{name} is already in {}.",
            kind.dir()
        )));
    }
    if path.is_dir() {
        copy_folder(path, &dir, &name)?;
    } else {
        let mut source = File::open(path).map_err(io_error(path))?;
        storage::write_atomic_with(&dest, |file| io::copy(&mut source, file).map(|_| ()))
            .map_err(io_error(&dest))?;
    }
    Ok(name)
}

/// Copies the folder `source` to `dir/name`, through a temporary folder. Symbolic links
/// are skipped, and every path is checked to stay inside the copy.
fn copy_folder(source: &Path, dir: &Path, name: &str) -> Result<(), ContentError> {
    let temp = dir.join(format!(
        ".lodestone-add-{:016x}",
        u64::from_le_bytes(crate::random_bytes())
    ));
    let result = (|| {
        fs::create_dir(&temp).map_err(io_error(&temp))?;
        let mut files = 0;
        let mut bytes = 0;
        copy_tree(source, &temp, "", &mut files, &mut bytes)?;
        let dest = dir.join(name);
        crate::fsx::rename(&temp, &dest).map_err(io_error(&dest))
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(&temp);
    }
    result
}

fn copy_tree(
    from: &Path,
    to_root: &Path,
    prefix: &str,
    files: &mut usize,
    bytes: &mut u64,
) -> Result<(), ContentError> {
    for entry in fs::read_dir(from).map_err(io_error(from))? {
        let entry = entry.map_err(io_error(from))?;
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            continue;
        };
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let Ok(safe) = safe_relative_path(&relative) else {
            continue;
        };
        let file_type = entry.file_type().map_err(io_error(&entry.path()))?;
        let dest = to_root.join(&safe);
        if file_type.is_dir() {
            fs::create_dir_all(&dest).map_err(io_error(&dest))?;
            copy_tree(&entry.path(), to_root, &relative, files, bytes)?;
        } else if file_type.is_file() {
            *files += 1;
            *bytes += entry.metadata().map(|m| m.len()).unwrap_or(0);
            if *files > MAX_FOLDER_FILES || *bytes > MAX_FOLDER_BYTES {
                return Err(ContentError::Invalid(
                    "The folder is too big to add.".into(),
                ));
            }
            fs::copy(entry.path(), &dest).map_err(io_error(&dest))?;
        }
    }
    Ok(())
}

/// The path of `file_name` in the content folder of `kind`, if it is a content name that
/// exists there (not following links).
pub fn existing(
    dirs: &InstanceDirs,
    kind: ContentKind,
    file_name: &str,
) -> Result<PathBuf, ContentError> {
    let path = dirs.content_dir(kind).join(file_name);
    let metadata = fs::symlink_metadata(&path)
        .map_err(|_| ContentError::NotFound(format!("{file_name} isn't in {}.", kind.dir())))?;
    if !is_content_name(kind, file_name, metadata.is_dir()) || metadata.file_type().is_symlink() {
        return Err(ContentError::Invalid(format!(
            "{file_name} can't be changed."
        )));
    }
    Ok(path)
}

/// Enables or disables a mod by renaming it to or from `<name>.jar.disabled`. Returns the
/// new name. Changing nothing is fine.
pub fn set_mod_enabled(
    dirs: &InstanceDirs,
    file_name: &str,
    enabled: bool,
) -> Result<String, ContentError> {
    let new_name = if enabled {
        enabled_name(file_name)
    } else {
        disabled_name(file_name)
    };
    if new_name == file_name {
        return Ok(new_name);
    }
    let path = match existing(dirs, ContentKind::Mod, file_name) {
        Ok(path) => path,
        // Already renamed, by an earlier click on a list not yet read again: it is in
        // the state asked for, which is what was wanted.
        Err(ContentError::NotFound(_)) if existing(dirs, ContentKind::Mod, &new_name).is_ok() => {
            return Ok(new_name);
        }
        Err(err) => return Err(err),
    };
    let new_path = dirs.content_dir(ContentKind::Mod).join(&new_name);
    if fs::symlink_metadata(&new_path).is_ok() {
        return Err(ContentError::Conflict(format!(
            "{new_name} already exists, so {file_name} can't be renamed to it."
        )));
    }
    crate::fsx::rename(&path, &new_path).map_err(io_error(&path))?;
    Ok(new_name)
}

/// Removes a file or folder with `remove` (the app moves it to the Trash), and forgets
/// it: its provenance, its place in `options.txt`, and the shader selection.
pub fn delete_with(
    dirs: &InstanceDirs,
    target: &Target,
    kind: ContentKind,
    file_name: &str,
    remove: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), ContentError> {
    let path = existing(dirs, kind, file_name)?;
    remove(&path).map_err(|reason| {
        ContentError::Invalid(format!("Couldn't remove {file_name}: {reason}"))
    })?;
    let _ = Provenance::update(&dirs.instance, |provenance| {
        provenance.remove(kind, file_name);
    });
    match kind {
        ContentKind::ResourcePack => {
            let listed = PackOptions::read(&dirs.game)
                .map(|packs| {
                    packs
                        .enabled_files(&target.game_version)
                        .iter()
                        .any(|n| n == file_name)
                })
                .unwrap_or(false);
            if listed {
                change_pack(
                    dirs,
                    target,
                    &PackChange::Disable {
                        name: file_name.to_string(),
                    },
                )?;
            }
        }
        ContentKind::ShaderPack => {
            if let Some(loader) = target.shader_loader
                && shaders::read_selection(&dirs.game, loader)?.pack.as_deref() == Some(file_name)
            {
                shaders::write_selection(&dirs.game, loader, None)?;
            }
        }
        ContentKind::Mod => {}
    }
    Ok(())
}

/// Moves a file or folder to the Trash, see [`delete_with`].
pub fn delete(
    dirs: &InstanceDirs,
    target: &Target,
    kind: ContentKind,
    file_name: &str,
) -> Result<(), ContentError> {
    delete_with(dirs, target, kind, file_name, |path| {
        trash::delete(path).map_err(|err| err.to_string())
    })
}

/// Changes the resource packs in `options.txt`.
pub fn change_pack(
    dirs: &InstanceDirs,
    target: &Target,
    change: &PackChange,
) -> Result<PackOptions, ContentError> {
    options::change_packs(&dirs.game, &target.game_version, change)
        .map_err(|err| ContentError::Invalid(err.to_string()))
}

/// Enables or disables a resource pack. `compatibility` is the pack's badge: one that
/// isn't known to fit is also listed as incompatible, so the game keeps it enabled.
pub fn set_pack_enabled(
    dirs: &InstanceDirs,
    target: &Target,
    file_name: &str,
    enabled: bool,
    compatibility: Compatibility,
) -> Result<PackOptions, ContentError> {
    existing(dirs, ContentKind::ResourcePack, file_name)?;
    let name = file_name.to_string();
    change_pack(
        dirs,
        target,
        &if enabled {
            PackChange::Enable {
                name,
                compatible: compatibility == Compatibility::Compatible,
            }
        } else {
            PackChange::Disable { name }
        },
    )
}

/// Makes the shader mod use the shader pack `file_name`, or no shaders.
pub fn select_shader(
    dirs: &InstanceDirs,
    loader: ShaderLoader,
    file_name: Option<&str>,
) -> Result<(), ContentError> {
    if let Some(name) = file_name {
        existing(dirs, ContentKind::ShaderPack, name)?;
    }
    shaders::write_selection(&dirs.game, loader, file_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::scan::tests::write_zip;

    fn setup(loader: Loader) -> (tempfile::TempDir, InstanceDirs, Target) {
        let dir = tempfile::tempdir().unwrap();
        let dirs = InstanceDirs::new(dir.path().join("instance"));
        fs::create_dir_all(&dirs.game).unwrap();
        (dir, dirs, Target::new("1.20.1", loader))
    }

    #[test]
    fn toggles_mods_with_the_disabled_suffix() {
        let (_dir, dirs, _) = setup(Loader::Fabric);
        let mods = dirs.content_dir(ContentKind::Mod);
        write_zip(&mods.join("sodium.jar"), &[("fabric.mod.json", b"{}")]);
        assert_eq!(
            set_mod_enabled(&dirs, "sodium.jar", false).unwrap(),
            "sodium.jar.disabled"
        );
        assert!(mods.join("sodium.jar.disabled").is_file());
        assert!(!mods.join("sodium.jar").exists());
        // idempotent
        assert_eq!(
            set_mod_enabled(&dirs, "sodium.jar.disabled", false).unwrap(),
            "sodium.jar.disabled"
        );
        assert_eq!(
            set_mod_enabled(&dirs, "sodium.jar.disabled", true).unwrap(),
            "sodium.jar"
        );
        // a conflict leaves both files alone
        write_zip(
            &mods.join("sodium.jar.disabled"),
            &[("fabric.mod.json", b"{}")],
        );
        assert!(matches!(
            set_mod_enabled(&dirs, "sodium.jar", false),
            Err(ContentError::Conflict(_))
        ));
        assert!(mods.join("sodium.jar").is_file());
        for bad in ["../sodium.jar", "missing.jar", "sodium.zip", ""] {
            assert!(set_mod_enabled(&dirs, bad, false).is_err(), "{bad}");
        }
    }

    #[test]
    fn lists_toggles_and_deletes_names_with_spaces() {
        let (_dir, dirs, target) = setup(Loader::Fabric);
        let mods = dirs.content_dir(ContentKind::Mod);
        write_zip(&mods.join(" leather.jar"), &[("fabric.mod.json", b"{}")]);
        let names = |dirs: &InstanceDirs| -> Vec<String> {
            crate::content::scan::scan(dirs, ContentKind::Mod)
                .unwrap()
                .into_iter()
                .map(|file| file.file_name)
                .collect()
        };
        assert_eq!(names(&dirs), [" leather.jar"]);
        assert_eq!(
            set_mod_enabled(&dirs, " leather.jar", false).unwrap(),
            " leather.jar.disabled"
        );
        assert!(mods.join(" leather.jar.disabled").is_file());
        assert_eq!(names(&dirs), [" leather.jar.disabled"]);
        assert_eq!(
            set_mod_enabled(&dirs, " leather.jar.disabled", true).unwrap(),
            " leather.jar"
        );
        // The trimmed name is a different file.
        assert!(set_mod_enabled(&dirs, "leather.jar", false).is_err());
        delete_with(&dirs, &target, ContentKind::Mod, " leather.jar", |path| {
            fs::remove_file(path).map_err(|e| e.to_string())
        })
        .unwrap();
        assert!(names(&dirs).is_empty());
    }

    #[test]
    fn checks_added_files() {
        let (dir, dirs, fabric) = setup(Loader::Fabric);
        let downloads = dir.path().join("Downloads");
        write_zip(
            &downloads.join("lithium.jar"),
            &[("fabric.mod.json", br#"{"id": "lithium"}"#)],
        );
        write_zip(
            &downloads.join("jei.jar"),
            &[("META-INF/mods.toml", b"[[mods]]\nmodId=\"jei\"\n")],
        );
        write_zip(&downloads.join("readme.jar"), &[("readme.txt", b"x")]);
        write_zip(
            &downloads.join("OptiFine.jar"),
            &[("net/optifine/Config.class", b"x")],
        );
        fs::write(downloads.join("fake.jar"), b"not a zip").unwrap();
        write_zip(
            &downloads.join("Pack.zip"),
            &[(
                "pack.mcmeta",
                br#"{"pack": {"pack_format": 15, "description": "x"}}"#,
            )],
        );
        write_zip(
            &downloads.join("Nested.zip"),
            &[(
                "Nested/pack.mcmeta",
                br#"{"pack": {"pack_format": 15, "description": "x"}}"#,
            )],
        );
        write_zip(&downloads.join("NoMeta.zip"), &[("assets/a.png", b"x")]);
        write_zip(
            &downloads.join("Shader.zip"),
            &[("shaders/final.fsh", b"x")],
        );
        fs::create_dir_all(downloads.join("Folder pack")).unwrap();
        fs::write(
            downloads.join("Folder pack/pack.mcmeta"),
            br#"{"pack": {"pack_format": 15, "description": "x"}}"#,
        )
        .unwrap();

        let check =
            |kind, name: &str, target: &Target| check_addition(kind, &downloads.join(name), target);
        assert_eq!(
            check(ContentKind::Mod, "lithium.jar", &fabric).unwrap(),
            "lithium.jar"
        );
        assert!(
            check(ContentKind::Mod, "jei.jar", &fabric)
                .unwrap_err()
                .contains("Forge mod")
        );
        assert!(check(ContentKind::Mod, "readme.jar", &fabric).is_err());
        assert!(check(ContentKind::Mod, "fake.jar", &fabric).is_err());
        assert!(check(ContentKind::Mod, "OptiFine.jar", &fabric).is_ok());
        assert!(check(ContentKind::Mod, "Pack.zip", &fabric).is_err());
        // NeoForge reads mods.toml before 1.20.5 only
        let neoforge = Target::new("1.20.4", Loader::NeoForge);
        assert!(check(ContentKind::Mod, "jei.jar", &neoforge).is_ok());
        let neoforge = Target::new("1.21.1", Loader::NeoForge);
        assert!(check(ContentKind::Mod, "jei.jar", &neoforge).is_err());
        let vanilla = Target::new("1.21.1", Loader::None);
        assert!(check(ContentKind::Mod, "lithium.jar", &vanilla).is_err());

        assert!(check(ContentKind::ResourcePack, "Pack.zip", &fabric).is_ok());
        assert!(check(ContentKind::ResourcePack, "Folder pack", &fabric).is_ok());
        assert!(
            check(ContentKind::ResourcePack, "Nested.zip", &fabric)
                .unwrap_err()
                .contains("inside a folder")
        );
        assert!(
            check(ContentKind::ResourcePack, "NoMeta.zip", &fabric)
                .unwrap_err()
                .contains("no pack.mcmeta")
        );
        assert!(check(ContentKind::ResourcePack, "lithium.jar", &fabric).is_err());
        assert!(check(ContentKind::ShaderPack, "Shader.zip", &fabric).is_ok());
        assert!(check(ContentKind::ShaderPack, "Pack.zip", &fabric).is_err());

        // adding copies the file; adding it again is a conflict
        assert_eq!(
            add(
                &dirs,
                &fabric,
                ContentKind::Mod,
                &downloads.join("lithium.jar")
            )
            .unwrap(),
            "lithium.jar"
        );
        assert!(
            dirs.content_dir(ContentKind::Mod)
                .join("lithium.jar")
                .is_file()
        );
        assert!(downloads.join("lithium.jar").is_file());
        assert!(matches!(
            add(
                &dirs,
                &fabric,
                ContentKind::Mod,
                &downloads.join("lithium.jar")
            ),
            Err(ContentError::Conflict(_))
        ));
        // a disabled copy counts as taken
        set_mod_enabled(&dirs, "lithium.jar", false).unwrap();
        assert!(
            add(
                &dirs,
                &fabric,
                ContentKind::Mod,
                &downloads.join("lithium.jar")
            )
            .is_err()
        );

        // folders are copied without their links
        #[cfg(unix)]
        std::os::unix::fs::symlink("/etc", downloads.join("Folder pack/etc")).unwrap();
        fs::create_dir_all(downloads.join("Folder pack/assets/minecraft")).unwrap();
        fs::write(downloads.join("Folder pack/assets/minecraft/a.json"), b"{}").unwrap();
        add(
            &dirs,
            &fabric,
            ContentKind::ResourcePack,
            &downloads.join("Folder pack"),
        )
        .unwrap();
        let copied = dirs
            .content_dir(ContentKind::ResourcePack)
            .join("Folder pack");
        assert!(copied.join("pack.mcmeta").is_file());
        assert!(copied.join("assets/minecraft/a.json").is_file());
        assert!(fs::symlink_metadata(copied.join("etc")).is_err());
        let leftovers: Vec<_> = fs::read_dir(dirs.content_dir(ContentKind::ResourcePack))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with('.'))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn delete_forgets_the_file() {
        let (_dir, dirs, mut target) = setup(Loader::Fabric);
        target.shader_loader = Some(ShaderLoader::Iris);
        write_zip(
            &dirs.content_dir(ContentKind::ResourcePack).join("Pack.zip"),
            &[(
                "pack.mcmeta",
                br#"{"pack": {"pack_format": 15, "description": "x"}}"#,
            )],
        );
        write_zip(
            &dirs.content_dir(ContentKind::ShaderPack).join("BSL.zip"),
            &[("shaders/a", b"x")],
        );
        set_pack_enabled(&dirs, &target, "Pack.zip", true, Compatibility::Compatible).unwrap();
        select_shader(&dirs, ShaderLoader::Iris, Some("BSL.zip")).unwrap();
        assert!(select_shader(&dirs, ShaderLoader::Iris, Some("Missing.zip")).is_err());
        let mut removed = Vec::new();
        delete_with(
            &dirs,
            &target,
            ContentKind::ResourcePack,
            "Pack.zip",
            |path| {
                removed.push(path.to_path_buf());
                fs::remove_file(path).map_err(|e| e.to_string())
            },
        )
        .unwrap();
        assert_eq!(
            removed,
            [dirs.content_dir(ContentKind::ResourcePack).join("Pack.zip")]
        );
        assert_eq!(
            fs::read_to_string(dirs.game.join("options.txt")).unwrap(),
            "resourcePacks:[\"vanilla\"]\n"
        );
        delete_with(&dirs, &target, ContentKind::ShaderPack, "BSL.zip", |path| {
            fs::remove_file(path).map_err(|e| e.to_string())
        })
        .unwrap();
        assert_eq!(
            shaders::read_selection(&dirs.game, ShaderLoader::Iris)
                .unwrap()
                .pack,
            None
        );
        assert!(
            delete_with(
                &dirs,
                &target,
                ContentKind::Mod,
                "../instance.json",
                |_| Ok(())
            )
            .is_err()
        );
    }
}
