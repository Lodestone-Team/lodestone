//! Getting the files an update writes ([`super::merge::Fetch`]) into a staging folder:
//! downloads from Modrinth, CurseForge or the hosts a pack may name, overrides out of a
//! pack's zip, and objects from the server whose image names them. Every file is checked
//! by the hash its source gives (SHA-1 and SHA-512 for downloads, the server's SHA-256 for
//! anything a server's view names) before it is moved anywhere.

use std::path::{Path, PathBuf};

use lodestone_minecraft::mrpack::PackDownload;

use super::{
    ModpackError, OverrideFile,
    import::{ParentCheck, download_files_checked, extract_overrides},
    io_error,
    merge::Fetch,
};
use crate::{
    content::{Platforms, install::InstallProgress},
    cores::player_image::{ObjectItem, ServerImage, sha256_of_file},
    game::download::{CancelToken, Downloader},
};

/// Where an update's files come from.
#[derive(Clone, Copy)]
pub struct Sources<'a> {
    pub platforms: Platforms<'a>,
    pub downloader: &'a Downloader,
    /// The pack's `.mrpack`, for its overrides.
    pub pack: Option<&'a Path>,
    /// The server whose image names the objects.
    pub objects: Option<&'a ServerImage>,
}

/// Puts the file each of `writes` writes at its path under `root` (a staging folder that
/// the caller throws away if this fails), checked.
pub(crate) async fn stage(
    sources: &Sources<'_>,
    root: &Path,
    title: &str,
    writes: &[(Fetch, String)],
    progress: &InstallProgress,
    cancel: &CancelToken,
) -> Result<(), ModpackError> {
    let mut downloads: Vec<(PackDownload, Option<String>, PathBuf)> = Vec::new();
    let mut overrides: Vec<(OverrideFile, PathBuf)> = Vec::new();
    let mut objects: Vec<ObjectItem> = Vec::new();
    // what is checked by its SHA-256 once staged, besides what checked it while fetching
    let mut check_after: Vec<(PathBuf, String)> = Vec::new();
    for (fetch, path) in writes {
        let dest = root.join(path);
        match fetch {
            Fetch::Download {
                download,
                expect_sha256,
            } => downloads.push((download.clone(), expect_sha256.clone(), dest)),
            Fetch::Override {
                file,
                expect_sha256,
            } => {
                if let Some(sha256) = expect_sha256 {
                    check_after.push((dest.clone(), sha256.clone()));
                }
                overrides.push((file.clone(), dest));
            }
            Fetch::Object { sha256, size } => objects.push(ObjectItem {
                sha256: sha256.clone(),
                size: *size,
                dest,
                path: path.clone(),
            }),
        }
    }
    if !downloads.is_empty() {
        progress.set_label(format!("Downloading {title}"));
        download_files_checked(
            sources.platforms,
            sources.downloader,
            root,
            &downloads,
            progress,
            cancel,
        )
        .await?;
    }
    if !overrides.is_empty() {
        let pack = sources.pack.ok_or_else(|| {
            ModpackError::Invalid("The pack's files are needed but weren't downloaded.".into())
        })?;
        progress.set_label("Copying the pack's configs");
        extract_overrides(pack, root, &overrides, cancel)?;
    }
    if !objects.is_empty() {
        let server = sources.objects.ok_or_else(|| {
            ModpackError::Invalid("The server's files are needed but it can't be reached.".into())
        })?;
        progress.set_label("Downloading the server's files");
        progress
            .downloads
            .set(crate::game::download::ProgressSnapshot {
                files_total: objects.len() as u64,
                bytes_total: objects.iter().map(|o| o.size).sum(),
                ..Default::default()
            });
        let mut parents = ParentCheck::new(root)?;
        for object in &objects {
            parents.prepare(&object.dest)?;
        }
        server
            .objects(&objects, &progress.downloads, cancel)
            .await?;
    }
    if cancel.is_cancelled() {
        return Err(ModpackError::Cancelled);
    }
    for (dest, sha256) in check_after {
        let actual = sha256_of_file(&dest).map_err(io_error(&dest))?;
        if !actual.eq_ignore_ascii_case(&sha256) {
            return Err(ModpackError::Invalid(format!(
                "The pack's copy of {} isn't the file the server has.",
                dest.strip_prefix(root).unwrap_or(&dest).display()
            )));
        }
    }
    Ok(())
}
