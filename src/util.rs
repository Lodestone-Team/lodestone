extern crate crypto;

use std::cmp::min;
use std::fs::File;
use std::io::Write;
use std::path::{PathBuf, Path};
use std::sync::{Arc, RwLock};
use std::sync::atomic::AtomicU64;

use crypto::digest::Digest;
use crypto::sha3::Sha3;

use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use rocket::{tokio, State};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Authentication {
    username: String,
    password: String,
}

use crate::traits::t_resource::DownloadReport;
use crate::traits::{Error, ErrorInner};
use crate::MyManagedState;
#[deprecated]
// copied from https://gist.github.com/giuliano-oliveira/4d11d6b3bb003dba3a1b53f43d81b30d
pub async fn download_file(
    url: &str,
    path: &str,
    state: &State<MyManagedState>,
    uuid: &str,
) -> Result<(), String> {
    let client = Client::new();
    // Reqwest setup
    let res = client
        .get(url)
        .send()
        .await
        .or(Err(format!("Failed to GET from '{}'", &url)))?;
    let total_size = res.content_length().unwrap_or(0);
    state
        .download_status
        .insert(uuid.to_string(), (0, total_size));
    // Indicatif setup
    let pb = ProgressBar::new(total_size);
    pb.set_style(ProgressStyle::default_bar()
        .template("{msg}\n{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec}, {eta})")
        .progress_chars("#>-"));
    pb.set_message(&format!("Downloading {}", url));

    // download chunks
    let mut file = File::create(path).or(Err(format!("Failed to create file '{}'", path)))?;
    let mut downloaded: u64 = 0;
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item.or(Err(format!("Error while downloading file")))?;
        file.write(&chunk)
            .or(Err(format!("Error while writing to file")))?;
        let new = min(downloaded + (chunk.len() as u64), total_size);
        downloaded = new;
        state
            .download_status
            .insert(uuid.to_string(), (new, total_size));
        pb.set_position(new);
    }

    // pb.finish_with_message(&format!("Downloaded {} to {}", url, path));
    return Ok(());
}

pub async fn download_resource(
    url: &str,
    path: &Path,
    name_override: Option<&str>,
) -> Result<DownloadReport, Error> {
    let client = Client::new();
    let response = client.get(url).send().await.map_err(|_| Error {
        inner: ErrorInner::FailedToUpload,
        detail: format!("Failed to send GET request to {}", url),
    })?;
    let mut file_name = String::new();
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
            .skip(1)
            .next()
            .unwrap_or("unknown")
            .split('=')
            .skip(1)
            .next()
            .unwrap_or("unknown")
            .replace("\"", "");
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
        inner: ErrorInner::FailedToWriteFile,
        detail: format!("Failed to create file {}", path.join(&file_name).display()),
    })?;
    let downloaded = Arc::new(AtomicU64::new(0));
    let mut stream = response.bytes_stream();
    let downloaded_clone = downloaded.clone();
    tokio::spawn(
        async move {
        while let Some(item) = stream.next().await {
            let chunk = item.expect("Error while downloading file");
            downloaded_file
                .write(&chunk)
                .expect("Error while writing to file");
                downloaded_clone.fetch_add(chunk.len() as u64, core::sync::atomic::Ordering::Relaxed);
            pb.set_position(downloaded_clone.load(core::sync::atomic::Ordering::Relaxed));
        }
    });
    Ok(DownloadReport {
        total: Arc::new(AtomicU64::new(total_size)),
        downloaded,
        download_name: Arc::new(RwLock::new(file_name)),
    })
}

/// List all files in a directory
/// files_or_dir = 0 -> files, 1 -> directories
pub fn list_dir(path: PathBuf, files_or_dirs: bool) -> Result<Vec<String>, String> {
    let mut files = Vec::new();
    if files_or_dirs {
        for entry in std::fs::read_dir(path.clone()).or(Err(format!(
            "Failed to read directory '{}'",
            path.to_str().unwrap()
        )))? {
            let entry = entry.or(Err(format!(
                "Failed to read directory '{}'",
                path.to_str().unwrap()
            )))?;
            if entry
                .file_type()
                .or(Err(format!(
                    "Failed to read directory '{}'",
                    path.to_str().unwrap()
                )))?
                .is_dir()
            {
                files.push(entry.file_name().to_str().unwrap().to_string());
            }
        }
    } else {
        for entry in std::fs::read_dir(path.clone()).or(Err(format!(
            "Failed to read directory '{}'",
            path.to_str().unwrap()
        )))? {
            let entry = entry.or(Err(format!(
                "Failed to read directory '{}'",
                path.to_str().unwrap()
            )))?;
            if entry
                .file_type()
                .or(Err(format!(
                    "Failed to read directory '{}'",
                    path.to_str().unwrap()
                )))?
                .is_file()
            {
                files.push(entry.file_name().to_str().unwrap().to_string());
            }
        }
    }
    return Ok(files);
}

pub fn hash_password(password: &String) -> String {
    let mut hasher = Sha3::sha3_256();
    hasher.input_str(format!("{}pepega", password).as_str());
    hasher.result_str()
}
