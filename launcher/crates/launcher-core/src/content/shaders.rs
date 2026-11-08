//! Shader packs need a shader mod: Iris (Fabric, Quilt, and NeoForge on newer versions),
//! Oculus (its Forge and NeoForge port) or OptiFine. This module finds which one an
//! instance has, which one could be installed, and reads and writes the pack it uses.
//!
//! The selected pack lives in the shader mod's own config, a Java `.properties` file:
//! - Iris: `config/iris.properties`, `shaderPack=<file or folder in shaderpacks/>` (empty
//!   or missing for none) and `enableShaders=true|false`;
//! - Oculus: `config/oculus.properties`, the same keys;
//! - OptiFine: `optionsshaders.txt` in the game folder, `shaderPack=<name>`, where `OFF`
//!   is no shaders and `(internal)` the built-in ones.
//!
//! The files are edited with [`PropertiesDocument`], which keeps every other key, comment
//! and line as it was. OptiFine is never downloaded: it isn't on Modrinth and its licence
//! doesn't allow it.

use std::{fs, io, path::Path};

use lodestone_minecraft::{modrinth::RawVersion, properties::PropertiesDocument};

use super::{
    ContentError, ContentKind, Modrinth, ModrinthError, Target, install::pick_version, io_error,
    provenance::Provenance, scan::ScannedFile,
};
use crate::{instances::Loader, storage};

/// Iris's Modrinth project.
pub const IRIS_PROJECT: &str = "YL57xq9U";
/// Oculus's Modrinth project.
pub const OCULUS_PROJECT: &str = "GchcoXML";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ShaderLoader {
    Iris,
    Oculus,
    OptiFine,
}

impl ShaderLoader {
    pub fn label(self) -> &'static str {
        match self {
            ShaderLoader::Iris => "Iris",
            ShaderLoader::Oculus => "Oculus",
            ShaderLoader::OptiFine => "OptiFine",
        }
    }

    /// The config file that holds the selected pack, relative to the game folder.
    pub fn config_file(self) -> &'static str {
        match self {
            ShaderLoader::Iris => "config/iris.properties",
            ShaderLoader::Oculus => "config/oculus.properties",
            ShaderLoader::OptiFine => "optionsshaders.txt",
        }
    }

    /// The Modrinth project, for the mods that are on Modrinth.
    pub fn project(self) -> Option<&'static str> {
        match self {
            ShaderLoader::Iris => Some(IRIS_PROJECT),
            ShaderLoader::Oculus => Some(OCULUS_PROJECT),
            ShaderLoader::OptiFine => None,
        }
    }
}

/// The shader mod among the enabled mods, if any. Iris wins over Oculus, and both over
/// OptiFine, when several are there (the game would crash anyway).
pub fn detect(mods: &[ScannedFile], provenance: &Provenance) -> Option<ShaderLoader> {
    let mut found = Vec::new();
    for file in mods
        .iter()
        .filter(|f| f.kind == ContentKind::Mod && f.enabled_by_name())
    {
        let project = provenance
            .get(ContentKind::Mod, &file.file_name)
            .and_then(|e| e.modrinth.as_ref())
            .map(|m| m.project_id.as_str());
        let id = file.meta.id.as_deref();
        if id == Some("iris") || project == Some(IRIS_PROJECT) {
            found.push(ShaderLoader::Iris);
        } else if id == Some("oculus") || project == Some(OCULUS_PROJECT) {
            found.push(ShaderLoader::Oculus);
        } else if file.meta.optifine
            || (file.meta.mod_formats.is_empty()
                && file.file_name.to_ascii_lowercase().starts_with("optifine"))
        {
            found.push(ShaderLoader::OptiFine);
        }
    }
    [
        ShaderLoader::Iris,
        ShaderLoader::Oculus,
        ShaderLoader::OptiFine,
    ]
    .into_iter()
    .find(|loader| found.contains(loader))
}

/// The pack a shader mod uses.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ShaderSelection {
    /// The file or folder name in `shaderpacks/`, when shaders are on.
    pub pack: Option<String>,
    /// OptiFine's built-in shaders are on.
    pub internal: bool,
}

fn read_document(path: &Path) -> Result<PropertiesDocument, ContentError> {
    match fs::read(path) {
        Ok(bytes) => Ok(PropertiesDocument::from_bytes(&bytes).0),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(PropertiesDocument::default()),
        Err(err) => Err(io_error(path)(err)),
    }
}

/// The pack `loader` uses in `game_dir`.
pub fn read_selection(
    game_dir: &Path,
    loader: ShaderLoader,
) -> Result<ShaderSelection, ContentError> {
    let document = read_document(&game_dir.join(loader.config_file()))?;
    let pack = document
        .get("shaderPack")
        .map(str::trim)
        .unwrap_or_default();
    Ok(match loader {
        ShaderLoader::OptiFine => match pack {
            "" | "OFF" => ShaderSelection::default(),
            "(internal)" => ShaderSelection {
                pack: None,
                internal: true,
            },
            name => ShaderSelection {
                pack: Some(name.to_string()),
                internal: false,
            },
        },
        ShaderLoader::Iris | ShaderLoader::Oculus => {
            let enabled = document.get("enableShaders").map(str::trim) != Some("false");
            ShaderSelection {
                pack: (enabled && !pack.is_empty() && pack != "(internal)")
                    .then(|| pack.to_string()),
                internal: false,
            }
        }
    })
}

/// Makes `loader` use `pack` (a name in `shaderpacks/`), or no shaders for `None`. Only
/// the selection keys change; the file is created if it doesn't exist.
pub fn write_selection(
    game_dir: &Path,
    loader: ShaderLoader,
    pack: Option<&str>,
) -> Result<(), ContentError> {
    let path = game_dir.join(loader.config_file());
    let mut document = read_document(&path)?;
    match loader {
        ShaderLoader::OptiFine => document.set("shaderPack", pack.unwrap_or("OFF")),
        ShaderLoader::Iris | ShaderLoader::Oculus => match pack {
            Some(pack) => {
                document.set("shaderPack", pack);
                document.set("enableShaders", "true");
            }
            None => document.set("enableShaders", "false"),
        },
    }
    storage::write_atomic(&path, &document.to_bytes()).map_err(io_error(&path))
}

/// The shader mods that can be installed for a loader, in the order they are tried.
pub fn candidates(loader: Loader) -> &'static [ShaderLoader] {
    match loader {
        Loader::Fabric | Loader::Quilt => &[ShaderLoader::Iris],
        // Iris has NeoForge builds for newer versions; Oculus for older ones.
        Loader::NeoForge => &[ShaderLoader::Iris, ShaderLoader::Oculus],
        Loader::Forge => &[ShaderLoader::Oculus],
        Loader::None => &[],
    }
}

/// What the Shaders section can offer when the instance has no shader mod.
#[derive(Clone, Debug)]
pub enum ShaderModOffer {
    /// Install `loader` at `version`; its required mods (Sodium, Embeddium) come with it.
    Install {
        loader: ShaderLoader,
        version: Box<RawVersion>,
    },
    /// Shaders need a mod loader; the instance is vanilla.
    NeedsModLoader,
    /// No shader mod on Modrinth runs on this loader and version.
    Unavailable { reason: String },
}

/// Finds the shader mod to offer for `target`, asking Modrinth which of the
/// [`candidates`] has a build for the instance's loader and Minecraft version.
pub async fn find_shader_mod(
    api: &Modrinth,
    target: &Target,
) -> Result<ShaderModOffer, ContentError> {
    let candidates = candidates(target.loader);
    if candidates.is_empty() {
        return Ok(ShaderModOffer::NeedsModLoader);
    }
    let loaders = target.accepted_loaders(ContentKind::Mod);
    for &loader in candidates {
        let Some(project) = loader.project() else {
            continue;
        };
        let versions = match api
            .versions(
                project,
                &loaders,
                std::slice::from_ref(&target.game_version),
            )
            .await
        {
            Ok(versions) => versions,
            Err(ModrinthError::NotFound(_)) => continue,
            Err(err) => return Err(err.into()),
        };
        if let Some(version) = pick_version(target, ContentKind::Mod, versions) {
            return Ok(ShaderModOffer::Install {
                loader,
                version: Box::new(version),
            });
        }
    }
    let names: Vec<&str> = candidates.iter().map(|l| l.label()).collect();
    Ok(ShaderModOffer::Unavailable {
        reason: format!(
            "{} has no build for {} on Minecraft {}.",
            names.join(" or "),
            target.loader.label(),
            target.game_version
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{InstanceDirs, scan};

    #[test]
    fn iris_and_oculus_keep_unknown_keys() {
        for loader in [ShaderLoader::Iris, ShaderLoader::Oculus] {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join(loader.config_file());
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let original = "#This file stores configuration options for Iris\n#Sat Nov 08 12:00:00 CET 2026\ncolorSpace=SRGB\nenableShaders=true\nshaderPack=BSL_v8.2.09.zip\nmaxShadowRenderDistance=32\nsomeFutureKey=keep me\n";
            fs::write(&path, original).unwrap();
            assert_eq!(
                read_selection(dir.path(), loader).unwrap().pack.as_deref(),
                Some("BSL_v8.2.09.zip")
            );
            write_selection(dir.path(), loader, Some("Complementary Reimagined r5.zip")).unwrap();
            let text = fs::read_to_string(&path).unwrap();
            assert_eq!(
                text,
                original.replace(
                    "shaderPack=BSL_v8.2.09.zip",
                    "shaderPack=Complementary Reimagined r5.zip"
                )
            );
            write_selection(dir.path(), loader, None).unwrap();
            let text = fs::read_to_string(&path).unwrap();
            assert!(text.contains("enableShaders=false\n"));
            assert!(text.contains("someFutureKey=keep me\n"));
            assert!(text.contains("#This file stores configuration options for Iris\n"));
            assert_eq!(
                read_selection(dir.path(), loader).unwrap(),
                ShaderSelection::default()
            );
        }
    }

    #[test]
    fn iris_selection_is_created_and_escaped() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            read_selection(dir.path(), ShaderLoader::Iris).unwrap(),
            ShaderSelection::default()
        );
        write_selection(dir.path(), ShaderLoader::Iris, Some("Pack: ünï=1.zip")).unwrap();
        let text = fs::read_to_string(dir.path().join("config/iris.properties")).unwrap();
        assert_eq!(
            text,
            "shaderPack=Pack\\: \\u00FCn\\u00EF\\=1.zip\nenableShaders=true\n"
        );
        assert_eq!(
            read_selection(dir.path(), ShaderLoader::Iris)
                .unwrap()
                .pack
                .as_deref(),
            Some("Pack: ünï=1.zip")
        );
        // Iris treats a missing enableShaders as on, and an empty pack as none
        fs::write(dir.path().join("config/iris.properties"), "shaderPack=\n").unwrap();
        assert_eq!(
            read_selection(dir.path(), ShaderLoader::Iris).unwrap().pack,
            None
        );
    }

    #[test]
    fn optifine_selection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("optionsshaders.txt");
        fs::write(
            &path,
            "shaderPack=(internal)\nantialiasingLevel=0\noldLighting=default\n",
        )
        .unwrap();
        assert_eq!(
            read_selection(dir.path(), ShaderLoader::OptiFine).unwrap(),
            ShaderSelection {
                pack: None,
                internal: true
            }
        );
        write_selection(dir.path(), ShaderLoader::OptiFine, Some("SEUS.zip")).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "shaderPack=SEUS.zip\nantialiasingLevel=0\noldLighting=default\n"
        );
        write_selection(dir.path(), ShaderLoader::OptiFine, None).unwrap();
        assert_eq!(
            fs::read_to_string(&path).unwrap(),
            "shaderPack=OFF\nantialiasingLevel=0\noldLighting=default\n"
        );
        assert_eq!(
            read_selection(dir.path(), ShaderLoader::OptiFine).unwrap(),
            ShaderSelection::default()
        );
    }

    #[test]
    fn detects_the_shader_mod() {
        use crate::content::scan::tests::write_zip;
        let dir = tempfile::tempdir().unwrap();
        let dirs = InstanceDirs::new(dir.path());
        let mods = dirs.content_dir(ContentKind::Mod);
        let detect_now = || {
            let files = scan::scan(&dirs, ContentKind::Mod).unwrap();
            detect(&files, &Provenance::default())
        };
        write_zip(
            &mods.join("sodium.jar"),
            &[("fabric.mod.json", br#"{"id": "sodium"}"#)],
        );
        assert_eq!(detect_now(), None);
        write_zip(
            &mods.join("OptiFine_1.20.1_HD_U_I6.jar"),
            &[("net/optifine/Config.class", b"")],
        );
        assert_eq!(detect_now(), Some(ShaderLoader::OptiFine));
        write_zip(
            &mods.join("oculus.jar"),
            &[("META-INF/mods.toml", b"[[mods]]\nmodId=\"oculus\"\n")],
        );
        assert_eq!(detect_now(), Some(ShaderLoader::Oculus));
        write_zip(
            &mods.join("iris.jar.disabled"),
            &[("fabric.mod.json", br#"{"id": "iris"}"#)],
        );
        // a disabled Iris doesn't count
        assert_eq!(detect_now(), Some(ShaderLoader::Oculus));
        fs::rename(mods.join("iris.jar.disabled"), mods.join("iris.jar")).unwrap();
        assert_eq!(detect_now(), Some(ShaderLoader::Iris));
        // a jar Modrinth knows as Iris counts even without readable metadata
        let dir = tempfile::tempdir().unwrap();
        let dirs = InstanceDirs::new(dir.path());
        write_zip(
            &dirs.content_dir(ContentKind::Mod).join("x.jar"),
            &[("a", b"")],
        );
        let files = scan::scan(&dirs, ContentKind::Mod).unwrap();
        let mut provenance = Provenance::default();
        provenance.insert(
            ContentKind::Mod,
            "x.jar",
            crate::content::provenance::Entry {
                modrinth: Some(crate::content::provenance::ModrinthSource {
                    project_id: IRIS_PROJECT.into(),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        assert_eq!(detect(&files, &provenance), Some(ShaderLoader::Iris));
    }

    #[test]
    fn candidates_per_loader() {
        assert_eq!(candidates(Loader::Fabric), [ShaderLoader::Iris]);
        assert_eq!(candidates(Loader::Quilt), [ShaderLoader::Iris]);
        assert_eq!(
            candidates(Loader::NeoForge),
            [ShaderLoader::Iris, ShaderLoader::Oculus]
        );
        assert_eq!(candidates(Loader::Forge), [ShaderLoader::Oculus]);
        assert!(candidates(Loader::None).is_empty());
    }
}
