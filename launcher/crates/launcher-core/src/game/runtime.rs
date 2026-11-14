//! Java runtimes from Mojang's runtime manifest (`all.json`), the one the official
//! launcher uses.
//!
//! The manifest lists, for each platform key (`mac-os-arm64`, `mac-os`, ...), the
//! runtime components (`java-runtime-gamma`, `jre-legacy`, ...). Each component has a
//! file manifest of files, directories and links, every file with its SHA-1. The
//! files are downloaded `raw`: Zed's lock has no LZMA crate.
//!
//! On Apple silicon the launcher runs Java natively wherever Minecraft can run natively
//! (see [`select`]), and never falls back to an Intel Java on its own:
//! - Mojang has no arm64 build of `java-runtime-alpha` (Java 16, for 1.17) or
//!   `java-runtime-beta` (Java 17, for 1.18): its arm64 `java-runtime-gamma` (Java 17),
//!   which those versions and their loaders run on, is used instead.
//! - Mojang has no arm64 build of `jre-legacy` (Java 8, for 1.14 to 1.16 and their
//!   Forge) either: Azul's Zulu 8 is used instead ([`super::zulu`]).
//! - Versions before 1.13 use LWJGL 2, which has no arm64 build from Mojang, and 1.13
//!   to 1.13.2 a narrator that can't load in an arm64 Java 8; they ask for the `mac-os`
//!   (x86_64) runtime themselves ([`super::libraries::java_arch`]) and run under
//!   Rosetta 2, which the launcher tells the player about. So does a Java override
//!   that is an Intel Java (`install::java_arch_for`).

use std::{
    collections::BTreeMap,
    fs, io,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

use serde::Deserialize;

use super::{
    download::DownloadItem,
    libraries::safe_relative_path,
    mojang::{MojangUrls, UrlError},
    platform::{Arch, Os, Platform},
    process::NoConsoleWindow as _,
};

/// `all.json`: platform key → component → builds (normally one).
pub type RuntimeIndex = BTreeMap<String, BTreeMap<String, Vec<RuntimeEntry>>>;

#[derive(Clone, Debug, Deserialize)]
pub struct RuntimeEntry {
    pub manifest: ManifestRef,
    pub version: RuntimeVersion,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ManifestRef {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RuntimeVersion {
    pub name: String,
    #[serde(default)]
    pub released: Option<String>,
}

/// A runtime's file manifest.
#[derive(Clone, Debug, Deserialize)]
pub struct RuntimeManifest {
    pub files: BTreeMap<String, RuntimeFile>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RuntimeFile {
    File {
        downloads: RuntimeDownloads,
        #[serde(default)]
        executable: bool,
    },
    Directory,
    Link {
        target: String,
    },
}

#[derive(Clone, Debug, Deserialize)]
pub struct RuntimeDownloads {
    pub raw: RawDownload,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RawDownload {
    pub sha1: String,
    pub size: u64,
    pub url: String,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuntimeError {
    #[error("Mojang has no {component} Java runtime for {platform}")]
    Unavailable { component: String, platform: String },
    #[error("the runtime manifest has an unsafe path {0:?}")]
    UnsafePath(String),
    #[error("the runtime manifest links {link:?} outside the runtime, to {target:?}")]
    UnsafeLink { link: String, target: String },
    #[error(transparent)]
    Url(#[from] UrlError),
}

/// The runtime to use for a component on a platform.
#[derive(Clone, Debug)]
pub struct RuntimeChoice {
    pub component: String,
    /// The platform key it is for, which names its folder.
    pub platform_key: &'static str,
    /// The architecture of its binaries.
    pub arch: Arch,
    pub entry: RuntimeEntry,
}

impl RuntimeChoice {
    /// Whether it is an x86_64 runtime on an Apple Silicon Mac.
    pub fn needs_rosetta(&self, machine: &Platform) -> bool {
        machine.os == Os::MacOs && machine.arch == Arch::Arm64 && self.arch == Arch::X86_64
    }
}

/// Where the Java for a version comes from.
#[derive(Clone, Debug)]
pub enum JavaSource {
    /// A runtime from Mojang's manifest.
    Mojang(RuntimeChoice),
    /// Azul's Zulu 8 for macOS on arm64, for `jre-legacy`, which Mojang only builds for
    /// Intel Macs ([`super::zulu`]).
    Zulu8,
}

impl JavaSource {
    /// Whether it is an x86_64 runtime on an Apple Silicon Mac.
    pub fn needs_rosetta(&self, machine: &Platform) -> bool {
        match self {
            JavaSource::Mojang(choice) => choice.needs_rosetta(machine),
            JavaSource::Zulu8 => false,
        }
    }
}

/// The components Mojang has no arm64 macOS build of whose versions run on its arm64
/// `java-runtime-gamma` (Java 17): 1.17 (Java 16) and 1.18 (an older Java 17), with
/// their loaders.
const RUN_ON_GAMMA_ON_ARM64_MACS: [&str; 2] = ["java-runtime-alpha", "java-runtime-beta"];

/// The Java an Apple silicon Mac uses for runtime `component`, which Mojang has no
/// arm64 build of, as its runtimes folder and Java major version: Mojang's arm64
/// `java-runtime-gamma` (Java 17) for Java 16 and the older Java 17, and Azul's Zulu 8
/// ([`super::zulu`]) for `jre-legacy`. `None` elsewhere, and for other components.
fn arm64_substitute(component: &str, java: &Platform) -> Option<(&'static str, u32)> {
    if java.os != Os::MacOs || java.arch != Arch::Arm64 {
        return None;
    }
    if RUN_ON_GAMMA_ON_ARM64_MACS.contains(&component) {
        Some(("java-runtime-gamma", 17))
    } else if component == "jre-legacy" {
        Some((super::zulu::COMPONENT, 8))
    } else {
        None
    }
}

/// The runtimes folder (`<runtimes>/<folder>`) and Java major version that a version
/// whose runtime is `component`, with `major`, uses in a Java built for `java`: the
/// choice [`select`] makes, for the storage settings to tell what is in use. (`select`
/// would take Mojang's own arm64 build first, were there one; there is none of these.)
pub fn folder_for<'a>(
    component: &'a str,
    major: Option<u32>,
    java: &Platform,
) -> (&'a str, Option<u32>) {
    match arm64_substitute(component, java) {
        Some((folder, major)) => (folder, Some(major)),
        None => (component, major),
    }
}

/// Picks the Java for the runtime `component` built for `java` (the platform with the
/// architecture the game needs). On an arm64 Mac, Java 16 and 17 components with no
/// arm64 build use Mojang's arm64 Java 17, and `jre-legacy` uses Azul's Zulu 8.
/// Nothing falls back to an x86_64 runtime: a version that runs under Rosetta asks
/// for the x86_64 platform itself.
pub fn select(
    index: &RuntimeIndex,
    component: &str,
    java: &Platform,
) -> Result<JavaSource, RuntimeError> {
    let lookup = |component: &str| {
        let key = java.runtime_key();
        index
            .get(key)
            .and_then(|components| components.get(component))
            .and_then(|builds| builds.first())
            .map(|entry| {
                JavaSource::Mojang(RuntimeChoice {
                    component: component.to_string(),
                    platform_key: key,
                    arch: java.arch,
                    entry: entry.clone(),
                })
            })
    };
    if let Some(source) = lookup(component) {
        return Ok(source);
    }
    match arm64_substitute(component, java) {
        Some((super::zulu::COMPONENT, _)) => return Ok(JavaSource::Zulu8),
        Some((folder, _)) => {
            if let Some(source) = lookup(folder) {
                return Ok(source);
            }
        }
        None => {}
    }
    Err(RuntimeError::Unavailable {
        component: component.to_string(),
        platform: java.runtime_key().to_string(),
    })
}

/// Where a runtime is installed: `<runtimes>/<component>/<platform key>`.
pub fn install_dir(runtimes_dir: &Path, choice: &RuntimeChoice) -> Result<PathBuf, RuntimeError> {
    let component = safe_relative_path(&choice.component)
        .map_err(|_| RuntimeError::UnsafePath(choice.component.clone()))?;
    Ok(runtimes_dir.join(component).join(choice.platform_key))
}

/// The `java` binary inside an installed runtime.
pub fn java_binary(runtime_dir: &Path, os: Os) -> PathBuf {
    match os {
        Os::MacOs => runtime_dir.join("jre.bundle/Contents/Home/bin/java"),
        Os::Windows => runtime_dir.join("bin/javaw.exe"),
        Os::Linux => runtime_dir.join("bin/java"),
    }
}

/// What installing a runtime's file manifest takes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RuntimePlan {
    pub directories: Vec<PathBuf>,
    pub files: Vec<DownloadItem>,
    /// Symbolic links to create: (link, target as written in the manifest).
    pub links: Vec<(PathBuf, String)>,
}

/// Turns a runtime's file manifest into downloads, folders and links under `root`.
/// Every path must stay inside `root`, and so must every link's target.
pub fn plan(
    manifest: &RuntimeManifest,
    root: &Path,
    urls: &MojangUrls,
) -> Result<RuntimePlan, RuntimeError> {
    let mut plan = RuntimePlan::default();
    for (name, file) in &manifest.files {
        let relative =
            safe_relative_path(name).map_err(|_| RuntimeError::UnsafePath(name.clone()))?;
        let path = root.join(&relative);
        match file {
            RuntimeFile::Directory => plan.directories.push(path),
            RuntimeFile::File {
                downloads,
                executable,
            } => plan.files.push(DownloadItem {
                url: urls.resolve(&downloads.raw.url)?,
                dest: path,
                sha1: downloads.raw.sha1.clone(),
                size: Some(downloads.raw.size),
                executable: *executable,
            }),
            RuntimeFile::Link { target } => {
                if !link_stays_inside(&relative, target) {
                    return Err(RuntimeError::UnsafeLink {
                        link: name.clone(),
                        target: target.clone(),
                    });
                }
                plan.links.push((path, target.clone()));
            }
        }
    }
    Ok(plan)
}

/// Whether the relative link target, resolved from the link's folder, stays inside
/// the runtime. Resolution is lexical: the runtime's own links are never followed.
pub(crate) fn link_stays_inside(link: &Path, target: &str) -> bool {
    if target.is_empty() || target.contains('\0') || Path::new(target).is_absolute() {
        return false;
    }
    let mut depth: Vec<&std::ffi::OsStr> = link
        .parent()
        .map(|parent| {
            parent
                .components()
                .filter_map(|c| match c {
                    Component::Normal(part) => Some(part),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    for component in Path::new(target).components() {
        match component {
            Component::Normal(part) => depth.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if depth.pop().is_none() {
                    return false;
                }
            }
            Component::RootDir | Component::Prefix(_) => return false,
        }
    }
    true
}

/// Creates the plan's folders. Run before the downloads.
pub fn create_directories(plan: &RuntimePlan) -> io::Result<()> {
    for dir in &plan.directories {
        fs::create_dir_all(dir)?;
    }
    Ok(())
}

/// Creates the plan's links, replacing whatever is at each path. Run after the
/// downloads.
#[cfg(unix)]
pub fn create_links(plan: &RuntimePlan) -> io::Result<()> {
    for (link, target) in &plan.links {
        if let Some(parent) = link.parent() {
            fs::create_dir_all(parent)?;
        }
        match fs::symlink_metadata(link) {
            Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => {
                fs::remove_dir_all(link)?
            }
            Ok(_) => fs::remove_file(link)?,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {}
            Err(err) => return Err(err),
        }
        std::os::unix::fs::symlink(target, link)?;
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn create_links(_: &RuntimePlan) -> io::Result<()> {
    Ok(())
}

/// The marker written into a runtime's folder once it is complete. It names the
/// manifest it was installed from, so a changed manifest reinstalls.
pub const INSTALLED_MARKER: &str = ".lodestone-runtime";

pub fn is_installed(runtime_dir: &Path, manifest_sha1: &str) -> bool {
    fs::read_to_string(runtime_dir.join(INSTALLED_MARKER))
        .is_ok_and(|contents| contents.trim().eq_ignore_ascii_case(manifest_sha1))
}

pub fn mark_installed(runtime_dir: &Path, manifest_sha1: &str) -> io::Result<()> {
    crate::storage::write_atomic(
        &runtime_dir.join(INSTALLED_MARKER),
        manifest_sha1.as_bytes(),
    )
}

/// A Java found at a path, from its `-version` output.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JavaInfo {
    /// The version string, such as `17.0.15` or `1.8.0_74`.
    pub version: String,
    /// The major version: 8 for `1.8.0_74`, 17 for `17.0.15`.
    pub major: u32,
}

/// Parses `java -version` output (Java prints it to stderr), such as
/// `openjdk version "17.0.15" 2025-04-15` or `java version "1.8.0_74"`.
pub fn parse_java_version(output: &str) -> Option<JavaInfo> {
    let line = output.lines().find(|line| line.contains(" version \""))?;
    let start = line.find('"')? + 1;
    let end = start + line[start..].find('"')?;
    let version = line[start..end].to_string();
    let mut parts = version.split(|c: char| !c.is_ascii_digit());
    let first: u32 = parts.next()?.parse().ok()?;
    let major = if first == 1 {
        parts.next()?.parse().ok()?
    } else {
        first
    };
    Some(JavaInfo { version, major })
}

#[derive(Debug, thiserror::Error)]
pub enum JavaCheckError {
    #[error("{0} doesn't exist")]
    Missing(PathBuf),
    #[error("couldn't run {path}: {source}")]
    Run {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("{0} didn't answer within {1} seconds")]
    TimedOut(PathBuf, u64),
    #[error("{0} didn't print a Java version")]
    NotJava(PathBuf),
}

/// Runs `<java> -version` and reads its version. It is only run on a path the user
/// chose as the Java override.
pub fn check_java(java: &Path, timeout: Duration) -> Result<JavaInfo, JavaCheckError> {
    if !java.is_file() {
        return Err(JavaCheckError::Missing(java.to_owned()));
    }
    let mut child = Command::new(java)
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .no_console_window()
        .spawn()
        .map_err(|source| JavaCheckError::Run {
            path: java.to_owned(),
            source,
        })?;
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(JavaCheckError::TimedOut(java.to_owned(), timeout.as_secs()));
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(source) => {
                return Err(JavaCheckError::Run {
                    path: java.to_owned(),
                    source,
                });
            }
        }
    }
    let output = child
        .wait_with_output()
        .map_err(|source| JavaCheckError::Run {
            path: java.to_owned(),
            source,
        })?;
    let text = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    parse_java_version(&text).ok_or_else(|| JavaCheckError::NotJava(java.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index() -> RuntimeIndex {
        let path = format!("{}/testdata/runtime/all.json", env!("CARGO_MANIFEST_DIR"));
        serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
    }

    fn mac(arch: Arch) -> Platform {
        Platform::new(Os::MacOs, arch, "14.6.1")
    }

    fn mojang(source: JavaSource) -> RuntimeChoice {
        match source {
            JavaSource::Mojang(choice) => choice,
            JavaSource::Zulu8 => panic!("expected a Mojang runtime, got Zulu 8"),
        }
    }

    #[test]
    fn selects_native_runtimes() {
        let index = index();
        let gamma = mojang(select(&index, "java-runtime-gamma", &mac(Arch::Arm64)).unwrap());
        assert_eq!(gamma.platform_key, "mac-os-arm64");
        assert_eq!(gamma.arch, Arch::Arm64);
        assert!(!gamma.needs_rosetta(&mac(Arch::Arm64)));
        assert_eq!(gamma.entry.version.name, "17.0.15");
        let epsilon = mojang(select(&index, "java-runtime-epsilon", &mac(Arch::X86_64)).unwrap());
        assert_eq!(epsilon.platform_key, "mac-os");
        assert_eq!(epsilon.entry.version.name, "25.0.1");
    }

    /// On Apple silicon nothing falls back to an Intel runtime: Java 16 and Mojang's
    /// older Java 17 use its arm64 Java 17, and Java 8 uses Azul's Zulu 8.
    #[test]
    fn apple_silicon_gets_native_java_for_every_component() {
        let index = index();
        let machine = mac(Arch::Arm64);
        for component in ["java-runtime-alpha", "java-runtime-beta"] {
            let source = select(&index, component, &machine).unwrap();
            assert!(!source.needs_rosetta(&machine), "{component}");
            let choice = mojang(source);
            assert_eq!(choice.component, "java-runtime-gamma", "{component}");
            assert_eq!(choice.platform_key, "mac-os-arm64");
            assert_eq!(choice.arch, Arch::Arm64);
        }
        let legacy = select(&index, "jre-legacy", &machine).unwrap();
        assert!(matches!(legacy, JavaSource::Zulu8));
        assert!(!legacy.needs_rosetta(&machine));
    }

    /// The storage settings' idea of which folder a component uses is the one `select`
    /// installs into.
    #[test]
    fn folder_for_agrees_with_select() {
        let index = index();
        for arch in [Arch::Arm64, Arch::X86_64] {
            let java = mac(arch);
            for component in [
                "jre-legacy",
                "java-runtime-alpha",
                "java-runtime-beta",
                "java-runtime-gamma",
                "java-runtime-delta",
            ] {
                let Ok(source) = select(&index, component, &java) else {
                    continue;
                };
                let folder = match source {
                    JavaSource::Mojang(choice) => choice.component,
                    JavaSource::Zulu8 => super::super::zulu::COMPONENT.to_string(),
                };
                assert_eq!(
                    folder_for(component, Some(1), &java).0,
                    folder,
                    "{component} {arch}"
                );
            }
        }
        assert_eq!(
            folder_for("java-runtime-alpha", Some(16), &mac(Arch::Arm64)),
            ("java-runtime-gamma", Some(17))
        );
        assert_eq!(
            folder_for("jre-legacy", Some(8), &mac(Arch::Arm64)),
            ("zulu-8", Some(8))
        );
        assert_eq!(
            folder_for("jre-legacy", Some(8), &mac(Arch::X86_64)),
            ("jre-legacy", Some(8))
        );
    }

    #[test]
    fn versions_that_need_rosetta_ask_for_the_x64_runtime() {
        let index = index();
        let machine = mac(Arch::Arm64);
        // 1.12.2 and older (LWJGL 2) ask for the x86_64 platform themselves.
        let legacy_x64 =
            mojang(select(&index, "jre-legacy", &machine.with_arch(Arch::X86_64)).unwrap());
        assert_eq!(legacy_x64.platform_key, "mac-os");
        assert_eq!(legacy_x64.arch, Arch::X86_64);
        assert!(legacy_x64.needs_rosetta(&machine));
        assert!(legacy_x64.entry.version.name.starts_with("8u"));
        // On an Intel Mac nothing needs Rosetta, and Mojang's own runtimes are used.
        let intel = mac(Arch::X86_64);
        for component in ["jre-legacy", "java-runtime-alpha", "java-runtime-beta"] {
            let choice = mojang(select(&index, component, &intel).unwrap());
            assert_eq!(choice.component, component);
            assert_eq!(choice.platform_key, "mac-os");
            assert!(!choice.needs_rosetta(&intel));
        }
    }

    /// Windows and Linux keep Mojang's runtime for each component.
    #[test]
    fn other_systems_are_unchanged() {
        let mut index = index();
        // The recorded index has no Windows runtimes; give Windows Linux's.
        let linux = index["linux"].clone();
        index.insert("windows-x64".into(), linux);
        for (platform, key) in [
            (
                Platform::new(Os::Windows, Arch::X86_64, "10.0.22631"),
                "windows-x64",
            ),
            (Platform::new(Os::Linux, Arch::X86_64, "6.1"), "linux"),
        ] {
            for component in ["jre-legacy", "java-runtime-alpha", "java-runtime-beta"] {
                let choice = mojang(select(&index, component, &platform).unwrap());
                assert_eq!(choice.component, component);
                assert_eq!(choice.platform_key, key);
                assert_eq!(choice.arch, Arch::X86_64);
            }
        }
    }

    #[test]
    fn unknown_components_are_unavailable() {
        let err = select(&index(), "java-runtime-zeta", &mac(Arch::Arm64)).unwrap_err();
        assert_eq!(
            err,
            RuntimeError::Unavailable {
                component: "java-runtime-zeta".into(),
                platform: "mac-os-arm64".into()
            }
        );
        // The substitutions are only for macOS.
        let linux = Platform::new(Os::Linux, Arch::Arm64, "6.1");
        assert!(select(&index(), "jre-legacy", &linux).is_err());
        assert!(select(&index(), "java-runtime-alpha", &linux).is_err());
    }

    #[test]
    fn plans_the_recorded_manifest() {
        let path = format!(
            "{}/testdata/runtime/java-runtime-gamma-mac-os-arm64.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let manifest: RuntimeManifest = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let root = Path::new("/data/runtimes/java-runtime-gamma/mac-os-arm64");
        let plan = plan(&manifest, root, &MojangUrls::production()).unwrap();
        let java = plan
            .files
            .iter()
            .find(|f| f.dest == java_binary(root, Os::MacOs))
            .unwrap();
        assert!(java.executable);
        assert!(java.url.starts_with("https://piston-data.mojang.com/"));
        assert!(
            plan.directories
                .contains(&root.join("jre.bundle/Contents/Home"))
        );
        assert!(plan.links.iter().any(
            |(link, target)| link.ends_with("legal/java.compiler/LICENSE")
                && target == "../java.base/LICENSE"
        ));
    }

    #[test]
    fn rejects_paths_and_links_that_leave_the_runtime() {
        let manifest = |json: &str| -> RuntimeManifest { serde_json::from_str(json).unwrap() };
        let root = Path::new("/rt");
        let urls = MojangUrls::production();
        for bad in [
            r#"{"files": {"../evil": {"type": "directory"}}}"#,
            r#"{"files": {"/abs": {"type": "directory"}}}"#,
            r#"{"files": {"a/link": {"type": "link", "target": "../../etc/passwd"}}}"#,
            r#"{"files": {"link": {"type": "link", "target": "/etc/passwd"}}}"#,
            r#"{"files": {"x": {"type": "file", "downloads": {"raw": {"sha1": "a", "size": 1, "url": "https://evil.example/x"}}}}}"#,
        ] {
            assert!(plan(&manifest(bad), root, &urls).is_err(), "{bad}");
        }
        let ok = manifest(r#"{"files": {"a/b/link": {"type": "link", "target": "../../c"}}}"#);
        assert_eq!(plan(&ok, root, &urls).unwrap().links.len(), 1);
    }

    #[cfg(unix)]
    #[test]
    fn creates_links_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let manifest: RuntimeManifest = serde_json::from_str(
            r#"{"files": {
                "home": {"type": "directory"},
                "home/legal": {"type": "directory"},
                "home/legal/b/LICENSE": {"type": "link", "target": "../a/LICENSE"}
            }}"#,
        )
        .unwrap();
        let plan = plan(&manifest, dir.path(), &MojangUrls::production()).unwrap();
        create_directories(&plan).unwrap();
        fs::create_dir_all(dir.path().join("home/legal/a")).unwrap();
        fs::write(dir.path().join("home/legal/a/LICENSE"), b"GPL").unwrap();
        create_links(&plan).unwrap();
        // Running it again replaces the link.
        create_links(&plan).unwrap();
        assert_eq!(
            fs::read(dir.path().join("home/legal/b/LICENSE")).unwrap(),
            b"GPL"
        );
    }

    #[test]
    fn install_markers() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_installed(dir.path(), "abc"));
        mark_installed(dir.path(), "abc").unwrap();
        assert!(is_installed(dir.path(), "ABC"));
        assert!(!is_installed(dir.path(), "def"));
    }

    #[test]
    fn parses_java_versions() {
        for (output, version, major) in [
            (
                "openjdk version \"17.0.15\" 2025-04-15\nOpenJDK Runtime Environment",
                "17.0.15",
                17,
            ),
            (
                "java version \"1.8.0_74\"\nJava(TM) SE Runtime Environment (build 1.8.0_74-b02)",
                "1.8.0_74",
                8,
            ),
            ("openjdk version \"21\" 2023-09-19", "21", 21),
            (
                "Picked up JAVA_TOOL_OPTIONS: -Xmx1g\nopenjdk version \"25.0.1\" 2025-10-21 LTS",
                "25.0.1",
                25,
            ),
            ("openjdk version \"9-ea\"", "9-ea", 9),
        ] {
            let info = parse_java_version(output).unwrap();
            assert_eq!(info.version, version);
            assert_eq!(info.major, major, "{output}");
        }
        assert_eq!(parse_java_version("bash: java: command not found"), None);
        assert_eq!(parse_java_version("version \"x\""), None);
    }

    /// A stand-in for `java`: a shell script that prints a version, as a mock.
    #[cfg(unix)]
    #[test]
    fn checks_a_java_override() {
        use std::os::unix::fs::PermissionsExt as _;
        let dir = tempfile::tempdir().unwrap();
        let fake = dir.path().join("java");
        fs::write(
            &fake,
            "#!/bin/sh\necho 'openjdk version \"21.0.7\" 2025-04-15 LTS' >&2\n",
        )
        .unwrap();
        fs::set_permissions(&fake, fs::Permissions::from_mode(0o755)).unwrap();
        let info = check_java(&fake, Duration::from_secs(10)).unwrap();
        assert_eq!(info.major, 21);

        let not_java = dir.path().join("not-java");
        fs::write(&not_java, "#!/bin/sh\necho hello\n").unwrap();
        fs::set_permissions(&not_java, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(matches!(
            check_java(&not_java, Duration::from_secs(10)),
            Err(JavaCheckError::NotJava(_))
        ));
        assert!(matches!(
            check_java(&dir.path().join("missing"), Duration::from_secs(1)),
            Err(JavaCheckError::Missing(_))
        ));
    }
}
