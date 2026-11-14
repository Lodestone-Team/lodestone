//! Which libraries a version needs on a platform: the jars on the classpath and the
//! jars whose native libraries are extracted before launch.
//!
//! Natives come in two formats:
//! - **old** (before 1.19): a `natives` map from OS to a classifier such as
//!   `natives-osx` or `natives-windows-${arch}`, a download under
//!   `downloads.classifiers`, and an `extract.exclude` list. These jars are extracted
//!   into the natives directory.
//! - **new** (1.19 and later): ordinary libraries named `...:natives-macos-arm64`
//!   with `os` rules. They go on the classpath, and LWJGL extracts them itself.
//!
//! On Apple silicon, versions from 1.13 to 1.18 list LWJGL 3.1 or 3.2, which Mojang
//! only built for Intel Macs. They run natively all the same: their LWJGL is swapped
//! for the arm64 LWJGL 3.3.1 that Mojang's own 1.19 and later versions use
//! ([`ARM64_LWJGL`]), and the installer hides the window icons from them, which that
//! GLFW refuses (see `Installer::hide_window_icons`). Versions before 1.13 use LWJGL 2,
//! which has no arm64 build from Mojang, and run in an Intel Java under Rosetta 2
//! instead (see [`java_arch`]), which the launcher tells the player.

use std::path::{Component, Path, PathBuf};

use super::{
    platform::{Arch, Os, Platform},
    rules::{self, Features},
    version::{Artifact, Library, VersionJson},
};

/// A file to download, with its hash and size.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub url: String,
    pub sha1: String,
    pub size: Option<u64>,
}

impl From<&Artifact> for Remote {
    fn from(artifact: &Artifact) -> Self {
        Self {
            url: artifact.url.clone(),
            sha1: artifact.sha1.clone(),
            size: artifact.size,
        }
    }
}

/// A library jar, at `path` under the libraries directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibraryFile {
    pub name: String,
    pub path: PathBuf,
    /// Where to get it. `None` for files a mod loader's installer provides; a missing
    /// one is an error at launch.
    pub remote: Option<Remote>,
    /// The expected SHA-1 of a file without a download, when the version file has it.
    pub sha1: Option<String>,
}

/// A jar of old-style natives to extract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativeJar {
    pub file: LibraryFile,
    /// Entry name prefixes not to extract, such as `META-INF/`.
    pub exclude: Vec<String>,
}

/// The libraries of a version on one platform.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Selection {
    /// In classpath order, without duplicates. The client jar is not included.
    pub classpath: Vec<LibraryFile>,
    pub natives: Vec<NativeJar>,
    /// Whether the version's LWJGL was swapped for Mojang's arm64 LWJGL 3.3.1
    /// ([`arm64_lwjgl`]), whose GLFW won't let it set a window icon.
    pub arm64_lwjgl: bool,
}

impl Selection {
    /// Every file to download: the classpath and the native jars.
    pub fn files(&self) -> impl Iterator<Item = &LibraryFile> {
        self.classpath
            .iter()
            .chain(self.natives.iter().map(|native| &native.file))
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LibraryError {
    #[error("the library name {0:?} is not a Maven coordinate")]
    BadName(String),
    #[error("the library path {0:?} is not a safe relative path")]
    UnsafePath(String),
}

/// Selects the libraries `version` needs on `platform`.
pub fn select(
    version: &VersionJson,
    platform: &Platform,
    features: &Features,
) -> Result<Selection, LibraryError> {
    let mut selection = Selection::default();
    for library in &version.libraries {
        if !rules::allows(&library.rules, platform, features) {
            continue;
        }
        if let Some(classifier) = native_classifier(library, platform) {
            // A natives entry for another OS, or one without a download for this
            // classifier, adds nothing on this platform.
            if let Some(artifact) = library.downloads.classifiers.get(&classifier) {
                let file = library_file(library, Some(artifact), Some(&classifier))?;
                if !selection.natives.iter().any(|n| n.file.path == file.path) {
                    selection.natives.push(NativeJar {
                        file,
                        exclude: library
                            .extract
                            .as_ref()
                            .map(|extract| extract.exclude.clone())
                            .unwrap_or_default(),
                    });
                }
            }
            if library.downloads.artifact.is_none() {
                continue;
            }
        } else if !library.natives.is_empty() {
            // Natives only for other OSes. Old files list the jar itself in another
            // entry, so there is nothing to put on the classpath here.
            if library.downloads.artifact.is_none() {
                continue;
            }
        }
        let file = library_file(library, library.downloads.artifact.as_ref(), None)?;
        if !selection.classpath.iter().any(|f| f.path == file.path) {
            selection.classpath.push(file);
        }
    }
    if platform.os == Os::MacOs {
        newer_macos_lwjgl(&mut selection)?;
        if platform.arch == Arch::Arm64 {
            arm64_lwjgl(&mut selection)?;
        }
    }
    Ok(selection)
}

/// A jar from Mojang's libraries host, with the path, SHA-1 and size its version
/// files give it.
struct MojangJar {
    name: &'static str,
    path: &'static str,
    sha1: &'static str,
    size: u64,
}

impl MojangJar {
    fn file(&self) -> Result<LibraryFile, LibraryError> {
        Ok(LibraryFile {
            name: self.name.to_string(),
            path: safe_relative_path(self.path)?,
            remote: Some(Remote {
                url: format!("https://libraries.minecraft.net/{}", self.path),
                sha1: self.sha1.to_string(),
                size: Some(self.size),
            }),
            sha1: None,
        })
    }
}

/// Mojang's LWJGL 3.3.1, the first LWJGL it built for Apple silicon: for each module,
/// its jar and its `natives-macos-arm64` jar. Copied from Minecraft 1.20.1's version
/// file on piston-meta.mojang.com (`testdata/versions/1.20.1.json`, which a test
/// checks this against); 1.19 to 1.20.2 list the same files. These are the only
/// LWJGL modules any Minecraft version uses.
const ARM64_LWJGL: [(&str, MojangJar, MojangJar); 7] = [
    (
        "lwjgl",
        MojangJar {
            name: "org.lwjgl:lwjgl:3.3.1",
            path: "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar",
            sha1: "ae58664f88e18a9bb2c77b063833ca7aaec484cb",
            size: 724_243,
        },
        MojangJar {
            name: "org.lwjgl:lwjgl:3.3.1:natives-macos-arm64",
            path: "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos-arm64.jar",
            sha1: "71d0d5e469c9c95351eb949064497e3391616ac9",
            size: 42_693,
        },
    ),
    (
        "lwjgl-glfw",
        MojangJar {
            name: "org.lwjgl:lwjgl-glfw:3.3.1",
            path: "org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1.jar",
            sha1: "cbac1b8d30cb4795149c1ef540f912671a8616d0",
            size: 128_801,
        },
        MojangJar {
            name: "org.lwjgl:lwjgl-glfw:3.3.1:natives-macos-arm64",
            path: "org/lwjgl/lwjgl-glfw/3.3.1/lwjgl-glfw-3.3.1-natives-macos-arm64.jar",
            sha1: "cac0d3f712a3da7641fa174735a5f315de7ffe0a",
            size: 129_077,
        },
    ),
    (
        "lwjgl-jemalloc",
        MojangJar {
            name: "org.lwjgl:lwjgl-jemalloc:3.3.1",
            path: "org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1.jar",
            sha1: "a817bcf213db49f710603677457567c37d53e103",
            size: 36_601,
        },
        MojangJar {
            name: "org.lwjgl:lwjgl-jemalloc:3.3.1:natives-macos-arm64",
            path: "org/lwjgl/lwjgl-jemalloc/3.3.1/lwjgl-jemalloc-3.3.1-natives-macos-arm64.jar",
            sha1: "e577b87d8ad2ade361aaea2fcf226c660b15dee8",
            size: 103_475,
        },
    ),
    (
        "lwjgl-openal",
        MojangJar {
            name: "org.lwjgl:lwjgl-openal:3.3.1",
            path: "org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1.jar",
            sha1: "2623a6b8ae1dfcd880738656a9f0243d2e6840bd",
            size: 88_237,
        },
        MojangJar {
            name: "org.lwjgl:lwjgl-openal:3.3.1:natives-macos-arm64",
            path: "org/lwjgl/lwjgl-openal/3.3.1/lwjgl-openal-3.3.1-natives-macos-arm64.jar",
            sha1: "23d55e7490b57495320f6c9e1936d78fd72c4ef8",
            size: 346_125,
        },
    ),
    (
        "lwjgl-opengl",
        MojangJar {
            name: "org.lwjgl:lwjgl-opengl:3.3.1",
            path: "org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1.jar",
            sha1: "831a5533a21a5f4f81bbc51bb13e9899319b5411",
            size: 921_563,
        },
        MojangJar {
            name: "org.lwjgl:lwjgl-opengl:3.3.1:natives-macos-arm64",
            path: "org/lwjgl/lwjgl-opengl/3.3.1/lwjgl-opengl-3.3.1-natives-macos-arm64.jar",
            sha1: "eafe34b871d966292e8db0f1f3d6b8b110d4e91d",
            size: 41_665,
        },
    ),
    (
        "lwjgl-stb",
        MojangJar {
            name: "org.lwjgl:lwjgl-stb:3.3.1",
            path: "org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1.jar",
            sha1: "b119297cf8ed01f247abe8685857f8e7fcf5980f",
            size: 112_380,
        },
        MojangJar {
            name: "org.lwjgl:lwjgl-stb:3.3.1:natives-macos-arm64",
            path: "org/lwjgl/lwjgl-stb/3.3.1/lwjgl-stb-3.3.1-natives-macos-arm64.jar",
            sha1: "fcf073ed911752abdca5f0b00a53cfdf17ff8e8b",
            size: 178_408,
        },
    ),
    (
        "lwjgl-tinyfd",
        MojangJar {
            name: "org.lwjgl:lwjgl-tinyfd:3.3.1",
            path: "org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1.jar",
            sha1: "0ff1914111ef2e3e0110ef2dabc8d8cdaad82347",
            size: 6_767,
        },
        MojangJar {
            name: "org.lwjgl:lwjgl-tinyfd:3.3.1:natives-macos-arm64",
            path: "org/lwjgl/lwjgl-tinyfd/3.3.1/lwjgl-tinyfd-3.3.1-natives-macos-arm64.jar",
            sha1: "972ecc17bad3571e81162153077b4d47b7b9eaa9",
            size: 41_380,
        },
    ),
];

/// Mojang's java-objc-bridge 1.1, whose `libjcocoa.dylib` is built for both Intel and
/// Apple silicon, from the same 1.20.1 version file. 1.13 to 1.18 list 1.0.0, whose
/// library is Intel only; the narrator uses it. 1.1 is built for Java 11, so on the
/// Java 8 of 1.13 to 1.16 the narrator stays off (as it would with 1.0.0, and with
/// their JNA 4.4.0, which is Intel only too); on 1.17 and 1.18 it works.
const UNIVERSAL_OBJC_BRIDGE: MojangJar = MojangJar {
    name: "ca.weblite:java-objc-bridge:1.1",
    path: "ca/weblite/java-objc-bridge/1.1/java-objc-bridge-1.1.jar",
    sha1: "1227f9e0666314f9de41477e3ec277e542ed7f7b",
    size: 1_330_045,
};

/// The module of an LWJGL 3 library from before 3.3 (`org.lwjgl:lwjgl-glfw:3.2.1`,
/// with or without a classifier), which has no arm64 macOS natives.
fn intel_only_lwjgl3(name: &str) -> Option<&str> {
    let mut parts = name.split(':');
    let (Some("org.lwjgl"), Some(module), Some(version)) =
        (parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let mut numbers = version
        .split(['.', '-'])
        .map(|part| part.parse::<u32>().ok());
    match (numbers.next(), numbers.next()) {
        (Some(Some(3)), Some(Some(minor))) if minor < 3 => Some(module),
        _ => None,
    }
}

fn arm64_lwjgl_module(module: &str) -> Option<&'static (&'static str, MojangJar, MojangJar)> {
    ARM64_LWJGL.iter().find(|(known, ..)| *known == module)
}

/// Whether every LWJGL library among `names` is LWJGL 3 before 3.3 in a module Mojang
/// built 3.3.1 for, and there is at least one: the libraries [`arm64_lwjgl`] swaps.
fn swappable_lwjgl<'a>(names: impl IntoIterator<Item = &'a str>) -> bool {
    let mut any = false;
    for name in names {
        if let Some(module) = intel_only_lwjgl3(name) {
            if arm64_lwjgl_module(module).is_none() {
                return false;
            }
            any = true;
        }
    }
    any
}

/// On Apple silicon, swaps LWJGL 3.1 and 3.2 (1.13 to 1.18, and loaders built on
/// them), which Mojang only built for Intel Macs, for Mojang's arm64 LWJGL 3.3.1: each
/// jar for 3.3.1's, followed by its `natives-macos-arm64` jar, which LWJGL loads from
/// the classpath as in 1.19 and later. Their Intel natives are no longer extracted.
/// java-objc-bridge 1.0.0 becomes Mojang's universal 1.1 likewise.
///
/// It runs on the final list, after a loader's profile is merged in, so a loader
/// that lists LWJGL itself is swapped too. When the list has an LWJGL module Mojang
/// made no 3.3.1 of, nothing is swapped: [`java_arch`] then picks an Intel Java.
fn arm64_lwjgl(selection: &mut Selection) -> Result<(), LibraryError> {
    let objc_bridge_1_0 = |name: &str| name.starts_with("ca.weblite:java-objc-bridge:1.0");
    let lwjgl = swappable_lwjgl(selection.files().map(|file| file.name.as_str()));
    let swapped =
        |name: &str| (lwjgl && intel_only_lwjgl3(name).is_some()) || objc_bridge_1_0(name);
    selection
        .natives
        .retain(|native| !swapped(&native.file.name));
    let mut classpath: Vec<LibraryFile> = Vec::with_capacity(selection.classpath.len() + 8);
    let mut push = |file: LibraryFile| {
        if !classpath.iter().any(|f| f.path == file.path) {
            classpath.push(file);
        }
    };
    for file in std::mem::take(&mut selection.classpath) {
        if objc_bridge_1_0(&file.name) {
            push(UNIVERSAL_OBJC_BRIDGE.file()?);
        } else if let Some((_, jar, natives)) = intel_only_lwjgl3(&file.name)
            .filter(|_| lwjgl)
            .and_then(arm64_lwjgl_module)
        {
            push(jar.file()?);
            push(natives.file()?);
        } else {
            push(file);
        }
    }
    selection.classpath = classpath;
    selection.arm64_lwjgl = lwjgl;
    Ok(())
}

/// The LWJGL 2 build Mojang ships for macOS from 1.8 on, with the hashes and sizes
/// from Mojang's own version files: `(artifact, file name suffix, sha1, size)`.
const MACOS_LWJGL: &str = "2.9.2-nightly-20140822";
const MACOS_LWJGL_FILES: [(&str, &str, &str, u64); 3] = [
    (
        "lwjgl",
        "",
        "7707204c9ffa5d91662de95f0a224e2f721b22af",
        1_045_632,
    ),
    (
        "lwjgl_util",
        "",
        "f0e612c840a7639c1f77f68d72a28dae2f0c8490",
        173_887,
    ),
    (
        "lwjgl-platform",
        "-natives-osx",
        "79f5ce2fea02e77fe47a3c745219167a542121d7",
        468_116,
    ),
];

/// Versions before 1.8 (and loaders built on them) use LWJGL 2.9.1 or older on every
/// OS, and on current macOS those abort in AppKit as the window opens. Like Prism
/// and MultiMC, use the macOS build Mojang switched to in 1.8 instead.
fn newer_macos_lwjgl(selection: &mut Selection) -> Result<(), LibraryError> {
    let replacement = |name: &str| -> Option<Result<LibraryFile, LibraryError>> {
        let mut parts = name.split(':');
        let (Some("org.lwjgl.lwjgl"), Some(artifact), Some(version)) =
            (parts.next(), parts.next(), parts.next())
        else {
            return None;
        };
        if !older_than_2_9_2(version) {
            return None;
        }
        let (_, suffix, sha1, size) = MACOS_LWJGL_FILES
            .iter()
            .find(|(known, ..)| *known == artifact)?;
        let path = format!(
            "org/lwjgl/lwjgl/{artifact}/{MACOS_LWJGL}/{artifact}-{MACOS_LWJGL}{suffix}.jar"
        );
        Some(safe_relative_path(&path).map(|relative| LibraryFile {
            name: format!("org.lwjgl.lwjgl:{artifact}:{MACOS_LWJGL}"),
            path: relative,
            remote: Some(Remote {
                url: format!("https://libraries.minecraft.net/{path}"),
                sha1: (*sha1).to_string(),
                size: Some(*size),
            }),
            sha1: None,
        }))
    };
    for file in &mut selection.classpath {
        if let Some(newer) = replacement(&file.name) {
            *file = newer?;
        }
    }
    for native in &mut selection.natives {
        if let Some(newer) = replacement(&native.file.name) {
            native.file = newer?;
        }
    }
    Ok(())
}

/// Whether an LWJGL version such as `2.9.1` or `2.9.1-nightly-20131120` is a 2.x
/// release before 2.9.2.
fn older_than_2_9_2(version: &str) -> bool {
    let mut numbers = version
        .split(['.', '-'])
        .map(|part| part.parse::<u32>().ok());
    match (numbers.next(), numbers.next(), numbers.next()) {
        (Some(Some(2)), Some(Some(minor)), patch) => (minor, patch.flatten().unwrap_or(0)) < (9, 2),
        _ => false,
    }
}

/// The old-style natives classifier of `library` on `platform`, if it has one.
fn native_classifier(library: &Library, platform: &Platform) -> Option<String> {
    let classifier = library.natives.get(platform.os.rule_name())?;
    Some(classifier.replace("${arch}", platform.arch.bits()))
}

fn library_file(
    library: &Library,
    artifact: Option<&Artifact>,
    classifier: Option<&str>,
) -> Result<LibraryFile, LibraryError> {
    let path = match artifact.and_then(|artifact| artifact.path.as_deref()) {
        Some(path) => path.to_string(),
        None => maven_path(&library.name, classifier)?,
    };
    Ok(LibraryFile {
        name: library.name.clone(),
        path: safe_relative_path(&path)?,
        remote: artifact
            .filter(|artifact| !artifact.url.is_empty())
            .map(Remote::from),
        sha1: artifact
            .filter(|artifact| artifact.url.is_empty() && !artifact.sha1.is_empty())
            .map(|artifact| artifact.sha1.clone()),
    })
}

/// The repository path of a Maven coordinate `group:artifact:version[:classifier][@ext]`,
/// with `classifier` overriding the one in the name.
pub fn maven_path(name: &str, classifier: Option<&str>) -> Result<String, LibraryError> {
    let bad = || LibraryError::BadName(name.to_string());
    let (coordinate, extension) = match name.split_once('@') {
        Some((coordinate, extension)) => (coordinate, extension),
        None => (name, "jar"),
    };
    let parts: Vec<&str> = coordinate.split(':').collect();
    let (group, artifact, version, named_classifier) = match parts.as_slice() {
        [group, artifact, version] => (*group, *artifact, *version, None),
        [group, artifact, version, classifier] => (*group, *artifact, *version, Some(*classifier)),
        _ => return Err(bad()),
    };
    if [group, artifact, version, extension]
        .iter()
        .any(|part| part.is_empty())
    {
        return Err(bad());
    }
    let classifier = classifier.or(named_classifier);
    let file = match classifier {
        Some(classifier) => format!("{artifact}-{version}-{classifier}.{extension}"),
        None => format!("{artifact}-{version}.{extension}"),
    };
    Ok(format!(
        "{}/{artifact}/{version}/{file}",
        group.replace('.', "/")
    ))
}

/// `path` as a relative path with only normal components, or an error. Paths from
/// version and manifest files are joined onto the launcher's directories, so they
/// must not be absolute or climb out with `..`.
pub fn safe_relative_path(path: &str) -> Result<PathBuf, LibraryError> {
    let unsafe_path = || LibraryError::UnsafePath(path.to_string());
    if path.is_empty() || path.contains('\\') || path.contains('\0') {
        return Err(unsafe_path());
    }
    let parsed = Path::new(path);
    if !parsed
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(unsafe_path());
    }
    Ok(parsed.to_path_buf())
}

/// Whether `version` has native libraries for Apple silicon in its own file. Versions
/// before 1.19 only have x86_64 natives on macOS; see [`swaps_to_arm64_lwjgl`] for
/// how 1.13 to 1.18 run natively all the same.
pub fn has_arm64_natives(version: &VersionJson, platform: &Platform) -> bool {
    let arm64 = platform.with_arch(Arch::Arm64);
    let features = Features::default();
    version.libraries.iter().any(|library| {
        if !rules::allows(&library.rules, &arm64, &features) {
            return false;
        }
        let new_style = library.name.rsplit(':').next().is_some_and(|classifier| {
            classifier.starts_with("natives-") && classifier.contains("arm64")
        });
        let old_style = native_classifier(library, &arm64)
            .is_some_and(|classifier| classifier.contains("arm64"));
        new_style || old_style
    })
}

/// Whether `version`, on an Apple silicon Mac, uses LWJGL 3 from before 3.3 that
/// [`select`] swaps for Mojang's arm64 LWJGL 3.3.1: 1.13 to 1.18, and loaders built
/// on them.
pub fn swaps_to_arm64_lwjgl(version: &VersionJson, platform: &Platform) -> bool {
    let arm64 = platform.with_arch(Arch::Arm64);
    let features = Features::default();
    swappable_lwjgl(
        version
            .libraries
            .iter()
            .filter(|library| rules::allows(&library.rules, &arm64, &features))
            .map(|library| library.name.as_str()),
    )
}

/// The architecture of the Java to run `version` with on `platform`: the machine's
/// own, except on Apple silicon for versions that have no arm64 natives and whose
/// LWJGL can't be swapped for an arm64 one. Those are the versions before 1.13 (LWJGL
/// 2), which run in an Intel Java under Rosetta 2. The launcher says so; it never
/// picks Rosetta silently.
pub fn java_arch(version: &VersionJson, platform: &Platform) -> Arch {
    if platform.os == super::platform::Os::MacOs
        && platform.arch == Arch::Arm64
        && !has_arm64_natives(version, platform)
        && !swaps_to_arm64_lwjgl(version, platform)
    {
        Arch::X86_64
    } else {
        platform.arch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::{platform::Os, version::tests::fixture};

    #[test]
    fn old_lwjgl_is_replaced_by_mojangs_macos_build_on_macos_only() {
        for id in ["1.7.10", "1.5.2"] {
            let version = fixture(id);
            let on_mac = select(&version, &mac(Arch::X86_64), &Features::default()).unwrap();
            let lwjgl: Vec<&str> = names(&on_mac.classpath)
                .into_iter()
                .filter(|name| name.starts_with("org.lwjgl.lwjgl:"))
                .collect();
            assert_eq!(
                lwjgl,
                [
                    "org.lwjgl.lwjgl:lwjgl:2.9.2-nightly-20140822",
                    "org.lwjgl.lwjgl:lwjgl_util:2.9.2-nightly-20140822",
                ],
                "{id}"
            );
            let natives = native_paths(&on_mac);
            assert!(
                natives.iter().any(|path| path
                    == "org/lwjgl/lwjgl/lwjgl-platform/2.9.2-nightly-20140822/lwjgl-platform-2.9.2-nightly-20140822-natives-osx.jar"),
                "{id}: {natives:?}"
            );
            assert!(
                !natives
                    .iter()
                    .any(|path| path.contains("/2.9.1/") || path.contains("/2.9.0/"))
            );
            let remote = on_mac
                .classpath
                .iter()
                .find(|f| f.name.ends_with(":lwjgl:2.9.2-nightly-20140822"))
                .unwrap()
                .remote
                .clone()
                .unwrap();
            assert_eq!(remote.sha1, "7707204c9ffa5d91662de95f0a224e2f721b22af");
            assert_eq!(
                remote.url,
                "https://libraries.minecraft.net/org/lwjgl/lwjgl/lwjgl/2.9.2-nightly-20140822/lwjgl-2.9.2-nightly-20140822.jar"
            );

            let on_windows = select(
                &version,
                &Platform::new(Os::Windows, Arch::X86_64, "10.0"),
                &Features::default(),
            )
            .unwrap();
            assert!(
                names(&on_windows.classpath)
                    .iter()
                    .all(|name| !name.contains("2.9.2-nightly")),
                "{id}"
            );
        }
        // 1.8.9 already lists the macOS build, and keeps it.
        let version = fixture("1.8.9");
        let on_mac = select(&version, &mac(Arch::X86_64), &Features::default()).unwrap();
        assert!(names(&on_mac.classpath).contains(&"org.lwjgl.lwjgl:lwjgl:2.9.2-nightly-20140822"));
    }

    #[test]
    fn lwjgl_versions_before_2_9_2() {
        for old in ["2.9.1", "2.9.0", "2.9.1-nightly-20131120", "2.8.5"] {
            assert!(older_than_2_9_2(old), "{old}");
        }
        for new in [
            "2.9.2-nightly-20140822",
            "2.9.4-nightly-20150209",
            "3.3.1",
            "2.10.0",
            "nightly",
        ] {
            assert!(!older_than_2_9_2(new), "{new}");
        }
    }

    fn mac(arch: Arch) -> Platform {
        Platform::new(Os::MacOs, arch, "14.6.1")
    }

    fn names(files: &[LibraryFile]) -> Vec<&str> {
        files.iter().map(|f| f.name.as_str()).collect()
    }

    fn native_paths(selection: &Selection) -> Vec<String> {
        selection
            .natives
            .iter()
            .map(|n| n.file.path.display().to_string())
            .collect()
    }

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.1", None).unwrap(),
            "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl:lwjgl:3.3.1:natives-macos-arm64", None).unwrap(),
            "org/lwjgl/lwjgl/3.3.1/lwjgl-3.3.1-natives-macos-arm64.jar"
        );
        assert_eq!(
            maven_path("org.lwjgl.lwjgl:lwjgl-platform:2.9.2", Some("natives-osx")).unwrap(),
            "org/lwjgl/lwjgl/lwjgl-platform/2.9.2/lwjgl-platform-2.9.2-natives-osx.jar"
        );
        assert_eq!(
            maven_path("net.minecraft:launchwrapper:1.5@zip", None).unwrap(),
            "net/minecraft/launchwrapper/1.5/launchwrapper-1.5.zip"
        );
        assert!(maven_path("nonsense", None).is_err());
        assert!(maven_path("a::1", None).is_err());
    }

    #[test]
    fn rejects_unsafe_paths() {
        for bad in [
            "/etc/passwd",
            "../x.jar",
            "a/../../b.jar",
            "a\\b.jar",
            "",
            "./a",
        ] {
            assert!(safe_relative_path(bad).is_err(), "{bad}");
        }
        assert!(safe_relative_path("org/lwjgl/lwjgl.jar").is_ok());
    }

    /// 1.8.9: LWJGL 2.9.2 for macOS (2.9.4 is disallowed there), old-style natives,
    /// and no arm64 natives at all.
    #[test]
    fn selects_1_8_9() {
        let version = fixture("1.8.9");
        for arch in [Arch::X86_64, Arch::Arm64] {
            let selection = select(&version, &mac(arch), &Features::default()).unwrap();
            let classpath = names(&selection.classpath);
            assert!(classpath.contains(&"org.lwjgl.lwjgl:lwjgl:2.9.2-nightly-20140822"));
            assert!(!classpath.contains(&"org.lwjgl.lwjgl:lwjgl:2.9.4-nightly-20150209"));
            assert!(!classpath.iter().any(|name| name.contains("-platform:")));
            assert_eq!(
                native_paths(&selection),
                [
                    "org/lwjgl/lwjgl/lwjgl-platform/2.9.2-nightly-20140822/lwjgl-platform-2.9.2-nightly-20140822-natives-osx.jar",
                    "net/java/jinput/jinput-platform/2.0.5/jinput-platform-2.0.5-natives-osx.jar",
                    "tv/twitch/twitch-platform/6.5/twitch-platform-6.5-natives-osx.jar",
                ]
            );
            assert!(selection.natives.iter().all(|n| n.exclude == ["META-INF/"]));
            assert!(selection.files().all(|f| f.remote.is_some()));
        }
        assert!(!has_arm64_natives(&version, &mac(Arch::Arm64)));
        // LWJGL 2: no arm64 build from Mojang, so an Intel Java under Rosetta.
        assert!(!swaps_to_arm64_lwjgl(&version, &mac(Arch::Arm64)));
        assert_eq!(java_arch(&version, &mac(Arch::Arm64)), Arch::X86_64);
        assert_eq!(java_arch(&version, &mac(Arch::X86_64)), Arch::X86_64);
    }

    #[test]
    fn selects_1_12_2() {
        let version = fixture("1.12.2");
        let selection = select(&version, &mac(Arch::X86_64), &Features::default()).unwrap();
        // The objc bridge appears twice: as natives and as a plain jar.
        assert!(
            native_paths(&selection)
                .iter()
                .any(|p| p.ends_with("java-objc-bridge-1.0.0-natives-osx.jar"))
        );
        assert_eq!(
            names(&selection.classpath)
                .iter()
                .filter(|n| n.starts_with("ca.weblite:java-objc-bridge"))
                .count(),
            1
        );
        // text2speech has no macOS natives.
        assert!(
            !native_paths(&selection)
                .iter()
                .any(|p| p.contains("text2speech"))
        );
        // LWJGL 2: an Intel Java and Intel natives under Rosetta on Apple silicon.
        assert!(!swaps_to_arm64_lwjgl(&version, &mac(Arch::Arm64)));
        assert_eq!(java_arch(&version, &mac(Arch::Arm64)), Arch::X86_64);
        assert!(
            native_paths(&selection)
                .iter()
                .any(|p| p.ends_with("lwjgl-platform-2.9.2-nightly-20140822-natives-osx.jar"))
        );
    }

    /// 1.16.5: LWJGL 3.2.1 for macOS with old-style `natives-macos` (x86_64 only). On an
    /// Intel Mac it is used as listed.
    #[test]
    fn selects_1_16_5_on_intel() {
        let version = fixture("1.16.5");
        let selection = select(&version, &mac(Arch::X86_64), &Features::default()).unwrap();
        let classpath = names(&selection.classpath);
        assert!(classpath.contains(&"org.lwjgl:lwjgl:3.2.1"));
        assert!(!classpath.contains(&"org.lwjgl:lwjgl:3.2.2"));
        // Listed twice (with and without natives), on the classpath once.
        assert_eq!(
            classpath
                .iter()
                .filter(|n| **n == "org.lwjgl:lwjgl:3.2.1")
                .count(),
            1
        );
        assert!(classpath.contains(&"ca.weblite:java-objc-bridge:1.0.0"));
        let natives = native_paths(&selection);
        assert!(
            natives.contains(&"org/lwjgl/lwjgl/3.2.1/lwjgl-3.2.1-natives-macos.jar".to_string())
        );
        assert_eq!(natives.len(), 8, "{natives:#?}");
        assert!(!has_arm64_natives(&version, &mac(Arch::Arm64)));
        assert_eq!(java_arch(&version, &mac(Arch::X86_64)), Arch::X86_64);
    }

    /// The LWJGL libraries of a selection, and its natives.
    fn lwjgl(selection: &Selection) -> (Vec<&str>, Vec<String>) {
        (
            names(&selection.classpath)
                .into_iter()
                .filter(|name| name.starts_with("org.lwjgl"))
                .collect(),
            native_paths(selection),
        )
    }

    /// What 1.13 to 1.18 select on Apple silicon: Mojang's arm64 LWJGL 3.3.1 in place of
    /// their Intel-only LWJGL, each jar with its arm64 natives on the classpath, nothing
    /// of LWJGL left to extract, and the universal objc bridge.
    fn assert_swapped(id: &str, selection: &Selection, modules: &[&str]) {
        let (lwjgl, natives) = lwjgl(selection);
        let expected: Vec<String> = modules
            .iter()
            .flat_map(|module| {
                [
                    format!("org.lwjgl:{module}:3.3.1"),
                    format!("org.lwjgl:{module}:3.3.1:natives-macos-arm64"),
                ]
            })
            .collect();
        let mut sorted = lwjgl.clone();
        sorted.sort();
        let mut expected_sorted: Vec<&str> = expected.iter().map(String::as_str).collect();
        expected_sorted.sort();
        assert_eq!(sorted, expected_sorted, "{id}");
        assert!(
            natives
                .iter()
                .all(|path| !path.contains("lwjgl") && !path.contains("objc")),
            "{id}: {natives:?}"
        );
        let classpath = names(&selection.classpath);
        assert!(
            classpath.contains(&"ca.weblite:java-objc-bridge:1.1"),
            "{id}"
        );
        assert!(
            !classpath.iter().any(|n| n.contains("java-objc-bridge:1.0")),
            "{id}"
        );
        for file in selection.files().filter(|f| f.name.contains(":3.3.1")) {
            let remote = file.remote.as_ref().unwrap();
            assert!(
                remote
                    .url
                    .starts_with("https://libraries.minecraft.net/org/lwjgl/"),
                "{remote:?}"
            );
            assert_eq!(remote.sha1.len(), 40);
            assert!(remote.size.is_some());
        }
        // Every path is unique.
        let mut paths: Vec<_> = selection.classpath.iter().map(|f| &f.path).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), selection.classpath.len(), "{id}");
    }

    const ALL_MODULES: [&str; 7] = [
        "lwjgl",
        "lwjgl-glfw",
        "lwjgl-jemalloc",
        "lwjgl-openal",
        "lwjgl-opengl",
        "lwjgl-stb",
        "lwjgl-tinyfd",
    ];

    /// 1.16.5 runs natively on Apple silicon: arm64 Java and Mojang's arm64 LWJGL 3.3.1.
    #[test]
    fn selects_1_16_5_on_apple_silicon() {
        let version = fixture("1.16.5");
        assert!(swaps_to_arm64_lwjgl(&version, &mac(Arch::Arm64)));
        assert_eq!(java_arch(&version, &mac(Arch::Arm64)), Arch::Arm64);
        let selection = select(&version, &mac(Arch::Arm64), &Features::default()).unwrap();
        assert_swapped("1.16.5", &selection, &ALL_MODULES);
        assert!(selection.arm64_lwjgl);
        // Its 8 Intel natives were LWJGL's 7 and the objc bridge's: nothing is left to
        // extract.
        assert!(
            selection.natives.is_empty(),
            "{:?}",
            native_paths(&selection)
        );
        // The swapped jars keep their place: LWJGL comes where 3.2.1 did.
        let classpath = names(&selection.classpath);
        let first_lwjgl = classpath
            .iter()
            .position(|n| n.starts_with("org.lwjgl"))
            .unwrap();
        let intel = select(&version, &mac(Arch::X86_64), &Features::default()).unwrap();
        let before = names(&intel.classpath);
        assert_eq!(
            classpath[..first_lwjgl],
            before[..before
                .iter()
                .position(|n| n.starts_with("org.lwjgl"))
                .unwrap()]
        );
    }

    /// 1.13.2 (LWJGL 3.1.6, no tinyfd) runs natively too.
    #[test]
    fn selects_1_13_2_on_apple_silicon() {
        let version = fixture("1.13.2");
        assert_eq!(java_arch(&version, &mac(Arch::Arm64)), Arch::Arm64);
        let selection = select(&version, &mac(Arch::Arm64), &Features::default()).unwrap();
        assert_swapped(
            "1.13.2",
            &selection,
            &[
                "lwjgl",
                "lwjgl-glfw",
                "lwjgl-jemalloc",
                "lwjgl-openal",
                "lwjgl-opengl",
                "lwjgl-stb",
            ],
        );
        // Intel Macs keep 3.1.6.
        let intel = select(&version, &mac(Arch::X86_64), &Features::default()).unwrap();
        assert!(names(&intel.classpath).contains(&"org.lwjgl:lwjgl:3.1.6"));
    }

    #[test]
    fn selects_1_17_1_and_1_18_2_on_apple_silicon() {
        for id in ["1.17.1", "1.18.2"] {
            let version = fixture(id);
            assert!(!has_arm64_natives(&version, &mac(Arch::Arm64)), "{id}");
            assert_eq!(java_arch(&version, &mac(Arch::Arm64)), Arch::Arm64, "{id}");
            let selection = select(&version, &mac(Arch::Arm64), &Features::default()).unwrap();
            assert_swapped(id, &selection, &ALL_MODULES);
            let intel = select(&version, &mac(Arch::X86_64), &Features::default()).unwrap();
            assert!(
                names(&intel.classpath).contains(&"org.lwjgl:lwjgl:3.2.1"),
                "{id}"
            );
            assert!(
                !names(&intel.classpath).iter().any(|n| n.contains("3.3.1")),
                "{id}"
            );
        }
    }

    /// Windows and Linux keep the LWJGL their versions list.
    #[test]
    fn other_systems_keep_their_lwjgl() {
        for id in ["1.13.2", "1.16.5", "1.18.2"] {
            let version = fixture(id);
            for platform in [
                Platform::new(Os::Windows, Arch::X86_64, "10.0.22631"),
                Platform::new(Os::Linux, Arch::X86_64, "6.1"),
            ] {
                let selection = select(&version, &platform, &Features::default()).unwrap();
                assert!(
                    !names(&selection.classpath)
                        .iter()
                        .any(|n| n.contains("3.3.1")),
                    "{id} {platform:?}"
                );
                assert!(native_paths(&selection).iter().any(|p| p.contains("lwjgl")));
                assert_eq!(java_arch(&version, &platform), Arch::X86_64);
            }
        }
    }

    /// A loader's profile that lists LWJGL itself (as some Forge builds pin a version)
    /// is swapped after the merge too; a module Mojang made no 3.3.1 of keeps the whole
    /// version on Intel, under Rosetta, rather than mixing LWJGL versions.
    #[test]
    fn swaps_loader_lwjgl_and_refuses_unknown_modules() {
        let mut version = fixture("1.16.5");
        let mut pinned = version
            .libraries
            .iter()
            .find(|l| l.name == "org.lwjgl:lwjgl-glfw:3.2.1" && l.natives.is_empty())
            .unwrap()
            .clone();
        pinned.name = "org.lwjgl:lwjgl-glfw:3.2.2".into();
        pinned.rules.clear();
        if let Some(artifact) = pinned.downloads.artifact.as_mut() {
            artifact.path = Some("org/lwjgl/lwjgl-glfw/3.2.2/lwjgl-glfw-3.2.2.jar".into());
        }
        version.libraries.insert(0, pinned);
        let selection = select(&version, &mac(Arch::Arm64), &Features::default()).unwrap();
        assert_swapped("pinned", &selection, &ALL_MODULES);

        let mut unknown = version.libraries[0].clone();
        unknown.name = "org.lwjgl:lwjgl-nanovg:3.2.2".into();
        version.libraries.push(unknown);
        assert!(!swaps_to_arm64_lwjgl(&version, &mac(Arch::Arm64)));
        assert_eq!(java_arch(&version, &mac(Arch::Arm64)), Arch::X86_64);
    }

    /// The embedded table is exactly Mojang's: every LWJGL 3.3.1 jar and arm64 natives
    /// jar of Minecraft 1.20.1, with the same path, SHA-1 and size, plus its objc bridge.
    #[test]
    fn arm64_lwjgl_matches_mojangs_1_20_1() {
        let version = fixture("1.20.1");
        let mut listed = Vec::new();
        for library in &version.libraries {
            let arm64 = library.name.ends_with(":natives-macos-arm64");
            let plain = library.name.split(':').count() == 3;
            if !(library.name.starts_with("org.lwjgl:") && (arm64 || plain))
                && library.name != "ca.weblite:java-objc-bridge:1.1"
            {
                continue;
            }
            let artifact = library.downloads.artifact.as_ref().unwrap();
            listed.push((
                library.name.clone(),
                artifact.path.clone().unwrap(),
                artifact.sha1.clone(),
                artifact.size.unwrap(),
                artifact.url.clone(),
            ));
        }
        let mut embedded: Vec<_> = ARM64_LWJGL
            .iter()
            .flat_map(|(_, jar, natives)| [jar, natives])
            .chain([&UNIVERSAL_OBJC_BRIDGE])
            .map(|jar| {
                let file = jar.file().unwrap();
                let remote = file.remote.unwrap();
                (
                    file.name,
                    file.path.display().to_string(),
                    remote.sha1,
                    remote.size.unwrap(),
                    remote.url,
                )
            })
            .collect();
        listed.sort();
        embedded.sort();
        assert_eq!(listed, embedded);
    }

    /// 1.20.1: new-style natives, both macOS architectures on the classpath, nothing to
    /// extract.
    #[test]
    fn selects_1_20_1() {
        let version = fixture("1.20.1");
        for arch in [Arch::Arm64, Arch::X86_64] {
            let selection = select(&version, &mac(arch), &Features::default()).unwrap();
            let classpath = names(&selection.classpath);
            assert!(classpath.contains(&"org.lwjgl:lwjgl:3.3.1:natives-macos-arm64"));
            assert!(classpath.contains(&"org.lwjgl:lwjgl:3.3.1:natives-macos"));
            assert!(classpath.contains(&"ca.weblite:java-objc-bridge:1.1"));
            assert!(
                !classpath
                    .iter()
                    .any(|n| n.contains("natives-linux") || n.contains("natives-windows"))
            );
            assert!(selection.natives.is_empty());
            assert!(!selection.arm64_lwjgl);
        }
        assert!(has_arm64_natives(&version, &mac(Arch::Arm64)));
        // Already arm64: nothing is swapped.
        assert!(!swaps_to_arm64_lwjgl(&version, &mac(Arch::Arm64)));
        assert_eq!(java_arch(&version, &mac(Arch::Arm64)), Arch::Arm64);
        let linux = Platform::new(Os::Linux, Arch::X86_64, "6.1");
        let selection = select(&version, &linux, &Features::default()).unwrap();
        assert!(names(&selection.classpath).contains(&"org.lwjgl:lwjgl:3.3.1:natives-linux"));
        assert!(!names(&selection.classpath).contains(&"ca.weblite:java-objc-bridge:1.1"));
    }

    #[test]
    fn selects_the_newest_release() {
        let version = fixture("26.3");
        let selection = select(&version, &mac(Arch::Arm64), &Features::default()).unwrap();
        let classpath = names(&selection.classpath);
        assert!(
            classpath.contains(&"io.netty:netty-transport-native-kqueue:4.2.16.Final:osx-aarch_64")
        );
        assert!(classpath.contains(&"com.mojang:jtracy:1.14.38:natives-macos-arm64"));
        assert!(!classpath.iter().any(|n| n.contains("native-epoll")));
        assert!(selection.natives.is_empty());
        assert_eq!(java_arch(&version, &mac(Arch::Arm64)), Arch::Arm64);
        // Every path is unique.
        let mut paths: Vec<_> = selection.classpath.iter().map(|f| &f.path).collect();
        paths.sort();
        paths.dedup();
        assert_eq!(paths.len(), selection.classpath.len());
    }
}
