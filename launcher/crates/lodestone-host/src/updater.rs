//! Lodestone Host's own updates: what Settings › Updates shows and does.
//!
//! The mechanism comes later (`launcher/docs/host-parity.md`, §8.7). It will be an
//! [`UpdateSource`]: the launcher's (`launcher_core::update`: a manifest on a GitHub
//! release per channel, a download checked against its size and hash, the executable
//! swapped in place, a restart) with the Host's own channels and bundle (the Host and
//! its core). Until a source is set, the page says the Host doesn't update itself.
//!
//! The model is the launcher's (`lodestone-launcher/src/updater.rs`): a check a while
//! after the start and every few hours, "Update automatically" downloads and installs
//! a newer build, "Beta builds" picks the channel, and a restart applies it. Restarting
//! the Host stops the core and its servers first, as Quit does.

use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use futures::future::BoxFuture;
use gpui::{App, AppContext as _, Context, Entity, Global, Task};

use crate::prefs::Prefs;

/// How long after the start the first check waits.
const FIRST_CHECK: Duration = Duration::from_secs(30);
/// How often the Host looks for a newer build while it runs.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// How often a download's progress is redrawn.
const PROGRESS_TICK: Duration = Duration::from_millis(250);

/// Where new builds come from: the part that comes later.
pub trait UpdateSource: Send + Sync + 'static {
    /// The newest build in the stable channel, or the stable and beta ones, newer than
    /// `running`; `None` when there is none.
    fn check(&self, beta: bool, running: u64) -> BoxFuture<'static, Result<Option<Found>, String>>;

    /// Downloads and installs `found`, telling `progress` how many bytes are done.
    fn install(
        &self,
        found: Found,
        progress: Arc<dyn Fn(u64) + Send + Sync>,
    ) -> BoxFuture<'static, Result<(), String>>;

    /// Starts the installed build. The Host then quits (stopping its core first).
    fn restart(&self) -> Result<(), String>;
}

/// A newer build.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Found {
    pub build: u64,
    /// The download's size in bytes.
    pub size: u64,
}

/// Where updating stands (the launcher's `UpdateState`).
#[derive(Clone, Debug)]
pub enum UpdateState {
    /// No source yet, or a build the publish script didn't make.
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
    /// Installed: runs after a restart.
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
    work: Option<Task<()>>,
    _poll: Option<Task<()>>,
}

/// The app's updater, for the Settings page.
pub struct GlobalUpdater(pub Entity<Updater>);

impl Global for GlobalUpdater {}

impl Updater {
    /// Makes the app's updater, with no source yet. Call once at startup.
    pub fn init(cx: &mut App) {
        let updater = cx.new(|_| Updater::new(None, None));
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
        let installed = match self.state {
            UpdateState::Ready { build } => Some(build),
            _ => None,
        };
        self.state = UpdateState::Checking;
        cx.notify();
        let beta = Prefs::get(cx).beta_builds;
        let running = installed.or(self.build).unwrap_or(0);
        let work = cx.background_spawn(source.check(beta, running));
        self.work = Some(cx.spawn(async move |this, cx| {
            let result = work.await;
            this.update(cx, |this, cx| {
                this.work = None;
                this.state = match result {
                    Err(err) => UpdateState::Failed(err),
                    Ok(None) => match installed {
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

    /// Downloads and installs the available build.
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
        let work = cx.background_spawn(source.install(found, progress));
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

    /// Starts the installed build; the caller then quits the Host as Quit does.
    pub fn restart(&mut self, cx: &mut Context<Self>) -> bool {
        let (UpdateState::Ready { .. }, Some(source)) = (&self.state, &self.source) else {
            return false;
        };
        match source.restart() {
            Ok(()) => true,
            Err(err) => {
                self.state = UpdateState::Failed(err);
                cx.notify();
                false
            }
        }
    }
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
}
