//! The mod loader part of an install: the version lists, the loader's profile, the
//! files from Forge's and NeoForge's installers, and their processors.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use futures::StreamExt as _;
use serde_json::Value;

use super::{InstallError, InstallProgress, Installer, Stage, io_err};
use crate::{
    game::{
        download::{CancelToken, DownloadItem, Verify, file_matches, sha1_hex, sha1_of},
        libraries::{maven_path, safe_relative_path},
        loaders::{
            self, LoaderChoice, LoaderError, LoaderKind, LoaderVersion,
            forge::{self, InstallProfile, InstallerJar, InstallerProfile},
            meta,
            processors::{self, Builtins, Values},
        },
        record::{self as install_record, InstallRecord},
        version::VersionJson,
    },
    http, storage,
};

impl From<LoaderError> for InstallError {
    fn from(err: LoaderError) -> Self {
        match err {
            LoaderError::Cancelled => InstallError::Cancelled,
            other => InstallError::Invalid(other.to_string()),
        }
    }
}

/// A loader's profile, ready to merge onto its vanilla version.
pub(crate) struct LoaderProfile {
    pub version: VersionJson,
    /// Forge's and NeoForge's installer jar.
    pub installer: Option<PathBuf>,
}

/// The file in `versions/<id>/` named `name`.
fn profile_file(installer: &Installer, id: &str, name: &str) -> PathBuf {
    installer.data.versions_dir().join(id).join(name)
}

impl Installer {
    /// Fetches a file without caching it.
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, InstallError> {
        let url = self.urls.resolve(url)?;
        let request =
            http::get(&url, None).map_err(|err| InstallError::Network(err.to_string()))?;
        let response = http::send(&*self.http, request)
            .await
            .map_err(|err| InstallError::Network(err.to_string()))?;
        if !response.is_success() {
            return Err(InstallError::Network(format!(
                "{url}: HTTP {}",
                response.status
            )));
        }
        Ok(response.body)
    }

    /// The loader's versions for `game_version`, newest first. The lists are cached
    /// for use offline.
    pub async fn loader_versions(
        &self,
        kind: LoaderKind,
        game_version: &str,
    ) -> Result<Vec<LoaderVersion>, InstallError> {
        let safe = safe_relative_path(game_version)
            .ok()
            .filter(|path| path.components().count() == 1)
            .ok_or_else(|| InstallError::Invalid(format!("{game_version:?} is not a version")))?;
        let cache = |name: &str| format!("loaders/{name}");
        let versions = match kind {
            LoaderKind::Fabric | LoaderKind::Quilt => {
                let name = format!("{}-{}.json", kind.label().to_lowercase(), safe.display());
                let url = self.urls.resolve(&meta::versions_url(kind, game_version))?;
                // Fabric's meta answers 400 and Quilt's 404 for a Minecraft version
                // they don't support (Fabric starts at 1.14): no versions, not a failure.
                let unsupported: &[u16] = match kind {
                    LoaderKind::Fabric => &[400],
                    _ => &[404],
                };
                let Some(body) = self
                    .fetch_cached_or_missing(&url, &cache(&name), unsupported)
                    .await?
                else {
                    return Ok(Vec::new());
                };
                match kind {
                    LoaderKind::Fabric => meta::parse_fabric_versions(&body, game_version)?,
                    _ => meta::parse_quilt_versions(&body)?,
                }
            }
            LoaderKind::Forge => {
                let metadata = self
                    .fetch_cached(
                        &self.urls.resolve(forge::FORGE_METADATA_URL)?,
                        &cache("forge-maven-metadata.json"),
                    )
                    .await?;
                // The promotions only mark the recommended build.
                let promotions = self
                    .fetch_cached(
                        &self.urls.resolve(forge::FORGE_PROMOTIONS_URL)?,
                        &cache("forge-promotions.json"),
                    )
                    .await
                    .unwrap_or_default();
                forge::parse_forge_versions(&metadata, &promotions, game_version)?
            }
            LoaderKind::NeoForge => {
                let body = self
                    .fetch_cached(
                        &self.urls.resolve(forge::NEOFORGE_VERSIONS_URL)?,
                        &cache("neoforge-versions.json"),
                    )
                    .await?;
                forge::parse_neoforge_versions(&body, game_version)?
            }
        };
        Ok(versions)
    }

    /// The SHA-1s of the files at `urls`, from the `.sha1` file Maven keeps next to
    /// each. Some old jars have no `.sha1` file (Forge 1.8.9's Scala libraries); those
    /// are downloaded and hashed instead, and [`forge::normalize_legacy`] still
    /// requires the result to be one of the profile's `checksums` when it lists any.
    /// The jar is read through the downloader, with its retries and no size limit:
    /// a `.sha1` request that failed once makes a jar of many megabytes (Scala's
    /// compiler) take this path too.
    async fn maven_sha1s(
        &self,
        urls: Vec<String>,
        cancel: &CancelToken,
    ) -> Result<HashMap<String, String>, InstallError> {
        let results: Vec<Result<(String, String), InstallError>> =
            futures::stream::iter(urls.into_iter().map(|url| async move {
                if cancel.is_cancelled() {
                    return Err(InstallError::Cancelled);
                }
                let listed = match self.fetch(&format!("{url}.sha1")).await {
                    Ok(body) => loaders::parse_sha1_file(&String::from_utf8_lossy(&body)),
                    Err(InstallError::Network(_)) => None,
                    Err(err) => return Err(err),
                };
                let sha1 = match listed {
                    Some(sha1) => sha1,
                    None => {
                        self.downloader
                            .sha1_of_url(&self.urls.resolve(&url)?, cancel)
                            .await?
                    }
                };
                Ok((url, sha1))
            }))
            .buffer_unordered(8)
            .collect()
            .await;
        results.into_iter().collect()
    }

    /// The profile of `choice` for `vanilla`: the installed one, or fetched,
    /// normalised and saved at `versions/<id>/<id>.json`.
    pub(crate) async fn loader_profile(
        &self,
        choice: &LoaderChoice,
        vanilla: &VersionJson,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<LoaderProfile, InstallError> {
        let id = loaders::profile_id(choice, &vanilla.id)?;
        let json_path = profile_file(self, &id, &format!("{id}.json"));
        let installer = choice.kind.has_installer().then(|| {
            self.data.libraries_dir().join(
                maven_path(
                    &forge::installer_coordinate(choice.kind, &choice.version),
                    None,
                )
                .unwrap_or_default(),
            )
        });
        if let Some(version) = fs::read(&json_path)
            .ok()
            .and_then(|bytes| VersionJson::parse(&bytes).ok())
        {
            return Ok(LoaderProfile { version, installer });
        }

        progress.set_stage(Stage::Loader);
        progress.note(format!(
            "Installing {} {}",
            choice.kind.label(),
            choice.version
        ));
        let normalized = match choice.kind {
            LoaderKind::Fabric | LoaderKind::Quilt => {
                let body = self
                    .fetch(&meta::profile_url(
                        choice.kind,
                        &vanilla.id,
                        &choice.version,
                    ))
                    .await?;
                let profile: Value = serde_json::from_slice(&body).map_err(|err| {
                    InstallError::Invalid(format!("the loader profile is invalid: {err}"))
                })?;
                let hashes = self
                    .maven_sha1s(meta::missing_hashes(&profile)?, cancel)
                    .await?;
                meta::normalize(&profile, &id, &hashes)?
            }
            LoaderKind::Forge | LoaderKind::NeoForge => {
                let installer = installer.clone().expect("Forge has an installer");
                self.install_forge(choice, vanilla, &id, &installer, progress, cancel)
                    .await?
            }
        };
        let bytes = serde_json::to_vec_pretty(&normalized)
            .map_err(|err| InstallError::Invalid(err.to_string()))?;
        let version = VersionJson::parse(&bytes).map_err(|err| {
            InstallError::Invalid(format!("the loader profile is invalid: {err}"))
        })?;
        if version.inherits_from.as_deref() != Some(vanilla.id.as_str()) {
            return Err(InstallError::Invalid(format!(
                "{} {} is not for Minecraft {}",
                choice.kind.label(),
                choice.version,
                vanilla.id
            )));
        }
        storage::write_atomic(&json_path, &bytes).map_err(io_err(&json_path))?;
        Ok(LoaderProfile { version, installer })
    }

    /// Downloads the installer and reads its profile. Modern installers' own
    /// `install_profile.json` is kept for the processors.
    async fn install_forge(
        &self,
        choice: &LoaderChoice,
        vanilla: &VersionJson,
        id: &str,
        installer: &Path,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<Value, InstallError> {
        self.fetch_installer(choice, installer, progress, cancel)
            .await?;
        let mut jar = InstallerJar::open(installer)?;
        let wrong_game = |wanted: &str| {
            InstallError::Invalid(format!(
                "{} {} is for Minecraft {wanted}, not {}",
                choice.kind.label(),
                choice.version,
                vanilla.id
            ))
        };
        match jar.profile()? {
            InstallerProfile::Modern {
                install_profile,
                install_profile_bytes,
                version,
            } => {
                if let Some(wanted) = &install_profile.minecraft
                    && *wanted != vanilla.id
                {
                    return Err(wrong_game(wanted));
                }
                let path = profile_file(self, id, "install_profile.json");
                storage::write_atomic(&path, &install_profile_bytes).map_err(io_err(&path))?;
                let mut normalized = forge::normalize_modern(&version, id)?;
                normalized["inheritsFrom"] = vanilla.id.clone().into();
                Ok(normalized)
            }
            InstallerProfile::Legacy { install, version } => {
                if let Some(wanted) = &install.minecraft
                    && *wanted != vanilla.id
                {
                    return Err(wrong_game(wanted));
                }
                let universal = jar
                    .read(&install.file_path, 256 * 1024 * 1024)?
                    .ok_or_else(|| {
                        InstallError::Invalid(format!("the installer has no {}", install.file_path))
                    })?;
                let hashes = self
                    .maven_sha1s(forge::legacy_downloads(&version, &install)?, cancel)
                    .await?;
                let mut normalized =
                    forge::normalize_legacy(&version, &install, id, &hashes, &universal)?;
                normalized["inheritsFrom"] = vanilla.id.clone().into();
                Ok(normalized)
            }
        }
    }

    /// Downloads Forge's or NeoForge's installer jar to `installer`, checked against
    /// its Maven `.sha1` file.
    pub(crate) async fn fetch_installer(
        &self,
        choice: &LoaderChoice,
        installer: &Path,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<(), InstallError> {
        let url = forge::installer_url(choice.kind, &choice.version)?;
        let sha1 = self
            .maven_sha1s(vec![url.clone()], cancel)
            .await?
            .remove(&url)
            .expect("asked for this URL");
        self.downloader
            .fetch(
                &DownloadItem {
                    url: self.urls.resolve(&url)?,
                    dest: installer.to_owned(),
                    sha1,
                    size: None,
                    executable: false,
                },
                &progress.downloads,
                cancel,
            )
            .await?;
        Ok(())
    }

    /// The libraries the processors need (`install_profile.json`'s), as downloads.
    pub(crate) fn processor_downloads(
        &self,
        profile_id: &str,
    ) -> Result<Vec<DownloadItem>, InstallError> {
        let Some(profile) = self.install_profile(profile_id)? else {
            return Ok(Vec::new());
        };
        let mut items = Vec::new();
        let libraries = self.data.libraries_dir();
        for library in &profile.0.libraries {
            let Some(artifact) = &library.downloads.artifact else {
                continue;
            };
            if artifact.url.is_empty() {
                continue;
            }
            let path = match &artifact.path {
                Some(path) => path.clone(),
                None => maven_path(&library.name, None)
                    .map_err(|err| InstallError::Invalid(err.to_string()))?,
            };
            let path =
                safe_relative_path(&path).map_err(|err| InstallError::Invalid(err.to_string()))?;
            items.push(DownloadItem {
                url: self.urls.resolve(&artifact.url)?,
                dest: libraries.join(path),
                sha1: artifact.sha1.clone(),
                size: artifact.size,
                executable: false,
            });
        }
        Ok(items)
    }

    /// A modern installer's `install_profile.json`, with its SHA-1.
    fn install_profile(
        &self,
        profile_id: &str,
    ) -> Result<Option<(InstallProfile, String)>, InstallError> {
        let path = profile_file(self, profile_id, "install_profile.json");
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(InstallError::Io { path, source }),
        };
        Ok(Some((InstallProfile::parse(&bytes)?, sha1_hex(&bytes))))
    }

    /// Puts the files the profile has without a download back from the installer
    /// (its `maven/` folder, or an old installer's universal jar) when they are
    /// missing, or, with [`Verify::Full`], have the wrong SHA-1. Files the
    /// processors make are left to them.
    ///
    /// Those of the installer's `install_profile.json` (`profile_id`'s) too: Forge 1.13
    /// to 1.16 list their universal jar only there, and FML won't start without it
    /// ("Missing forge!").
    pub(crate) fn restore_installer_files(
        &self,
        child: &VersionJson,
        profile_id: &str,
        installer: &Path,
        verify: Verify,
    ) -> Result<(), InstallError> {
        let libraries = self.data.libraries_dir();
        let profile = self.install_profile(profile_id)?;
        let profile_libraries = profile
            .iter()
            .flat_map(|(profile, _)| profile.libraries.iter());
        let mut wanted: Vec<(PathBuf, Option<String>)> = Vec::new();
        for library in child.libraries.iter().chain(profile_libraries) {
            let Some(artifact) = &library.downloads.artifact else {
                continue;
            };
            if !artifact.url.is_empty() {
                continue;
            }
            let path = match &artifact.path {
                Some(path) => path.clone(),
                None => maven_path(&library.name, None)
                    .map_err(|err| InstallError::Invalid(err.to_string()))?,
            };
            let path =
                safe_relative_path(&path).map_err(|err| InstallError::Invalid(err.to_string()))?;
            let dest = libraries.join(&path);
            let sha1 = (!artifact.sha1.is_empty()).then(|| artifact.sha1.clone());
            let good = match (&sha1, verify) {
                (Some(sha1), Verify::Full) => file_matches(&dest, sha1),
                _ => dest.is_file(),
            };
            if !good && !wanted.iter().any(|(listed, _)| *listed == path) {
                wanted.push((path, sha1));
            }
        }
        if wanted.is_empty() {
            return Ok(());
        }
        let mut jar = InstallerJar::open(installer)?;
        let mut sources: HashMap<PathBuf, String> = jar
            .maven_files()?
            .into_iter()
            .map(|(entry, path)| (path, entry))
            .collect();
        if let InstallerProfile::Legacy { install, .. } = jar.profile()? {
            let path = maven_path(&install.path, None)
                .map_err(|err| InstallError::Invalid(err.to_string()))?;
            sources.insert(PathBuf::from(path), install.file_path.clone());
        }
        for (path, sha1) in wanted {
            let Some(entry) = sources.get(&path) else {
                // A processor makes it.
                continue;
            };
            let dest = libraries.join(&path);
            jar.extract(entry, &dest)?;
            if let Some(sha1) = sha1
                && !file_matches(&dest, &sha1)
            {
                return Err(InstallError::Invalid(format!(
                    "{} in the installer has the wrong SHA-1",
                    path.display()
                )));
            }
        }
        Ok(())
    }

    /// Runs the installer's client processors with `java`, unless the processors'
    /// record shows their files are all still there (with [`Verify::Full`]: still
    /// have the SHA-1s they were made with).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn run_processors(
        &self,
        profile_id: &str,
        installer: &Path,
        vanilla: &VersionJson,
        client_jar: &Path,
        java: &Path,
        verify: Verify,
        progress: &InstallProgress,
        cancel: &CancelToken,
    ) -> Result<(), InstallError> {
        let Some((profile, profile_sha1)) = self.install_profile(profile_id)? else {
            return Ok(());
        };
        if profile.client_processors().next().is_none() {
            return Ok(());
        }
        let client_sha1 = vanilla
            .client_download()
            .map(|client| client.sha1.as_str())
            .unwrap_or_default();
        let key = format!("{profile_sha1}:{client_sha1}");
        let record_path = profile_file(self, profile_id, &format!("{profile_id}.processors.json"));
        let root = self.data.root();
        let previous = InstallRecord::load(&record_path, &key);
        let intact = !previous.files.is_empty()
            && previous.files.iter().all(|(relative, file)| {
                let path = root.join(relative);
                match verify {
                    Verify::Recorded => {
                        previous.trusts(relative, &file.sha1, install_record::stat(&path))
                    }
                    Verify::Full => file_matches(&path, &file.sha1),
                }
            });
        if intact {
            progress.stats.trusted.fetch_add(
                previous.files.len() as u64,
                std::sync::atomic::Ordering::Relaxed,
            );
            return Ok(());
        }

        progress.set_stage(Stage::Processors);
        let libraries = self.data.libraries_dir();
        let work = profile_file(self, profile_id, "installer-data");
        let mut jar = InstallerJar::open(installer)?;
        let values = Values::new(
            &profile,
            &Builtins {
                minecraft_jar: client_jar,
                minecraft_version: &vanilla.id,
                root,
                installer,
                libraries_dir: &libraries,
            },
            |name| {
                let relative = safe_relative_path(name.trim_start_matches('/')).map_err(|_| {
                    LoaderError::Invalid(format!("the installer has an unsafe path {name:?}"))
                })?;
                let dest = work.join(relative);
                jar.extract(name, &dest)?;
                Ok(dest)
            },
        )?;
        let planned = processors::plan(&profile, &values, &libraries)?;
        let total = planned.len();
        for (index, processor) in planned.iter().enumerate() {
            if cancel.is_cancelled() {
                return Err(InstallError::Cancelled);
            }
            if processor.is_done() {
                continue;
            }
            progress.note(format!(
                "Running the installer's processor {}/{total}: {}",
                index + 1,
                processor
                    .jar
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default()
            ));
            processors::run(java, processor, root, cancel, &|line| progress.note(line))?;
        }

        let mut record = InstallRecord::new(key);
        for path in values.artifacts() {
            let (Some(stat), Some(relative)) = (
                install_record::stat(path),
                install_record::relative_key(root, path),
            ) else {
                continue;
            };
            let sha1 = sha1_of(path).map_err(io_err(path))?;
            record.insert(relative, &sha1, stat);
        }
        if let Err(err) = record.save(&record_path) {
            eprintln!(
                "lodestone-launcher: saving {}: {err}",
                record_path.display()
            );
        }
        let _ = fs::remove_dir_all(&work);
        Ok(())
    }
}
