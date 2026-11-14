//! Instances' game icons (`GameIcon.tsx`) and the pictures the Host downloads: modpack
//! and project icons from Modrinth's CDN, and players' heads from mc-heads.net.
//!
//! Downloaded pictures are kept for the session in [`RemoteImages`], fetched once each.
//! Only those two hosts are ever asked, over HTTPS.

use std::{collections::HashMap, sync::Arc};

use gpui::{
    AnyElement, App, Global, IntoElement, ObjectFit, Pixels, RenderImage, SharedString,
    StyledImage as _, div, img, prelude::*,
};
use launcher_core::{
    content::modrinth::Modrinth,
    cores::admin::models::{Game, MinecraftVariant},
};
use lodestone_ui::{components::text_tooltip, theme::radius};

use crate::i18n::{InstancesMsg, tr};

/// A downloaded picture.
enum Remote {
    Loading,
    Ready(Arc<RenderImage>),
    Failed,
}

/// The pictures downloaded this session, by URL.
#[derive(Default)]
pub struct RemoteImages {
    images: HashMap<String, Remote>,
    modrinth: Option<Arc<Modrinth>>,
}

impl Global for RemoteImages {}

/// Where players' heads come from (`InstancePlayerList.tsx`).
const HEADS_HOST: &str = "mc-heads.net";
const MAX_HEAD_BYTES: usize = 256 * 1024;

/// A Modrinth CDN picture (`https://cdn.modrinth.com/…`), or `None` while it downloads
/// or if it can't be had. The windows are drawn again when it arrives.
pub fn modrinth_image(url: &str, cx: &mut App) -> Option<Arc<RenderImage>> {
    if !is_modrinth_cdn(url) {
        return None;
    }
    fetch(url, cx, |url, modrinth| async move {
        modrinth.image(&url).await.ok()
    })
}

/// A player's 16 × 16 head from mc-heads.net, by their Minecraft UUID.
pub fn player_head(uuid: &str, cx: &mut App) -> Option<Arc<RenderImage>> {
    let uuid: String = uuid
        .chars()
        .filter(|c| c.is_ascii_hexdigit() || *c == '-')
        .collect();
    if uuid.is_empty() {
        return None;
    }
    let url = format!("https://{HEADS_HOST}/avatar/{uuid}/16.png");
    fetch(&url, cx, |url, _| async move {
        let http = launcher_core::http::default_client().ok()?;
        let request = launcher_core::http::get(&url, None).ok()?;
        let response = launcher_core::http::send(http.as_ref(), request)
            .await
            .ok()?;
        (response.is_success() && response.body.len() <= MAX_HEAD_BYTES).then_some(response.body)
    })
}

/// Whether `url` is an HTTPS URL on Modrinth's CDN (`modpackIconUrl`).
pub fn is_modrinth_cdn(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https" && u.host_str() == Some("cdn.modrinth.com") && u.port().is_none()
    })
}

fn fetch<F, Fut>(url: &str, cx: &mut App, download: F) -> Option<Arc<RenderImage>>
where
    F: FnOnce(String, Arc<Modrinth>) -> Fut,
    Fut: std::future::Future<Output = Option<Vec<u8>>> + Send + 'static,
{
    let modrinth = {
        let images = cx.default_global::<RemoteImages>();
        match images.images.get(url) {
            Some(Remote::Ready(image)) => return Some(image.clone()),
            Some(_) => return None,
            None => {}
        }
        let modrinth = match &images.modrinth {
            Some(modrinth) => modrinth.clone(),
            None => {
                let http = launcher_core::http::default_client().ok()?;
                let modrinth = Arc::new(Modrinth::production(http));
                images.modrinth = Some(modrinth.clone());
                modrinth
            }
        };
        images.images.insert(url.to_string(), Remote::Loading);
        modrinth
    };
    let key = url.to_string();
    let work = cx.background_spawn(launcher_core::http::in_http_runtime({
        let download = download(key.clone(), modrinth);
        async move {
            let bytes = download.await?;
            launcher_core::content::scan::decode_icon(&bytes).map(|image| (*image).clone())
        }
    }));
    cx.spawn(async move |cx| {
        let image = work.await;
        cx.update(|cx| {
            let image = match image {
                Some(image) => Remote::Ready(lodestone_ui::images::render_image(image)),
                None => Remote::Failed,
            };
            cx.default_global::<RemoteImages>()
                .images
                .insert(key, image);
            cx.refresh_windows();
        });
    })
    .detach();
    None
}

/// The bundled picture of a game (`game_to_game_icon`).
pub fn game_icon_path(game: &Game) -> &'static str {
    match game {
        Game::MinecraftJava { variant } => match variant {
            MinecraftVariant::Vanilla => "images/games/minecraft-vanilla.png",
            MinecraftVariant::Fabric => "images/games/minecraft-fabric.png",
            MinecraftVariant::Forge => "images/games/minecraft-forge.png",
            MinecraftVariant::Paper => "images/games/minecraft-paper.png",
            MinecraftVariant::NeoForge => "images/games/minecraft-neoforge.svg",
            _ => "images/games/minecraft-missing-texture.svg",
        },
        Game::Generic { .. } => "images/games/generic.svg",
        Game::MinecraftBedrock | Game::Unknown => "images/games/minecraft-missing-texture.svg",
    }
}

/// A game's name as the dashboard shows it (`game_to_game_title`).
pub fn game_title(game: &Game) -> String {
    game.title()
}

/// An instance's icon: its modpack's (from Modrinth's CDN) when it has one, else its
/// game's; with the game's name as a tooltip.
pub fn game_icon(
    id: impl Into<SharedString>,
    game: &Game,
    icon_url: Option<&str>,
    size: Pixels,
    cx: &mut App,
) -> AnyElement {
    let remote = icon_url.and_then(|url| modrinth_image(url, cx));
    let picture = match remote {
        Some(image) => img(image)
            .size(size)
            .rounded(radius::DEFAULT)
            .object_fit(ObjectFit::Cover)
            .into_any_element(),
        None => img(game_icon_path(game))
            .size(size)
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
    };
    div()
        .id(id.into())
        .flex_none()
        .size(size)
        .child(picture)
        .tooltip(text_tooltip(game_title(game)))
        .into_any_element()
}

/// "Copied …", for a toast after copying `text`.
pub fn copied(text: &str) -> String {
    tr!(InstancesMsg::CopiedText, text = text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_modrinths_cdn() {
        assert!(is_modrinth_cdn(
            "https://cdn.modrinth.com/data/abc/icon.png"
        ));
        assert!(!is_modrinth_cdn(
            "http://cdn.modrinth.com/data/abc/icon.png"
        ));
        assert!(!is_modrinth_cdn(
            "https://cdn.modrinth.com.evil.example/x.png"
        ));
        assert!(!is_modrinth_cdn("https://cdn.modrinth.com:8443/x.png"));
        assert!(!is_modrinth_cdn("not a url"));
    }

    #[test]
    fn every_game_has_an_icon() {
        let fabric = Game::MinecraftJava {
            variant: MinecraftVariant::Fabric,
        };
        assert_eq!(game_icon_path(&fabric), "images/games/minecraft-fabric.png");
        assert!(game_icon_path(&Game::Unknown).ends_with(".svg"));
    }
}
