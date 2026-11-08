//! Java runtimes from Mojang's runtime manifest (`all.json`), the one the official
//! launcher uses.
//!
//! The manifest lists, for each platform key (`mac-os-arm64`, `mac-os`, ...), the
//! runtime components (`java-runtime-gamma`, `jre-legacy`, ...). Each component has a
//! file manifest of files, directories and links, every file with its SHA-1. The
//! files are downloaded `raw`: Zed's lock has no LZMA crate.
//!
//! Mojang has no arm64 build of `jre-legacy` (Java 8), and versions before 1.19 have
//! only x86_64 natives on macOS. On Apple Silicon those run in the `mac-os` (x86_64)
//! runtime under Rosetta 2.

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

/// Picks the runtime `component` for Java built for `java` (the platform with the
/// architecture the game needs). On an arm64 Mac, a component with no arm64 build
/// falls back to the x86_64 one, which needs Rosetta.
pub fn select(
    index: &RuntimeIndex,
    component: &str,
    java: &Platform,
) -> Result<RuntimeChoice, RuntimeError> {
    let lookup = |platform: &Platform| {
        let key = platform.runtime_key();
        index
            .get(key)
            .and_then(|components| components.get(component))
            .and_then(|builds| builds.first())
            .map(|entry| RuntimeChoice {
                component: component.to_string(),
                platform_key: key,
                arch: platform.arch,
                entry: entry.clone(),
            })
    };
    if let Some(choice) = lookup(java) {
        return Ok(choice);
    }
    if java.os == Os::MacOs
        && java.arch == Arch::Arm64
        && let Some(choice) = lookup(&java.with_arch(Arch::X86_64))
    {
        return Ok(choice);
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
fn link_stays_inside(link: &Path, target: &str) -> bool {
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

    #[test]
    fn selects_native_runtimes() {
        let index = index();
        let gamma = select(&index, "java-runtime-gamma", &mac(Arch::Arm64)).unwrap();
        assert_eq!(gamma.platform_key, "mac-os-arm64");
        assert_eq!(gamma.arch, Arch::Arm64);
        assert!(!gamma.needs_rosetta(&mac(Arch::Arm64)));
        assert_eq!(gamma.entry.version.name, "17.0.15");
        let epsilon = select(&index, "java-runtime-epsilon", &mac(Arch::X86_64)).unwrap();
        assert_eq!(epsilon.platform_key, "mac-os");
        assert_eq!(epsilon.entry.version.name, "25.0.1");
    }

    #[test]
    fn jre_legacy_falls_back_to_x64_on_apple_silicon() {
        let index = index();
        let machine = mac(Arch::Arm64);
        let legacy = select(&index, "jre-legacy", &machine).unwrap();
        assert_eq!(legacy.platform_key, "mac-os");
        assert_eq!(legacy.arch, Arch::X86_64);
        assert!(legacy.needs_rosetta(&machine));
        assert!(legacy.entry.version.name.starts_with("8u"));
        // java-runtime-alpha (Java 16) has no arm64 build either.
        let alpha = select(&index, "java-runtime-alpha", &machine).unwrap();
        assert_eq!(alpha.platform_key, "mac-os");
        // A version that needs x86_64 natives asks for the x86_64 platform directly.
        let gamma_x64 = select(
            &index,
            "java-runtime-gamma",
            &machine.with_arch(Arch::X86_64),
        )
        .unwrap();
        assert_eq!(gamma_x64.platform_key, "mac-os");
        assert!(gamma_x64.needs_rosetta(&machine));
        // On an Intel Mac nothing needs Rosetta.
        let intel = select(&index, "jre-legacy", &mac(Arch::X86_64)).unwrap();
        assert!(!intel.needs_rosetta(&mac(Arch::X86_64)));
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
        // The fallback is only for macOS.
        let linux = Platform::new(Os::Linux, Arch::Arm64, "6.1");
        assert!(select(&index(), "jre-legacy", &linux).is_err());
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
