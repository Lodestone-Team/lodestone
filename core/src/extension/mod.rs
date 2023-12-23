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
                source: eyre::eyre!("GitHub API returned {}: {}", status_code, e)
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

pub async fn get_manifest(
    username: impl AsRef<str>,
    repo: impl AsRef<str>,
) -> Result<Manifest, FetchExtensionManifestError> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/contents/lodestone.json",
        username.as_ref(),
        repo.as_ref()
    );

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
            Ok(manifest)
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

    pub async fn install_extension(&self, username: impl AsRef<str>, repo: impl AsRef<str>) -> Result<PathBuf, Error> {
        // a possible race condition, but it's fine
        let manifest = get_manifest(&username, &repo).await?;
        let extension_path = match manifest.r#type {
            Type::Atom => self.install_atom(username, repo, &manifest.name).await?,
            Type::Macro => todo!(),
        };
        Ok(extension_path)

    }

    async fn install_atom(&self, username: impl AsRef<str>, repo: impl AsRef<str>, name : &str) -> Result<PathBuf, Error> {
        tokio::fs::create_dir_all(&self.atom_path).await.context("Failed to create atom directory")?;
        // check if the extension already exists
        let path = self.atom_path.join(name);
        if path.exists() {
            return Ok(path);
        }
        let url = format!("https://github.com/{}/{}", username.as_ref(), repo.as_ref());
        let git = git::GitClient::clone(url, &self.atom_path, name).await?;
        Ok(git.cwd())
    }
}
