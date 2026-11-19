//! CurseForge in the Host: the client, when this build has Lodestone's API key
//! (`LODESTONE_CURSEFORGE_API_KEY` when it was built, or in the environment), and what
//! downloads its files.
//!
//! The core has no CurseForge client: the Host asks CurseForge itself and hands the
//! core what it downloaded as an upload (`launcher_core::cores::admin::curseforge`). So
//! a build without the key, like a core's own dashboard, simply offers Modrinth alone.

use std::sync::Arc;

use gpui::{App, Global};
use launcher_core::{
    content::{CurseForge, Modrinth},
    game::download::{Downloader, Sleep},
};

/// Where a project comes from: Modrinth, through the core, or CurseForge, asked here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Source {
    #[default]
    Modrinth,
    CurseForge,
}

impl Source {
    /// The platform's name, which isn't translated.
    pub fn name(self) -> &'static str {
        match self {
            Source::Modrinth => "Modrinth",
            Source::CurseForge => "CurseForge",
        }
    }
}

#[derive(Default)]
struct Handle {
    /// `None` until first asked for; `Some(None)` when this build has no key.
    client: Option<Option<Arc<CurseForge>>>,
    /// Modrinth, asked directly (not through the core) about a CurseForge pack's files.
    modrinth: Option<Arc<Modrinth>>,
}

impl Global for Handle {}

/// The CurseForge client, or `None` when this build has no API key.
pub fn client(cx: &mut App) -> Option<Arc<CurseForge>> {
    let handle = cx.default_global::<Handle>();
    if let Some(client) = &handle.client {
        return client.clone();
    }
    let client = launcher_core::http::default_client()
        .ok()
        .and_then(CurseForge::production)
        .map(Arc::new);
    handle.client = Some(client.clone());
    client
}

/// Modrinth, asked from the Host itself: which of a CurseForge modpack's files it has
/// too (their SHA-512 and its CDN), when the pack is made into one for a server.
pub fn modrinth(cx: &mut App) -> Option<Arc<Modrinth>> {
    let handle = cx.default_global::<Handle>();
    if handle.modrinth.is_none() {
        let http = launcher_core::http::default_client().ok()?;
        handle.modrinth = Some(Arc::new(Modrinth::production(http)));
    }
    handle.modrinth.clone()
}

/// Whether this build offers CurseForge.
pub fn available(cx: &mut App) -> bool {
    client(cx).is_some()
}

/// Sets the client: a mock's in tests, none for the development screenshots (so they
/// look the same whether or not the build has a key).
pub fn set_client(client: Option<Arc<CurseForge>>, cx: &mut App) {
    cx.default_global::<Handle>().client = Some(client);
}

/// What downloads CurseForge's files before they are uploaded to the core.
pub fn downloader(cx: &App) -> Option<Downloader> {
    let executor = cx.background_executor().clone();
    let sleep: Sleep = Arc::new(move |duration| Box::pin(executor.timer(duration)));
    Some(Downloader::new(
        launcher_core::http::download_client().ok()?,
        sleep,
    ))
}

/// What background work calls to open the CurseForge page `url` (a file's download
/// page) in the browser, the first call only, with the task on the app's thread that
/// opens it. Never with the local stand-in for CurseForge's API
/// (`LODESTONE_CURSEFORGE_API_URL`, for tests and screenshots): its projects have no
/// pages on curseforge.com.
pub fn page_opener<T: 'static>(
    url: String,
    cx: &mut gpui::Context<T>,
) -> impl Fn() + Send + Sync + 'static {
    let (asked, mut asks) = futures::channel::mpsc::unbounded::<()>();
    cx.spawn(async move |_, cx| {
        use futures::StreamExt as _;
        if asks.next().await.is_none() {
            return;
        }
        if std::env::var_os("LODESTONE_CURSEFORGE_API_URL").is_some() {
            log::info!("not opening {url}: CurseForge's API is a stand-in");
            return;
        }
        cx.update(|cx| cx.open_url(&url));
    })
    .detach();
    move || {
        asked.unbounded_send(()).ok();
    }
}
