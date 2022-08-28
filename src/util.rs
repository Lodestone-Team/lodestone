use std::collections::HashSet;
use std::fs::File;

use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

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

use crate::traits::{Error, ErrorInner};
#[derive(Debug, Clone, Serialize, Deserialize, TS)]

pub struct SetupProgress {
    pub current_step: (u8, String),
    pub total_steps: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct DownloadProgress {
    pub total: u64,
    pub downloaded: u64,
    pub download_name: String,
}
pub async fn download_file(
    url: &str,
    path: &Path,
    name_override: Option<&str>,
    on_download: &(dyn Fn(DownloadProgress) + Send + Sync),
) -> Result<PathBuf, Error> {
    let client = Client::new();
    let response = client.get(url).send().await.map_err(|_| Error {
        inner: ErrorInner::FailedToUpload,
        detail: format!("Failed to send GET request to {}", url),
    })?;
    std::fs::create_dir_all(path).map_err(|_| Error {
        inner: ErrorInner::FailedToUpload,
        detail: format!("Failed to create directory {}", path.display()),
    })?;

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
    if path.join(&file_name).exists() {
        return Err(Error {
            inner: ErrorInner::FiledOrDirAlreadyExists,
            detail: format!("{} already exists", path.join(&file_name).display()),
        });
    }
    let total_size = response.content_length().unwrap_or(0);
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .progress_chars("#>-"));
    pb.set_message(&format!("Downloading {}", url));

    let mut downloaded_file = File::create(path.join(&file_name)).map_err(|_| Error {
        inner: ErrorInner::FailedToWriteFileOrDir,
        detail: format!("Failed to create file {}", path.join(&file_name).display()),
    })?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    while let Some(item) = stream.next().await {
        let chunk = item.expect("Error while downloading file");
        downloaded_file
            .write_all(&chunk)
            .expect("Error while writing to file");
        downloaded += chunk.len() as u64;
        on_download(DownloadProgress {
            total: total_size,
            downloaded,
            download_name: file_name.clone(),
        });
        pb.set_position(downloaded as u64);
    }
    Ok(path.join(&file_name))
}

/// List all files in a directory
/// files_or_dir = 0 -> files, 1 -> directories
pub fn list_dir(path: &Path, filter_file_or_dir: Option<bool>) -> Result<Vec<PathBuf>, Error> {
    let ret: Vec<PathBuf> = std::fs::read_dir(&path)
        .map_err(|_| Error {
            inner: ErrorInner::FailedToReadFileOrDir,
            detail: "".to_string(),
        })?
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
        .collect();
    Ok(ret)
}

pub fn unzip_file(
    file: &Path,
    dest: &Path,
    path_to_runtimes: &Path,
) -> Result<HashSet<PathBuf>, Error> {
    let os = std::env::consts::OS;
    let arch = if std::env::consts::ARCH == "x86_64" {
        "x64"
    } else {
        std::env::consts::ARCH
    };
    let _7zip_name = format!("7z_{}_{}", os, arch);
    let _7zip_path = path_to_runtimes.join("7zip").join(&_7zip_name);
    if !_7zip_path.is_file() {
        return Err(Error{ inner: ErrorInner::FileOrDirNotFound, detail: format!("Runtime dependency {} is not found at {}. Consider downloading the dependency to .lodestone/bin/7zip/, or reinstall Lodestone", _7zip_name, _7zip_path.display()) });
    }
    std::fs::create_dir_all(dest).map_err(|_| Error {
        inner: ErrorInner::FailedToWriteFileOrDir,
        detail: format!("Failed to create directory {}", dest.display()),
    })?;
    let before: HashSet<PathBuf>;

    let tmp_dir = dest.join("tmp_1c92md");
    if file.extension().ok_or(Error {
        inner: ErrorInner::MalformedFile,
        detail: "Not a zip file".to_string(),
    })? == "gz"
    {
        Command::new(&_7zip_path)
            .arg("x")
            .arg(file)
            .arg("-aoa")
            .arg(format!("-o{}", tmp_dir.display()))
            .status()
            .map_err(|_| Error {
                inner: ErrorInner::FailedToExecute,
                detail: "Failed to execute 7zip".to_string(),
            })?;

        before = list_dir(dest, None)
            .map_err(|_| Error {
                inner: ErrorInner::FailedToReadFileOrDir,
                detail: "".to_string(),
            })?
            .iter()
            .cloned()
            .collect();

        Command::new(&_7zip_path)
            .arg("x")
            .arg(&tmp_dir)
            .arg("-aoa")
            .arg("-ttar")
            .arg(format!("-o{}", dest.display()))
            .status()
            .map_err(|_| Error {
                inner: ErrorInner::FailedToExecute,
                detail: "Failed to execute 7zip".to_string(),
            })?;
    } else {
        before = list_dir(dest, None)
            .map_err(|_| Error {
                inner: ErrorInner::FailedToReadFileOrDir,
                detail: "".to_string(),
            })?
            .iter()
            .cloned()
            .collect();
        Command::new(&_7zip_path)
            .arg("x")
            .arg(file)
            .arg(format!("-o{}", dest.display()))
            .arg("aoa")
            .status()
            .map_err(|_| Error {
                inner: ErrorInner::FailedToExecute,
                detail: "Failed to execute 7zip".to_string(),
            })?;
    }
    let after: HashSet<PathBuf> = list_dir(dest, None)
        .map_err(|_| Error {
            inner: ErrorInner::FailedToReadFileOrDir,
            detail: "".to_string(),
        })?
        .iter()
        .cloned()
        .collect();
    std::fs::remove_dir_all(tmp_dir).map_err(|_| Error {
        inner: ErrorInner::FailedToRemoveFileOrDir,
        detail: "Failed to remove tmp dir".to_string(),
    })?;
    Ok((&after - &before).iter().cloned().collect())
}

pub fn rand_alphanumeric(len: usize) -> String {
    thread_rng().sample_iter(&Alphanumeric).take(len).collect()
}
