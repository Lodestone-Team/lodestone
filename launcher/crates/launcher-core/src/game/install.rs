//! Installing a version: everything a launch needs, downloaded and checked.
//!
//! [`Installer::prepare`] fetches, in order:
//! 1. the version manifest (cached for offline use) and the version's details,
//!    checked against the manifest's SHA-1;
//! 2. the Java runtime from Mojang's runtime manifest, unless the instance has a Java
//!    override;
//! 3. the client jar, the libraries and the Log4j config;
//! 4. the asset index and its objects, plus the by-name copies old versions read.
//!
//! Each step reports its [`Stage`] and the download engine's counters, and stops when
//! the [`CancelToken`] is set.

use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use http_client::HttpClient;
use lodestone_minecraft::versions::mojang::{MojangVersion, MojangVersionManifest};

use super::{
    assets::{self, AssetIndex, LegacyLayout},
    download::{
        CancelToken, DownloadError, DownloadItem, Downloader, Progress, ProgressSnapshot, RunStats,
        Verify,
    },
    launch::LaunchPaths,
    libraries::{self, Selection},
    mojang::{MojangUrls, UrlError},
    natives,
    platform::{self, Arch, Platform},
    record::{self as install_record, InstallRecord},
    rules::Features,
    runtime::{self, RuntimeIndex, RuntimeManifest},
    version::VersionJson,
};
use crate::{http, paths::DataDir, storage};

/// What the installer is doing, for the Play button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Preparing,
    Java,
    Libraries,
    Assets,
    Natives,
    Starting,
}

impl Stage {
    pub fn label(self) -> &'static str {
        match self {
            Stage::Preparing => "Preparing",
            Stage::Java => "Downloading Java",
            Stage::Libraries => "Downloading libraries",
            Stage::Assets => "Downloading assets",
            Stage::Natives => "Extracting natives",
            Stage::Starting => "Starting Java",
        }
    }
}

/// Shared between the installer and the UI.
#[derive(Debug, Default)]
pub struct InstallProgress {
    stage: Mutex<Stage>,
    pub downloads: Progress,
    /// Files trusted from install records, hashed and downloaded, over the whole
    /// install.
    pub stats: RunStats,
}

impl InstallProgress {
    pub fn stage(&self) -> Stage {
        *self.stage.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn set_stage(&self, stage: Stage) {
        *self.stage.lock().unwrap_or_else(|p| p.into_inner()) = stage;
    }

    pub fn snapshot(&self) -> (Stage, ProgressSnapshot) {
        (self.stage(), self.downloads.snapshot())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("cancelled")]
    Cancelled,
    #[error("couldn't reach Mojang's servers: {0}")]
    Network(String),
    #[error("Minecraft {0} isn't in Mojang's version list")]
    UnknownVersion(String),
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Download(DownloadError),
    #[error(transparent)]
    Runtime(#[from] runtime::RuntimeError),
    #[error(transparent)]
    Url(#[from] UrlError),
    #[error("{}", platform::ROSETTA_MISSING)]
    RosettaMissing,
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

impl From<DownloadError> for InstallError {
    fn from(err: DownloadError) -> Self {
        match err {
            DownloadError::Cancelled => InstallError::Cancelled,
            other => InstallError::Download(other),
        }
    }
}

fn io_err(path: &Path) -> impl FnOnce(io::Error) -> InstallError + '_ {
    move |source| InstallError::Io {
        path: path.to_owned(),
        source,
    }
}

/// Which Java runs the game.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JavaChoice {
    /// Mojang's runtime for the version.
    Mojang,
    /// The instance's Java override.
    Override(PathBuf),
}

/// A version ready to launch.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub version: VersionJson,
    pub java: PathBuf,
    /// The platform of that Java: the machine with the Java's architecture.
    pub java_platform: Platform,
    /// Whether the Java is x86_64 on an Apple Silicon Mac.
    pub rosetta: bool,
    pub libraries: Selection,
    pub client_jar: PathBuf,
    pub logging_config: Option<PathBuf>,
    pub assets_layout: Option<LegacyLayout>,
}

/// Installs versions into a data directory.
#[derive(Clone)]
pub struct Installer {
    http: Arc<dyn HttpClient>,
    downloader: Downloader,
    urls: MojangUrls,
    data: DataDir,
    /// Whether x86_64 programs can run (Rosetta). Replaced in tests.
    can_run_x86_64: fn() -> bool,
}

impl Installer {
    pub fn new(
        http: Arc<dyn HttpClient>,
        downloader: Downloader,
        urls: MojangUrls,
        data: DataDir,
    ) -> Self {
        Self {
            http,
            downloader,
            urls,
            data,
            can_run_x86_64: platform::can_run_x86_64,
        }
    }

    pub fn with_x86_64_check(mut self, check: fn() -> bool) -> Self {
        self.can_run_x86_64 = check;
        self
    }

    pub fn data_dir(&self) -> &DataDir {
        &self.data
    }

    /// Fetches a small JSON file, keeping a copy in the cache to fall back on when
    /// the network is down.
    async fn fetch_cached(&self, url: &str, cache_name: &str) -> Result<Vec<u8>, InstallError> {
        let cache = self.data.cache_dir().join(cache_name);
        let fetched = async {
            let request = http::get(url, None).map_err(|err| err.to_string())?;
            let response = http::send(&*self.http, request)
                .await
                .map_err(|err| err.to_string())?;
            if !response.is_success() {
                return Err(format!("HTTP {}", response.status));
            }
            Ok(response.body)
        }
        .await;
        match fetched {
            Ok(body) => {
                // Best effort: the cache only matters when offline.
                let _ = storage::write_atomic(&cache, &body);
                Ok(body)
            }
            Err(reason) => fs::read(&cache).map_err(|_| InstallError::Network(reason)),
        }
    }

    /// Mojang's version list, newest first.
    pub async fn version_manifest(&self) -> Result<MojangVersionManifest, InstallError> {
        let body = self
            .fetch_cached(&self.urls.version_manifest(), "version_manifest_v2.json")
            .await?;
        serde_json::from_slice(&body).map_err(|err| {
            InstallError::Invalid(format!("Mojang's version list is invalid: {err}"))
        })
    }

    fn version_json_path(&self, id: &str) -> Result<PathBuf, InstallError> {
        let id = libraries::safe_relative_path(id)
            .map_err(|_| InstallError::Invalid(format!("{id:?} is not a valid version id")))?;
        let name = format!("{}.json", id.display());
        Ok(self.data.versions_dir().join(&id).join(name))
    }

    /// The version's details: downloaded and checked against the manifest, or the
    /// installed copy when Mojang can't be reached.
    pub async fn version_json(
        &self,
        id: &str,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<VersionJson, InstallError> {
        let path = self.version_json_path(id)?;
        let entry = match self.version_manifest().await {
            Ok(manifest) => Some(
                manifest
                    .versions
                    .into_iter()
                    .find(|version| version.id == id)
                    .ok_or_else(|| InstallError::UnknownVersion(id.to_string()))?,
            ),
            Err(InstallError::Network(_)) if path.is_file() => None,
            Err(err) => return Err(err),
        };
        if let Some(MojangVersion { url, sha1, .. }) = entry {
            let sha1 = sha1.ok_or_else(|| {
                InstallError::Invalid(format!("Mojang's version list has no SHA-1 for {id}"))
            })?;
            let item = DownloadItem {
                url: self.urls.resolve(&url)?,
                dest: path.clone(),
                sha1,
                size: None,
                executable: false,
            };
            self.downloader
                .fetch(&item, &progress.downloads, cancel)
                .await?;
        }
        let bytes = fs::read(&path).map_err(io_err(&path))?;
        VersionJson::parse(&bytes)
            .map_err(|err| InstallError::Invalid(format!("the details of {id} are invalid: {err}")))
    }

    /// Installs `id` and returns what launching it needs. Files the install records
    /// vouch for are not hashed again.
    pub async fn prepare(
        &self,
        id: &str,
        machine: &Platform,
        java: &JavaChoice,
        game_dir: &Path,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<Prepared, InstallError> {
        self.prepare_with(
            id,
            machine,
            java,
            game_dir,
            Verify::Recorded,
            progress,
            cancel,
        )
        .await
    }

    /// Like [`Installer::prepare`]; with [`Verify::Full`] ("Verify game files") the
    /// install records and the runtime's marker are ignored, every file is hashed,
    /// broken ones are downloaded again, and the records are rewritten.
    #[allow(clippy::too_many_arguments)]
    pub async fn prepare_with(
        &self,
        id: &str,
        machine: &Platform,
        java: &JavaChoice,
        game_dir: &Path,
        verify: Verify,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<Prepared, InstallError> {
        progress.set_stage(Stage::Preparing);
        let version = self.version_json(id, progress, cancel).await?;
        let java_arch = libraries::java_arch(&version, machine);

        // The Java runtime.
        let (java_path, java_platform) = match java {
            JavaChoice::Override(path) => (path.clone(), machine.with_arch(java_arch)),
            JavaChoice::Mojang => {
                progress.set_stage(Stage::Java);
                self.install_runtime(&version, machine, java_arch, verify, progress, cancel)
                    .await?
            }
        };
        let rosetta = machine.os == platform::Os::MacOs
            && machine.arch == Arch::Arm64
            && java_platform.arch == Arch::X86_64;

        // The client jar, the libraries and the logging config.
        progress.set_stage(Stage::Libraries);
        let libraries = libraries::select(&version, &java_platform, &Features::default())
            .map_err(|err| InstallError::Invalid(err.to_string()))?;
        let version_dir = self.data.versions_dir().join(&version.id);
        let client_jar = version_dir.join(format!("{}.jar", version.id));
        let mut items = Vec::new();
        let client = version.client_download().ok_or_else(|| {
            InstallError::Invalid(format!("Minecraft {id} has no client download"))
        })?;
        items.push(DownloadItem {
            url: self.urls.resolve(&client.url)?,
            dest: client_jar.clone(),
            sha1: client.sha1.clone(),
            size: Some(client.size),
            executable: false,
        });
        let libraries_dir = self.data.libraries_dir();
        for file in libraries.files() {
            let dest = libraries_dir.join(&file.path);
            match &file.remote {
                Some(remote) => items.push(DownloadItem {
                    url: self.urls.resolve(&remote.url)?,
                    dest,
                    sha1: remote.sha1.clone(),
                    size: Some(remote.size),
                    executable: false,
                }),
                None if dest.is_file() => {}
                None => {
                    return Err(InstallError::Invalid(format!(
                        "the library {} has no download and isn't installed",
                        file.name
                    )));
                }
            }
        }
        let logging_config = match version.client_logging() {
            Some(logging) => {
                let name = libraries::safe_relative_path(&logging.file.id)
                    .map_err(|err| InstallError::Invalid(err.to_string()))?;
                let dest = self.data.assets_dir().join("log_configs").join(name);
                items.push(DownloadItem {
                    url: self.urls.resolve(&logging.file.url)?,
                    dest: dest.clone(),
                    sha1: logging.file.sha1.clone(),
                    size: Some(logging.file.size),
                    executable: false,
                });
                Some(dest)
            }
            None => None,
        };
        let record_path =
            install_record::version_record_path(&self.data.versions_dir(), &version.id);
        self.run_recorded(items, &record_path, &version.sha1, verify, progress, cancel)
            .await?;

        // The assets.
        progress.set_stage(Stage::Assets);
        let assets_layout = self
            .install_assets(&version, game_dir, verify, progress, cancel)
            .await?;

        Ok(Prepared {
            version,
            java: java_path,
            java_platform,
            rosetta,
            libraries,
            client_jar,
            logging_config,
            assets_layout,
        })
    }

    /// Runs the downloads with the install record at `record_path`, keyed by `key`,
    /// and saves the record afterwards, also after an error, so the files checked
    /// so far need not be hashed again.
    async fn run_recorded(
        &self,
        items: Vec<DownloadItem>,
        record_path: &Path,
        key: &str,
        verify: Verify,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<(), InstallError> {
        let mut record = InstallRecord::load(record_path, key);
        let result = self
            .downloader
            .run_recorded(
                items,
                self.data.root(),
                &mut record,
                verify,
                &progress.downloads,
                cancel,
                &progress.stats,
            )
            .await;
        if let Err(err) = record.save(record_path) {
            // Only the next launch's speed depends on it.
            eprintln!(
                "lodestone-launcher: saving {}: {err}",
                record_path.display()
            );
        }
        Ok(result?)
    }

    async fn install_runtime(
        &self,
        version: &VersionJson,
        machine: &Platform,
        java_arch: Arch,
        verify: Verify,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<(PathBuf, Platform), InstallError> {
        let body = self
            .fetch_cached(&self.urls.runtime_manifest(), "java_runtimes.json")
            .await?;
        let index: RuntimeIndex = serde_json::from_slice(&body).map_err(|err| {
            InstallError::Invalid(format!("Mojang's Java runtime list is invalid: {err}"))
        })?;
        let choice = runtime::select(
            &index,
            version.runtime_component(),
            &machine.with_arch(java_arch),
        )?;
        if choice.needs_rosetta(machine) && !(self.can_run_x86_64)() {
            return Err(InstallError::RosettaMissing);
        }
        let java_platform = machine.with_arch(choice.arch);
        let root = runtime::install_dir(&self.data.runtimes_dir(), &choice)?;
        let java = runtime::java_binary(&root, machine.os);
        let manifest_sha1 = &choice.entry.manifest.sha1;
        if verify == Verify::Recorded
            && runtime::is_installed(&root, manifest_sha1)
            && java.is_file()
        {
            return Ok((java, java_platform));
        }

        // The file manifest, kept beside the runtime.
        let manifest_path = root.with_extension("manifest.json");
        self.downloader
            .fetch(
                &DownloadItem {
                    url: self.urls.resolve(&choice.entry.manifest.url)?,
                    dest: manifest_path.clone(),
                    sha1: manifest_sha1.clone(),
                    size: Some(choice.entry.manifest.size),
                    executable: false,
                },
                &progress.downloads,
                cancel,
            )
            .await?;
        let bytes = fs::read(&manifest_path).map_err(io_err(&manifest_path))?;
        let manifest: RuntimeManifest = serde_json::from_slice(&bytes).map_err(|err| {
            InstallError::Invalid(format!("the Java runtime's file list is invalid: {err}"))
        })?;
        let plan = runtime::plan(&manifest, &root, &self.urls)?;
        runtime::create_directories(&plan).map_err(io_err(&root))?;
        self.downloader
            .run(plan.files.clone(), &progress.downloads, cancel)
            .await?;
        runtime::create_links(&plan).map_err(io_err(&root))?;
        if !java.is_file() {
            return Err(InstallError::Invalid(format!(
                "the Java runtime has no java at {}",
                java.display()
            )));
        }
        runtime::mark_installed(&root, manifest_sha1).map_err(io_err(&root))?;
        Ok((java, java_platform))
    }

    async fn install_assets(
        &self,
        version: &VersionJson,
        game_dir: &Path,
        verify: Verify,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<Option<LegacyLayout>, InstallError> {
        let Some(index_ref) = &version.asset_index else {
            return Ok(None);
        };
        let assets_dir = self.data.assets_dir();
        let index_name = libraries::safe_relative_path(&format!("{}.json", index_ref.id))
            .map_err(|err| InstallError::Invalid(err.to_string()))?;
        let index_path = assets_dir.join("indexes").join(index_name);
        self.downloader
            .fetch(
                &DownloadItem {
                    url: self.urls.resolve(&index_ref.url)?,
                    dest: index_path.clone(),
                    sha1: index_ref.sha1.clone(),
                    size: Some(index_ref.size),
                    executable: false,
                },
                &progress.downloads,
                cancel,
            )
            .await?;
        let bytes = fs::read(&index_path).map_err(io_err(&index_path))?;
        let index = AssetIndex::parse(&bytes)
            .map_err(|err| InstallError::Invalid(format!("the asset index is invalid: {err}")))?;
        let base = self.urls.resources_base();
        let mut items = Vec::with_capacity(index.objects.len());
        for object in index.objects.values() {
            let invalid = |err: assets::AssetError| InstallError::Invalid(err.to_string());
            items.push(DownloadItem {
                url: assets::object_url(&base, &object.hash).map_err(invalid)?,
                dest: assets_dir.join(assets::object_path(&object.hash).map_err(invalid)?),
                sha1: object.hash.clone(),
                size: Some(object.size),
                executable: false,
            });
        }
        let record_path = install_record::assets_record_path(&assets_dir, &index_ref.id);
        self.run_recorded(
            items,
            &record_path,
            &index_ref.sha1,
            verify,
            progress,
            cancel,
        )
        .await?;

        let layout = index.legacy_layout();
        if let Some(layout) = layout {
            for (name, object) in &index.objects {
                if cancel.is_cancelled() {
                    return Err(InstallError::Cancelled);
                }
                let invalid = |err: assets::AssetError| InstallError::Invalid(err.to_string());
                let target =
                    assets::legacy_path(layout, &assets_dir, &index_ref.id, game_dir, name)
                        .map_err(invalid)?;
                if fs::metadata(&target).is_ok_and(|meta| meta.len() == object.size) {
                    continue;
                }
                let source = assets_dir.join(assets::object_path(&object.hash).map_err(invalid)?);
                if let Some(parent) = target.parent() {
                    fs::create_dir_all(parent).map_err(io_err(parent))?;
                }
                fs::copy(&source, &target).map_err(io_err(&target))?;
            }
        }
        Ok(layout)
    }
}

impl Prepared {
    /// The paths for [`launch::build`](super::launch::build), with the natives in
    /// `natives_dir` and the game in `game_dir`.
    pub fn launch_paths(
        &self,
        data: &DataDir,
        game_dir: &Path,
        natives_dir: &Path,
    ) -> Result<LaunchPaths, InstallError> {
        let libraries_dir = data.libraries_dir();
        let assets_dir = data.assets_dir();
        let mut classpath: Vec<PathBuf> = self
            .libraries
            .classpath
            .iter()
            .map(|file| libraries_dir.join(&file.path))
            .collect();
        classpath.push(self.client_jar.clone());
        let game_assets = assets::game_assets_dir(
            self.assets_layout,
            &assets_dir,
            self.version.assets_id(),
            game_dir,
        )
        .map_err(|err| InstallError::Invalid(err.to_string()))?;
        Ok(LaunchPaths {
            java: self.java.clone(),
            game_dir: game_dir.to_owned(),
            natives_dir: natives_dir.to_owned(),
            libraries_dir,
            assets_dir,
            game_assets,
            classpath,
            logging_config: self.logging_config.clone(),
        })
    }

    /// Empties `natives_dir` and extracts the old-style natives into it.
    pub fn extract_natives(&self, data: &DataDir, natives_dir: &Path) -> Result<(), InstallError> {
        natives::reset_dir(natives_dir).map_err(io_err(natives_dir))?;
        let libraries_dir = data.libraries_dir();
        for native in &self.libraries.natives {
            natives::extract(
                &libraries_dir.join(&native.file.path),
                natives_dir,
                &native.exclude,
            )
            .map_err(|err| InstallError::Invalid(err.to_string()))?;
        }
        Ok(())
    }
}
