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

use std::path::{Component, Path, PathBuf};

use super::{
    platform::{Arch, Platform},
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
    Ok(selection)
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

/// Whether `version` has native libraries for Apple Silicon. Versions before 1.19 only
/// have x86_64 natives on macOS, so on an arm64 Mac they run in an x86_64 Java under
/// Rosetta.
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

/// The architecture of the Java to run `version` with on `platform`: the machine's
/// own, except on Apple Silicon for versions without arm64 natives.
pub fn java_arch(version: &VersionJson, platform: &Platform) -> Arch {
    if platform.os == super::platform::Os::MacOs
        && platform.arch == Arch::Arm64
        && !has_arm64_natives(version, platform)
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
        assert_eq!(java_arch(&version, &mac(Arch::Arm64)), Arch::X86_64);
    }

    /// 1.16.5: LWJGL 3.2.1 for macOS with old-style `natives-macos` (x86_64 only).
    #[test]
    fn selects_1_16_5() {
        let version = fixture("1.16.5");
        let selection = select(&version, &mac(Arch::Arm64), &Features::default()).unwrap();
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
        let natives = native_paths(&selection);
        assert!(
            natives.contains(&"org/lwjgl/lwjgl/3.2.1/lwjgl-3.2.1-natives-macos.jar".to_string())
        );
        assert_eq!(natives.len(), 8, "{natives:#?}");
        assert!(!has_arm64_natives(&version, &mac(Arch::Arm64)));
        assert_eq!(java_arch(&version, &mac(Arch::Arm64)), Arch::X86_64);
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
        }
        assert!(has_arm64_natives(&version, &mac(Arch::Arm64)));
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
