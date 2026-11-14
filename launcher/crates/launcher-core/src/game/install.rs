//! Installing a version: everything a launch needs, downloaded and checked.
//!
//! [`Installer::prepare`] fetches, in order:
//! 1. the version manifest (cached for offline use) and the version's details,
//!    checked against the manifest's SHA-1;
//! 2. the Java runtime from Mojang's runtime manifest (or, for Java 8 on Apple
//!    silicon, Azul's Zulu 8: see [`super::zulu`]), unless the instance has a Java
//!    override;
//! 3. the client jar, the libraries and the Log4j config;
//! 4. for an instance with a mod loader: the loader's profile, merged onto the
//!    version (see [`super::loaders`]), its libraries, and for Forge and NeoForge the
//!    installer's files and processors;
//! 5. the asset index and its objects, plus the by-name copies old versions read.
//!
//! Each step reports its [`Stage`] and the download engine's counters, and stops when
//! the [`CancelToken`] is set.

mod loader;

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
    loaders::{self, LoaderChoice, LoaderKind},
    mojang::{MojangUrls, UrlError},
    natives,
    platform::{self, Arch, Os, Platform},
    record::{self as install_record, InstallRecord},
    rules::Features,
    runtime::{self, JavaSource, RuntimeIndex, RuntimeManifest},
    version::VersionJson,
    zulu,
};
use crate::{http, paths::DataDir, storage};

/// What the installer is doing, for the Play button.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Preparing,
    Java,
    Libraries,
    /// Fetching the mod loader's profile or installer.
    Loader,
    /// Running Forge's or NeoForge's installer processors.
    Processors,
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
            Stage::Loader => "Installing the mod loader",
            Stage::Processors => "Patching Minecraft for the mod loader",
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
    /// Lines for the console (the loader installer's output), taken by the UI.
    notes: Mutex<Vec<String>>,
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

    /// Adds a line for the console.
    pub fn note(&self, line: impl Into<String>) {
        self.notes
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(line.into());
    }

    /// The lines added since the last call.
    pub fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut *self.notes.lock().unwrap_or_else(|p| p.into_inner()))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("cancelled")]
    Cancelled,
    #[error("couldn't download a file: {0}")]
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
    /// Azul has no Zulu 8 for Apple silicon that the version can run in (its list is
    /// empty, not found, or has only newer builds than [`zulu::Zulu8::Before8u321`]
    /// takes), none was installed before, and Mojang makes no arm64 Java 8.
    #[error(
        "This Minecraft version needs Java 8, and no Java 8 made for Apple silicon is available: Mojang makes none, and Azul lists none this version can run in. Lodestone doesn't run it in an Intel Java without saying so; try again later, or choose a Java 8 made for Apple silicon (arm64) in the instance's settings."
    )]
    NoNativeJava8,
    /// The version runs only in an Intel Java on Apple silicon (its libraries have no
    /// arm64 build), and the instance's Java override is made for Apple silicon.
    #[error(
        "Minecraft {0} runs only in an Intel (x86_64) Java on Apple silicon, through Rosetta 2, and the instance's Java override is made for Apple silicon. Choose an Intel Java in the instance's settings, or remove the override to use the Java Lodestone provides."
    )]
    NeedsIntelJava(String),
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

/// Forge before 1.13 draws its loading splash from a second thread, which current
/// macOS rejects: the game aborts in AppKit as the window opens (seen with Forge
/// 1.7.10). Later 1.x builds turn the splash off on macOS by default; for the rest,
/// write `enabled=false` the first time, and never touch a file that already exists.
fn disable_forge_splash(game_dir: &Path) -> Result<(), InstallError> {
    let path = game_dir.join("config").join("splash.properties");
    if path.exists() {
        return Ok(());
    }
    storage::write_atomic(&path, b"enabled=false\n").map_err(io_err(&path))
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

/// Why a game runs in an Intel Java under Rosetta 2 on an Apple silicon Mac.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rosetta {
    /// The version needs libraries Mojang made no Apple silicon build of
    /// ([`libraries::java_arch`]).
    Version,
    /// The instance's Java override is an Intel Java (the version could run natively).
    JavaOverride,
}

/// The architecture of the Java `version` runs in on `machine` with `java`.
///
/// With Mojang's Java it is [`libraries::java_arch`]. A Java override on an Apple
/// silicon Mac is read ([`platform::mac_binary_arch`]): an Intel one runs any version
/// under Rosetta 2, with the version's Intel libraries; one made for Apple silicon
/// can't run a version that needs an Intel Java ([`InstallError::NeedsIntelJava`]).
/// Elsewhere, or when the file isn't a Mac program (a script), the override is taken to
/// be what the version needs, as it always was.
pub fn java_arch_for(
    version: &VersionJson,
    machine: &Platform,
    java: &JavaChoice,
) -> Result<Arch, InstallError> {
    let needed = libraries::java_arch(version, machine);
    let JavaChoice::Override(path) = java else {
        return Ok(needed);
    };
    if machine.os != Os::MacOs || machine.arch != Arch::Arm64 {
        return Ok(needed);
    }
    match platform::mac_binary_arch(path) {
        Some(Arch::X86_64) => Ok(Arch::X86_64),
        Some(Arch::Arm64) if needed == Arch::X86_64 => {
            Err(InstallError::NeedsIntelJava(version.id.clone()))
        }
        Some(arch) => Ok(arch),
        None => Ok(needed),
    }
}

/// Whether, and why, `version` runs under Rosetta 2 on `machine` with `java`: an
/// Intel Java on an Apple silicon Mac ([`java_arch_for`]). `None` when it can't run
/// at all with that Java.
pub fn rosetta_for(
    version: &VersionJson,
    machine: &Platform,
    java: &JavaChoice,
) -> Option<Rosetta> {
    let arch = java_arch_for(version, machine, java).ok()?;
    rosetta_reason(version, machine, java, arch)
}

fn rosetta_reason(
    version: &VersionJson,
    machine: &Platform,
    java: &JavaChoice,
    arch: Arch,
) -> Option<Rosetta> {
    if machine.os != Os::MacOs || machine.arch != Arch::Arm64 || arch != Arch::X86_64 {
        return None;
    }
    Some(
        if matches!(java, JavaChoice::Override(_))
            && libraries::java_arch(version, machine) == Arch::Arm64
        {
            Rosetta::JavaOverride
        } else {
            Rosetta::Version
        },
    )
}

/// A version ready to launch.
#[derive(Clone, Debug)]
pub struct Prepared {
    pub version: VersionJson,
    pub java: PathBuf,
    /// The platform of that Java: the machine with the Java's architecture.
    pub java_platform: Platform,
    /// Why the Java is x86_64 on an Apple silicon Mac, if it is: the game then runs
    /// under Rosetta 2, which the player must be told about.
    pub rosetta: Option<Rosetta>,
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
        Ok(self
            .fetch_cached_or_missing(url, cache_name, &[])
            .await?
            .unwrap_or_default())
    }

    /// Like [`Self::fetch_cached`], but a response with one of the `missing` statuses
    /// means "there is nothing for this" (`None`) rather than an error: Fabric's meta
    /// answers 400, and Quilt's 404, for a Minecraft version they don't support.
    pub(crate) async fn fetch_cached_or_missing(
        &self,
        url: &str,
        cache_name: &str,
        missing: &[u16],
    ) -> Result<Option<Vec<u8>>, InstallError> {
        let cache = self.data.cache_dir().join(cache_name);
        match self.fetch_or_missing(url, missing).await {
            Ok(Some(body)) => {
                // Best effort: the cache only matters when offline.
                let _ = storage::write_atomic(&cache, &body);
                Ok(Some(body))
            }
            Ok(None) => {
                // A copy from before would now be wrong.
                let _ = fs::remove_file(&cache);
                Ok(None)
            }
            Err(InstallError::Network(reason)) => fs::read(&cache)
                .map(Some)
                .map_err(|_| InstallError::Network(reason)),
            Err(err) => Err(err),
        }
    }

    /// Fetches a small file, with no cache: `None` for one of the `missing` statuses,
    /// [`InstallError::Network`] when it can't be fetched.
    async fn fetch_or_missing(
        &self,
        url: &str,
        missing: &[u16],
    ) -> Result<Option<Vec<u8>>, InstallError> {
        let request =
            http::get(url, None).map_err(|err| InstallError::Network(format!("{url}: {err}")))?;
        let response = http::send(&*self.http, request)
            .await
            .map_err(|err| InstallError::Network(format!("{url}: {err}")))?;
        if missing.contains(&response.status) {
            return Ok(None);
        }
        if !response.is_success() {
            return Err(InstallError::Network(format!(
                "{url}: HTTP {}",
                response.status
            )));
        }
        Ok(Some(response.body))
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
    #[allow(clippy::too_many_arguments)]
    pub async fn prepare(
        &self,
        id: &str,
        loader: Option<&LoaderChoice>,
        machine: &Platform,
        java: &JavaChoice,
        game_dir: &Path,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<Prepared, InstallError> {
        self.prepare_with(
            id,
            loader,
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
        loader: Option<&LoaderChoice>,
        machine: &Platform,
        java: &JavaChoice,
        game_dir: &Path,
        verify: Verify,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<Prepared, InstallError> {
        progress.set_stage(Stage::Preparing);
        let vanilla = self.version_json(id, progress, cancel).await?;
        let child = match loader {
            Some(choice) => Some(
                self.loader_profile(choice, &vanilla, progress, cancel)
                    .await?,
            ),
            None => None,
        };
        let version = match &child {
            Some(child) => loaders::merge(&child.version, &vanilla),
            None => vanilla.clone(),
        };
        let java_arch = java_arch_for(&version, machine, java)?;
        if machine.os == Os::MacOs
            && loader.is_some_and(|choice| choice.kind == LoaderKind::Forge)
            && version.minecraft_arguments.is_some()
        {
            disable_forge_splash(game_dir)?;
        }

        // The Java runtime.
        let (java_path, java_platform) = match java {
            JavaChoice::Override(path) => {
                let java_platform = machine.with_arch(java_arch);
                // An Intel override needs Rosetta as much as Mojang's Intel Java does.
                if java_platform.arch == Arch::X86_64
                    && machine.os == Os::MacOs
                    && machine.arch == Arch::Arm64
                    && !(self.can_run_x86_64)()
                {
                    return Err(InstallError::RosettaMissing);
                }
                (path.clone(), java_platform)
            }
            JavaChoice::Mojang => {
                progress.set_stage(Stage::Java);
                self.install_runtime(&version, machine, java_arch, verify, progress, cancel)
                    .await?
            }
        };
        let rosetta = rosetta_reason(&version, machine, java, java_platform.arch);

        // The vanilla client jar, libraries and logging config, under the vanilla
        // version's record, as for an instance without a loader.
        progress.set_stage(Stage::Libraries);
        let vanilla_libraries =
            libraries::select_on(&vanilla, &java_platform, machine.arch, &Features::default())
                .map_err(|err| InstallError::Invalid(err.to_string()))?;
        let vanilla_jar = self
            .data
            .versions_dir()
            .join(&vanilla.id)
            .join(format!("{}.jar", vanilla.id));
        let client = vanilla.client_download().ok_or_else(|| {
            InstallError::Invalid(format!("Minecraft {id} has no client download"))
        })?;
        let client_item = |dest: PathBuf| -> Result<DownloadItem, InstallError> {
            Ok(DownloadItem {
                url: self.urls.resolve(&client.url)?,
                dest,
                sha1: client.sha1.clone(),
                size: Some(client.size),
                executable: false,
            })
        };
        let mut items = vec![client_item(vanilla_jar.clone())?];
        items.extend(self.library_downloads(&vanilla_libraries)?);
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
            install_record::version_record_path(&self.data.versions_dir(), &vanilla.id);
        self.run_recorded(items, &record_path, &vanilla.sha1, verify, progress, cancel)
            .await?;

        // The loader's libraries, the processors' libraries, and the client jar under
        // the loader's name, which Forge's `-DignoreList` expects.
        let (libraries, client_jar) = match &child {
            None => (vanilla_libraries, vanilla_jar),
            Some(child) => {
                let merged = libraries::select_on(
                    &version,
                    &java_platform,
                    machine.arch,
                    &Features::default(),
                )
                .map_err(|err| InstallError::Invalid(err.to_string()))?;
                let client_jar = self
                    .data
                    .versions_dir()
                    .join(&version.id)
                    .join(format!("{}.jar", version.id));
                link_or_copy(&vanilla_jar, &client_jar).map_err(io_err(&client_jar))?;
                let mut items = vec![client_item(client_jar.clone())?];
                let vanilla_paths: std::collections::HashSet<&Path> = vanilla_libraries
                    .files()
                    .map(|f| f.path.as_path())
                    .collect();
                let loader_only = Selection {
                    classpath: merged
                        .classpath
                        .iter()
                        .filter(|f| !vanilla_paths.contains(f.path.as_path()))
                        .cloned()
                        .collect(),
                    ..Selection::default()
                };
                items.extend(self.library_downloads(&loader_only)?);
                items.extend(self.processor_downloads(&version.id)?);
                let record_path =
                    install_record::version_record_path(&self.data.versions_dir(), &version.id);
                self.run_recorded(items, &record_path, &version.sha1, verify, progress, cancel)
                    .await?;
                if let Some(installer) = &child.installer {
                    let from_installer = || -> Result<(), InstallError> {
                        self.restore_installer_files(
                            &child.version,
                            &version.id,
                            installer,
                            verify,
                        )?;
                        self.run_processors(
                            &version.id,
                            installer,
                            &vanilla,
                            &vanilla_jar,
                            &java_path,
                            verify,
                            progress,
                            cancel,
                        )
                    };
                    match (from_installer(), loader) {
                        // The installer jar is only downloaded with the profile, and
                        // only opened when something has to come out of it. Gone
                        // since (deleted with `libraries/`, say), it is downloaded
                        // again, so Play and "Verify game files" can repair the files.
                        (Err(_), Some(choice)) if !installer.is_file() => {
                            self.fetch_installer(choice, installer, progress, cancel)
                                .await?;
                            from_installer()?;
                        }
                        (result, _) => result?,
                    }
                }
                (merged, client_jar)
            }
        };
        // Files without a download come from the loader's installer.
        for file in libraries.files().filter(|file| file.remote.is_none()) {
            if !self.data.libraries_dir().join(&file.path).is_file() {
                return Err(InstallError::Invalid(format!(
                    "the library {} has no download and isn't installed",
                    file.name
                )));
            }
        }

        // The assets.
        progress.set_stage(Stage::Assets);
        let assets_layout = self
            .install_assets(&version, game_dir, verify, progress, cancel)
            .await?;
        let mut version = version;
        if libraries.swapped_lwjgl && assets_layout.is_none() {
            self.hide_window_icons(&mut version)?;
        }
        if libraries.swapped_lwjgl && forge_early_window_sets_icon(&version) {
            without_forge_early_window(&mut version);
        }

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

    /// The downloads of the libraries in `selection` that have one.
    fn library_downloads(&self, selection: &Selection) -> Result<Vec<DownloadItem>, InstallError> {
        let libraries_dir = self.data.libraries_dir();
        selection
            .files()
            .filter_map(|file| {
                let remote = file.remote.as_ref()?;
                Some(self.urls.resolve(&remote.url).map(|url| DownloadItem {
                    url,
                    dest: libraries_dir.join(&file.path),
                    sha1: remote.sha1.clone(),
                    size: remote.size,
                    executable: false,
                }))
            })
            .collect::<Result<_, _>>()
            .map_err(InstallError::from)
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
        let source = runtime::select(
            &index,
            version.runtime_component(),
            &machine.with_arch(java_arch),
        )?;
        if source.needs_rosetta(machine) && !(self.can_run_x86_64)() {
            return Err(InstallError::RosettaMissing);
        }
        let choice = match source {
            JavaSource::Mojang(choice) => choice,
            JavaSource::Zulu8 => {
                let line = zulu::Zulu8::for_version(version);
                return self
                    .install_zulu(machine, line, verify, progress, cancel)
                    .await;
            }
        };
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

    /// Installs Azul's newest Zulu 8 of `line` for macOS on arm64 ([`zulu`]) into
    /// `<runtimes>/<line's component>/mac-os-arm64/<build>`, unless that build is
    /// installed already.
    ///
    /// When Azul can't be reached, lists no build (a 404 or an empty list), answers
    /// something that isn't a list, or its build can't be downloaded, the newest build
    /// installed before is used; only with none installed is that an error:
    /// [`InstallError::NoNativeJava8`] when Azul has none, the network error otherwise
    /// (an outage is never taken for "none"). "Verify game files" ([`Verify::Full`])
    /// unpacks the build again from its archive, which is kept in the cache, and
    /// doesn't fall back.
    async fn install_zulu(
        &self,
        machine: &Platform,
        line: zulu::Zulu8,
        verify: Verify,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<(PathBuf, Platform), InstallError> {
        let java_platform = machine.with_arch(Arch::Arm64);
        let builds = self
            .data
            .runtimes_dir()
            .join(line.component())
            .join(java_platform.runtime_key());
        let installed_before = |why: InstallError| match zulu::newest_installed(&builds) {
            Some(java) if verify == Verify::Recorded => {
                eprintln!(
                    "lodestone-launcher: using the Zulu 8 installed before ({}): {why}",
                    java.display()
                );
                Ok((java, java_platform.clone()))
            }
            _ => Err(why),
        };
        let packages_url = self.urls.resolve_java(line.packages_url())?;
        let build = match self.fetch_or_missing(&packages_url, &[404]).await {
            Ok(Some(body)) => {
                // A body that isn't Azul's list (a proxy's or a captive portal's page
                // served as 200) is no reason to stop using the build installed.
                let packages: Vec<zulu::ZuluPackage> = match serde_json::from_slice(&body) {
                    Ok(packages) => packages,
                    Err(err) => {
                        return installed_before(InstallError::Invalid(format!(
                            "Azul's list of Java builds is invalid: {err}"
                        )));
                    }
                };
                match zulu::pick(&packages, line) {
                    Some(build) => build,
                    None => return installed_before(InstallError::NoNativeJava8),
                }
            }
            Ok(None) => return installed_before(InstallError::NoNativeJava8),
            Err(err @ InstallError::Network(_)) => return installed_before(err),
            Err(err) => return Err(err),
        };
        let folder = libraries::safe_relative_path(zulu::build_folder(&build))
            .map_err(|err| InstallError::Invalid(err.to_string()))?;
        let root = builds.join(&folder);
        if verify == Verify::Recorded
            && runtime::is_installed(&root, &build.sha256)
            && let Some(java) = zulu::java_binary(&root)
        {
            return Ok((java, java_platform));
        }

        // The archive, checked against Azul's SHA-256, into the cache, where it stays:
        // "Verify game files" unpacks it again without downloading it.
        let name = libraries::safe_relative_path(&build.name)
            .map_err(|err| InstallError::Invalid(err.to_string()))?;
        let archives = self.data.cache_dir().join("java");
        let archive = archives.join(&name);
        let fetched = self
            .downloader
            .fetch_sha256(
                &DownloadItem {
                    url: self.urls.resolve_java(&build.url)?,
                    dest: archive.clone(),
                    sha1: String::new(),
                    // Azul's sizes are rounded (40346800 for a 40346754-byte zip):
                    // only shown.
                    size: None,
                    executable: false,
                },
                &build.sha256,
                build.size,
                &progress.downloads,
                cancel,
            )
            .await;
        match fetched {
            Ok(()) => {}
            Err(DownloadError::Failed { url, reason }) => {
                return installed_before(InstallError::Network(format!("{url}: {reason}")));
            }
            Err(err) => return Err(err.into()),
        }
        if cancel.is_cancelled() {
            return Err(InstallError::Cancelled);
        }

        // Unpacked into a folder of this install's own (two games may start at once),
        // checked and marked there, then moved into place: a build's folder is either
        // complete or absent, and nothing here deletes a build a running game uses.
        let staging = builds.join(format!(
            ".{}.unpacking-{}-{:016x}",
            folder.display(),
            std::process::id(),
            u64::from_le_bytes(crate::random_bytes())
        ));
        let unpacked = (|| {
            zulu::unpack(&archive, &staging)
                .map_err(|err| InstallError::Invalid(err.to_string()))?;
            if zulu::java_binary(&staging).is_none() {
                return Err(InstallError::Invalid(format!(
                    "Azul's Java 8 ({}) has no java binary",
                    build.name
                )));
            }
            runtime::mark_installed(&staging, &build.sha256).map_err(io_err(&staging))
        })();
        if let Err(err) = unpacked {
            let _ = fs::remove_dir_all(&staging);
            return Err(err);
        }
        if let Err(err) = self.move_build_into_place(&staging, &root, &build.sha256) {
            let _ = fs::remove_dir_all(&staging);
            return Err(err);
        }
        // Best effort: this line's older archives only take space (the other line's
        // archive stays, for its "Verify game files"). Older builds may be in use, so
        // they stay until the storage cleanup, which runs while no game does.
        if let Ok(entries) = fs::read_dir(&archives) {
            for entry in entries.flatten() {
                let file = entry.file_name();
                let file = file.to_string_lossy();
                if file.starts_with("zulu8")
                    && file.ends_with(".zip")
                    && line.owns_archive(&file)
                    && entry.path() != archive
                {
                    let _ = fs::remove_file(entry.path());
                }
            }
        }
        let java = zulu::java_binary(&root).ok_or_else(|| {
            InstallError::Invalid(format!(
                "the Java runtime in {} has no java",
                root.display()
            ))
        })?;
        Ok((java, java_platform))
    }

    /// Moves an unpacked, marked build from `staging` to `root`. When another launch
    /// put the same build there first, its copy is kept. A broken copy at `root` (or
    /// one "Verify game files" redoes) is swapped out: moved aside and the new one
    /// moved in, so the folder is never missing for longer than two renames; the files
    /// are the same build either way. The old copy is left beside it, because a game
    /// another instance started may still run from it (and load its libraries later):
    /// the storage cleanup removes it ([`zulu::older_builds`]), only while no game
    /// runs.
    fn move_build_into_place(
        &self,
        staging: &Path,
        root: &Path,
        sha256: &str,
    ) -> Result<(), InstallError> {
        if crate::fsx::rename(staging, root).is_ok() {
            return Ok(());
        }
        if !root.exists() {
            // The rename failed for another reason: try once more, to report it.
            return crate::fsx::rename(staging, root).map_err(io_err(root));
        }
        let aside = root.with_file_name(format!(
            ".{}.old-{}-{:016x}",
            root.file_name().unwrap_or_default().to_string_lossy(),
            std::process::id(),
            u64::from_le_bytes(crate::random_bytes())
        ));
        match crate::fsx::rename(root, &aside) {
            Ok(()) => {}
            // Another launch is moving it at this moment: theirs will do.
            Err(_) if runtime::is_installed(root, sha256) => {
                let _ = fs::remove_dir_all(staging);
                return Ok(());
            }
            Err(err) => return Err(io_err(root)(err)),
        }
        let moved = crate::fsx::rename(staging, root);
        if moved.is_err() && !root.exists() {
            // Put the old copy back rather than leave no build.
            let _ = crate::fsx::rename(&aside, root);
        }
        match moved {
            Ok(()) => Ok(()),
            Err(_) if runtime::is_installed(root, sha256) => {
                let _ = fs::remove_dir_all(staging);
                Ok(())
            }
            Err(err) => Err(io_err(root)(err)),
        }
    }

    /// Points `version` at a copy of its asset index without the window icons.
    ///
    /// Mojang's LWJGL 3.3.1, which 1.13 to 1.18 run with on Apple silicon, natively or
    /// under Rosetta ([`libraries::select_on`]), has a GLFW that reports an error when a game sets a
    /// window icon on macOS ("Cocoa: Regular windows do not have icons on macOS"), and
    /// those versions take any GLFW error while starting as fatal: they show it in a
    /// dialog and quit ("Failed to create window"). Mojang stopped setting the icon on
    /// macOS in 1.19. Without the icons in their index, these versions skip setting it
    /// (they log "Couldn't set icon" and go on), which changes nothing on screen: macOS
    /// windows have no icons.
    fn hide_window_icons(&self, version: &mut VersionJson) -> Result<(), InstallError> {
        let Some(index_ref) = version.asset_index.as_mut() else {
            return Ok(());
        };
        let indexes = self.data.assets_dir().join("indexes");
        let source = indexes.join(
            libraries::safe_relative_path(&format!("{}.json", index_ref.id))
                .map_err(|err| InstallError::Invalid(err.to_string()))?,
        );
        let id = format!("{}{WITHOUT_WINDOW_ICONS}", index_ref.id);
        let dest = indexes.join(
            libraries::safe_relative_path(&format!("{id}.json"))
                .map_err(|err| InstallError::Invalid(err.to_string()))?,
        );
        let bytes = fs::read(&source).map_err(io_err(&source))?;
        let filtered = without_window_icons(&bytes)
            .map_err(|err| InstallError::Invalid(format!("the asset index is invalid: {err}")))?;
        if fs::read(&dest).ok().as_deref() != Some(filtered.as_slice()) {
            storage::write_atomic(&dest, &filtered).map_err(io_err(&dest))?;
        }
        index_ref.id = id;
        Ok(())
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

/// What the copy of an asset index without the window icons adds to its id:
/// `1.16-no-window-icons` ([`Installer::hide_window_icons`]).
///
/// The JVM argument that turns off Forge's early loading window
/// ([`forge_early_window_sets_icon`]).
pub const NO_FORGE_EARLY_WINDOW: &str = "-Dfml.earlyprogresswindow=false";

/// Whether `version` is Forge for 1.14 to 1.16.5 whose early loading window (FML's
/// `ClientVisualization`, opened before Minecraft's own) sets a window icon on macOS.
/// With Mojang's LWJGL 3.3, which those versions get on Apple silicon
/// ([`libraries::select_on`]), GLFW reports that as an error, and Minecraft then stops
/// at startup ("GLFW error before init: Cocoa: Regular windows do not have icons on
/// macOS"). Forge 1.16.5-36.2.30 and later skip the icon on macOS; 1.14 and 1.15 never
/// got that fix, and 1.13 and 1.17 and later have no such window.
pub fn forge_early_window_sets_icon(version: &VersionJson) -> bool {
    let Some(forge) = version.libraries.iter().find_map(|library| {
        let mut parts = library.name.split(':');
        (parts.next() == Some("net.minecraftforge") && parts.next() == Some("forge"))
            .then(|| parts.next())
            .flatten()
    }) else {
        return false;
    };
    let Some((game, build)) = forge.split_once('-') else {
        return false;
    };
    let numbers = |text: &str| -> Vec<u64> {
        text.split('.')
            .map_while(|part| part.parse().ok())
            .collect()
    };
    let game = numbers(game);
    if game.len() < 2 || game[0] != 1 || !(14..=16).contains(&game[1]) {
        return false;
    }
    !(game == [1, 16, 5] && numbers(build).as_slice() >= [36, 2, 30].as_slice())
}

/// Turns off Forge's early loading window ([`forge_early_window_sets_icon`]): Forge
/// then shows its loading progress in Minecraft's own window, as it does where the
/// early window can't open.
fn without_forge_early_window(version: &mut VersionJson) {
    use super::version::Argument;
    // 1.14 and later always have `arguments`.
    let Some(arguments) = version.arguments.as_mut() else {
        return;
    };
    let present = arguments
        .jvm
        .iter()
        .any(|argument| matches!(argument, Argument::Plain(text) if text == NO_FORGE_EARLY_WINDOW));
    if !present {
        arguments
            .jvm
            .push(Argument::Plain(NO_FORGE_EARLY_WINDOW.to_string()));
    }
}
pub const WITHOUT_WINDOW_ICONS: &str = "-no-window-icons";

/// Whether an asset index entry is one of the window icons 1.13 to 1.18 set at
/// startup, listed with and without the `minecraft/` namespace.
fn is_window_icon(name: &str) -> bool {
    let name = name.strip_prefix("minecraft/").unwrap_or(name);
    matches!(name, "icons/icon_16x16.png" | "icons/icon_32x32.png")
}

/// An asset index without the window icons, everything else as it was.
fn without_window_icons(index: &[u8]) -> serde_json::Result<Vec<u8>> {
    let mut index: serde_json::Value = serde_json::from_slice(index)?;
    if let Some(objects) = index
        .get_mut("objects")
        .and_then(serde_json::Value::as_object_mut)
    {
        objects.retain(|name, _| !is_window_icon(name));
    }
    serde_json::to_vec(&index)
}

/// Makes `to` the same file as `from`: a hard link, or a copy where links don't
/// work. An existing `to` with the same size is kept; the download engine checks it.
fn link_or_copy(from: &Path, to: &Path) -> io::Result<()> {
    if let (Ok(a), Ok(b)) = (fs::metadata(from), fs::metadata(to))
        && a.len() == b.len()
    {
        return Ok(());
    }
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)?;
    }
    let tmp = to.with_extension("jar.linking");
    let _ = fs::remove_file(&tmp);
    if fs::hard_link(from, &tmp).is_err() {
        fs::copy(from, &tmp)?;
    }
    crate::fsx::rename(&tmp, to)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Mojang's 1.16 index lists the window icons with and without the `minecraft/`
    /// namespace; both go, and nothing else does.
    #[test]
    fn the_window_icons_are_left_out_of_the_index() {
        let index = br#"{"objects": {
            "icons/icon_16x16.png": {"hash": "bdf48ef6b5d0d23bbb02e17d04865216179f510a", "size": 3665},
            "icons/icon_32x32.png": {"hash": "92750c5f93c312ba9ab413d546f32190c56d6f1f", "size": 5362},
            "minecraft/icons/icon_16x16.png": {"hash": "bdf48ef6b5d0d23bbb02e17d04865216179f510a", "size": 3665},
            "minecraft/icons/icon_32x32.png": {"hash": "92750c5f93c312ba9ab413d546f32190c56d6f1f", "size": 5362},
            "icons/minecraft.icns": {"hash": "991b421dfd401f115241601b2b373140a8d78572", "size": 114786},
            "minecraft/sounds/ambient/cave/cave1.ogg": {"hash": "aaa", "size": 1}
        }}"#;
        let filtered: serde_json::Value =
            serde_json::from_slice(&without_window_icons(index).unwrap()).unwrap();
        let mut names: Vec<&String> = filtered["objects"].as_object().unwrap().keys().collect();
        names.sort();
        assert_eq!(
            names,
            [
                "icons/minecraft.icns",
                "minecraft/sounds/ambient/cave/cave1.ogg"
            ]
        );
        assert!(without_window_icons(b"not json").is_err());
    }

    /// Forge's early window sets an icon on macOS in 1.14 and 1.15, and in 1.16.x
    /// before 1.16.5-36.2.30 (checked in Forge's jars); those get it turned off, once.
    #[test]
    fn old_forge_early_windows_are_turned_off() {
        let forge = |coordinates: &str, arguments: bool| {
            let mut json = serde_json::json!({
                "id": "forge",
                "mainClass": "cpw.mods.modlauncher.Launcher",
                "libraries": [
                    {"name": "cpw.mods:modlauncher:8.0.9"},
                    {"name": coordinates},
                ],
            });
            if arguments {
                json["arguments"] = serde_json::json!({"jvm": ["-Xss1M"], "game": []});
            }
            VersionJson::parse(json.to_string().as_bytes()).unwrap()
        };
        for old in [
            "net.minecraftforge:forge:1.14.4-28.2.26",
            "net.minecraftforge:forge:1.15.2-31.2.57",
            "net.minecraftforge:forge:1.16.4-35.1.37",
            "net.minecraftforge:forge:1.16.5-36.2.20",
            "net.minecraftforge:forge:1.16.5-36.2.29",
        ] {
            assert!(forge_early_window_sets_icon(&forge(old, true)), "{old}");
        }
        for fixed in [
            "net.minecraftforge:forge:1.16.5-36.2.30",
            "net.minecraftforge:forge:1.16.5-36.2.42",
            "net.minecraftforge:forge:1.13.2-25.0.223",
            "net.minecraftforge:forge:1.17.1-37.1.1",
            "net.minecraftforge:forge:1.20.1-47.4.0",
            "net.minecraftforge:fmlloader:1.16.5-36.2.20",
        ] {
            assert!(
                !forge_early_window_sets_icon(&forge(fixed, true)),
                "{fixed}"
            );
        }

        let mut version = forge("net.minecraftforge:forge:1.16.5-36.2.20", true);
        without_forge_early_window(&mut version);
        without_forge_early_window(&mut version);
        let jvm: Vec<String> = version
            .arguments
            .unwrap()
            .jvm
            .into_iter()
            .filter_map(|argument| match argument {
                super::super::version::Argument::Plain(text) => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(jvm, ["-Xss1M", NO_FORGE_EARLY_WINDOW]);
        // Old-style arguments are left alone.
        let mut legacy = forge("net.minecraftforge:forge:1.16.5-36.2.20", false);
        without_forge_early_window(&mut legacy);
        assert!(legacy.arguments.is_none());
    }
}
