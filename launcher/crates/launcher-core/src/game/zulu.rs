//! Azul's Zulu 8 JRE for macOS on arm64: the Java that runs Minecraft 1.14 to 1.16.5
//! (and their Forge) natively on Apple silicon, where Mojang's `jre-legacy` is Intel
//! only.
//!
//! The build is found through Azul's metadata API, downloaded only from Azul's CDN,
//! checked against the SHA-256 Azul publishes, and unpacked into the runtimes folder
//! beside Mojang's runtimes: `<runtimes>/`[`COMPONENT`]`/mac-os-arm64/<build>`, each
//! build in a folder of its own ([`build_folder`]). A new build goes into a new folder
//! and the game uses the newest installed one ([`newest_installed`]), so installing
//! never deletes or changes the Java a running game uses; older builds are removed with
//! the unused runtimes in Settings › Storage, which only runs while no game does
//! ([`older_builds`]). It comes as a zip (Azul publishes zips for macOS too, and Zed's
//! lock has no tar crate). Lodestone Core looks up Java the same way for servers
//! (`core/src/implementations/minecraft/util.rs`); this is a port of its Zulu lookup,
//! not a dependency on it.
//!
//! Forge for 1.16.3 to 1.16.5 before its January 2022 builds (ModLauncher 8.0) can't
//! start in Java 8u321 or newer, so those get the newest build before it, in a
//! component of its own ([`Zulu8::Before8u321`]).

use std::{
    fs::{self, File},
    io::{self, Read as _},
    path::{Path, PathBuf},
};

use serde::Deserialize;

use super::{natives, runtime, version::VersionJson};

/// The runtime folder's component name: `<runtimes>/zulu-8/mac-os-arm64`.
pub const COMPONENT: &str = "zulu-8";

/// The component of the builds before 8u321, for [`Zulu8::Before8u321`].
pub const COMPONENT_BEFORE_8U321: &str = "zulu-8-before-8u321";

/// Azul's newest Zulu 8 JRE builds for macOS on arm64, as zips.
pub const PACKAGES_URL: &str = "https://api.azul.com/metadata/v1/zulu/packages/?java_version=8&os=macos&arch=aarch64&archive_type=zip&java_package_type=jre&javafx_bundled=false&crac_supported=false&release_status=ga&availability_types=CA&latest=true&include_fields=sha256_hash,size&page_size=20";

/// Every Zulu 8 JRE build for macOS on arm64 (about 30 in 2026; Azul started with
/// 8u282), for [`Zulu8::Before8u321`].
pub const ALL_PACKAGES_URL: &str = "https://api.azul.com/metadata/v1/zulu/packages/?java_version=8&os=macos&arch=aarch64&archive_type=zip&java_package_type=jre&javafx_bundled=false&crac_supported=false&release_status=ga&availability_types=CA&include_fields=sha256_hash,size&page_size=1000";

/// Which Zulu 8 builds a version can start in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Zulu8 {
    /// The newest build, for everything but [`Zulu8::Before8u321`].
    Newest,
    /// The newest build before Java 8u321. ModLauncher 8.0, which Forge for 1.16.3 to
    /// 1.16.5 used until its January 2022 builds (1.16.5-36.2.26 has 8.1.3), makes a
    /// `sun.security.util.ManifestEntryVerifier` with the constructor 8u321 replaced,
    /// and stops at startup with a `NoSuchMethodError` in a newer Java. Mojang's
    /// `jre-legacy` (8u51) is older, so they run on Intel Macs and Windows. Older
    /// ModLauncher versions (Forge for 1.14 to 1.16.2) and 8.1 don't use it.
    Before8u321,
}

impl Zulu8 {
    /// The builds `version` (with its loader's profile merged) can start in.
    pub fn for_version(version: &VersionJson) -> Self {
        let old_modlauncher = version.libraries.iter().any(|library| {
            let mut parts = library.name.split(':');
            parts.next() == Some("cpw.mods")
                && parts.next() == Some("modlauncher")
                && parts
                    .next()
                    .is_some_and(|v| v == "8.0" || v.starts_with("8.0."))
        });
        if old_modlauncher {
            Zulu8::Before8u321
        } else {
            Zulu8::Newest
        }
    }

    /// Its runtimes folder: `<runtimes>/<component>/mac-os-arm64/<build>`.
    pub fn component(self) -> &'static str {
        match self {
            Zulu8::Newest => COMPONENT,
            Zulu8::Before8u321 => COMPONENT_BEFORE_8U321,
        }
    }

    /// Where Azul lists its builds.
    pub fn packages_url(self) -> &'static str {
        match self {
            Zulu8::Newest => PACKAGES_URL,
            Zulu8::Before8u321 => ALL_PACKAGES_URL,
        }
    }

    /// Whether a build of Java `java_version` (`[8, 0, 512]`) is one of these.
    fn accepts(self, java_version: &[u64]) -> bool {
        match self {
            Zulu8::Newest => true,
            Zulu8::Before8u321 => java_version < [8, 0, 321].as_slice(),
        }
    }

    /// Whether the archive `name` (`zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip`)
    /// is a build of these, by the Java version in its name.
    pub fn owns_archive(self, name: &str) -> bool {
        let Some(update) = name
            .split("-jre8.0.")
            .nth(1)
            .and_then(|rest| rest.split('-').next())
            .and_then(|update| update.parse::<u64>().ok())
        else {
            return false;
        };
        let before = Zulu8::Before8u321.accepts(&[8, 0, update]);
        before == (self == Zulu8::Before8u321)
    }
}

/// Where downloads must come from.
const CDN: &str = "https://cdn.azul.com/zulu/bin/";

/// A build in Azul's answer.
#[derive(Clone, Debug, Deserialize)]
pub struct ZuluPackage {
    pub name: String,
    pub download_url: String,
    pub java_version: Vec<u64>,
    #[serde(default)]
    pub distro_version: Vec<u64>,
    #[serde(default)]
    pub sha256_hash: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
}

/// The build to install.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ZuluBuild {
    /// The archive's file name, such as `zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip`.
    pub name: String,
    pub url: String,
    /// Lowercase hex.
    pub sha256: String,
    /// About the size, for progress: Azul rounds it, so it is never checked.
    pub size: Option<u64>,
}

/// The folder a build is unpacked into: its archive's name without `.zip`, such as
/// `zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64`. [`pick`] only lets through plain
/// names, so it is a single path component.
pub fn build_folder(build: &ZuluBuild) -> &str {
    build.name.strip_suffix(".zip").unwrap_or(&build.name)
}

/// Azul's build number from a build's archive or folder name: `[8, 98, 0, 111]` for
/// `zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64`.
fn distro_version(name: &str) -> Option<Vec<u64>> {
    let numbers = name.strip_prefix("zulu")?.split('-').next()?;
    numbers
        .split('.')
        .map(|part| part.parse().ok())
        .collect::<Option<Vec<u64>>>()
        .filter(|numbers| !numbers.is_empty())
}

/// The builds installed in `dir` (`<runtimes>/zulu-8/mac-os-arm64`): each folder that
/// was marked installed (only done once it was complete) and has a `java`, with its
/// build number, oldest first.
fn installed_builds(dir: &Path) -> Vec<(Vec<u64>, PathBuf)> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut builds: Vec<(Vec<u64>, PathBuf)> = entries
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_str()?.to_string();
            let version = distro_version(&name)?;
            let path = entry.path();
            (path.join(runtime::INSTALLED_MARKER).is_file() && java_binary(&path).is_some())
                .then_some((version, path))
        })
        .collect();
    builds.sort();
    builds
}

/// The `java` of the newest build installed in `dir`, for when Azul can't be asked
/// or lists none.
pub fn newest_installed(dir: &Path) -> Option<PathBuf> {
    installed_builds(dir)
        .pop()
        .and_then(|(_, path)| java_binary(&path))
}

/// Everything in `dir` but the newest installed build: older builds, and folders left
/// by an install that didn't finish. Only for removing while no game runs.
pub fn older_builds(dir: &Path) -> Vec<PathBuf> {
    let newest = installed_builds(dir).pop().map(|(_, path)| path);
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut older: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && Some(path) != newest.as_ref())
        .collect();
    older.sort();
    older
}

/// Whether `sha256` looks like a SHA-256 in hex.
fn is_sha256(sha256: &str) -> bool {
    sha256.len() == 64 && sha256.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Picks the newest Zulu 8 JRE of `line` from Azul's answer, checking that it is Java
/// 8, a JRE zip for macOS on arm64, from Azul's own CDN, with a SHA-256. `None` when
/// there is no such build.
pub fn pick(packages: &[ZuluPackage], line: Zulu8) -> Option<ZuluBuild> {
    // Azul names its files zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip.
    let plain_name = |name: &str| {
        !name.is_empty()
            && name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
            && !name.starts_with('.')
    };
    packages
        .iter()
        .filter(|p| {
            p.java_version.first() == Some(&8)
                && line.accepts(&p.java_version)
                && plain_name(&p.name)
                && p.name.starts_with("zulu")
                && p.name.contains("-jre8")
                && p.name.ends_with("-macosx_aarch64.zip")
                && p.download_url.starts_with(CDN)
                && p.download_url.ends_with(&format!("/{}", p.name))
                && p.download_url.len() == CDN.len() + p.name.len()
                && p.sha256_hash.as_deref().is_some_and(is_sha256)
        })
        .max_by(|a, b| {
            (&a.java_version, &a.distro_version).cmp(&(&b.java_version, &b.distro_version))
        })
        .map(|p| ZuluBuild {
            name: p.name.clone(),
            url: p.download_url.clone(),
            sha256: p
                .sha256_hash
                .clone()
                .unwrap_or_default()
                .to_ascii_lowercase(),
            size: p.size,
        })
}

/// The `java` binary of an unpacked Zulu JRE in `root`. A macOS Zulu keeps its Java
/// home inside a bundle, `zulu-8.jre/Contents/Home`, with links to it at the top of
/// the archive; the bundle's own binary is preferred over the links, and any
/// `*.jre`/`*.jdk` bundle name is accepted, so a renamed bundle is still found.
pub fn java_binary(root: &Path) -> Option<PathBuf> {
    let mut candidates = vec![root.join("zulu-8.jre/Contents/Home/bin/java")];
    if let Ok(entries) = fs::read_dir(root) {
        let mut bundles: Vec<PathBuf> = entries
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "jre" || extension == "jdk")
            })
            .collect();
        bundles.sort();
        candidates.extend(
            bundles
                .into_iter()
                .map(|bundle| bundle.join("Contents/Home/bin/java")),
        );
    }
    candidates.push(root.join("Contents/Home/bin/java"));
    candidates.push(root.join("bin/java"));
    candidates.into_iter().find(|path| path.is_file())
}

#[derive(Debug, thiserror::Error)]
pub enum UnpackError {
    #[error("could not read {archive}: {reason}")]
    BadArchive { archive: PathBuf, reason: String },
    #[error("{archive} has an unsafe entry {entry:?}")]
    UnsafeEntry { archive: PathBuf, entry: String },
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

/// A zip entry's mode says symlink when its file type bits are `S_IFLNK`.
fn is_symlink(mode: Option<u32>) -> bool {
    mode.is_some_and(|mode| mode & 0o170000 == 0o120000)
}

/// Unpacks the Zulu zip `archive` into `dest`, which must not exist yet, without the
/// folder all its entries are in (`zulu8...-macosx_aarch64/`). Every entry and every
/// link's target must stay inside `dest`, checked as written and again once all the
/// links are made, through any other link (Azul's own zips have none). Files keep only
/// their executable bits; no other mode from the archive reaches the disk.
pub fn unpack(archive: &Path, dest: &Path) -> Result<(), UnpackError> {
    let bad = |reason: String| UnpackError::BadArchive {
        archive: archive.to_owned(),
        reason,
    };
    let unsafe_entry = |entry: &str| UnpackError::UnsafeEntry {
        archive: archive.to_owned(),
        entry: entry.to_string(),
    };
    let io_err = |path: &Path| {
        let path = path.to_owned();
        move |source| UnpackError::Io { path, source }
    };
    let file = File::open(archive).map_err(|err| bad(err.to_string()))?;
    let mut zip =
        zip::ZipArchive::new(io::BufReader::new(file)).map_err(|err| bad(err.to_string()))?;

    // Every entry's checked path, with the common top folder taken off afterwards.
    let mut paths = Vec::with_capacity(zip.len());
    for index in 0..zip.len() {
        let entry = zip
            .by_index_raw(index)
            .map_err(|err| bad(err.to_string()))?;
        let name = entry
            .name()
            .map_err(|err| bad(err.to_string()))?
            .into_owned();
        let path = natives::safe_entry_path(&name).ok_or_else(|| unsafe_entry(&name))?;
        paths.push(path);
    }
    let top = paths
        .first()
        .and_then(|path| path.components().next())
        .map(|c| c.as_os_str().to_owned());
    let strip = top.filter(|top| {
        paths
            .iter()
            .all(|path| path.components().next().map(|c| c.as_os_str()) == Some(top.as_os_str()))
    });

    fs::create_dir_all(dest).map_err(io_err(dest))?;
    let mut links = Vec::new();
    for (index, path) in paths.iter().enumerate() {
        let relative = match &strip {
            Some(top) => path.strip_prefix(top).unwrap_or(path).to_path_buf(),
            None => path.clone(),
        };
        if relative.as_os_str().is_empty() {
            continue;
        }
        let mut entry = zip.by_index(index).map_err(|err| bad(err.to_string()))?;
        let name = entry
            .name()
            .map_err(|err| bad(err.to_string()))?
            .into_owned();
        // Checked again: the name is read anew, and must never be trusted unchecked.
        if natives::safe_entry_path(&name).as_ref() != Some(path) {
            return Err(unsafe_entry(&name));
        }
        let target = dest.join(&relative);
        let mode = entry.unix_mode();
        if entry.is_dir() {
            fs::create_dir_all(&target).map_err(io_err(&target))?;
        } else if is_symlink(mode) {
            let mut link_target = String::new();
            entry
                .read_to_string(&mut link_target)
                .map_err(|err| bad(err.to_string()))?;
            if !runtime::link_stays_inside(&relative, &link_target) {
                return Err(unsafe_entry(&name));
            }
            links.push((target, link_target, name));
        } else {
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(io_err(parent))?;
            }
            let mut out = File::create(&target).map_err(io_err(&target))?;
            io::copy(&mut entry, &mut out).map_err(io_err(&target))?;
            drop(out);
            #[cfg(unix)]
            if mode.is_some_and(|mode| mode & 0o111 != 0) {
                use std::os::unix::fs::PermissionsExt as _;
                fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
                    .map_err(io_err(&target))?;
            }
        }
    }
    // Links last, so none is followed while files are written.
    #[cfg(unix)]
    {
        for (link, link_target, _) in &links {
            if let Some(parent) = link.parent() {
                fs::create_dir_all(parent).map_err(io_err(parent))?;
            }
            if fs::symlink_metadata(link).is_ok() {
                continue;
            }
            std::os::unix::fs::symlink(link_target, link).map_err(io_err(link))?;
        }
        // Each link was checked on its own, which a link through another link can get
        // past (`a/l1 -> ..`, `a/l2 -> l1/..`): with all of them made, each must
        // really end inside, at something that exists.
        let inside = fs::canonicalize(dest).map_err(io_err(dest))?;
        for (link, _, name) in &links {
            if !fs::canonicalize(link).is_ok_and(|real| real.starts_with(&inside)) {
                return Err(unsafe_entry(name));
            }
        }
    }
    #[cfg(not(unix))]
    drop(links);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Write as _;

    use super::*;

    const SHA: &str = "5f805656659cf9a4b65b23006f2a7e3f68d898663721412408808dd92a6f4eb2";

    fn package(name: &str, java: [u64; 3], distro: [u64; 4]) -> ZuluPackage {
        ZuluPackage {
            name: name.to_string(),
            download_url: format!("{CDN}{name}"),
            java_version: java.to_vec(),
            distro_version: distro.to_vec(),
            sha256_hash: Some(SHA.to_string()),
            size: Some(40_346_800),
        }
    }

    /// Azul's answer as recorded in November 2026 (two builds, the newest first).
    #[test]
    fn picks_the_newest_build() {
        let answer = r#"[
            {"availability_type": "CA", "distro_version": [8, 98, 0, 111],
             "download_url": "https://cdn.azul.com/zulu/bin/zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip",
             "java_version": [8, 0, 512], "latest": true,
             "name": "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip", "openjdk_build_number": 9,
             "package_uuid": "626adbec-002d-4acf-b05f-133ac29b323d", "product": "zulu",
             "sha256_hash": "5f805656659cf9a4b65b23006f2a7e3f68d898663721412408808dd92a6f4eb2", "size": 40346800},
            {"availability_type": "CA", "distro_version": [8, 94, 0, 17],
             "download_url": "https://cdn.azul.com/zulu/bin/zulu8.94.0.17-ca-jre8.0.492-macosx_aarch64.zip",
             "java_version": [8, 0, 492], "latest": true,
             "name": "zulu8.94.0.17-ca-jre8.0.492-macosx_aarch64.zip", "openjdk_build_number": 9,
             "package_uuid": "38d2431e-2ada-4894-ac19-5822b09eef02", "product": "zulu",
             "sha256_hash": "d2175167d93ecdc39005582943a21f6470389d1044c08390e659d87148338cf8", "size": 40888000}
        ]"#;
        let mut packages: Vec<ZuluPackage> = serde_json::from_str(answer).unwrap();
        let build = pick(&packages, Zulu8::Newest).unwrap();
        assert_eq!(
            build.name,
            "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip"
        );
        assert_eq!(build.sha256, SHA);
        assert_eq!(build.size, Some(40_346_800));
        // The order of the answer doesn't matter.
        packages.reverse();
        assert_eq!(pick(&packages, Zulu8::Newest).unwrap(), build);
        assert_eq!(pick(&[], Zulu8::Newest), None);
    }

    #[test]
    fn refuses_builds_that_are_not_what_was_asked_for() {
        let good = package(
            "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip",
            [8, 0, 512],
            [8, 98, 0, 111],
        );
        assert!(pick(std::slice::from_ref(&good), Zulu8::Newest).is_some());
        let bad = [
            // Another host, or a URL that isn't the file named.
            ZuluPackage {
                download_url: format!("https://evil.example/zulu/bin/{}", good.name),
                ..good.clone()
            },
            ZuluPackage {
                download_url: format!("{CDN}x/{}", good.name),
                ..good.clone()
            },
            ZuluPackage {
                download_url: format!("{CDN}other.zip"),
                ..good.clone()
            },
            // Intel, Java 11, a JDK, a tarball.
            package(
                "zulu8.98.0.111-ca-jre8.0.512-macosx_x64.zip",
                [8, 0, 512],
                [8, 98, 0, 111],
            ),
            package(
                "zulu11.80.21-ca-jre11.0.27-macosx_aarch64.zip",
                [11, 0, 27],
                [11, 80, 21, 0],
            ),
            package(
                "zulu8.98.0.111-ca-jdk8.0.512-macosx_aarch64.zip",
                [8, 0, 512],
                [8, 98, 0, 111],
            ),
            package(
                "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.tar.gz",
                [8, 0, 512],
                [8, 98, 0, 111],
            ),
            // No usable checksum.
            ZuluPackage {
                sha256_hash: None,
                ..good.clone()
            },
            ZuluPackage {
                sha256_hash: Some("abc".into()),
                ..good.clone()
            },
            // A name that could leave the folder it is saved in.
            package(
                "zulu8/../-jre8-macosx_aarch64.zip",
                [8, 0, 512],
                [8, 98, 0, 111],
            ),
        ];
        for package in bad {
            assert_eq!(
                pick(std::slice::from_ref(&package), Zulu8::Newest),
                None,
                "{package:?}"
            );
        }
    }

    /// Forge with ModLauncher 8.0 gets the newest build before 8u321, from Azul's whole
    /// list; everything else the newest.
    #[test]
    fn modlauncher_8_0_gets_a_build_before_8u321() {
        let packages = [
            package(
                "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip",
                [8, 0, 512],
                [8, 98, 0, 111],
            ),
            package(
                "zulu8.60.0.21-ca-jre8.0.322-macosx_aarch64.zip",
                [8, 0, 322],
                [8, 60, 0, 21],
            ),
            package(
                "zulu8.58.0.13-ca-jre8.0.312-macosx_aarch64.zip",
                [8, 0, 312],
                [8, 58, 0, 13],
            ),
            package(
                "zulu8.56.0.23-ca-jre8.0.302-macosx_aarch64.zip",
                [8, 0, 302],
                [8, 56, 0, 23],
            ),
        ];
        assert_eq!(
            pick(&packages, Zulu8::Before8u321).unwrap().name,
            "zulu8.58.0.13-ca-jre8.0.312-macosx_aarch64.zip"
        );
        assert_eq!(
            pick(&packages, Zulu8::Newest).unwrap().name,
            "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip"
        );
        assert_eq!(pick(&packages[..2], Zulu8::Before8u321), None);

        let version = |modlauncher: &str| {
            VersionJson::parse(
                serde_json::json!({
                    "id": "1.16.5-forge",
                    "mainClass": "cpw.mods.modlauncher.Launcher",
                    "libraries": [
                        {"name": "net.minecraftforge:forge:1.16.5-36.2.20"},
                        {"name": format!("cpw.mods:modlauncher:{modlauncher}")},
                    ],
                })
                .to_string()
                .as_bytes(),
            )
            .unwrap()
        };
        // Forge 1.16.5-36.2.20, 1.16.3-34.1.42, and 36.2.26 and later, 1.15.2, 1.16.1.
        assert_eq!(Zulu8::for_version(&version("8.0.9")), Zulu8::Before8u321);
        assert_eq!(Zulu8::for_version(&version("8.0.6")), Zulu8::Before8u321);
        for fixed in ["8.1.3", "5.1.2", "6.1.1", "4.1.0"] {
            assert_eq!(
                Zulu8::for_version(&version(fixed)),
                Zulu8::Newest,
                "{fixed}"
            );
        }
        assert_eq!(Zulu8::Before8u321.component(), "zulu-8-before-8u321");

        // Each line keeps its own archives in the cache.
        let new = "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip";
        let old = "zulu8.58.0.13-ca-jre8.0.312-macosx_aarch64.zip";
        assert!(Zulu8::Newest.owns_archive(new) && !Zulu8::Newest.owns_archive(old));
        assert!(Zulu8::Before8u321.owns_archive(old) && !Zulu8::Before8u321.owns_archive(new));
        assert!(!Zulu8::Newest.owns_archive("zulu8.zip"));
    }

    fn zip_file(entries: &[(&str, Option<&str>, u32, &[u8])]) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut zip = zip::ZipWriter::new(file.reopen().unwrap());
        for (name, link, mode, contents) in entries {
            let options = zip::write::SimpleFileOptions::default().unix_permissions(*mode);
            match link {
                Some(target) => zip.add_symlink(*name, *target, options).unwrap(),
                None if name.ends_with('/') => zip.add_directory(*name, options).unwrap(),
                None => {
                    zip.start_file(*name, options).unwrap();
                    zip.write_all(contents).unwrap();
                }
            }
        }
        zip.finish().unwrap();
        file
    }

    #[cfg(unix)]
    #[test]
    fn unpacks_a_zulu_layout_and_finds_java() {
        use std::os::unix::fs::PermissionsExt as _;
        let top = "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64";
        let archive = zip_file(&[
            (&format!("{top}/"), None, 0o755, b""),
            (
                &format!("{top}/zulu-8.jre/Contents/Home/bin/java"),
                None,
                0o755,
                b"java",
            ),
            (
                &format!("{top}/zulu-8.jre/Contents/Home/lib/rt.jar"),
                None,
                0o644,
                b"rt",
            ),
            (
                &format!("{top}/bin"),
                Some("zulu-8.jre/Contents/Home/bin"),
                0o777,
                b"",
            ),
        ]);
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("rt");
        unpack(archive.path(), &dest).unwrap();
        let java = java_binary(&dest).unwrap();
        assert_eq!(java, dest.join("zulu-8.jre/Contents/Home/bin/java"));
        assert_ne!(fs::metadata(&java).unwrap().permissions().mode() & 0o111, 0);
        let rt = fs::metadata(dest.join("zulu-8.jre/Contents/Home/lib/rt.jar")).unwrap();
        assert_eq!(rt.permissions().mode() & 0o111, 0);
        assert_eq!(fs::read(dest.join("bin/java")).unwrap(), b"java");
    }

    /// Each build has its own folder; the newest one installed is used, and the rest
    /// is what the storage cleanup may remove.
    #[test]
    fn the_newest_installed_build_is_used() {
        let dir = tempfile::tempdir().unwrap();
        let build = |name: &str, installed: bool| {
            let home = dir.path().join(name).join("zulu-8.jre/Contents/Home/bin");
            fs::create_dir_all(&home).unwrap();
            fs::write(home.join("java"), b"").unwrap();
            if installed {
                runtime::mark_installed(&dir.path().join(name), SHA).unwrap();
            }
            dir.path().join(name)
        };
        assert_eq!(newest_installed(dir.path()), None);
        let old = build("zulu8.94.0.17-ca-jre8.0.492-macosx_aarch64", true);
        let new = build("zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64", true);
        // Never finished: not marked installed.
        let half = build("zulu8.99.0.1-ca-jre8.0.522-macosx_aarch64", false);
        let staging = build(
            ".zulu8.99.0.1-ca-jre8.0.522-macosx_aarch64.unpacking-1-2",
            false,
        );
        assert_eq!(
            newest_installed(dir.path()),
            Some(new.join("zulu-8.jre/Contents/Home/bin/java"))
        );
        let mut older = vec![old, half, staging];
        older.sort();
        assert_eq!(older_builds(dir.path()), older);
        assert_eq!(
            build_folder(&ZuluBuild {
                name: "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64.zip".into(),
                url: String::new(),
                sha256: SHA.into(),
                size: None,
            }),
            "zulu8.98.0.111-ca-jre8.0.512-macosx_aarch64"
        );
        assert_eq!(
            distro_version("zulu8.98.0.111-ca"),
            Some(vec![8, 98, 0, 111])
        );
        assert_eq!(distro_version("zulu-8.jre"), None);
    }

    #[test]
    fn finds_java_in_a_renamed_bundle_or_a_plain_home() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(java_binary(dir.path()), None);
        let renamed = dir.path().join("zulu-8.0.512.jre/Contents/Home/bin");
        fs::create_dir_all(&renamed).unwrap();
        fs::write(renamed.join("java"), b"").unwrap();
        assert_eq!(java_binary(dir.path()), Some(renamed.join("java")));

        let plain = tempfile::tempdir().unwrap();
        fs::create_dir_all(plain.path().join("bin")).unwrap();
        fs::write(plain.path().join("bin/java"), b"").unwrap();
        assert_eq!(
            java_binary(plain.path()),
            Some(plain.path().join("bin/java"))
        );
    }

    #[test]
    fn refuses_entries_and_links_that_leave_the_folder() {
        let dir = tempfile::tempdir().unwrap();
        for (i, entries) in [
            vec![("top/../../evil", None, 0o644, &b"x"[..])],
            vec![("/abs", None, 0o644, &b"x"[..])],
            vec![("top/link", Some("../../etc/passwd"), 0o777, &b""[..])],
            vec![("top/link", Some("/etc/passwd"), 0o777, &b""[..])],
            // Each fine on its own, out of the folder together.
            vec![
                ("top/a/b/", None, 0o755, &b""[..]),
                ("top/a/b/l3", Some("../.."), 0o777, &b""[..]),
                ("top/a/b/l4", Some("l3/../.."), 0o777, &b""[..]),
            ],
            // A link to nothing.
            vec![("top/link", Some("missing"), 0o777, &b""[..])],
        ]
        .into_iter()
        .enumerate()
        {
            let archive = zip_file(&entries);
            let dest = dir.path().join(format!("rt{i}"));
            assert!(unpack(archive.path(), &dest).is_err(), "{entries:?}");
            assert!(!dir.path().join("evil").exists());
        }
    }
}
