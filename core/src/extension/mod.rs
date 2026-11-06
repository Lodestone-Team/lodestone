use std::path::PathBuf;

use color_eyre::eyre::{self, Context};
use serde_json::Value;
use tracing::error;
use ts_rs::TS;

use crate::error::{Error, ErrorKind};

pub mod atom;
pub mod git;
pub mod r#macro;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[ts(export)]
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[ts(export)]
pub enum ExtensionType {
    Atom,
    Macro,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, TS)]
#[ts(export)]
pub struct Manifest {
    pub r#type: ExtensionType,
    pub author: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub release_notes: Option<String>,
    #[serde(default)]
    pub permission: Option<Permission>,
}

/// Manifest but with github username, repo, url, and domain
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ManifestWithMetadata {
    pub manifest: Manifest,
    /// GitHub username
    pub username: String,
    pub repo: String,
    pub url: String,
    pub domain: String,
}

#[derive(serde::Serialize, Debug)]
pub enum FetchExtensionManifestError {
    NotFound,
    /// The URL the caller sent is not a GitHub repository URL.
    InvalidUrl(String),
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
            FetchExtensionManifestError::InvalidUrl(e) => Error {
                kind: ErrorKind::BadRequest,
                source: eyre::eyre!(e),
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

/// Whether `name` is non-empty, at most 100 characters, and only has ASCII letters, digits,
/// and the characters in `extra`.
fn is_plain_name(name: &str, extra: &[char]) -> bool {
    !name.is_empty()
        && name.len() <= 100
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || extra.contains(&c))
}

/// GitHub user and organization names: letters, digits and `-`, not starting with `-`.
fn is_valid_github_user(name: &str) -> bool {
    is_plain_name(name, &['-']) && !name.starts_with('-')
}

/// GitHub repository names: letters, digits, `-`, `_` and `.`, but not `.` or `..`, and not
/// starting with `-` (so it can never look like a command-line option).
fn is_valid_github_repo(name: &str) -> bool {
    is_plain_name(name, &['-', '_', '.']) && name != "." && name != ".." && !name.starts_with('-')
}

/// Extension names from a manifest become folder names: letters, digits, `-` and `_`.
pub fn is_valid_extension_name(name: &str) -> bool {
    is_plain_name(name, &['-', '_']) && !name.starts_with('-')
}

/// Parses `https://github.com/<user>/<repo>` into the user and repo.
fn parse_github_url(url: &str) -> Result<(String, String, String), FetchExtensionManifestError> {
    let invalid = |msg: &str| {
        error!("Invalid extension url {}: {}", url, msg);
        FetchExtensionManifestError::InvalidUrl(format!("Invalid extension URL: {msg}"))
    };
    let parsed = url::Url::parse(url).map_err(|e| invalid(&e.to_string()))?;
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(invalid("the URL must start with https://"));
    }
    let domain = parsed
        .domain()
        .ok_or_else(|| invalid("the URL has no domain"))?
        .to_string();
    if domain != "github.com" && domain != "www.github.com" {
        return Err(invalid("only GitHub repositories are supported"));
    }
    let mut segments = parsed
        .path_segments()
        .ok_or_else(|| invalid("the URL has no path"))?;
    let username = segments
        .next()
        .ok_or_else(|| invalid("the URL has no GitHub user"))?
        .to_string();
    let repo = segments
        .next()
        .ok_or_else(|| invalid("the URL has no repository"))?;
    let repo = repo.strip_suffix(".git").unwrap_or(repo).to_string();
    if !is_valid_github_user(&username) {
        return Err(invalid("the GitHub user name is not valid"));
    }
    if !is_valid_github_repo(&repo) {
        return Err(invalid("the repository name is not valid"));
    }
    Ok((domain, username, repo))
}

pub async fn get_manifest(
    url: impl AsRef<str>,
) -> Result<ManifestWithMetadata, FetchExtensionManifestError> {
    // https://github.com/Lodestone-Team/lodestone
    let (domain, username, repo) = parse_github_url(url.as_ref())?;

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
            // clone from the same repository the manifest was read from, not from the URL
            // the caller sent
            let url = format!("https://github.com/{username}/{repo}.git");
            Ok(ManifestWithMetadata {
                manifest,
                username,
                repo,
                url,
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
    atom_path: PathBuf,
}

impl ExtensionManager {
    pub fn new(extension_path: PathBuf) -> Self {
        Self {
            atom_path: extension_path.join("atom"),
        }
    }

    pub async fn install_extension(&self, url: impl AsRef<str>) -> Result<PathBuf, Error> {
        // a possible race condition, but it's fine
        let manifest = get_manifest(url).await?;
        let extension_path = match manifest.manifest.r#type {
            ExtensionType::Atom => {
                self.install_atom(&manifest.username, &manifest.url, &manifest.manifest.name)
                    .await?
            }
            ExtensionType::Macro => {
                return Err(Error {
                    kind: ErrorKind::UnsupportedOperation,
                    source: eyre::eyre!("Installing macro extensions is not supported yet"),
                })
            }
        };
        Ok(extension_path)
    }

    async fn install_atom(
        &self,
        username: &str,
        url: &str,
        atom_name: &str,
    ) -> Result<PathBuf, Error> {
        let dir_name = atom_dir_name(username, atom_name)?;
        tokio::fs::create_dir_all(&self.atom_path)
            .await
            .context("Failed to create atom directory")?;
        // check if the extension already exists
        let path = self.atom_path.join(&dir_name);
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(path);
        }
        let git = git::GitClient::clone(url, &self.atom_path, &dir_name).await?;
        Ok(git.cwd())
    }
}

/// The folder an atom is cloned into: `<atom name>.<GitHub user>`.
fn atom_dir_name(username: &str, atom_name: &str) -> Result<String, Error> {
    if !is_valid_extension_name(atom_name) {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre::eyre!(
                "Extension name must only contain letters, digits, '-' and '_' and not start with '-'"
            ),
        });
    }
    if !is_valid_github_user(username) {
        return Err(Error {
            kind: ErrorKind::BadRequest,
            source: eyre::eyre!("Invalid GitHub user name"),
        });
    }
    Ok(format!("{atom_name}.{username}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_github_urls() {
        let (domain, user, repo) =
            parse_github_url("https://github.com/Lodestone-Team/lodestone").unwrap();
        assert_eq!(domain, "github.com");
        assert_eq!(user, "Lodestone-Team");
        assert_eq!(repo, "lodestone");
        let (_, _, repo) = parse_github_url("https://github.com/a/b.c.git").unwrap();
        assert_eq!(repo, "b.c");
    }

    #[test]
    fn rejects_bad_urls() {
        for url in [
            "not a url",
            "https://example.com/a/b",
            "file:///etc/passwd",
            "https://github.com/a",
            "https://github.com/-a/b",
            "https://github.com/a/..",
            "https://github.com/a/-b",
            "https://github.com/a/b%20c",
        ] {
            assert!(
                matches!(
                    parse_github_url(url),
                    Err(FetchExtensionManifestError::InvalidUrl(_))
                ),
                "{url} should be rejected"
            );
        }
    }

    #[test]
    fn atom_dir_names_are_plain() {
        assert_eq!(
            atom_dir_name("user", "my_atom-1").unwrap(),
            "my_atom-1.user"
        );
        for name in ["", "../x", "a/b", "--config=core.sshCommand=x", "a b", ".."] {
            assert!(
                atom_dir_name("user", name).is_err(),
                "{name} should be rejected"
            );
        }
        assert!(atom_dir_name("bad/user", "atom").is_err());
    }
}
