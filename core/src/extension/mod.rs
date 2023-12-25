use std::path::PathBuf;

use axum::Json;
use color_eyre::eyre::{self, Context};
use serde_json::Value;
use tracing::error;

use crate::error::Error;

pub mod atom;
pub mod git;
pub mod r#macro;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
/// https://docs.deno.com/runtime/manual/basics/permissions
pub struct Permission {
    // override permissions
    /// Allow read and write to the file system.
    pub full_disk_access: bool,
    /// Allow any network access.
    pub full_network_access: bool,
    /// Allow access to all environment variables.
    pub full_env_access: bool,
    /// Allow reading the file system.
    pub disk_read: bool,
    /// Allow writing to the file system.
    pub disk_write: bool,
    pub sys_info: bool,
    /// Allow running subprocesses.
    pub subprocess: bool,

    // specific permissions
    pub allow_env: Option<Vec<String>>,
    pub allow_read: Option<Vec<PathBuf>>,
    pub allow_write: Option<Vec<PathBuf>>,
    pub allow_net: Option<Vec<String>>,
    pub allow_run: Option<Vec<String>>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum r#Type {
    Atom,
    Macro,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Manifest {
    pub r#type: Type,
    pub author: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub permission: Option<Permission>,
}

/// Manifest but with github username, repo, url, and domain
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManifestWithMetadata {
    pub manifest : Manifest,
    /// GitHub username
    pub username: String,
    pub repo: String,
    pub url: String,
    pub domain: String,
}

#[derive(serde::Serialize)]
pub enum FetchExtensionManifestError {
    NotFound,
    Http(String),
    BadResponse(String),
    BadManifest(String),
    Other(u16, String),
}

impl From<FetchExtensionManifestError> for Error {
    fn from(e: FetchExtensionManifestError) -> Self {
        match e {
            FetchExtensionManifestError::NotFound => Error {
                kind: crate::error::ErrorKind::NotFound,
                source: eyre::eyre!("GitHub API returned 404. Does the user and repo exist?"),
            },
            FetchExtensionManifestError::Other(status_code, e) => Error {
                kind: crate::error::ErrorKind::External,
                source: eyre::eyre!("GitHub API returned {}: {}", status_code, e),
            },
            FetchExtensionManifestError::Http(e) => Error {
                kind: crate::error::ErrorKind::Internal,
                source: eyre::eyre!(e),
            },
            FetchExtensionManifestError::BadResponse(e) => Error {
                kind: crate::error::ErrorKind::External,
                source: eyre::eyre!("Failed to get json from extension manifest: {}", e),
            },
            FetchExtensionManifestError::BadManifest(e) => Error {
                kind: crate::error::ErrorKind::Internal,
                source: eyre::eyre!(e),
            },
        }
    }
}

pub async fn get_manifest(url: impl AsRef<str>) -> Result<ManifestWithMetadata, FetchExtensionManifestError> {
    // https://github.com/Lodestone-Team/lodestone

    let _url = url::Url::parse(url.as_ref()).map_err(|e| {
        error!("Failed to parse url: {}", e);
        FetchExtensionManifestError::Other(500, e.to_string())
    })?;
    let domain = _url
        .domain()
        .ok_or_else(|| {
            error!("Failed to get domain");
            FetchExtensionManifestError::Other(500, "Failed to get domain".to_string())
        })?
        .to_string();
    let username = _url
        .path_segments()
        .ok_or_else(|| {
            error!("Failed to get path segments");
            FetchExtensionManifestError::Other(500, "Failed to get path segments".to_string())
        })?
        .nth(0)
        .ok_or_else(|| {
            error!("Failed to get username");
            FetchExtensionManifestError::Other(500, "Failed to get username".to_string())
        })?
        .to_string();
    let repo = _url
        .path_segments()
        .ok_or_else(|| {
            error!("Failed to get path segments");
            FetchExtensionManifestError::Other(500, "Failed to get path segments".to_string())
        })?
        .nth(1)
        .ok_or_else(|| {
            error!("Failed to get repo");
            FetchExtensionManifestError::Other(500, "Failed to get repo".to_string())
        })?
        .to_string();

    let url = format!("https://api.github.com/repos/{username}/{repo}/contents/lodestone.json",);

    let http = reqwest::Client::new();

    let resp = http
        .get(url)
        .header("User-Agent", "Lodestone-Core")
        .header("accept", "application/vnd.github.VERSION.raw")
        .send()
        .await
        .map_err(|e| {
            error!("Failed to fetch extension manifest: {}", e);
            FetchExtensionManifestError::Http(e.to_string())
        })?;
    match resp.error_for_status() {
        Ok(resp) => {
            let resp: Value = resp.json().await.map_err(|e| {
                error!("Failed to get json from extension manifest: {}", e);
                FetchExtensionManifestError::BadResponse(e.to_string())
            })?;
            let manifest: Manifest = serde_json::from_value(resp).map_err(|e| {
                error!("Failed to parse extension manifest: {}", e);
                FetchExtensionManifestError::BadManifest(e.to_string())
            })?;
            Ok(ManifestWithMetadata {
                manifest,
                username,
                repo,
                url: _url.to_string(),
                domain,
            })
        }
        Err(e) => {
            error!("Failed to fetch extension manifest: {}", e);
            if e.status() == Some(reqwest::StatusCode::NOT_FOUND) {
                Err(FetchExtensionManifestError::NotFound)
            } else {
                Err(FetchExtensionManifestError::Other(
                    e.status()
                        .unwrap_or(reqwest::StatusCode::INTERNAL_SERVER_ERROR)
                        .as_u16(),
                    e.to_string(),
                ))
            }
        }
    }
}

pub struct ExtensionManager {
    extension_path: PathBuf,
    atom_path: PathBuf,
    macro_path: PathBuf,
}

impl ExtensionManager {
    pub fn new(extension_path: PathBuf) -> Self {
        let atom_path = extension_path.join("atom");
        let macro_path = extension_path.join("macro");
        Self {
            extension_path,
            atom_path,
            macro_path,
        }
    }

    pub async fn install_extension(&self, url: impl AsRef<str>) -> Result<PathBuf, Error> {
        // a possible race condition, but it's fine
        let manifest = get_manifest(url).await?;
        let extension_path = match manifest.manifest.r#type {
            Type::Atom => self.install_atom(&manifest.username, &manifest.url, &manifest.manifest.name).await?,
            Type::Macro => todo!(),
        };
        Ok(extension_path)
    }

    async fn install_atom(
        &self,
        username: &str,
        url: &str,
        atom_name: &str,
    ) -> Result<PathBuf, Error> {
        tokio::fs::create_dir_all(&self.atom_path)
            .await
            .context("Failed to create atom directory")?;
        // check if the extension already exists
        let path = self.atom_path.join(atom_name);
        if path.exists() {
            return Ok(path);
        }
        let git = git::GitClient::clone(url, &self.atom_path, format!("{atom_name}.{username}")).await?;
        Ok(git.cwd())
    }
}
