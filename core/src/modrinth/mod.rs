//! Modrinth (<https://modrinth.com>): the API client the core uses to browse projects, look
//! up versions and jar hashes, and download files.
//!
//! Browsers and the CLI never call Modrinth themselves; the core proxies every call (see
//! `handlers/modrinth.rs`). The [`ModrinthApi`] trait lets the install and modpack logic run
//! against a fake in tests.
//!
//! The API types, link parsing and search facets are in [`lodestone_minecraft::modrinth`],
//! which Lodestone Launcher shares.

mod cache;
pub mod client;
#[cfg(test)]
pub mod fake;
pub mod links;
pub use lodestone_minecraft::modrinth::search;
pub mod types;

use std::collections::HashMap;

use async_trait::async_trait;

use crate::error::Error;

pub use client::ModrinthClient;
pub use search::SearchParams;
pub use types::*;

/// The Modrinth API.
pub const API_BASE: &str = "https://api.modrinth.com/v2";

/// Modrinth asks API clients to identify themselves. No personal data.
pub const USER_AGENT: &str = concat!(
    "Lodestone/",
    env!("CARGO_PKG_VERSION"),
    " (https://github.com/Lodestone-Team/lodestone)"
);

pub use lodestone_minecraft::modrinth::CDN_HOST;

/// The size and hashes of a downloaded file, as lowercase hex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileHashes {
    pub size: u64,
    pub sha1: String,
    pub sha512: String,
}

/// The Modrinth calls the core makes. [`ModrinthClient`] is the real implementation.
#[async_trait]
pub trait ModrinthApi: Send + Sync {
    async fn search(&self, params: &SearchParams) -> Result<ModrinthSearchResponse, Error>;

    async fn project(&self, id_or_slug: &str) -> Result<ModrinthProject, Error>;

    /// The projects with these ids. Unknown ids are left out.
    async fn projects(&self, ids: &[String]) -> Result<Vec<ModrinthProject>, Error>;

    /// The versions of a project, newest first, optionally only those for one of `loaders`
    /// and one of `game_versions`.
    async fn project_versions(
        &self,
        id_or_slug: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Vec<ModrinthVersion>, Error>;

    async fn version(&self, version_id: &str) -> Result<ModrinthVersion, Error>;

    /// A version of `project`, by version id or version number.
    async fn project_version(
        &self,
        project: &str,
        id_or_number: &str,
    ) -> Result<ModrinthVersion, Error>;

    /// The versions files with these SHA-512 hashes belong to, keyed by hash. Hashes that
    /// Modrinth does not know are left out.
    async fn versions_by_hash(
        &self,
        sha512: &[String],
    ) -> Result<HashMap<String, ModrinthVersion>, Error>;

    /// For each file hash, the newest version of the same project for one of `loaders` and
    /// one of `game_versions`, keyed by hash.
    async fn latest_versions_by_hash(
        &self,
        sha512: &[String],
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<HashMap<String, ModrinthVersion>, Error>;

    /// Streams `url` into `file`, failing as soon as more than `max_bytes` arrive. The
    /// caller checks the URL and the hashes.
    async fn download(
        &self,
        url: &url::Url,
        file: &mut tokio::fs::File,
        max_bytes: u64,
    ) -> Result<FileHashes, Error>;
}
