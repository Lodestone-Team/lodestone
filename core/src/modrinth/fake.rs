//! An in-memory [`ModrinthApi`] for tests.

use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;
use color_eyre::eyre::eyre;
use sha1::Digest;
use tokio::io::AsyncWriteExt;

use super::{
    FileHashes, ModrinthApi, ModrinthDependency, ModrinthDependencyType, ModrinthProject,
    ModrinthSearchResponse, ModrinthVersion, ModrinthVersionFile, ModrinthVersionType,
    SearchParams,
};
use crate::error::{Error, ErrorKind};

#[derive(Clone)]
pub struct FakeVersion {
    pub version: ModrinthVersion,
    pub content: Vec<u8>,
}

pub fn sha512_hex(bytes: &[u8]) -> String {
    hex::encode(sha2::Sha512::digest(bytes))
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    hex::encode(sha1::Sha1::digest(bytes))
}

impl FakeVersion {
    /// A release of `project` for Fabric 1.20.1 with one file, whose content is unique to
    /// the version.
    pub fn new(project: &str, id: &str, filename: &str) -> Self {
        let content = format!("content of {id}").into_bytes();
        FakeVersion {
            version: ModrinthVersion {
                id: id.into(),
                project_id: project.into(),
                name: format!("Name of {project}"),
                version_number: format!("{id}-number"),
                version_type: ModrinthVersionType::Release,
                game_versions: vec!["1.20.1".into()],
                loaders: vec!["fabric".into()],
                date_published: "2024-01-01T00:00:00Z".into(),
                downloads: 0,
                files: vec![ModrinthVersionFile {
                    filename: filename.into(),
                    size: content.len() as u64,
                    primary: true,
                    sha512: sha512_hex(&content),
                    url: format!("https://cdn.modrinth.com/data/{project}/versions/{id}/file.jar"),
                }],
                dependencies: vec![],
            },
            content,
        }
    }

    pub fn date(mut self, date: &str) -> Self {
        self.version.date_published = date.into();
        self
    }

    pub fn beta(mut self) -> Self {
        self.version.version_type = ModrinthVersionType::Beta;
        self
    }

    pub fn game_versions(mut self, versions: &[&str]) -> Self {
        self.version.game_versions = versions.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn loaders(mut self, loaders: &[&str]) -> Self {
        self.version.loaders = loaders.iter().map(|s| s.to_string()).collect();
        self
    }

    pub fn url(mut self, url: &str) -> Self {
        self.version.files[0].url = url.into();
        self
    }

    pub fn wrong_hash(mut self) -> Self {
        self.version.files[0].sha512 = sha512_hex(b"something else");
        self
    }

    fn dependency(
        mut self,
        project: &str,
        version: Option<&str>,
        kind: ModrinthDependencyType,
    ) -> Self {
        self.version.dependencies.push(ModrinthDependency {
            project_id: Some(project.into()),
            version_id: version.map(String::from),
            dependency_type: kind,
            file_name: None,
        });
        self
    }

    pub fn requires(self, project: &str, version: Option<&str>) -> Self {
        self.dependency(project, version, ModrinthDependencyType::Required)
    }

    pub fn optional(self, project: &str) -> Self {
        self.dependency(project, None, ModrinthDependencyType::Optional)
    }

    pub fn embeds(self, project: &str) -> Self {
        self.dependency(project, None, ModrinthDependencyType::Embedded)
    }

    pub fn incompatible(self, project: &str) -> Self {
        self.dependency(project, None, ModrinthDependencyType::Incompatible)
    }
}

#[derive(Default)]
pub struct FakeModrinth {
    versions: Mutex<Vec<FakeVersion>>,
    /// Other downloadable files, by URL.
    files: Mutex<HashMap<String, Vec<u8>>>,
    /// URLs that were downloaded, in order.
    pub downloaded: Mutex<Vec<String>>,
    /// `client_side` of projects; `optional` when not set.
    client_sides: Mutex<HashMap<String, String>>,
}

fn not_found() -> Error {
    Error {
        kind: ErrorKind::NotFound,
        source: eyre!("Not found on Modrinth"),
    }
}

impl FakeModrinth {
    pub fn add(&self, version: FakeVersion) {
        self.versions.lock().unwrap().push(version);
    }

    pub fn add_file(&self, url: &str, content: Vec<u8>) {
        self.files.lock().unwrap().insert(url.into(), content);
    }

    fn all(&self) -> Vec<FakeVersion> {
        self.versions.lock().unwrap().clone()
    }

    pub fn set_client_side(&self, project: &str, client_side: &str) {
        self.client_sides
            .lock()
            .unwrap()
            .insert(project.into(), client_side.into());
    }

    fn project_for(&self, id: &str) -> ModrinthProject {
        let mut project = Self::fake_project(id);
        if let Some(side) = self.client_sides.lock().unwrap().get(id) {
            project.client_side = side.clone();
        }
        project
    }

    fn fake_project(id: &str) -> ModrinthProject {
        ModrinthProject {
            id: id.into(),
            slug: id.into(),
            project_type: "mod".into(),
            title: format!("Title of {id}"),
            description: String::new(),
            body: String::new(),
            icon_url: None,
            categories: vec![],
            loaders: vec![],
            game_versions: vec![],
            downloads: 0,
            followers: 0,
            client_side: "optional".into(),
            server_side: "required".into(),
            license_id: None,
            source_url: None,
            issues_url: None,
            wiki_url: None,
            discord_url: None,
            gallery: vec![],
            published: String::new(),
            updated: String::new(),
        }
    }
}

#[async_trait]
impl ModrinthApi for FakeModrinth {
    async fn search(&self, params: &SearchParams) -> Result<ModrinthSearchResponse, Error> {
        Ok(ModrinthSearchResponse {
            hits: vec![],
            offset: params.offset,
            limit: params.limit,
            total_hits: 0,
        })
    }

    async fn project(&self, id_or_slug: &str) -> Result<ModrinthProject, Error> {
        if self
            .all()
            .iter()
            .any(|v| v.version.project_id == id_or_slug)
        {
            Ok(self.project_for(id_or_slug))
        } else {
            Err(not_found())
        }
    }

    async fn projects(&self, ids: &[String]) -> Result<Vec<ModrinthProject>, Error> {
        let all = self.all();
        Ok(ids
            .iter()
            .filter(|id| all.iter().any(|v| &v.version.project_id == *id))
            .map(|id| self.project_for(id))
            .collect())
    }

    async fn project_versions(
        &self,
        id_or_slug: &str,
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<Vec<ModrinthVersion>, Error> {
        let all: Vec<ModrinthVersion> = self
            .all()
            .into_iter()
            .map(|v| v.version)
            .filter(|v| v.project_id == id_or_slug)
            .collect();
        if all.is_empty() {
            return Err(not_found());
        }
        let mut versions: Vec<ModrinthVersion> = all
            .into_iter()
            .filter(|v| loaders.is_empty() || v.loaders.iter().any(|l| loaders.contains(l)))
            .filter(|v| {
                game_versions.is_empty()
                    || v.game_versions.iter().any(|g| game_versions.contains(g))
            })
            .collect();
        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        Ok(versions)
    }

    async fn version(&self, version_id: &str) -> Result<ModrinthVersion, Error> {
        self.all()
            .into_iter()
            .map(|v| v.version)
            .find(|v| v.id == version_id)
            .ok_or_else(not_found)
    }

    async fn project_version(
        &self,
        project: &str,
        id_or_number: &str,
    ) -> Result<ModrinthVersion, Error> {
        self.all()
            .into_iter()
            .map(|v| v.version)
            .find(|v| {
                v.project_id == project
                    && (v.id == id_or_number || v.version_number == id_or_number)
            })
            .ok_or_else(not_found)
    }

    async fn versions_by_hash(
        &self,
        sha512: &[String],
    ) -> Result<HashMap<String, ModrinthVersion>, Error> {
        let all = self.all();
        Ok(sha512
            .iter()
            .filter_map(|hash| {
                all.iter()
                    .find(|v| v.version.files.iter().any(|f| &f.sha512 == hash))
                    .map(|v| (hash.clone(), v.version.clone()))
            })
            .collect())
    }

    async fn latest_versions_by_hash(
        &self,
        sha512: &[String],
        loaders: &[String],
        game_versions: &[String],
    ) -> Result<HashMap<String, ModrinthVersion>, Error> {
        let current = self.versions_by_hash(sha512).await?;
        let mut latest = HashMap::new();
        for (hash, version) in current {
            let versions = self
                .project_versions(&version.project_id, loaders, game_versions)
                .await?;
            if let Some(newest) = versions.into_iter().next() {
                latest.insert(hash, newest);
            }
        }
        Ok(latest)
    }

    async fn download(
        &self,
        url: &url::Url,
        file: &mut tokio::fs::File,
        max_bytes: u64,
    ) -> Result<FileHashes, Error> {
        self.downloaded.lock().unwrap().push(url.to_string());
        let content = self
            .all()
            .into_iter()
            .find(|v| v.version.files.iter().any(|f| f.url == url.as_str()))
            .map(|v| v.content)
            .or_else(|| self.files.lock().unwrap().get(url.as_str()).cloned())
            .ok_or_else(|| Error {
                kind: ErrorKind::External,
                source: eyre!("Download from {} failed: 404", url.host_str().unwrap_or("")),
            })?;
        if content.len() as u64 > max_bytes {
            return Err(super::client::too_large(max_bytes));
        }
        file.write_all(&content).await.unwrap();
        file.flush().await.unwrap();
        Ok(FileHashes {
            size: content.len() as u64,
            sha1: sha1_hex(&content),
            sha512: sha512_hex(&content),
        })
    }
}
