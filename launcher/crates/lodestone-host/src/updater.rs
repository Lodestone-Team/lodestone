//! Lodestone Host's own updates: what Settings › Updates shows and does.
//!
//! The model is the launcher's (`lodestone-launcher/src/updater.rs`): a check a while
//! after the start and every few hours, "Update automatically" downloads a newer
//! build, and "Beta builds" picks the channel. Builds come from an [`UpdateSource`]:
//! [`ReleaseSource`] reads the Host's GitHub releases (`host-vX.Y.Z`) through
//! `launcher_core::update`, with the launcher's manifest, checks and file swap.
//!
//! The Host is two programs: itself and the core it runs. A download stages both
//! next to the running ones, checked against the manifest's size and SHA-256. They
//! are only swapped in once the core has stopped: when the Host quits
//! ([`Updater::apply`], from `HostApp::quit`), or restarts to update, which stops the
//! core (and its servers) the same way. Restarting asks first when servers run
//! (`pages/updates.rs`).
//!
//! Only builds made by `launcher/scripts/publish_release.py --product host` update:
//! it compiles in `LODESTONE_HOST_BUILD` and `LODESTONE_HOST_COMMIT`. A plain
//! `cargo build` has no build number and never updates.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::future::BoxFuture;
use gpui::{App, AppContext as _, Context, Entity, Global, Task};
use http_client::HttpClient;
use launcher_core::update::{self, Build, Product, UpdateChannel};

use crate::prefs::Prefs;

/// How long after the start the first check waits.
const FIRST_CHECK: Duration = Duration::from_secs(30);
/// How often the Host looks for a newer build while it runs.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// How often a download's progress is redrawn.
const PROGRESS_TICK: Duration = Duration::from_millis(250);

/// This build, as the publish script compiled it in; `None` for `cargo build`.
pub fn this_build() -> Option<Build> {
    Build::parse(
        option_env!("LODESTONE_HOST_BUILD"),
        option_env!("LODESTONE_HOST_COMMIT"),
    )
    .map(|build| build.with_channel(option_env!("LODESTONE_HOST_CHANNEL")))
}

/// Where new builds come from.
pub trait UpdateSource: Send + Sync + 'static {
    /// The newest build `channel` reads (stable releases; betas too; developer builds
    /// too), newer than `running`; `None` when there is none.
    fn check(
        &self,
        channel: UpdateChannel,
        running: u64,
    ) -> BoxFuture<'static, Result<Option<Found>, String>>;

    /// Downloads `found` and keeps it ready to install, telling `progress` how many
    /// bytes are done.
    fn download(
        &self,
        found: Found,
        progress: Arc<dyn Fn(u64) + Send + Sync>,
    ) -> BoxFuture<'static, Result<(), String>>;

    /// Swaps the downloaded build in. Called once the core has stopped.
    fn install(&self) -> Result<(), String>;

    /// Starts the Host again (the installed build, if [`Self::install`] worked). The
    /// Host quits right after.
    fn restart(&self) -> Result<(), String>;
}

/// A newer build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub build: u64,
    /// The download's size in bytes: the Host and its core.
    pub size: u64,
    /// The release and manifest it is in; `None` from a test source.
    pub release: Option<update::Found>,
}

/// Where updating stands (the launcher's `UpdateState`).
#[derive(Clone, Debug)]
pub enum UpdateState {
    /// A build the publish script didn't make, which doesn't update itself.
    Unavailable,
    Idle,
    Checking,
    UpToDate,
    /// A newer build, waiting because "Update automatically" is off.
    Available(Found),
    Downloading {
        build: u64,
        done: Arc<AtomicU64>,
        total: u64,
    },
    /// Downloaded: installed when the Host restarts or quits.
    Ready {
        build: u64,
    },
    Failed(String),
}

impl UpdateState {
    /// The share of a download done, 0 to 100.
    pub fn percent(done: &AtomicU64, total: u64) -> u64 {
        (done.load(Ordering::Relaxed).saturating_mul(100) / total.max(1)).min(100)
    }
}

pub struct Updater {
    state: UpdateState,
    source: Option<Arc<dyn UpdateSource>>,
    /// This build's number, `None` for a development build.
    pub build: Option<u64>,
    /// This build's commit, as the publish script compiled it in.
    pub commit: Option<&'static str>,
    /// The Host quits to restart into the downloaded build.
    restart_requested: bool,
    work: Option<Task<()>>,
    _poll: Option<Task<()>>,
}

/// The app's updater, for the Settings page.
pub struct GlobalUpdater(pub Entity<Updater>);

impl Global for GlobalUpdater {}

impl Updater {
    /// Makes the app's updater. A build the publish script made reads the Host's
    /// releases; others don't update. Call once at startup.
    pub fn init(cx: &mut App) {
        let build = this_build();
        let source = build.and_then(|_| ReleaseSource::new().ok());
        let updater = cx.new(|cx| {
            let mut updater = Updater::new(None, None);
            if let (Some(build), Some(source)) = (build, source) {
                updater.commit = build.commit;
                updater.set_source(Arc::new(source), build.number, cx);
            }
            updater
        });
        cx.set_global(GlobalUpdater(updater));
    }

    /// The app's updater.
    pub fn global(cx: &App) -> Option<Entity<Updater>> {
        cx.try_global::<GlobalUpdater>().map(|g| g.0.clone())
    }

    pub fn new(source: Option<Arc<dyn UpdateSource>>, build: Option<u64>) -> Self {
        Self {
            state: if source.is_some() && build.is_some() {
                UpdateState::Idle
            } else {
                UpdateState::Unavailable
            },
            source,
            build,
            commit: None,
            restart_requested: false,
            work: None,
            _poll: None,
        }
    }

    /// Plugs in where builds come from, for this build, and starts checking.
    pub fn set_source(
        &mut self,
        source: Arc<dyn UpdateSource>,
        build: u64,
        cx: &mut Context<Self>,
    ) {
        self.source = Some(source);
        self.build = Some(build);
        self.state = UpdateState::Idle;
        self._poll = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(FIRST_CHECK).await;
            loop {
                if this.update(cx, |this, cx| this.check(cx)).is_err() {
                    return;
                }
                cx.background_executor().timer(CHECK_EVERY).await;
            }
        }));
        cx.notify();
    }

    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    /// Looks for a newer build now. Does nothing while a check or download runs.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        let Some(source) = self.source.clone() else {
            return;
        };
        if matches!(
            self.state,
            UpdateState::Unavailable | UpdateState::Checking | UpdateState::Downloading { .. }
        ) {
            return;
        }
        let downloaded = match self.state {
            UpdateState::Ready { build } => Some(build),
            _ => None,
        };
        self.state = UpdateState::Checking;
        cx.notify();
        let channel = Prefs::get(cx).update_channel();
        let running = downloaded.or(self.build).unwrap_or(0);
        let work = cx.background_spawn(source.check(channel, running));
        self.work = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.work = None;
                this.state = match result {
                    Err(err) => match downloaded {
                        // a failed check doesn't lose the download
                        Some(build) => UpdateState::Ready { build },
                        None => UpdateState::Failed(err),
                    },
                    Ok(None) => match downloaded {
                        Some(build) => UpdateState::Ready { build },
                        None => UpdateState::UpToDate,
                    },
                    Ok(Some(found)) if Prefs::get(cx).auto_update => {
                        this.download(found, cx);
                        return;
                    }
                    Ok(Some(found)) => UpdateState::Available(found),
                };
                cx.notify();
            })
            .ok();
        }));
    }

    /// Downloads the available build.
    pub fn download_available(&mut self, cx: &mut Context<Self>) {
        if let UpdateState::Available(found) = &self.state {
            let found = found.clone();
            self.download(found, cx);
        }
    }

    /// "Update automatically" or "Beta builds" changed.
    pub fn settings_changed(&mut self, cx: &mut Context<Self>) {
        match &self.state {
            UpdateState::Available(found) if Prefs::get(cx).auto_update => {
                let found = found.clone();
                self.download(found, cx);
            }
            UpdateState::Unavailable | UpdateState::Downloading { .. } => {}
            _ => {
                self.work = None;
                if matches!(self.state, UpdateState::Checking) {
                    self.state = UpdateState::Idle;
                }
                self.check(cx);
            }
        }
    }

    fn download(&mut self, found: Found, cx: &mut Context<Self>) {
        let Some(source) = self.source.clone() else {
            return;
        };
        let done = Arc::new(AtomicU64::new(0));
        let build = found.build;
        self.state = UpdateState::Downloading {
            build,
            done: done.clone(),
            total: found.size,
        };
        cx.notify();
        let progress: Arc<dyn Fn(u64) + Send + Sync> =
            Arc::new(move |bytes| done.store(bytes, Ordering::Relaxed));
        let work = cx.background_spawn(source.download(found, progress));
        self.work = Some(cx.spawn(async move |this, cx| {
            let mut work = std::pin::pin!(work);
            loop {
                let tick = std::pin::pin!(cx.background_executor().timer(PROGRESS_TICK));
                match futures::future::select(work.as_mut(), tick).await {
                    futures::future::Either::Left((result, _)) => {
                        this.update(cx, |this, cx| {
                            this.work = None;
                            this.state = match result {
                                Ok(()) => UpdateState::Ready { build },
                                Err(err) => UpdateState::Failed(err),
                            };
                            cx.notify();
                        })
                        .ok();
                        return;
                    }
                    futures::future::Either::Right(_) => {
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            return;
                        }
                    }
                }
            }
        }));
    }

    /// Whether a downloaded build waits to be installed.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, UpdateState::Ready { .. })
    }

    /// Asks for a restart into the downloaded build. Returns whether there is one; the
    /// caller then quits the Host as Quit does (`crate::Quit`): the core stops, its
    /// servers first, and [`Self::apply`] installs the build and starts it.
    pub fn restart(&mut self, cx: &mut Context<Self>) -> bool {
        if !self.is_ready() || self.source.is_none() {
            return false;
        }
        self.restart_requested = true;
        cx.notify();
        true
    }

    /// Installs a downloaded build, and restarts the Host if that was asked for. Call
    /// it while quitting, once the core has stopped. Errors are only logged: the Host
    /// is quitting, and the next start checks again.
    pub fn apply(cx: &mut App) {
        let Some(updater) = Self::global(cx) else {
            return;
        };
        let (source, ready, restart) = {
            let updater = updater.read(cx);
            (
                updater.source.clone(),
                updater.is_ready(),
                updater.restart_requested,
            )
        };
        let Some(source) = source else {
            return;
        };
        if ready && let Err(err) = source.install() {
            eprintln!("Lodestone Host couldn't install the update: {err}");
        }
        // Even when the install failed, the Host comes back (as the old build).
        if restart && let Err(err) = source.restart() {
            eprintln!("Lodestone Host couldn't restart: {err}");
        }
    }
}

/// The Host's GitHub releases: `launcher_core::update` with [`Product::HOST`].
pub struct ReleaseSource {
    /// The Host's executable, as it was when the process started.
    exe: PathBuf,
    /// The core's program, next to it.
    core: PathBuf,
    http: Arc<dyn HttpClient>,
    download_http: Arc<dyn HttpClient>,
    /// The downloaded build, once both files are staged.
    staged: Arc<Mutex<Option<u64>>>,
}

impl ReleaseSource {
    /// For the running Host. Removes what an earlier update left behind.
    pub fn new() -> Result<Self, String> {
        let exe = update::exe()
            .map(Path::to_path_buf)
            .ok_or("couldn't tell where Lodestone Host is")?;
        let core = core_path(&exe);
        update::clean_up(&exe);
        update::clean_up(&core);
        if let Some(build) = this_build() {
            // Installed with the Windows installer: Settings › Apps shows this build.
            let display = update::display_version(env!("CARGO_PKG_VERSION"), build.number);
            update::refresh_uninstall_entry_for(update::HOST_WINDOWS_APP_ID, &exe, &display);
        }
        Ok(Self {
            exe,
            core,
            http: launcher_core::http::default_client().map_err(|err| err.to_string())?,
            download_http: launcher_core::http::download_client().map_err(|err| err.to_string())?,
            staged: Arc::new(Mutex::new(None)),
        })
    }
}

/// Where the core's program goes: next to the Host's executable, where
/// `core_process::find_core` looks.
pub fn core_path(exe: &Path) -> PathBuf {
    exe.with_file_name(crate::core_process::CORE_PROGRAM)
}

impl UpdateSource for ReleaseSource {
    fn check(
        &self,
        channel: UpdateChannel,
        running: u64,
    ) -> BoxFuture<'static, Result<Option<Found>, String>> {
        let http = self.http.clone();
        Box::pin(launcher_core::http::in_http_runtime(async move {
            let found = update::find_update_for(Product::HOST, http.as_ref(), channel)
                .await
                .map_err(|err| err.to_string())?;
            // Never older than what runs: leaving the beta waits for stable.
            let Some(found) = found else {
                return Ok(None);
            };
            let Some((build, asset)) = found.manifest.newer(running) else {
                return Ok(None);
            };
            let Some(core) = &asset.core else {
                return Err(format!(
                    "build {build} has no Lodestone Core for this computer"
                ));
            };
            Ok(Some(Found {
                build,
                size: asset.size + core.size,
                release: Some(found.clone()),
            }))
        }))
    }

    fn download(
        &self,
        found: Found,
        progress: Arc<dyn Fn(u64) + Send + Sync>,
    ) -> BoxFuture<'static, Result<(), String>> {
        let (exe, core) = (self.exe.clone(), self.core.clone());
        let http = self.download_http.clone();
        let staged = self.staged.clone();
        Box::pin(launcher_core::http::in_http_runtime(async move {
            let release = found.release.ok_or("there's no release to download from")?;
            let (build, asset) = release
                .manifest
                .for_platform()
                .ok_or("the release has no build for this computer")?;
            let core_asset = asset
                .core
                .as_ref()
                .ok_or("the release has no Lodestone Core for this computer")?
                .as_asset();
            *staged.lock().unwrap_or_else(|e| e.into_inner()) = None;
            let host_url =
                update::Manifest::asset_url(&release.url, asset).map_err(|err| err.to_string())?;
            let core_url = update::Manifest::asset_url(&release.url, &core_asset)
                .map_err(|err| err.to_string())?;
            let host_size = asset.size;
            update::download(
                http.as_ref(),
                &host_url,
                asset,
                &update::staging_path(&exe),
                |bytes| progress(bytes),
            )
            .await
            .map_err(|err| err.to_string())?;
            update::download(
                http.as_ref(),
                &core_url,
                &core_asset,
                &update::staging_path(&core),
                |bytes| progress(host_size + bytes),
            )
            .await
            .map_err(|err| err.to_string())?;
            *staged.lock().unwrap_or_else(|e| e.into_inner()) = Some(build);
            Ok(())
        }))
    }

    fn install(&self) -> Result<(), String> {
        if self
            .staged
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .is_none()
        {
            return Ok(());
        }
        install_pair(&self.exe, &self.core)
    }

    fn restart(&self) -> Result<(), String> {
        update::restart(&self.exe).map_err(|err| err.to_string())
    }
}

/// Swaps the staged core and Host in for `core` and `exe`: the core first, so a Host
/// that fails half way still runs its old self with a newer core, which it can talk
/// to. If the Host can't be swapped, the old core is put back.
pub fn install_pair(exe: &Path, core: &Path) -> Result<(), String> {
    let (staged_exe, staged_core) = (update::staging_path(exe), update::staging_path(core));
    if !staged_exe.is_file() || !staged_core.is_file() {
        return Err("the download is missing; it starts again at the next check".into());
    }
    // Keep the old core, to put it back if the Host can't be replaced.
    let backup = core.with_file_name(format!(
        "{}.previous",
        core.file_name().unwrap_or_default().to_string_lossy()
    ));
    let had_core = core.is_file();
    if had_core {
        std::fs::copy(core, &backup).map_err(|err| err.to_string())?;
    }
    let result = update::install(&staged_core, core)
        .and_then(|()| update::install(&staged_exe, exe))
        .map_err(|err| err.to_string());
    if result.is_err() && had_core {
        let _ = std::fs::rename(&backup, core);
    } else {
        let _ = std::fs::remove_file(&backup);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_a_source_nothing_updates() {
        let updater = Updater::new(None, Some(12));
        assert!(matches!(updater.state(), UpdateState::Unavailable));
    }

    #[test]
    fn percent_of_a_download() {
        let done = AtomicU64::new(50);
        assert_eq!(UpdateState::percent(&done, 200), 25);
        assert_eq!(UpdateState::percent(&done, 0), 100);
    }

    #[test]
    fn installs_the_host_and_its_core_together() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("lodestone-host");
        let core = core_path(&exe);
        std::fs::write(&exe, b"old host").unwrap();
        std::fs::write(&core, b"old core").unwrap();
        // nothing staged: nothing changes
        assert!(install_pair(&exe, &core).is_err());
        assert_eq!(std::fs::read(&exe).unwrap(), b"old host");

        std::fs::write(update::staging_path(&exe), b"new host").unwrap();
        std::fs::write(update::staging_path(&core), b"new core").unwrap();
        install_pair(&exe, &core).unwrap();
        assert_eq!(std::fs::read(&exe).unwrap(), b"new host");
        assert_eq!(std::fs::read(&core).unwrap(), b"new core");
        for leftover in [
            update::staging_path(&exe),
            update::staging_path(&core),
            core.with_file_name(format!("{}.previous", crate::core_process::CORE_PROGRAM)),
        ] {
            assert!(!leftover.exists(), "{}", leftover.display());
        }
    }

    #[test]
    #[cfg(unix)]
    fn a_host_that_cant_be_replaced_keeps_its_core() {
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("lodestone-host");
        let core = core_path(&exe);
        std::fs::write(&core, b"old core").unwrap();
        std::fs::write(update::staging_path(&core), b"new core").unwrap();
        // a file can't replace a folder that isn't empty
        std::fs::create_dir(&exe).unwrap();
        std::fs::write(exe.join("busy"), b"").unwrap();
        std::fs::write(update::staging_path(&exe), b"new host").unwrap();
        assert!(install_pair(&exe, &core).is_err());
        assert_eq!(std::fs::read(&core).unwrap(), b"old core");
        assert!(exe.join("busy").is_file());
    }
}
