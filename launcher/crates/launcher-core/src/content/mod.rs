//! An instance's content: mods (`mods/`), resource packs (`resourcepacks/`) and shader
//! packs (`shaderpacks/`), all in the instance's game folder.
//!
//! - [`scan`] lists what is there, with the metadata that is cheap to read;
//! - [`provenance`] remembers which Modrinth version each file came from (`content.json`);
//! - [`modrinth`] is the Modrinth API client, [`install`] installs versions from it with
//!   their dependencies, and [`files`] adds, enables, disables and deletes files;
//! - [`options`] edits the resource packs in `options.txt`, [`packs`] reads `pack.mcmeta`,
//!   and [`shaders`] finds the shader mod and edits its selected pack.
//!
//! Nothing here checks whether the game is running; the app refuses changes then, since
//! the game rewrites `options.txt` and the shader configs when it exits.

pub mod files;
pub mod install;
pub mod listing;
pub mod loader_change;
pub mod modrinth;
pub mod options;
pub mod packs;
pub mod provenance;
pub mod scan;
pub mod shaders;

use std::{
    io,
    path::{Path, PathBuf},
};

use lodestone_minecraft::modrinth::{ClientProjectType, RawVersion};

use crate::{game::download::DownloadError, instances::Loader};

pub use modrinth::{Modrinth, ModrinthError};
pub use shaders::ShaderLoader;

/// The kinds of content an instance holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ContentKind {
    Mod,
    ResourcePack,
    ShaderPack,
}

impl ContentKind {
    pub const ALL: [ContentKind; 3] = [
        ContentKind::Mod,
        ContentKind::ResourcePack,
        ContentKind::ShaderPack,
    ];

    /// The folder in the game folder.
    pub fn dir(self) -> &'static str {
        match self {
            ContentKind::Mod => "mods",
            ContentKind::ResourcePack => "resourcepacks",
            ContentKind::ShaderPack => "shaderpacks",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ContentKind::Mod => "Mods",
            ContentKind::ResourcePack => "Resource packs",
            ContentKind::ShaderPack => "Shaders",
        }
    }

    /// "mod", "resource pack" or "shader pack".
    pub fn noun(self) -> &'static str {
        match self {
            ContentKind::Mod => "mod",
            ContentKind::ResourcePack => "resource pack",
            ContentKind::ShaderPack => "shader pack",
        }
    }

    pub fn project_type(self) -> ClientProjectType {
        match self {
            ContentKind::Mod => ClientProjectType::Mod,
            ContentKind::ResourcePack => ClientProjectType::ResourcePack,
            ContentKind::ShaderPack => ClientProjectType::Shader,
        }
    }

    pub fn from_project_type(project_type: &str) -> Option<Self> {
        Some(match ClientProjectType::from_project_type(project_type)? {
            ClientProjectType::Mod => ContentKind::Mod,
            ClientProjectType::ResourcePack => ContentKind::ResourcePack,
            ClientProjectType::Shader => ContentKind::ShaderPack,
        })
    }

    /// The extension of the files Modrinth hands out for this kind.
    pub fn extension(self) -> &'static str {
        match self {
            ContentKind::Mod => ".jar",
            ContentKind::ResourcePack | ContentKind::ShaderPack => ".zip",
        }
    }

    /// Whether a folder (rather than an archive) can be this kind of content.
    pub fn allows_folders(self) -> bool {
        !matches!(self, ContentKind::Mod)
    }
}

/// The suffix of a disabled mod, the same convention as the dashboard and other launchers.
pub const DISABLED_SUFFIX: &str = ".disabled";

/// Files bigger than this are never downloaded or added.
pub const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;

/// Whether a mod file name is disabled (`.jar.disabled`, ignoring case).
pub fn is_disabled_name(name: &str) -> bool {
    name.to_ascii_lowercase().ends_with(".jar.disabled")
}

/// The name of a file without `.disabled`.
pub fn enabled_name(name: &str) -> String {
    if is_disabled_name(name) {
        name[..name.len() - DISABLED_SUFFIX.len()].to_string()
    } else {
        name.to_string()
    }
}

/// The name of a mod file with `.disabled`.
pub fn disabled_name(name: &str) -> String {
    if is_disabled_name(name) {
        name.to_string()
    } else {
        format!("{name}{DISABLED_SUFFIX}")
    }
}

/// Whether `name` is a single plain file or folder name: no separators, `..`, NUL, `:`,
/// control characters or leading dot, and not too long.
pub fn is_plain_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.contains(['/', '\\', ':', '\0'])
        && !name.contains("..")
        && !name.starts_with('.')
        && name.trim() == name
        && !name.chars().any(|c| c.is_control())
}

/// Whether `name` is a file name the content list shows and changes, for `kind`.
/// `is_dir` is whether it names a folder (packs only).
pub fn is_content_name(kind: ContentKind, name: &str, is_dir: bool) -> bool {
    if !is_plain_name(name) {
        return false;
    }
    let lower = name.to_ascii_lowercase();
    match kind {
        ContentKind::Mod => {
            !is_dir
                && (lower.ends_with(".jar") || lower.ends_with(".jar.disabled"))
                && enabled_name(name).len() > ".jar".len()
        }
        ContentKind::ResourcePack | ContentKind::ShaderPack => {
            is_dir || (lower.ends_with(".zip") && name.len() > ".zip".len())
        }
    }
}

/// A plain file name for a file Modrinth calls `name`, or `None` if none can be made: the
/// last path segment, without characters that can't be in file names, and with the
/// kind's extension.
pub fn safe_download_name(kind: ContentKind, name: &str) -> Option<String> {
    let base = name.rsplit(['/', '\\']).next().unwrap_or_default();
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && !matches!(c, ':' | '*' | '?' | '"' | '<' | '>' | '|'))
        .collect();
    let cleaned = cleaned.trim().trim_start_matches('.').trim().to_string();
    let lower = cleaned.to_ascii_lowercase();
    (lower.ends_with(kind.extension()) && is_content_name(kind, &cleaned, false)).then_some(cleaned)
}

/// What an instance accepts: its Minecraft version, loader and shader mod.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub game_version: String,
    pub loader: Loader,
    /// The shader mod installed, if any; it decides which shader packs fit.
    pub shader_loader: Option<ShaderLoader>,
}

impl Target {
    pub fn new(game_version: impl Into<String>, loader: Loader) -> Self {
        Self {
            game_version: game_version.into(),
            loader,
            shader_loader: None,
        }
    }

    /// The Modrinth loaders whose files fit, for `kind`. Quilt runs Fabric mods too.
    pub fn accepted_loaders(&self, kind: ContentKind) -> Vec<String> {
        let list: &[&str] = match kind {
            ContentKind::Mod => match self.loader {
                Loader::None => &[],
                Loader::Fabric => &["fabric"],
                Loader::Quilt => &["quilt", "fabric"],
                Loader::Forge => &["forge"],
                Loader::NeoForge => &["neoforge"],
            },
            ContentKind::ResourcePack => &["minecraft"],
            ContentKind::ShaderPack => match self.shader_loader {
                Some(ShaderLoader::OptiFine) => &["optifine"],
                // Iris (and so Oculus) reads OptiFine-format packs too.
                Some(ShaderLoader::Iris | ShaderLoader::Oculus) | None => &["iris", "optifine"],
            },
        };
        list.iter().map(|l| l.to_string()).collect()
    }

    /// The loaders to search with by default. For shader packs, only the instance's shader
    /// loader's own tag (`iris`, or `optifine`), since nearly every pack lists both.
    pub fn search_loaders(&self, kind: ContentKind) -> Vec<String> {
        match kind {
            ContentKind::ShaderPack => vec![match self.shader_loader {
                Some(ShaderLoader::OptiFine) => "optifine".to_string(),
                _ => "iris".to_string(),
            }],
            // Resource packs are all tagged `minecraft`; the filter adds nothing.
            ContentKind::ResourcePack => Vec::new(),
            ContentKind::Mod => self.accepted_loaders(kind),
        }
    }

    /// Why `version` doesn't fit this instance as `kind`, or `None` if it does.
    pub fn incompatibility(&self, kind: ContentKind, version: &RawVersion) -> Option<String> {
        fn list(values: &[String]) -> String {
            let mut shown: Vec<&str> = values.iter().take(5).map(String::as_str).collect();
            if values.len() > shown.len() {
                shown.push("…");
            }
            if shown.is_empty() {
                "nothing".to_string()
            } else {
                shown.join(", ")
            }
        }
        let accepted = self.accepted_loaders(kind);
        if kind == ContentKind::Mod && accepted.is_empty() {
            return Some("Mods need a mod loader, and this instance is vanilla".to_string());
        }
        if !version.loaders.iter().any(|l| accepted.contains(l)) {
            let uses = match kind {
                ContentKind::Mod => self.loader.label().to_string(),
                ContentKind::ShaderPack => self
                    .shader_loader
                    .map_or("Iris".to_string(), |l| l.label().to_string()),
                ContentKind::ResourcePack => "Minecraft".to_string(),
            };
            return Some(format!(
                "{} {} is for {}, but this instance uses {uses}",
                version.name,
                version.version_number,
                list(&version.loaders),
            ));
        }
        if !version.game_versions.contains(&self.game_version) {
            return Some(format!(
                "{} {} is for Minecraft {}, but this instance runs {}",
                version.name,
                version.version_number,
                list(&version.game_versions),
                self.game_version
            ));
        }
        None
    }
}

/// Where an instance's content lives.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstanceDirs {
    /// The instance's folder, with `instance.json` and `content.json`.
    pub instance: PathBuf,
    /// The game folder, `minecraft/`.
    pub game: PathBuf,
}

impl InstanceDirs {
    pub fn new(instance: impl Into<PathBuf>) -> Self {
        let instance = instance.into();
        let game = instance.join("minecraft");
        Self { instance, game }
    }

    pub fn content_dir(&self, kind: ContentKind) -> PathBuf {
        self.game.join(kind.dir())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(transparent)]
    Modrinth(#[from] ModrinthError),
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    NotFound(String),
    #[error(transparent)]
    Download(#[from] DownloadError),
}

pub(crate) fn io_error(path: &Path) -> impl FnOnce(io::Error) -> ContentError + '_ {
    move |source| ContentError::Io {
        path: path.to_owned(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert!(is_content_name(ContentKind::Mod, "sodium.jar", false));
        assert!(is_content_name(
            ContentKind::Mod,
            "Sodium.JAR.disabled",
            false
        ));
        assert!(!is_content_name(ContentKind::Mod, ".jar", false));
        assert!(!is_content_name(ContentKind::Mod, "a.zip", false));
        assert!(!is_content_name(ContentKind::Mod, "folder", true));
        assert!(is_content_name(
            ContentKind::ResourcePack,
            "Faithful 32x.zip",
            false
        ));
        assert!(is_content_name(ContentKind::ResourcePack, "My pack", true));
        assert!(is_content_name(
            ContentKind::ShaderPack,
            "BSL_v8.zip",
            false
        ));
        for bad in [
            "",
            "../x.zip",
            "a/b.zip",
            "a\\b.zip",
            ".hidden.zip",
            "c:x.zip",
            "a\0.zip",
            " x.zip",
            "line\nbreak.zip",
        ] {
            assert!(
                !is_content_name(ContentKind::ResourcePack, bad, false),
                "{bad:?}"
            );
        }
        assert_eq!(enabled_name("a.jar.disabled"), "a.jar");
        assert_eq!(disabled_name("a.jar"), "a.jar.disabled");
        assert_eq!(disabled_name("a.jar.disabled"), "a.jar.disabled");
    }

    #[test]
    fn download_names() {
        assert_eq!(
            safe_download_name(ContentKind::Mod, "sodium-fabric-0.5.13+mc1.20.1.jar").as_deref(),
            Some("sodium-fabric-0.5.13+mc1.20.1.jar")
        );
        assert_eq!(
            safe_download_name(ContentKind::Mod, "../../evil/../x.jar").as_deref(),
            Some("x.jar")
        );
        assert_eq!(
            safe_download_name(ContentKind::ShaderPack, "C:\\x\\BSL?.zip").as_deref(),
            Some("BSL.zip")
        );
        assert_eq!(safe_download_name(ContentKind::Mod, "x.zip"), None);
        assert_eq!(safe_download_name(ContentKind::Mod, "..jar"), None);
        assert_eq!(safe_download_name(ContentKind::ResourcePack, "..."), None);
        assert_eq!(
            safe_download_name(ContentKind::ResourcePack, "a..b.zip"),
            None
        );
    }

    fn version(loaders: &[&str], game_versions: &[&str]) -> RawVersion {
        serde_json::from_value(serde_json::json!({
            "id": "v", "project_id": "p", "name": "P", "version_number": "1.0",
            "version_type": "release", "loaders": loaders, "game_versions": game_versions,
        }))
        .unwrap()
    }

    #[test]
    fn compatibility() {
        let quilt = Target::new("1.20.1", Loader::Quilt);
        assert_eq!(
            quilt.incompatibility(ContentKind::Mod, &version(&["fabric"], &["1.20.1"])),
            None
        );
        let forge = Target::new("1.20.1", Loader::Forge);
        assert!(
            forge
                .incompatibility(ContentKind::Mod, &version(&["fabric"], &["1.20.1"]))
                .unwrap()
                .contains("is for fabric, but this instance uses Forge")
        );
        assert!(
            forge
                .incompatibility(ContentKind::Mod, &version(&["forge"], &["1.19.2"]))
                .unwrap()
                .contains("is for Minecraft 1.19.2, but this instance runs 1.20.1")
        );
        let vanilla = Target::new("1.21.1", Loader::None);
        assert!(
            vanilla
                .incompatibility(ContentKind::Mod, &version(&["fabric"], &["1.21.1"]))
                .is_some()
        );
        assert_eq!(
            vanilla.incompatibility(
                ContentKind::ResourcePack,
                &version(&["minecraft"], &["1.21.1"])
            ),
            None
        );
        let mut iris = Target::new("1.21.1", Loader::Fabric);
        assert_eq!(
            iris.incompatibility(
                ContentKind::ShaderPack,
                &version(&["optifine"], &["1.21.1"])
            ),
            None
        );
        iris.shader_loader = Some(ShaderLoader::OptiFine);
        assert!(
            iris.incompatibility(ContentKind::ShaderPack, &version(&["iris"], &["1.21.1"]))
                .is_some()
        );
        assert_eq!(iris.search_loaders(ContentKind::ShaderPack), ["optifine"]);
        iris.shader_loader = None;
        assert_eq!(iris.search_loaders(ContentKind::ShaderPack), ["iris"]);
        assert_eq!(quilt.search_loaders(ContentKind::Mod), ["quilt", "fabric"]);
    }
}
