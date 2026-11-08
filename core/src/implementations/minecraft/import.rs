//! Importing an existing Minecraft server folder into a new instance.
//!
//! The caller must already have checked that the requester may read the folder (see
//! `handlers::instance`). The checks here protect the new instance: nothing outside the
//! folder is ever read, links are refused, and a failed import leaves nothing behind.

use std::path::{Path, PathBuf};

use color_eyre::eyre::{eyre, Context};
use tracing::info;

use crate::error::{Error, ErrorKind};

use super::properties::PropertiesDocument;
use super::write_file_atomically;

/// Top-level entries that are never imported: Lodestone's own files (so importing another
/// Lodestone instance does not overwrite the identity of the new one) and the macros
/// folder, whose scripts Lodestone runs.
const SKIPPED: [&str; 3] = [
    ".lodestone_config",
    ".lodestone_minecraft_config.json",
    "macros",
];

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

fn is_skipped(name: &std::ffi::OsStr) -> bool {
    let name = name.to_string_lossy().to_lowercase();
    SKIPPED.contains(&name.as_str())
}

/// Checks that `source` is a plain folder holding a `server.jar`, with no links or special
/// files below it, so that it can be imported.
pub async fn validate_source(source: PathBuf) -> Result<(), Error> {
    tokio::task::spawn_blocking(move || {
        let metadata = std::fs::symlink_metadata(&source)
            .context(format!("Failed to read {}", source.display()))?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err(bad_request(format!("{} is not a folder", source.display())));
        }
        let jar = std::fs::symlink_metadata(source.join("server.jar"));
        if !jar.map(|m| m.is_file()).unwrap_or(false) {
            return Err(bad_request(
                "The existing server folder must contain a server.jar",
            ));
        }
        walk(&source, true)
    })
    .await
    .context("The import check task failed")?
}

fn walk(dir: &Path, top: bool) -> Result<(), Error> {
    for entry in std::fs::read_dir(dir).context(format!("Failed to read {}", dir.display()))? {
        let entry = entry.context(format!("Failed to read {}", dir.display()))?;
        if top && is_skipped(&entry.file_name()) {
            continue;
        }
        let file_type = entry
            .file_type()
            .context(format!("Failed to read {}", entry.path().display()))?;
        if file_type.is_dir() {
            walk(&entry.path(), false)?;
        } else if !file_type.is_file() {
            return Err(refused(&entry.path()));
        }
    }
    Ok(())
}

fn refused(path: &Path) -> Error {
    bad_request(format!(
        "{} is a symbolic link or special file, which cannot be imported",
        path.display()
    ))
}

/// Copies the contents of `source` into the existing folder `destination`.
///
/// The files are copied into a temporary folder inside `destination` and then moved into
/// place, so a failure never leaves a half-copied server: whatever was moved is removed
/// again, and files that were in `destination` before are never touched.
pub async fn import_into(source: PathBuf, destination: PathBuf) -> Result<(), Error> {
    tokio::task::spawn_blocking(move || import_blocking(&source, &destination))
        .await
        .context("The import task failed")?
}

fn import_blocking(source: &Path, destination: &Path) -> Result<(), Error> {
    let root = std::fs::symlink_metadata(source)
        .context(format!("Failed to read {}", source.display()))?;
    if root.file_type().is_symlink() || !root.is_dir() {
        return Err(bad_request(format!("{} is not a folder", source.display())));
    }
    let canonical_source =
        std::fs::canonicalize(source).context(format!("Failed to resolve {}", source.display()))?;
    let canonical_destination = std::fs::canonicalize(destination)
        .context(format!("Failed to resolve {}", destination.display()))?;
    if canonical_destination.starts_with(&canonical_source) {
        return Err(bad_request(
            "The existing server folder cannot contain the new instance",
        ));
    }

    let staging = tempfile::Builder::new()
        .prefix(".lodestone-import-")
        .tempdir_in(destination)
        .context("Failed to create a temporary folder for the import")?;
    copy_tree(&canonical_source, staging.path(), true)?;

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
            let removed = if path.is_dir() {
                std::fs::remove_dir_all(&path)
            } else {
                std::fs::remove_file(&path)
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

fn copy_tree(source: &Path, destination: &Path, top: bool) -> Result<(), Error> {
    for entry in
        std::fs::read_dir(source).context(format!("Failed to read {}", source.display()))?
    {
        let entry = entry.context(format!("Failed to read {}", source.display()))?;
        let name = entry.file_name();
        if top && is_skipped(&name) {
            info!("Not importing {}", entry.path().display());
            continue;
        }
        let from = entry.path();
        let to = destination.join(&name);
        // does not follow links
        let file_type = entry
            .file_type()
            .context(format!("Failed to read {}", from.display()))?;
        if file_type.is_dir() {
            std::fs::create_dir(&to).context(format!("Failed to create {}", to.display()))?;
            copy_tree(&from, &to, false)?;
        } else if file_type.is_file() {
            std::fs::copy(&from, &to).context(format!(
                "Failed to copy {} to {}",
                from.display(),
                to.display()
            ))?;
        } else {
            return Err(refused(&from));
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
        validate_source(server.clone()).await.unwrap();
        import_into(server.clone(), instance.clone()).await.unwrap();
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
        import_into(server, instance.clone()).await.unwrap();
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
        assert!(validate_source(server).await.is_err());
        assert!(validate_source(root.path().join("missing")).await.is_err());
        assert!(
            validate_source(root.path().join("My Server-506d951c/world/level.dat"))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_folder_that_holds_the_instance_is_refused() {
        let root = tempfile::tempdir().unwrap();
        let instance = instance_dir(root.path());
        std::fs::write(root.path().join("server.jar"), b"jar").unwrap();
        let err = import_into(root.path().to_path_buf(), instance.clone())
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
        assert!(validate_source(server.clone()).await.is_err());
        let instance = instance_dir(root.path());
        // even if the link appears after validation, the copy refuses it
        assert!(import_into(server.clone(), instance.clone()).await.is_err());
        assert_eq!(std::fs::read_dir(&instance).unwrap().count(), 1);

        let linked_root = root.path().join("linked");
        std::os::unix::fs::symlink(&server, &linked_root).unwrap();
        assert!(validate_source(linked_root.clone()).await.is_err());
        assert!(import_into(linked_root, instance).await.is_err());
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
        assert!(validate_source(server.clone()).await.is_err());
        let instance = instance_dir(root.path());
        assert!(import_into(server, instance.clone()).await.is_err());
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
        let err = import_into(server, instance.clone()).await.unwrap_err();
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
        assert!(import_into(server, not_a_folder.clone()).await.is_err());
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
}
