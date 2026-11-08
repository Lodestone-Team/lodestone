//! Keeps the launcher up to date with the builds GitHub publishes from `main`
//! ([`launcher_core::update`]).
//!
//! A while after the window opens, and every few hours after that, the latest build
//! is looked up. With "Update automatically" on (the default) a newer one is
//! downloaded and installed in the background; the top bar then offers to restart.
//! With it off, the top bar offers the download instead.

use std::{
    path::{Path, PathBuf},
    pin::pin,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::future::{Either, select};
use gpui::{AppContext as _, Context, Entity, SharedString, Task};
use http_client::HttpClient;
use launcher_core::update::{self, Build, Manifest, UpdateError};

use crate::instances::Instances;

/// How long after the window opens the first check waits, so it doesn't compete
/// with loading the instances and accounts.
const FIRST_CHECK: Duration = Duration::from_secs(10);
/// How often the launcher looks for a newer build while it runs.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// How often a download's progress is redrawn.
const PROGRESS_TICK: Duration = Duration::from_millis(250);

/// Where updating stands.
#[derive(Clone, Debug)]
pub enum UpdateState {
    /// A build the publish script didn't make (`cargo build`), which doesn't update
    /// itself.
    Unavailable,
    /// Not checked yet.
    Idle,
    Checking,
    /// The newest build is running.
    UpToDate,
    /// A newer build, not downloaded because "Update automatically" is off.
    Available(Manifest),
    Downloading {
        build: u64,
        done: Arc<AtomicU64>,
        total: u64,
    },
    /// A newer build is installed and runs once the launcher restarts.
    Ready {
        build: u64,
    },
    Failed(SharedString),
}

impl UpdateState {
    /// The share of the download done, 0 to 100.
    pub fn percent(done: &AtomicU64, total: u64) -> u64 {
        (done.load(Ordering::Relaxed).saturating_mul(100) / total.max(1)).min(100)
    }
}

pub struct Updater {
    state: UpdateState,
    build: Option<Build>,
    exe: Option<PathBuf>,
    manifest_url: String,
    http: Arc<dyn HttpClient>,
    download_http: Arc<dyn HttpClient>,
    instances: Entity<Instances>,
    /// The build installed by this run, which a later check compares against.
    installed: Option<u64>,
    work: Option<Task<()>>,
    _poll: Option<Task<()>>,
}

impl Updater {
    pub fn new(instances: Entity<Instances>, cx: &mut Context<Self>) -> Self {
        let build = Build::current();
        let exe = update::exe().map(Path::to_path_buf);
        let enabled = build.is_some() && exe.is_some();
        if enabled && let Some(exe) = exe.clone() {
            // What the update before this start left behind.
            cx.background_spawn(async move { update::clean_up(&exe) })
                .detach();
        }
        let poll = enabled.then(|| {
            cx.spawn(async move |this, cx| {
                cx.background_executor().timer(FIRST_CHECK).await;
                loop {
                    if this.update(cx, |this, cx| this.check(cx)).is_err() {
                        return;
                    }
                    cx.background_executor().timer(CHECK_EVERY).await;
                }
            })
        });
        Self {
            state: if enabled {
                UpdateState::Idle
            } else {
                UpdateState::Unavailable
            },
            build,
            exe,
            manifest_url: update::manifest_url(),
            http: launcher_core::http::default_client().expect("could not start the HTTP client"),
            download_http: launcher_core::http::download_client()
                .expect("could not start the HTTP client"),
            instances,
            installed: None,
            work: None,
            _poll: poll,
        }
    }

    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    /// The running build, or `None` for a local build.
    pub fn build(&self) -> Option<Build> {
        self.build
    }

    fn automatic(&self, cx: &Context<Self>) -> bool {
        self.instances.read(cx).auto_update()
    }

    /// Looks for a newer build now. Does nothing while a check or download runs.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        if matches!(
            self.state,
            UpdateState::Unavailable | UpdateState::Checking | UpdateState::Downloading { .. }
        ) {
            return;
        }
        self.state = UpdateState::Checking;
        cx.notify();
        let http = self.http.clone();
        let url = self.manifest_url.clone();
        let fetch = cx.background_spawn(launcher_core::http::in_http_runtime(async move {
            update::fetch_manifest(http.as_ref(), &url).await
        }));
        self.work = Some(cx.spawn(async move |this, cx| {
            let result = fetch.await;
            this.update(cx, |this, cx| this.checked(result, cx)).ok();
        }));
    }

    fn checked(&mut self, result: Result<Manifest, UpdateError>, cx: &mut Context<Self>) {
        self.work = None;
        match result {
            Err(err) => self.state = UpdateState::Failed(err.to_string().into()),
            Ok(manifest) => {
                let running = self.build.map_or(0, |build| build.number);
                let have = running.max(self.installed.unwrap_or(0));
                if manifest.newer(have).is_none() {
                    self.state = match self.installed {
                        Some(build) => UpdateState::Ready { build },
                        None => UpdateState::UpToDate,
                    };
                } else if self.automatic(cx) {
                    self.download(manifest, cx);
                    return;
                } else {
                    self.state = UpdateState::Available(manifest);
                }
            }
        }
        cx.notify();
    }

    /// "Update automatically" changed: with it on, an update waiting to be
    /// downloaded starts downloading.
    pub fn automatic_changed(&mut self, cx: &mut Context<Self>) {
        if let UpdateState::Available(manifest) = &self.state
            && self.automatic(cx)
        {
            let manifest = manifest.clone();
            self.download(manifest, cx);
        }
    }

    /// Downloads and installs the available update.
    pub fn download_available(&mut self, cx: &mut Context<Self>) {
        if let UpdateState::Available(manifest) = &self.state {
            let manifest = manifest.clone();
            self.download(manifest, cx);
        }
    }

    fn download(&mut self, manifest: Manifest, cx: &mut Context<Self>) {
        let Some(exe) = self.exe.clone() else {
            return;
        };
        let Some((build, asset)) = manifest
            .for_platform()
            .map(|(build, asset)| (build, asset.clone()))
        else {
            return;
        };
        let url = match Manifest::asset_url(&self.manifest_url, &asset) {
            Ok(url) => url,
            Err(err) => {
                self.state = UpdateState::Failed(err.to_string().into());
                cx.notify();
                return;
            }
        };
        let done = Arc::new(AtomicU64::new(0));
        self.state = UpdateState::Downloading {
            build,
            done: done.clone(),
            total: asset.size,
        };
        cx.notify();
        let http = self.download_http.clone();
        let work = cx.background_spawn(launcher_core::http::in_http_runtime(async move {
            let staged = update::staging_path(&exe);
            update::download(http.as_ref(), &url, &asset, &staged, |bytes| {
                done.store(bytes, Ordering::Relaxed)
            })
            .await?;
            update::install(&staged, &exe)
        }));
        self.work = Some(cx.spawn(async move |this, cx| {
            let mut work = pin!(work);
            loop {
                let tick = pin!(cx.background_executor().timer(PROGRESS_TICK));
                match select(work.as_mut(), tick).await {
                    Either::Left((result, _)) => {
                        this.update(cx, |this, cx| this.downloaded(build, result, cx))
                            .ok();
                        return;
                    }
                    Either::Right(_) => {
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            return;
                        }
                    }
                }
            }
        }));
    }

    fn downloaded(&mut self, build: u64, result: Result<(), UpdateError>, cx: &mut Context<Self>) {
        self.work = None;
        self.state = match result {
            Ok(()) => {
                self.installed = Some(build);
                UpdateState::Ready { build }
            }
            Err(err) => UpdateState::Failed(err.to_string().into()),
        };
        cx.notify();
    }

    /// Whether restarting now is safe: not while a game runs, which would lose its
    /// log and crash report.
    pub fn can_restart(&self, cx: &gpui::App) -> bool {
        matches!(self.state, UpdateState::Ready { .. }) && !self.instances.read(cx).any_active()
    }

    /// Starts the installed build and quits this one.
    pub fn restart(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.state, UpdateState::Ready { .. }) || self.instances.read(cx).any_active()
        {
            return;
        }
        let Some(exe) = self.exe.clone() else {
            return;
        };
        match update::restart(&exe) {
            Ok(()) => cx.quit(),
            Err(err) => {
                self.state = UpdateState::Failed(err.to_string().into());
                cx.notify();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percent_of_a_download() {
        let done = AtomicU64::new(0);
        assert_eq!(UpdateState::percent(&done, 200), 0);
        done.store(50, Ordering::Relaxed);
        assert_eq!(UpdateState::percent(&done, 200), 25);
        done.store(500, Ordering::Relaxed);
        assert_eq!(UpdateState::percent(&done, 200), 100);
        assert_eq!(UpdateState::percent(&done, 0), 100);
    }
}
