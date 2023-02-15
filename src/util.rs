use color_eyre::eyre::{eyre, Context};
use std::collections::HashSet;
use tempdir::TempDir;
use tokio::fs::File;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Serialize, Deserialize)]
pub struct Authentication {
    username: String,
    password: String,
}

use crate::error::Error;
use crate::prelude::PATH_TO_BINARIES;
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SetupProgress {
    pub current_step: (u8, String),
    pub total_steps: u8,
}

#[derive(Debug, Clone)]
pub struct DownloadProgress {
    pub total: Option<u64>,
    pub downloaded: u64,
    pub step: u64,
    pub download_name: String,
}
pub async fn download_file(
    url: &str,
    path: &Path,
    name_override: Option<&str>,
    on_download: &(dyn Fn(DownloadProgress) + Send + Sync),
    overwrite_old: bool,
) -> Result<PathBuf, Error> {
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to send GET request")?;
    response.error_for_status_ref().context(
        "
        Failed to download file
    ",
    )?;
    tokio::fs::create_dir_all(path)
        .await
        .context(format!("Failed to create dir {}", &path.display()))?;

    let file_name;
    if let Some(name) = name_override {
        file_name = name.to_string();
    } else {
        file_name = response
            .headers()
            .get("Content-Disposition")
            .map_or_else(
                || "unknown".to_string(),
                |h| {
                    h.to_str()
                        .map_or_else(|_| "unknown".to_string(), |s| s.to_string())
                },
            )
            // parse filename's value from the header, remove the ""
            .split(';')
            .nth(1)
            .unwrap_or("unknown")
            .split('=')
            .nth(1)
            .unwrap_or("unknown")
            .replace('\"', "");
    }
    if !overwrite_old && path.join(&file_name).exists() {
        return Err(eyre!("File {} already exists", path.join(&file_name).display()).into());
    }
    fs::remove_file(path.join(&file_name)).await.ok();
    let total_size = response.content_length();
    let pb = ProgressBar::new(total_size.unwrap_or(0));
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .progress_chars("#>-"));
    pb.set_message(&format!("Downloading {}", url));

    let mut downloaded_file = File::create(path.join(&file_name))
        .await
        .context(format!("Failed to create file {}", &path.display()))?;
    let mut downloaded: u64 = 0;
    let mut new_downloaded: u64 = 0;
    let threshold = total_size.unwrap_or(500000) / 100;
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item.expect("Error while downloading file");
        downloaded_file
            .write_all(&chunk)
            .await
            .context(format!("Failed to write to file {}", &file_name))?;
        new_downloaded += chunk.len() as u64;
        let step = new_downloaded - downloaded;
        if step > threshold {
            on_download(DownloadProgress {
                total: total_size,
                downloaded,
                step,
                download_name: file_name.clone(),
            });
            downloaded = new_downloaded;
        }

        pb.set_position(new_downloaded);
    }
    Ok(path.join(&file_name))
}

/// List all files in a directory
/// files_or_dir = 0 -> files, 1 -> directories
pub async fn list_dir(
    path: &Path,
    filter_file_or_dir: Option<bool>,
) -> Result<Vec<PathBuf>, Error> {
    let ret: Result<Vec<PathBuf>, Error> = tokio::task::spawn_blocking({
        let path = path.to_owned();
        move || {
            Ok(std::fs::read_dir(&path)
                .context(format!("failed to read directory {}", path.display()))?
                .into_iter()
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.file_type().is_ok())
                .filter(|entry| match filter_file_or_dir {
                    // unwrap is safe because we checked if file_type is ok
                    Some(true) => entry.file_type().unwrap().is_dir(),
                    Some(false) => entry.file_type().unwrap().is_file(),
                    None => true,
                })
                .map(|entry| entry.path())
                .collect())
        }
    })
    .await.context("Failed to list directory")?;
    ret
}

pub async fn unzip_file(
    file: impl AsRef<Path>,
    dest: impl AsRef<Path>,
    overwrite_old: bool,
) -> Result<HashSet<PathBuf>, Error> {
    let file = file.as_ref();
    let dest = dest.as_ref();
    let os = std::env::consts::OS;
    let arch = if std::env::consts::ARCH == "x86_64" {
        "x64"
    } else {
        std::env::consts::ARCH
    };
    let _7zip_name = format!("7z_{}_{}", os, arch);
    let _7zip_path = PATH_TO_BINARIES
        .with(|v| v.clone())
        .join("7zip")
        .join(&_7zip_name);
    if !_7zip_path.is_file() {
        return Err(
            eyre!(
                "Runtime depedency 7zip is missing, please download it from https://github.com/Lodestone-Team/dependencies or reinstall Lodestone"
            ).into()
        );
    }
    tokio::fs::create_dir_all(dest)
        .await
        .context(format!("Failed to create directory {}", dest.display()))?;
    let before: HashSet<PathBuf>;

    let tmp_dir = TempDir::new("lodestone")
        .context(format!(
            "Failed to create temporary directory for unzipping {}",
            file.display()
        ))?
        .path()
        .to_owned();

    let overwrite_arg = if overwrite_old { "-aoa" } else { "-aou" };

    if file
        .extension()
        .ok_or_else(|| eyre!("Failed to get file extension for {}", file.display()))?
        == "gz"
    {
        dont_spawn_terminal(
            Command::new(&_7zip_path)
                .arg("x")
                .arg(file)
                .arg(overwrite_arg)
                .arg(format!("-o{}", tmp_dir.display())),
        )
        .status()
        .await
        .context("Failed to execute 7zip")?;

        before = list_dir(dest, None).await?.iter().cloned().collect();

        dont_spawn_terminal(
            Command::new(&_7zip_path)
                .arg("x")
                .arg(&tmp_dir)
                .arg(overwrite_arg)
                .arg("-ttar")
                .arg(format!("-o{}", dest.display())),
        )
        .status()
        .await
        .context("Failed to execute 7zip")?;
    } else {
        before = list_dir(dest, None).await?.iter().cloned().collect();
        dont_spawn_terminal(
            Command::new(&_7zip_path)
                .arg("x")
                .arg(file)
                .arg(format!("-o{}", dest.display()))
                .arg(overwrite_arg),
        )
        .status()
        .await
        .context("Failed to execute 7zip")?;
    }
    let after: HashSet<PathBuf> = list_dir(dest, None).await?.iter().cloned().collect();
    Ok((&after - &before).iter().cloned().collect())
}

pub fn rand_alphanumeric(len: usize) -> String {
    thread_rng().sample_iter(&Alphanumeric).take(len).collect()
}

// safe_path only works on linux and messes up on windows
// this is a hacky solution
pub fn scoped_join_win_safe<R: AsRef<Path>, U: AsRef<Path>>(
    root: R,
    unsafe_path: U,
) -> Result<PathBuf, Error> {
    let mut ret = safe_path::scoped_join(&root, &unsafe_path).context(format!(
        "Failed to join path {} with {}",
        root.as_ref().display(),
        unsafe_path.as_ref().display()
    ))?;
    if cfg!(windows) {
        // construct a new path
        // that replace the prefix component with the component of the root path
        ret = ret
            .components()
            .skip(1)
            .fold(root.as_ref().to_path_buf(), |mut acc, c| {
                acc.push(c.as_os_str());
                acc
            });
    }
    Ok(ret)
}
pub mod fs {
    use std::path::Path;

    use color_eyre::eyre::Context;
    use tokio::fs::File;

    use crate::error::Error;

    pub async fn remove_file(file: impl AsRef<Path>) -> Result<(), Error> {
        let file = file.as_ref();
        if file.is_file() {
            tokio::fs::remove_file(file)
                .await
                .context(format!("Failed to remove file at {}", file.display()))?;
        }
        Ok(())
    }

    pub async fn write_all(file: impl AsRef<Path>, data: impl AsRef<[u8]>) -> Result<(), Error> {
        let file = file.as_ref();
        tokio::fs::write(file, data)
            .await
            .context(format!("Failed to write to file at {}", file.display()))?;
        Ok(())
    }

    pub async fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<(), Error> {
        let from = from.as_ref();
        let to = to.as_ref();
        tokio::fs::rename(from, to).await.context(format!(
            "Failed to rename file {} to {}",
            from.display(),
            to.display()
        ))?;
        Ok(())
    }

    pub async fn create_dir_all(dir: impl AsRef<Path>) -> Result<(), Error> {
        let dir = dir.as_ref();
        tokio::fs::create_dir_all(dir)
            .await
            .context(format!("Failed to create directory at {}", dir.display()))?;
        Ok(())
    }

    pub async fn remove_dir_all(dir: impl AsRef<Path>) -> Result<(), Error> {
        let dir = dir.as_ref();
        tokio::fs::remove_dir_all(dir)
            .await
            .context(format!("Failed to remove directory at {}", dir.display()))?;
        Ok(())
    }

    pub async fn read_to_string(file: impl AsRef<Path>) -> Result<String, Error> {
        let file = file.as_ref();
        let data = tokio::fs::read_to_string(file)
            .await
            .context(format!("Failed to read file at {}", file.display()))?;
        Ok(data)
    }

    pub async fn create(file: impl AsRef<Path>) -> Result<File, Error> {
        let file = file.as_ref();
        let file = tokio::fs::File::create(file)
            .await
            .context(format!("Failed to create file at {}", file.display()))?;
        Ok(file)
    }
}
pub fn dont_spawn_terminal(cmd: &mut tokio::process::Command) -> &mut tokio::process::Command {
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);

    cmd
}

pub fn format_byte_download(bytes: u64, total: u64) -> String {
    let mut bytes = bytes as f64;
    let mut total = total as f64;
    let mut unit = "B";
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "KB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "MB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "GB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "TB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "PB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "EB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "ZB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        total /= 1024.0;
        unit = "YB";
    }
    format!("{:.1} / {:.1} {}", bytes, total, unit)
}

pub fn format_byte(bytes: u64) -> String {
    let mut bytes = bytes as f64;
    let mut unit = "B";
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "KB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "MB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "GB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "TB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "PB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "EB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "ZB";
    }
    if bytes > 1024.0 {
        bytes /= 1024.0;
        unit = "YB";
    }
    format!("{:.1} {}", bytes, unit)
}
