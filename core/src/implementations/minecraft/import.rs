//! Importing an existing Minecraft server, a folder or a `.zip` of one, into a new instance.
//!
//! The caller must already have checked that the requester may read the source (see
//! `handlers::instance`). The checks here protect the new instance and the host: nothing
//! outside the source is read, links are refused and never followed, zip entries cannot
//! leave the instance folder, and a failed import leaves nothing behind.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use color_eyre::eyre::{eyre, Context};
use tracing::info;

use crate::error::{Error, ErrorKind};
use crate::handlers::util::{is_in_macros_dir, is_script_path};

use super::properties::PropertiesDocument;
use super::write_file_atomically;

/// The most entries a zip may have.
const MAX_ZIP_ENTRIES: usize = 1_000_000;

/// Top-level folders of a zip that are ignored when looking for the server in it.
const ZIP_JUNK: [&str; 1] = ["__macosx"];

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

/// A name the way Windows and macOS resolve it: ignoring case and trailing dots and spaces.
fn normalized_name(name: &OsStr) -> Option<String> {
    name.to_str()
        .map(|name| name.trim_end_matches(['.', ' ']).to_lowercase())
}

/// Whether a top-level entry is never imported: Lodestone's own files (so importing another
/// Lodestone instance does not replace the identity or the settings, such as the Java
/// command, of the new one) and the macros folder, whose scripts Lodestone runs.
fn is_skipped(name: &OsStr) -> bool {
    is_in_macros_dir(Path::new(name))
        || normalized_name(name).is_some_and(|name| name.starts_with(".lodestone"))
}

/// Refuses a name that would not mean the same file everywhere. On Windows: reserved device
/// names (`CON`, `NUL`, ...), characters Windows does not allow (`:` would write an
/// alternate data stream), and a trailing dot or space, which Windows drops (so `macros.`
/// would become `macros`).
fn check_name(name: &OsStr, relative: &Path) -> Result<(), Error> {
    let unsafe_name = || {
        bad_request(format!(
            "{} has a name that cannot be imported",
            relative.display()
        ))
    };
    let Some(name) = name.to_str() else {
        return if cfg!(windows) {
            Err(unsafe_name())
        } else {
            Ok(())
        };
    };
    if name.is_empty() || name == "." || name == ".." || name.chars().any(char::is_control) {
        return Err(unsafe_name());
    }
    if cfg!(windows) {
        if name.ends_with(['.', ' ']) || name.contains(['<', '>', ':', '"', '|', '?', '*', '\\']) {
            return Err(unsafe_name());
        }
        let stem = name
            .split('.')
            .next()
            .unwrap_or_default()
            .trim_end()
            .to_uppercase();
        let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || ((stem.starts_with("COM") || stem.starts_with("LPT"))
                && stem.len() == 4
                && stem.as_bytes()[3].is_ascii_digit());
        if reserved {
            return Err(unsafe_name());
        }
    }
    Ok(())
}

/// Refuses a script file unless the requester may add scripts: like the file APIs (see
/// `handlers::util::check_code_write`), only the owner, while code execution is on.
fn check_script(relative: &Path, allow_scripts: bool) -> Result<(), Error> {
    if !allow_scripts && is_script_path(relative) {
        return Err(Error {
            kind: ErrorKind::PermissionDenied,
            source: eyre!(
                "{} is a script file: only the owner can import a server with script files, while code execution is on",
                relative.display()
            ),
        });
    }
    Ok(())
}

fn refused(path: &Path) -> Error {
    bad_request(format!(
        "{} is a symbolic link or special file, which cannot be imported",
        path.display()
    ))
}

/// What is imported: a server folder, or a zip file holding one.
enum Source {
    Folder,
    Zip,
}

fn source_kind(source: &Path) -> Result<Source, Error> {
    let metadata = std::fs::symlink_metadata(source)
        .context(format!("Failed to read {}", source.display()))?;
    let file_type = metadata.file_type();
    if file_type.is_symlink() {
        return Err(refused(source));
    }
    let is_zip = source
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"));
    if file_type.is_dir() {
        Ok(Source::Folder)
    } else if file_type.is_file() && is_zip {
        Ok(Source::Zip)
    } else {
        Err(bad_request(format!(
            "{} is not a folder or a .zip file",
            source.display()
        )))
    }
}

/// Checks that `source` is a server folder, or a `.zip` of one, that can be imported: it
/// holds a `server.jar`, and has no links, special files, unsafe names, or (unless
/// `allow_scripts`) script files.
pub async fn validate_source(source: PathBuf, allow_scripts: bool) -> Result<(), Error> {
    tokio::task::spawn_blocking(move || match source_kind(&source)? {
        Source::Folder => {
            let jar = std::fs::symlink_metadata(source.join("server.jar"));
            if !jar.map(|m| m.is_file()).unwrap_or(false) {
                return Err(bad_request(
                    "The existing server folder must contain a server.jar",
                ));
            }
            walk(&source, Path::new(""), allow_scripts)
        }
        Source::Zip => {
            let mut archive = open_zip(&source)?;
            plan_zip(&mut archive, allow_scripts).map(|_| ())
        }
    })
    .await
    .context("The import check task failed")?
}

fn walk(root: &Path, relative: &Path, allow_scripts: bool) -> Result<(), Error> {
    let dir = root.join(relative);
    for entry in std::fs::read_dir(&dir).context(format!("Failed to read {}", dir.display()))? {
        let entry = entry.context(format!("Failed to read {}", dir.display()))?;
        let name = entry.file_name();
        if relative.as_os_str().is_empty() && is_skipped(&name) {
            continue;
        }
        let entry_relative = relative.join(&name);
        check_name(&name, &entry_relative)?;
        let file_type = entry
            .file_type()
            .context(format!("Failed to read {}", entry.path().display()))?;
        if file_type.is_dir() {
            walk(root, &entry_relative, allow_scripts)?;
        } else if file_type.is_file() {
            check_script(&entry_relative, allow_scripts)?;
        } else {
            return Err(refused(&entry.path()));
        }
    }
    Ok(())
}

/// Copies the contents of `source`, a server folder or a `.zip` of one, into the existing
/// folder `destination`. Every check of [`validate_source`] is made again while copying.
///
/// The files go into a temporary folder inside `destination` first and are then moved into
/// place, so a failure never leaves a half-imported server: whatever was moved is removed
/// again, and files that were in `destination` before are never touched.
pub async fn import_into(
    source: PathBuf,
    destination: PathBuf,
    allow_scripts: bool,
) -> Result<(), Error> {
    tokio::task::spawn_blocking(move || import_blocking(&source, &destination, allow_scripts))
        .await
        .context("The import task failed")?
}

fn import_blocking(source: &Path, destination: &Path, allow_scripts: bool) -> Result<(), Error> {
    let kind = source_kind(source)?;
    let canonical_source =
        std::fs::canonicalize(source).context(format!("Failed to resolve {}", source.display()))?;
    // The handler checked the canonical path. If it resolves elsewhere now, one of its
    // folders was swapped for a link since.
    if canonical_source != source {
        return Err(refused(source));
    }
    let canonical_destination = std::fs::canonicalize(destination)
        .context(format!("Failed to resolve {}", destination.display()))?;
    if matches!(kind, Source::Folder) && canonical_destination.starts_with(&canonical_source) {
        return Err(bad_request(
            "The existing server folder cannot contain the new instance",
        ));
    }

    let staging = tempfile::Builder::new()
        .prefix(".lodestone-import-")
        .tempdir_in(destination)
        .context("Failed to create a temporary folder for the import")?;
    match kind {
        Source::Folder => copy_tree(
            &canonical_source,
            Path::new(""),
            staging.path(),
            allow_scripts,
        )?,
        Source::Zip => extract_zip(&canonical_source, staging.path(), allow_scripts)?,
    }

    let mut moved: Vec<PathBuf> = Vec::new();
    let result = (|| -> Result<(), Error> {
        for entry in std::fs::read_dir(staging.path())
            .context("Failed to read the temporary import folder")?
        {
            let entry = entry.context("Failed to read the temporary import folder")?;
            let target = destination.join(entry.file_name());
            if std::fs::symlink_metadata(&target).is_ok() {
                return Err(eyre!("{} already exists", target.display()).into());
            }
            std::fs::rename(entry.path(), &target)
                .context(format!("Failed to move {}", target.display()))?;
            moved.push(target);
        }
        Ok(())
    })();
    if let Err(e) = result {
        for path in moved {
            let removed = match std::fs::symlink_metadata(&path) {
                Ok(m) if m.is_dir() => std::fs::remove_dir_all(&path),
                _ => std::fs::remove_file(&path),
            };
            if let Err(e) = removed {
                tracing::error!(
                    "Failed to remove {} after the import failed: {e}",
                    path.display()
                );
            }
        }
        return Err(e);
    }
    Ok(())
}

/// Copies the folder `root/relative` into `staging/relative`, refusing links and special
/// files. `root` is canonical.
fn copy_tree(
    root: &Path,
    relative: &Path,
    staging: &Path,
    allow_scripts: bool,
) -> Result<(), Error> {
    let dir = root.join(relative);
    // `read_dir` follows a folder that was swapped for a link after it was checked, so
    // check each folder again right before reading it
    let resolved =
        std::fs::canonicalize(&dir).context(format!("Failed to resolve {}", dir.display()))?;
    if resolved != dir {
        return Err(refused(&dir));
    }
    for entry in std::fs::read_dir(&dir).context(format!("Failed to read {}", dir.display()))? {
        let entry = entry.context(format!("Failed to read {}", dir.display()))?;
        let name = entry.file_name();
        if relative.as_os_str().is_empty() && is_skipped(&name) {
            info!("Not importing {}", entry.path().display());
            continue;
        }
        let entry_relative = relative.join(&name);
        check_name(&name, &entry_relative)?;
        let from = entry.path();
        let to = staging.join(&entry_relative);
        // does not follow links
        let file_type = entry
            .file_type()
            .context(format!("Failed to read {}", from.display()))?;
        if file_type.is_dir() {
            std::fs::create_dir(&to).context(format!("Failed to create {}", to.display()))?;
            copy_tree(root, &entry_relative, staging, allow_scripts)?;
        } else if file_type.is_file() {
            check_script(&entry_relative, allow_scripts)?;
            copy_file_no_follow(&from, &to)?;
        } else {
            return Err(refused(&from));
        }
    }
    Ok(())
}

/// Opens the regular file `path` for reading without following a link, so a file swapped
/// for a link after it was checked is refused rather than followed, and without blocking on
/// a named pipe.
fn open_no_follow(path: &Path) -> Result<std::fs::File, Error> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        // FILE_FLAG_OPEN_REPARSE_POINT: opens a link or junction itself, not its target
        options.custom_flags(0x0020_0000);
    }
    let file = options.open(path).map_err(|_| refused(path))?;
    let metadata = file
        .metadata()
        .context(format!("Failed to read {}", path.display()))?;
    if !metadata.is_file() {
        return Err(refused(path));
    }
    Ok(file)
}

/// Copies the regular file `from` to the new file `to`, see [`open_no_follow`].
fn copy_file_no_follow(from: &Path, to: &Path) -> Result<(), Error> {
    let mut input = open_no_follow(from)?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .context(format!("Failed to create {}", to.display()))?;
    std::io::copy(&mut input, &mut output).context(format!(
        "Failed to copy {} to {}",
        from.display(),
        to.display()
    ))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = input
            .metadata()
            .context(format!("Failed to read {}", from.display()))?
            .permissions()
            .mode()
            & 0o777;
        output
            .set_permissions(std::fs::Permissions::from_mode(mode))
            .context(format!("Failed to set the permissions of {}", to.display()))?;
    }
    Ok(())
}

fn open_zip(path: &Path) -> Result<zip::ZipArchive<std::fs::File>, Error> {
    let file = open_no_follow(path)?;
    zip::ZipArchive::new(file)
        .map_err(|e| bad_request(format!("{} is not a valid zip file: {e}", path.display())))
}

/// An entry of a zip to extract.
struct ZipEntry {
    index: usize,
    /// Where it goes, relative to the instance folder
    relative: PathBuf,
    is_dir: bool,
    size: u64,
}

/// The names in the path of a zip entry, or `None` if the path is absolute, has a drive, or
/// goes up a folder. Both `/` and `\` separate names, so an entry means the same on every
/// platform.
fn zip_entry_components(raw_name: &str) -> Option<Vec<String>> {
    if raw_name.starts_with(['/', '\\']) {
        return None;
    }
    let mut components = Vec::new();
    for part in raw_name.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => return None,
            part => components.push(part.to_string()),
        }
    }
    let has_prefix = components.first().is_some_and(|first| {
        first.contains(':')
            || Path::new(first)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
    });
    if has_prefix {
        return None;
    }
    Some(components)
}

/// Reads the zip's central directory and works out what to extract: the server's files,
/// at the root of the zip or in its only folder. Refuses links, encrypted entries, unsafe
/// paths and names, (unless `allow_scripts`) script files, and zips without a `server.jar`.
fn plan_zip(
    archive: &mut zip::ZipArchive<std::fs::File>,
    allow_scripts: bool,
) -> Result<Vec<ZipEntry>, Error> {
    if archive.len() > MAX_ZIP_ENTRIES {
        return Err(bad_request(format!(
            "The zip has more than {MAX_ZIP_ENTRIES} entries"
        )));
    }
    let read_error = |e: zip::result::ZipError| bad_request(format!("Failed to read the zip: {e}"));
    let mut entries: Vec<(usize, Vec<String>, bool, u64)> = Vec::new();
    for index in 0..archive.len() {
        let file = archive.by_index_raw(index).map_err(read_error)?;
        let raw_name = file.name().map_err(read_error)?.to_string();
        let components = match (zip_entry_components(&raw_name), file.enclosed_name()) {
            (Some(components), Some(_)) => components,
            _ => {
                return Err(bad_request(format!(
                    "The zip entry {raw_name} points outside the server folder"
                )))
            }
        };
        if file.is_symlink() {
            return Err(bad_request(format!(
                "The zip entry {raw_name} is a symbolic link, which cannot be imported"
            )));
        }
        if file.encrypted() {
            return Err(bad_request(format!(
                "The zip entry {raw_name} is encrypted, which cannot be imported"
            )));
        }
        let Some(first) = components.first() else {
            continue;
        };
        if ZIP_JUNK.contains(&first.to_lowercase().as_str()) {
            continue;
        }
        entries.push((index, components, file.is_dir(), file.size()));
    }

    let is_jar =
        |components: &[String], is_dir: bool| !is_dir && components == ["server.jar".to_string()];
    let strip = if entries.iter().any(|(_, c, d, _)| is_jar(c, *d)) {
        0
    } else {
        let first = entries.first().map(|(_, c, _, _)| c[0].clone());
        let single_folder = first.as_ref().is_some_and(|first| {
            entries
                .iter()
                .all(|(_, c, d, _)| &c[0] == first && (*d || c.len() > 1))
        });
        let jar_inside = entries
            .iter()
            .any(|(_, c, d, _)| c.len() > 1 && is_jar(&c[1..], *d));
        if single_folder && jar_inside {
            1
        } else {
            return Err(bad_request(
                "The zip must contain a server.jar, at its root or in its only folder",
            ));
        }
    };

    let mut plan = Vec::new();
    for (index, components, is_dir, size) in entries {
        let components = &components[strip..];
        let Some(first) = components.first() else {
            continue;
        };
        if is_skipped(OsStr::new(first)) {
            continue;
        }
        let mut relative = PathBuf::new();
        for name in components {
            relative.push(name);
            check_name(OsStr::new(name), &relative)?;
        }
        if !is_dir {
            check_script(&relative, allow_scripts)?;
        }
        plan.push(ZipEntry {
            index,
            relative,
            is_dir,
            size,
        });
    }
    Ok(plan)
}

/// Free space on the disk holding the canonical `path`, if it can be found.
fn available_space(path: &Path) -> Option<u64> {
    // canonical paths on Windows are verbatim (`\\?\C:\...`), mount points are not
    #[cfg(windows)]
    let path = &match path.to_str().and_then(|p| p.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC\\") => PathBuf::from(rest),
        _ => path.to_path_buf(),
    };
    let disks = sysinfo::Disks::new_with_refreshed_list();
    disks
        .list()
        .iter()
        .filter(|disk| path.starts_with(disk.mount_point()))
        .max_by_key(|disk| disk.mount_point().as_os_str().len())
        .map(|disk| disk.available_space())
}

/// Extracts the server in the zip at `source` into the empty folder `staging`.
fn extract_zip(source: &Path, staging: &Path, allow_scripts: bool) -> Result<(), Error> {
    let mut archive = open_zip(source)?;
    let plan = plan_zip(&mut archive, allow_scripts)?;
    let total = plan.iter().map(|e| e.size).fold(0, u64::saturating_add);
    let canonical_staging = std::fs::canonicalize(staging)
        .context(format!("Failed to resolve {}", staging.display()))?;
    if let Some(available) = available_space(&canonical_staging) {
        if total > available {
            return Err(bad_request(format!(
                "The zip holds {total} bytes, more than the {available} bytes free on the disk"
            )));
        }
    }
    for entry in plan {
        let target = staging.join(&entry.relative);
        if entry.is_dir {
            std::fs::create_dir_all(&target)
                .context(format!("Failed to create {}", entry.relative.display()))?;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .context(format!("Failed to create {}", parent.display()))?;
        }
        let mut file = archive
            .by_index(entry.index)
            .map_err(|e| bad_request(format!("Failed to read the zip: {e}")))?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|e| {
                bad_request(format!(
                    "Failed to create {} (is it in the zip twice?): {e}",
                    entry.relative.display()
                ))
            })?;
        // a zip cannot write more than its headers say, so the free space check holds
        let copied = std::io::copy(
            &mut (&mut file).take(entry.size.saturating_add(1)),
            &mut output,
        )
        .map_err(|e| {
            bad_request(format!(
                "Failed to extract {}: {e}",
                entry.relative.display()
            ))
        })?;
        if copied != entry.size {
            return Err(bad_request(format!(
                "{} does not have the size the zip says",
                entry.relative.display()
            )));
        }
        output
            .flush()
            .context(format!("Failed to write {}", entry.relative.display()))?;
        #[cfg(unix)]
        if let Some(mode) = file.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            output
                .set_permissions(std::fs::Permissions::from_mode(mode & 0o777))
                .context(format!(
                    "Failed to set the permissions of {}",
                    entry.relative.display()
                ))?;
        }
    }
    Ok(())
}

/// Sets `server-port` in an imported `server.properties`, keeping every other line (and
/// comment) as it was.
pub async fn set_port(path: &Path, port: u32) -> Result<(), Error> {
    let bytes = tokio::fs::read(path)
        .await
        .context(format!("Failed to read {}", path.display()))?;
    let (mut document, _warnings) = PropertiesDocument::from_bytes(&bytes);
    document.set("server-port", &port.to_string());
    write_file_atomically(path, document.to_bytes()).await
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The handler gives the import the canonical path it checked.
    fn canon(path: &Path) -> PathBuf {
        std::fs::canonicalize(path).unwrap()
    }

    fn sample_server(root: &Path) -> PathBuf {
        let server = root.join("My Server-506d951c");
        std::fs::create_dir_all(server.join("world").join("region")).unwrap();
        std::fs::write(server.join("server.jar"), b"jar bytes").unwrap();
        std::fs::write(
            server.join("server.properties"),
            "#Minecraft server properties\nmotd=Hello\nserver-port=25570\n",
        )
        .unwrap();
        std::fs::write(server.join("eula.txt"), "eula=false\n").unwrap();
        std::fs::write(server.join("world").join("level.dat"), b"level").unwrap();
        std::fs::write(server.join("world/region/r.0.0.mca"), b"region").unwrap();
        server
    }

    fn instance_dir(root: &Path) -> PathBuf {
        let instance = root.join("instances").join("new instance-12345678");
        std::fs::create_dir_all(&instance).unwrap();
        std::fs::write(instance.join(".lodestone_config"), "new identity").unwrap();
        instance
    }

    #[tokio::test]
    async fn imports_a_server_with_spaces_in_its_path() {
        let root = tempfile::tempdir().unwrap();
        let server = sample_server(root.path());
        let instance = instance_dir(root.path());
        validate_source(server.clone(), true).await.unwrap();
        import_into(canon(&server), instance.clone(), true)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(instance.join("server.jar")).unwrap(),
            b"jar bytes"
        );
        assert_eq!(
            std::fs::read(instance.join("world/level.dat")).unwrap(),
            b"level"
        );
        assert_eq!(
            std::fs::read(instance.join("world/region/r.0.0.mca")).unwrap(),
            b"region"
        );
        assert_eq!(
            std::fs::read_to_string(instance.join("eula.txt")).unwrap(),
            "eula=false\n"
        );
        assert_eq!(
            std::fs::read_to_string(instance.join(".lodestone_config")).unwrap(),
            "new identity"
        );
        // the source is untouched and no temporary folder is left
        assert!(server.join("server.jar").exists());
        let names: Vec<_> = std::fs::read_dir(&instance)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        assert!(!names.iter().any(|n| n.starts_with(".lodestone-import-")));
    }

    #[tokio::test]
    async fn lodestone_files_and_macros_are_not_imported() {
        let root = tempfile::tempdir().unwrap();
        let server = sample_server(root.path());
        std::fs::write(server.join(".lodestone_config"), "old identity").unwrap();
        std::fs::write(server.join(".lodestone_minecraft_config.json"), "{}").unwrap();
        std::fs::create_dir_all(server.join("macros")).unwrap();
        std::fs::write(server.join("macros/run.ts"), "Deno.exit(1)").unwrap();
        let instance = instance_dir(root.path());
        import_into(canon(&server), instance.clone(), true)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(instance.join(".lodestone_config")).unwrap(),
            "new identity"
        );
        assert!(!instance.join(".lodestone_minecraft_config.json").exists());
        assert!(!instance.join("macros").exists());
    }

    #[tokio::test]
    async fn a_server_needs_a_jar() {
        let root = tempfile::tempdir().unwrap();
        let server = sample_server(root.path());
        std::fs::remove_file(server.join("server.jar")).unwrap();
        assert!(validate_source(server, true).await.is_err());
        assert!(validate_source(root.path().join("missing"), true)
            .await
            .is_err());
        assert!(
            validate_source(root.path().join("My Server-506d951c/world/level.dat"), true)
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_folder_that_holds_the_instance_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let instance = instance_dir(root.path());
        std::fs::write(root.path().join("server.jar"), b"jar").unwrap();
        let err = import_into(canon(root.path()), instance.clone(), true)
            .await
            .unwrap_err();
        assert!(err.source.to_string().contains("cannot contain"));
        assert_eq!(std::fs::read_dir(&instance).unwrap().count(), 1);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symbolic_links_are_refused_and_not_followed() {
        let root = tempfile::tempdir().unwrap();
        let server = sample_server(root.path());
        let secret = root.path().join("secret.txt");
        std::fs::write(&secret, "secret").unwrap();
        std::os::unix::fs::symlink(&secret, server.join("world/stolen.txt")).unwrap();
        assert!(validate_source(server.clone(), true).await.is_err());
        let instance = instance_dir(root.path());
        // even if the link appears after validation, the copy refuses it
        assert!(import_into(canon(&server), instance.clone(), true)
            .await
            .is_err());
        assert_eq!(std::fs::read_dir(&instance).unwrap().count(), 1);

        let linked_root = root.path().join("linked");
        std::os::unix::fs::symlink(&server, &linked_root).unwrap();
        assert!(validate_source(linked_root.clone(), true).await.is_err());
        assert!(import_into(linked_root, instance, true).await.is_err());
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn junctions_are_refused_and_not_followed() {
        let root = tempfile::tempdir().unwrap();
        let server = sample_server(root.path());
        let outside = root.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        let junction = server.join("world").join("link");
        let status = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&outside)
            .output()
            .unwrap();
        assert!(status.status.success(), "mklink failed");
        assert!(validate_source(server.clone(), true).await.is_err());
        let instance = instance_dir(root.path());
        assert!(import_into(canon(&server), instance.clone(), true)
            .await
            .is_err());
        assert_eq!(std::fs::read_dir(&instance).unwrap().count(), 1);
        assert!(outside.join("secret.txt").exists());
    }

    #[tokio::test]
    async fn a_failed_move_removes_what_was_imported() {
        let root = tempfile::tempdir().unwrap();
        let server = sample_server(root.path());
        let instance = instance_dir(root.path());
        // a file in the instance that collides with an imported one makes the import fail
        // part way, whichever order the entries are moved in
        std::fs::write(instance.join("world"), "in the way").unwrap();
        std::fs::write(instance.join("server.properties"), "mine").unwrap();
        let err = import_into(canon(&server), instance.clone(), true)
            .await
            .unwrap_err();
        assert!(err.source.to_string().contains("already exists"));
        let mut names: Vec<_> = std::fs::read_dir(&instance)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        assert_eq!(names, [".lodestone_config", "server.properties", "world"]);
        assert_eq!(
            std::fs::read_to_string(instance.join("world")).unwrap(),
            "in the way"
        );
        assert_eq!(
            std::fs::read_to_string(instance.join("server.properties")).unwrap(),
            "mine"
        );
    }

    #[tokio::test]
    async fn a_failed_copy_leaves_nothing_behind() {
        let root = tempfile::tempdir().unwrap();
        let server = sample_server(root.path());
        let instance = instance_dir(root.path());
        // the temporary folder cannot be created, because the destination is not a folder
        let not_a_folder = root.path().join("file");
        std::fs::write(&not_a_folder, "x").unwrap();
        assert!(import_into(canon(&server), not_a_folder.clone(), true)
            .await
            .is_err());
        assert_eq!(std::fs::read_to_string(&not_a_folder).unwrap(), "x");
        assert_eq!(std::fs::read_dir(&instance).unwrap().count(), 1);
    }

    #[tokio::test]
    async fn set_port_keeps_the_other_lines() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("server.properties");
        std::fs::write(
            &path,
            "#Minecraft server properties\nmotd=Hello world\nserver-port=25570\nlevel-name=world\n",
        )
        .unwrap();
        set_port(&path, 25599).await.unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("#Minecraft server properties"));
        assert!(text.contains("motd=Hello world"));
        assert!(text.contains("level-name=world"));
        assert!(text.contains("server-port=25599"));
        assert!(!text.contains("25570"));

        std::fs::write(&path, "motd=x\n").unwrap();
        set_port(&path, 25565).await.unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("motd=x") && text.contains("server-port=25565"));
    }

    fn write_zip(path: &Path, entries: &[(&str, Option<&[u8]>)]) {
        let file = std::fs::File::create(path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        for (name, content) in entries {
            match content {
                Some(content) => {
                    writer.start_file(*name, options).unwrap();
                    writer.write_all(content).unwrap();
                }
                None => writer.add_directory(*name, options).unwrap(),
            }
        }
        writer.finish().unwrap();
    }

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
            .collect();
        names.sort();
        names
    }

    #[tokio::test]
    async fn imports_a_zip_with_the_server_at_its_root() {
        let root = tempfile::tempdir().unwrap();
        let zip = root.path().join("My Server.ZIP");
        write_zip(
            &zip,
            &[
                ("server.jar", Some(b"jar bytes")),
                ("world/", None),
                ("world/level.dat", Some(b"level")),
                ("server.properties", Some(b"server-port=25570\n")),
            ],
        );
        let instance = instance_dir(root.path());
        validate_source(zip.clone(), true).await.unwrap();
        import_into(canon(&zip), instance.clone(), true)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(instance.join("server.jar")).unwrap(),
            b"jar bytes"
        );
        assert_eq!(
            std::fs::read(instance.join("world/level.dat")).unwrap(),
            b"level"
        );
        assert_eq!(
            names(&instance),
            [
                ".lodestone_config",
                "server.jar",
                "server.properties",
                "world"
            ]
        );
        assert!(zip.exists());
    }

    #[tokio::test]
    async fn imports_a_zip_with_the_server_in_its_only_folder() {
        let root = tempfile::tempdir().unwrap();
        let zip = root.path().join("server.zip");
        write_zip(
            &zip,
            &[
                ("survival/", None),
                ("survival/server.jar", Some(b"jar")),
                ("survival/world/level.dat", Some(b"level")),
                ("__MACOSX/survival/._server.jar", Some(b"junk")),
            ],
        );
        let instance = instance_dir(root.path());
        validate_source(zip.clone(), true).await.unwrap();
        import_into(canon(&zip), instance.clone(), true)
            .await
            .unwrap();
        assert_eq!(
            names(&instance),
            [".lodestone_config", "server.jar", "world"]
        );
        assert_eq!(
            std::fs::read(instance.join("world/level.dat")).unwrap(),
            b"level"
        );
    }

    #[tokio::test]
    async fn a_zip_needs_a_jar() {
        let root = tempfile::tempdir().unwrap();
        let zip = root.path().join("server.zip");
        write_zip(&zip, &[("world/level.dat", Some(b"level"))]);
        assert!(validate_source(zip.clone(), true).await.is_err());
        // two folders: which one is the server is unclear
        write_zip(
            &zip,
            &[
                ("a/server.jar", Some(b"jar")),
                ("b/server.jar", Some(b"jar")),
            ],
        );
        assert!(validate_source(zip.clone(), true).await.is_err());
        // not a zip
        std::fs::write(&zip, "not a zip").unwrap();
        assert!(validate_source(zip, true).await.is_err());
        let other = root.path().join("server.tar");
        std::fs::write(&other, "x").unwrap();
        assert!(validate_source(other, true).await.is_err());
    }

    #[tokio::test]
    async fn zip_entries_cannot_leave_the_instance() {
        let root = tempfile::tempdir().unwrap();
        for (i, bad) in [
            "../evil.txt",
            "world/../../evil.txt",
            "..\\evil.txt",
            "/evil.txt",
            "\\evil.txt",
            "C:/evil.txt",
            "C:evil.txt",
        ]
        .into_iter()
        .enumerate()
        {
            let zip = root.path().join("server.zip");
            write_zip(&zip, &[("server.jar", Some(b"jar")), (bad, Some(b"evil"))]);
            assert!(
                validate_source(zip.clone(), true).await.is_err(),
                "{bad} was accepted"
            );
            let instance = instance_dir(&root.path().join(i.to_string()));
            assert!(
                import_into(canon(&zip), instance.clone(), true)
                    .await
                    .is_err(),
                "{bad} was imported"
            );
            assert_eq!(names(&instance), [".lodestone_config"]);
        }
        assert!(!root.path().join("evil.txt").exists());
        assert!(!root.path().join("instances/evil.txt").exists());
    }

    #[tokio::test]
    async fn zip_links_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let zip = root.path().join("server.zip");
        let file = std::fs::File::create(&zip).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default();
        writer.start_file("server.jar", options).unwrap();
        writer.write_all(b"jar").unwrap();
        writer
            .add_symlink("world/stolen", "../../../stores/users.json", options)
            .unwrap();
        writer.finish().unwrap();
        assert!(validate_source(zip.clone(), true).await.is_err());
        let instance = instance_dir(root.path());
        assert!(import_into(canon(&zip), instance.clone(), true)
            .await
            .is_err());
        assert_eq!(names(&instance), [".lodestone_config"]);
    }

    #[tokio::test]
    async fn zip_lodestone_files_and_macros_are_not_imported() {
        let root = tempfile::tempdir().unwrap();
        let zip = root.path().join("server.zip");
        write_zip(
            &zip,
            &[
                ("server.jar", Some(b"jar")),
                (".lodestone_config", Some(b"old identity")),
                (".Lodestone_Minecraft_Config.json.", Some(b"{}")),
                ("macros/run.ts", Some(b"Deno.exit(1)")),
                ("MACROS /x.js", Some(b"Deno.exit(1)")),
            ],
        );
        let instance = instance_dir(root.path());
        import_into(canon(&zip), instance.clone(), true)
            .await
            .unwrap();
        assert_eq!(names(&instance), [".lodestone_config", "server.jar"]);
        assert_eq!(
            std::fs::read_to_string(instance.join(".lodestone_config")).unwrap(),
            "new identity"
        );
    }

    #[tokio::test]
    async fn only_the_owner_imports_script_files() {
        let root = tempfile::tempdir().unwrap();
        let server = sample_server(root.path());
        std::fs::create_dir_all(server.join("plugins/web")).unwrap();
        std::fs::write(server.join("plugins/web/map.JS"), "alert(1)").unwrap();
        let err = validate_source(server.clone(), false).await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::PermissionDenied));
        let instance = instance_dir(root.path());
        assert!(import_into(canon(&server), instance.clone(), false)
            .await
            .is_err());
        assert_eq!(names(&instance), [".lodestone_config"]);
        validate_source(server.clone(), true).await.unwrap();
        import_into(canon(&server), instance.clone(), true)
            .await
            .unwrap();
        assert!(instance.join("plugins/web/map.JS").exists());

        let zip = root.path().join("server.zip");
        write_zip(
            &zip,
            &[("server.jar", Some(b"jar")), ("plugins/a.ts", Some(b"x"))],
        );
        let err = validate_source(zip.clone(), false).await.unwrap_err();
        assert!(matches!(err.kind, ErrorKind::PermissionDenied));
        validate_source(zip, true).await.unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    async fn names_windows_would_change_are_refused() {
        let root = tempfile::tempdir().unwrap();
        for bad in [
            "world/CON",
            "world/nul.txt",
            "world/a:b",
            "world/name.",
            "world/name ",
        ] {
            let zip = root.path().join("server.zip");
            write_zip(&zip, &[("server.jar", Some(b"jar")), (bad, Some(b"x"))]);
            assert!(
                validate_source(zip, true).await.is_err(),
                "{bad} was accepted"
            );
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn named_pipes_are_refused() {
        let root = tempfile::tempdir().unwrap();
        let server = sample_server(root.path());
        let fifo = server.join("world/pipe");
        let c_path = std::ffi::CString::new(fifo.to_str().unwrap()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o644) }, 0);
        assert!(validate_source(server.clone(), true).await.is_err());
        let instance = instance_dir(root.path());
        assert!(import_into(canon(&server), instance.clone(), true)
            .await
            .is_err());
        assert_eq!(names(&instance), [".lodestone_config"]);
    }

    #[cfg(windows)]
    #[test]
    fn finds_the_free_space_of_a_canonical_path() {
        let root = tempfile::tempdir().unwrap();
        let canonical = std::fs::canonicalize(root.path()).unwrap();
        assert!(available_space(&canonical).is_some());
    }

    #[tokio::test]
    async fn a_source_that_resolves_elsewhere_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let server = sample_server(root.path());
        let instance = instance_dir(root.path());
        // not canonical: the import is only given the path the handler checked
        let indirect = server.join("world").join("..");
        assert!(import_into(indirect, instance.clone(), true).await.is_err());
        assert_eq!(names(&instance), [".lodestone_config"]);
    }

    #[cfg(unix)]
    #[test]
    fn copying_refuses_a_file_swapped_for_a_link() {
        let root = tempfile::tempdir().unwrap();
        let secret = root.path().join("secret.txt");
        std::fs::write(&secret, "secret").unwrap();
        let link = root.path().join("link");
        std::os::unix::fs::symlink(&secret, &link).unwrap();
        assert!(copy_file_no_follow(&link, &root.path().join("copy")).is_err());
        assert!(!root.path().join("copy").exists());
    }
}
