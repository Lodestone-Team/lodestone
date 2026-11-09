//! How an instance looks: its icon (one of the launcher's pixel-art icons, an emoji on
//! a coloured tile, or its own image) and its Play background, with the scrim that
//! keeps the text over the background readable.

use std::path::{Path, PathBuf};
use std::{sync::Arc, time::SystemTime};

use gpui::{
    AnyElement, Context, Hsla, IntoElement, Pixels, RenderImage, SharedString, Task, div, img,
    prelude::*,
};
use launcher_core::instances::{BUILTIN_ICON, EMOJI_ICON, Instance};

use super::Instances;
use crate::theme::{Color, colors};

/// The icon an instance has until one is chosen.
pub const DEFAULT_ICON: &str = "grass";

/// The launcher's pixel-art icons, by file name, with what they show.
pub const LIBRARY: &[(&str, &str)] = &[
    ("grass", "Grass block"),
    ("dirt", "Dirt"),
    ("stone", "Stone"),
    ("cobblestone", "Cobblestone"),
    ("sand", "Sand"),
    ("gravel", "Gravel"),
    ("log", "Log"),
    ("planks", "Planks"),
    ("leaves", "Leaves"),
    ("snow", "Snowy block"),
    ("ice", "Ice"),
    ("diamond-ore", "Diamond ore"),
    ("gold-ore", "Gold ore"),
    ("emerald-ore", "Emerald ore"),
    ("redstone-ore", "Redstone ore"),
    ("netherrack", "Netherrack"),
    ("end-stone", "End stone"),
    ("obsidian", "Obsidian"),
    ("pumpkin", "Pumpkin"),
    ("bricks", "Bricks"),
    ("copper", "Copper"),
    ("amethyst", "Amethyst"),
    ("glowstone", "Glowstone"),
    ("lodestone", "Lodestone"),
    ("sword", "Sword"),
    ("pickaxe", "Pickaxe"),
    ("heart", "Heart"),
    ("apple", "Apple"),
    ("potion", "Potion"),
    ("gem", "Gem"),
    ("torch", "Torch"),
    ("star", "Star"),
];

/// The emoji offered for icons.
pub const EMOJI: &[&str] = &[
    "⛏️",
    "🗡️",
    "🏹",
    "🛡️",
    "🪓",
    "🧭",
    "🗺️",
    "💎",
    "🔥",
    "⚡",
    "❄️",
    "🌙",
    "☀️",
    "⭐",
    "🌲",
    "🌵",
    "🌋",
    "🏔️",
    "🏝️",
    "🌊",
    "🍄",
    "🌸",
    "🌻",
    "🍎",
    "🎃",
    "🍖",
    "🧪",
    "📦",
    "🏰",
    "🏠",
    "⛺",
    "🏗️",
    "🚀",
    "🤖",
    "👾",
    "🎮",
    "🏆",
    "👑",
    "💀",
    "👻",
    "🐉",
    "🐺",
    "🐱",
    "🐷",
    "🐮",
    "🐔",
    "🐝",
    "🐟",
    "🦊",
    "🐸",
    "🧙",
    "🧑‍🚀",
    "🔮",
    "🎵",
];

/// The tile colours an emoji can sit on, as RRGGBB.
pub const TILE_COLORS: &[u32] = &[
    0x36393F, 0x1D8EB2, 0x3DAE5E, 0xEFB440, 0xCF4545, 0x8736C7, 0xD06A2B, 0x2B6CB0,
];

/// The image of a library icon, falling back to the default for unknown names.
pub fn library_image(name: &str) -> SharedString {
    let known = LIBRARY.iter().any(|(n, _)| *n == name);
    format!(
        "images/instance-icons/{}.png",
        if known { name } else { DEFAULT_ICON }
    )
    .into()
}

/// What an instance's `icon` asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IconChoice {
    Default,
    Custom,
    Library(String),
    Emoji { tile: u32, emoji: String },
}

impl IconChoice {
    pub fn parse(icon: Option<&str>) -> Self {
        let Some(icon) = icon else {
            return IconChoice::Default;
        };
        if icon == launcher_core::instances::CUSTOM_ICON {
            return IconChoice::Custom;
        }
        if let Some(name) = icon.strip_prefix(BUILTIN_ICON) {
            return IconChoice::Library(name.to_string());
        }
        if let Some((tile, emoji)) = icon
            .strip_prefix(EMOJI_ICON)
            .and_then(|rest| rest.split_once(':'))
            && let Ok(tile) = u32::from_str_radix(tile, 16)
        {
            return IconChoice::Emoji {
                tile,
                emoji: emoji.to_string(),
            };
        }
        IconChoice::Default
    }

    /// The value stored in `instance.json`.
    pub fn library(name: &str) -> String {
        format!("{BUILTIN_ICON}{name}")
    }

    pub fn emoji(tile: u32, emoji: &str) -> String {
        format!("{EMOJI_ICON}{tile:06X}:{emoji}")
    }
}

/// An emoji on a rounded tile of `tile`'s colour.
pub fn emoji_tile(emoji: &str, tile: u32, size: Pixels) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(size)
        .rounded(size * 0.22)
        .bg(Color::rgb(tile))
        .text_size(size * 0.62)
        .line_height(size)
        .child(emoji.to_string())
        .into_any_element()
}

/// The instance's icon at `size`.
pub fn instance_icon(instances: &Instances, instance: &Instance, size: Pixels) -> AnyElement {
    match IconChoice::parse(instance.icon.as_deref()) {
        IconChoice::Custom => match instances.icon(&instance.id) {
            Some(image) => img(image)
                .flex_none()
                .size(size)
                .rounded(size * 0.2)
                .into_any_element(),
            None => img(library_image(DEFAULT_ICON))
                .flex_none()
                .size(size)
                .into_any_element(),
        },
        IconChoice::Library(name) => img(library_image(&name))
            .flex_none()
            .size(size)
            .into_any_element(),
        IconChoice::Emoji { tile, emoji } => emoji_tile(&emoji, tile, size),
        IconChoice::Default => img(library_image(DEFAULT_ICON))
            .flex_none()
            .size(size)
            .into_any_element(),
    }
}

/// How dark to make the scrim over a background so white text on it stays readable.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scrim {
    /// The scrim's opacity where the text is, at the bottom left, from 0 to 1.
    pub text: f32,
    /// An even dim over the whole image, for bright images, from 0 to 1.
    pub dim: f32,
}

/// The colour the scrim darkens towards (the theme's darkest grey).
const SCRIM_LUMINANCE: f32 = 0.012;
/// The highest background luminance under the text that keeps white text at a
/// contrast of at least 4.5:1: (1.0 + 0.05) / (L + 0.05) >= 4.5.
const TEXT_LUMINANCE: f32 = 0.183;

fn linear(channel: u8) -> f32 {
    let c = channel as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn luminance(px: &image::Rgba<u8>) -> f32 {
    0.2126 * linear(px.0[0]) + 0.7152 * linear(px.0[1]) + 0.0722 * linear(px.0[2])
}

/// Measures the image where the Play tab draws text (the bottom-left part) and over
/// all of it, and picks the scrim. The text area is judged by its bright parts (the
/// 90th percentile), so a white patch under the name darkens enough even on a dark
/// image; a dark image gets only a light scrim, kept for depth.
pub fn scrim_for(image: &image::RgbaImage) -> Scrim {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return Scrim {
            text: 0.5,
            dim: 0.0,
        };
    }
    let step = ((w as u64 * h as u64 / 20_000) as f64).sqrt().max(1.0) as u32;
    let mut text_area = Vec::new();
    let mut total = 0.0;
    let mut count = 0usize;
    for y in (0..h).step_by(step as usize) {
        for x in (0..w).step_by(step as usize) {
            let l = luminance(image.get_pixel(x, y));
            total += l;
            count += 1;
            if x < w * 6 / 10 && y >= h / 2 {
                text_area.push(l);
            }
        }
    }
    text_area.sort_by(|a, b| a.total_cmp(b));
    let bright = text_area
        .get(text_area.len() * 9 / 10)
        .copied()
        .unwrap_or(0.0);
    let needed = if bright > TEXT_LUMINANCE {
        (bright - TEXT_LUMINANCE) / (bright - SCRIM_LUMINANCE)
    } else {
        0.0
    };
    let mean = total / count.max(1) as f32;
    Scrim {
        text: needed.clamp(0.35, 0.92),
        dim: ((mean - 0.35) * 0.5).clamp(0.0, 0.25),
    }
}

/// Decoded backgrounds are kept at most this size: the Play tab's banner at 2× on a
/// typical window, without the megabytes of a full-size image.
pub const DECODED_MAX: (u32, u32) = (1920, 1080);

/// `image` scaled down to fit [`DECODED_MAX`], as RGBA for drawing.
pub fn fit_for_hero(image: image::DynamicImage) -> image::RgbaImage {
    let (max_w, max_h) = DECODED_MAX;
    if image.width() > max_w || image.height() > max_h {
        image
            .resize(max_w, max_h, image::imageops::FilterType::Triangle)
            .to_rgba8()
    } else {
        image.to_rgba8()
    }
}

/// A decoded Play background and its scrim.
#[derive(Clone)]
pub struct Background {
    pub image: Arc<RenderImage>,
    pub scrim: Scrim,
}

/// Where an instance's own Play background comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackgroundSource {
    /// Its own `background.jpg`, through the display-size copy in the hero cache.
    Own(PathBuf),
    /// A skin's banner image in the skin's pack.
    Pack(PathBuf),
    /// A skin's banner drawn by the launcher (`builtin:`).
    Art(String),
}

impl BackgroundSource {
    /// Reads and decodes it, at most the hero's size.
    pub fn decode(&self, cache: &launcher_core::hero_cache::HeroCache) -> Option<image::RgbaImage> {
        match self {
            BackgroundSource::Own(path) => {
                // Only the display-size copy is read, never the original.
                let copy = cache.copy_of(path).ok()?;
                Some(fit_for_hero(image::open(&copy).ok()?))
            }
            BackgroundSource::Pack(path) => Some(fit_for_hero(image::open(path).ok()?)),
            BackgroundSource::Art(name) => launcher_core::ui_skin::art::draw(name)
                .map(|image| fit_for_hero(image::DynamicImage::ImageRgba8(image))),
        }
    }
}

impl Instances {
    /// The instance's own Play background, once it is decoded.
    pub fn background(&self, id: &str) -> Option<&Background> {
        self.backgrounds
            .get(id)
            .map(|(_, _, background)| background)
    }

    /// Where `instance`'s own Play background comes from, and when it last changed:
    /// its image, or its skin banner while that skin and banner exist. `None` for the
    /// default (the skin's hero, or the launcher's artwork).
    pub fn background_source(
        &self,
        instance: &Instance,
    ) -> Option<(BackgroundSource, Option<SystemTime>)> {
        let modified = |path: &Path| std::fs::metadata(path).and_then(|m| m.modified()).ok();
        match instance.background_mode() {
            launcher_core::instances::BackgroundMode::Image => {
                let path = self.store.background_path(instance)?;
                let time = modified(&path);
                Some((BackgroundSource::Own(path), time))
            }
            launcher_core::instances::BackgroundMode::Banner => {
                let (skin, banner) = instance.skin_banner()?;
                let store = self.skins();
                let entry = store.get(skin).ok()?;
                let image = entry.spec.banner(banner)?.image.clone();
                match image.strip_prefix("builtin:") {
                    Some(name) => Some((BackgroundSource::Art(name.to_string()), None)),
                    None => {
                        let path = store.image_path(skin, &image)?;
                        let time = modified(&path);
                        Some((BackgroundSource::Pack(path), time))
                    }
                }
            }
            _ => None,
        }
    }

    /// Decodes, in the background, the Play background of the selected instance (and
    /// keeps the one before, for a quick switch back), and forgets the others: each
    /// decoded image is several megabytes.
    pub(super) fn load_backgrounds(&mut self, cx: &mut Context<Self>) {
        if self.selected != self.background_shown {
            self.background_previous = self.background_shown.take();
            self.background_shown = self.selected.clone();
        }
        let keep = [
            self.background_shown.clone(),
            self.background_previous.clone(),
        ];
        let mut wanted = Vec::new();
        for instance in &self.list {
            if !keep.contains(&Some(instance.id.clone())) {
                continue;
            }
            let Some((source, modified)) = self.background_source(instance) else {
                continue;
            };
            wanted.push((instance.id.clone(), source, modified));
        }
        self.backgrounds.retain(|id, (time, source, _)| {
            wanted
                .iter()
                .any(|(w, s, m)| w == id && m == time && s == source)
        });
        self.background_tasks
            .retain(|id, _| wanted.iter().any(|(w, _, _)| w == id));
        for (id, source, modified) in wanted {
            if self.backgrounds.contains_key(&id) || self.background_tasks.contains_key(&id) {
                continue;
            }
            let cache = self.hero_cache.clone();
            let decode = cx.background_spawn({
                let source = source.clone();
                async move {
                    let image = source.decode(&cache)?;
                    let scrim = scrim_for(&image);
                    Some((crate::images::render_image(image), scrim))
                }
            });
            let task_id = id.clone();
            let task = cx.spawn(async move |this, cx| {
                let decoded = decode.await;
                this.update(cx, |this, cx| {
                    this.background_tasks.remove(&task_id);
                    if let Some((image, scrim)) = decoded {
                        this.backgrounds
                            .insert(task_id, (modified, source, Background { image, scrim }));
                    }
                    cx.notify();
                })
                .ok();
            });
            self.background_tasks.insert(id, task);
        }
    }

    /// Makes banner `banner` of skin `skin` the instance's Play background.
    pub fn set_skin_banner(
        &mut self,
        id: &str,
        skin: &str,
        banner: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.store
            .set_skin_banner(id, skin, banner)
            .map_err(|err| err.to_string())?;
        self.backgrounds.remove(id);
        self.reload(cx);
        Ok(())
    }

    /// Gives a new instance one of the chosen skin's banners, at random. Nothing when
    /// the skin has none.
    pub(super) fn pick_skin_banner(&mut self, id: &str) {
        let Some(skin) = self.settings.skin.clone() else {
            return;
        };
        let Ok(entry) = self.skins().get(&skin) else {
            return;
        };
        if entry.spec.banners.is_empty() {
            return;
        }
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.subsec_nanos() as usize);
        let banner = &entry.spec.banners[nanos % entry.spec.banners.len()];
        if let Err(err) = self.store.set_skin_banner(id, &skin, &banner.id) {
            log::warn!("couldn't give {id} a banner: {err}");
        }
    }

    pub fn set_background(
        &mut self,
        id: &str,
        image: &std::path::Path,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.store
            .set_background(id, image)
            .map_err(|err| err.to_string())?;
        // A picked image is shown on its own, not in the slideshow.
        self.store
            .set_slideshow(id, None)
            .map_err(|err| err.to_string())?;
        self.backgrounds.remove(id);
        self.reload(cx);
        Ok(())
    }

    pub fn clear_background(&mut self, id: &str, cx: &mut Context<Self>) -> Result<(), String> {
        self.store
            .clear_background(id)
            .map_err(|err| err.to_string())?;
        self.reload(cx);
        Ok(())
    }

    /// The display-size copies of the banner's images.
    pub fn hero_cache(&self) -> &launcher_core::hero_cache::HeroCache {
        &self.hero_cache
    }

    /// How much space the banner's image copies take, in bytes. Blocking.
    pub fn hero_cache_size(&self) -> u64 {
        self.hero_cache.size()
    }

    /// Removes the banner's image copies; they are made again when shown.
    pub fn clear_hero_cache(&mut self) -> std::io::Result<()> {
        self.hero_cache.clear()
    }

    /// Goes up when an instance's slideshow or its own images change.
    pub fn slideshow_revision(&self) -> u64 {
        self.slideshow_revision
    }

    /// Shows a slideshow from `source` on the Play tab, or, for `None`, the
    /// instance's image again.
    pub fn set_slideshow(
        &mut self,
        id: &str,
        source: Option<launcher_core::instances::SlideshowSource>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.store
            .set_slideshow(id, source)
            .map_err(|err| err.to_string())?;
        self.slideshow_revision += 1;
        self.reload(cx);
        Ok(())
    }

    /// Adds images to the instance's own slideshow set. Returns the first error, after
    /// adding the images it could.
    pub fn add_slideshow_images(
        &mut self,
        id: &str,
        images: &[std::path::PathBuf],
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let mut error = None;
        for image in images {
            if let Err(err) = self.store.add_slideshow_image(id, image) {
                error.get_or_insert(err.to_string());
            }
        }
        self.slideshow_revision += 1;
        cx.notify();
        error.map_or(Ok(()), Err)
    }

    /// Removes one of the instance's own slideshow images, by file name.
    pub fn remove_slideshow_image(
        &mut self,
        id: &str,
        name: &str,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.store
            .remove_slideshow_image(id, name)
            .map_err(|err| err.to_string())?;
        self.slideshow_revision += 1;
        cx.notify();
        Ok(())
    }

    /// Uses a library icon or an emoji, or the default for `None`.
    pub fn set_icon_name(
        &mut self,
        id: &str,
        icon: Option<&str>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        self.store
            .set_icon_name(id, icon)
            .map_err(|err| err.to_string())?;
        self.reload(cx);
        Ok(())
    }
}

/// The scrim's layers over a background, bottom first: an even dim, then darkening
/// from the bottom and from the left, strongest behind the name.
/// The scrim's layers, rounded like the box they cover (children are clipped to a
/// rectangle, not to rounded corners).
pub fn scrim_layers(scrim: Scrim, radius: gpui::Rems) -> Vec<AnyElement> {
    let shade = |alpha: f32| -> Hsla {
        let mut color: Hsla = colors::GRAY_900.into();
        color.a = alpha.clamp(0.0, 1.0);
        color
    };
    let clear = shade(0.0);
    let bottom = (scrim.text / 0.6 * 0.75).min(0.97);
    let left = scrim.text.min(0.9);
    vec![
        div()
            .absolute()
            .inset_0()
            .rounded(radius)
            .bg(shade(scrim.dim))
            .into_any_element(),
        div()
            .absolute()
            .inset_0()
            .rounded(radius)
            .bg(gpui::linear_gradient(
                0.,
                gpui::linear_color_stop(shade(bottom), 0.),
                gpui::linear_color_stop(clear, 0.75),
            ))
            .into_any_element(),
        div()
            .absolute()
            .inset_0()
            .rounded(radius)
            .bg(gpui::linear_gradient(
                90.,
                gpui::linear_color_stop(shade(left), 0.),
                gpui::linear_color_stop(clear, 0.7),
            ))
            .into_any_element(),
    ]
}

/// A decoded background with the modification time of the file it was read from.
pub(super) type BackgroundEntry = (Option<SystemTime>, BackgroundSource, Background);
/// A background being decoded.
pub(super) type BackgroundTask = Task<()>;

#[cfg(test)]
mod tests {
    use super::*;

    fn filled(rgb: [u8; 3]) -> image::RgbaImage {
        image::RgbaImage::from_pixel(400, 200, image::Rgba([rgb[0], rgb[1], rgb[2], 255]))
    }

    #[test]
    fn a_white_image_gets_a_strong_scrim_and_a_dark_one_a_light_scrim() {
        let white = scrim_for(&filled([255, 255, 255]));
        assert!(white.text > 0.8, "{white:?}");
        assert!(white.dim > 0.2, "{white:?}");
        let dark = scrim_for(&filled([20, 22, 26]));
        assert_eq!(dark.text, 0.35);
        assert_eq!(dark.dim, 0.0);
    }

    #[test]
    fn a_bright_patch_under_the_text_counts_even_on_a_dark_image() {
        let mut image = filled([20, 22, 26]);
        for y in 100..200 {
            for x in 0..240 {
                image.put_pixel(x, y, image::Rgba([250, 250, 250, 255]));
            }
        }
        assert!(scrim_for(&image).text > 0.8);
    }

    #[test]
    fn the_scrim_is_just_enough_for_white_text() {
        // Mid grey: luminance about 0.22, a little too bright for 4.5:1.
        let grey = scrim_for(&filled([130, 130, 130]));
        let under = 0.2232 * (1.0 - grey.text) + SCRIM_LUMINANCE * grey.text;
        assert!((1.05 / (under + 0.05)) >= 4.5, "{grey:?}");
        assert!(grey.text < 0.5, "{grey:?}");
    }

    #[test]
    fn icon_choices_round_trip() {
        assert_eq!(IconChoice::parse(None), IconChoice::Default);
        assert_eq!(IconChoice::parse(Some("custom")), IconChoice::Custom);
        assert_eq!(
            IconChoice::parse(Some(&IconChoice::library("gold-ore"))),
            IconChoice::Library("gold-ore".into())
        );
        let emoji = IconChoice::emoji(0x3DAE5E, "🌲");
        assert_eq!(emoji, "emoji:3DAE5E:🌲");
        assert!(launcher_core::instances::valid_icon_name(&emoji));
        assert_eq!(
            IconChoice::parse(Some(&emoji)),
            IconChoice::Emoji {
                tile: 0x3DAE5E,
                emoji: "🌲".into()
            }
        );
        assert_eq!(
            library_image("nope"),
            SharedString::from("images/instance-icons/grass.png")
        );
        for emoji in EMOJI {
            let value = IconChoice::emoji(TILE_COLORS[0], emoji);
            assert!(launcher_core::instances::valid_icon_name(&value), "{emoji}");
        }
    }
}
