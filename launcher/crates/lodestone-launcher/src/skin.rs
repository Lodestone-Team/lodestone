//! The launcher's skin as the window shows it: the chosen pack's colours right away,
//! and its images once they are decoded in the background (see
//! [`launcher_core::ui_skin`] for packs).
//!
//! Images are prepared once when a skin is applied: the sidebar's tiled and blurred,
//! the Play hero's scaled to the hero's size with its scrim worked out. Nothing is
//! redrawn or animated while the window is idle.
//!
//! An instance can override the skin's accent and Play button colour; the window
//! uses the selected instance's ([`resolve`]).
//!
//! Safe mode skips the skin: `--no-skin`, `LODESTONE_LAUNCHER_NO_SKIN=1`, or a skin
//! that fails to load falls back to the default look.

use std::sync::Arc;

use gpui::{App, Global, RenderImage, Task};
use image::RgbaImage;
use launcher_core::{
    instances::Instance,
    ui_skin::{Fit, ImageRole, PlaySpec, Rgb, SkinEntry, SkinSpec, SkinStore},
};

use crate::instances::appearance::{Background, scrim_for};

/// The skin the window shows.
pub struct Look {
    pub id: String,
    pub spec: SkinSpec,
    /// The sidebar's prepared image: tiled and blurred as the skin says.
    pub sidebar: Option<Arc<RenderImage>>,
    /// The Play tab's image for an instance without its own.
    pub hero: Option<Background>,
}

impl Look {
    fn plain(id: &str, spec: SkinSpec) -> Self {
        Self {
            id: id.into(),
            spec,
            sidebar: None,
            hero: None,
        }
    }

    fn default_look() -> Self {
        Self::plain("default", SkinSpec::default())
    }
}

/// The skin now, and the decoding of its images.
pub struct SkinState {
    look: Arc<Look>,
    /// Bumped by each apply, so a slower decode of an older skin is dropped.
    generation: u64,
    _decode: Option<Task<()>>,
    /// Why the chosen skin couldn't be used, if it couldn't.
    pub error: Option<String>,
}

impl Global for SkinState {}

impl Default for SkinState {
    fn default() -> Self {
        Self {
            look: Arc::new(Look::default_look()),
            generation: 0,
            _decode: None,
            error: None,
        }
    }
}

/// The skin the window shows.
pub fn look(cx: &App) -> Arc<Look> {
    cx.try_global::<SkinState>()
        .map(|state| state.look.clone())
        .unwrap_or_else(|| Arc::new(Look::default_look()))
}

/// Why the chosen skin isn't shown, if it isn't.
pub fn error(cx: &App) -> Option<String> {
    cx.try_global::<SkinState>().and_then(|s| s.error.clone())
}

/// Whether the launcher was started without its skin: `--no-skin` or
/// `LODESTONE_LAUNCHER_NO_SKIN=1`.
pub fn safe_mode() -> bool {
    std::env::args().any(|arg| arg == "--no-skin")
        || std::env::var("LODESTONE_LAUNCHER_NO_SKIN").is_ok_and(|v| !v.is_empty() && v != "0")
}

/// Shows skin `id` from `store` (the default look for `None`, in safe mode, or if it
/// can't be read). Its colours apply now; its images when they are decoded.
pub fn apply(id: Option<&str>, store: &SkinStore, cx: &mut App) {
    let wanted = if safe_mode() { None } else { id };
    let (entry, error) = match wanted.map(|id| store.get(id)) {
        None => (None, None),
        Some(Ok(entry)) => (Some(entry), None),
        Some(Err(err)) => (None, Some(err.to_string())),
    };
    apply_entry(entry, store.clone(), error, cx);
}

/// Shows `entry` (the default look for `None`).
pub fn apply_entry(
    entry: Option<SkinEntry>,
    store: SkinStore,
    error: Option<String>,
    cx: &mut App,
) {
    let (id, spec) = entry.map_or_else(
        || ("default".to_string(), SkinSpec::default()),
        |e| (e.id, e.spec),
    );
    let state = cx.default_global::<SkinState>();
    state.generation += 1;
    let generation = state.generation;
    state.error = error;
    state.look = Arc::new(Look::plain(&id, spec.clone()));
    if spec.images().is_empty() {
        state._decode = None;
        cx.refresh_windows();
        return;
    }
    let work = cx.background_executor().spawn({
        let (id, spec) = (id.clone(), spec.clone());
        async move { prepare(&id, &spec, &store) }
    });
    let task = cx.spawn(async move |cx| {
        let (sidebar, hero) = work.await;
        cx.update(|cx| {
            let state = cx.default_global::<SkinState>();
            if state.generation != generation {
                return;
            }
            state.look = Arc::new(Look {
                id,
                spec,
                sidebar: sidebar.map(crate::images::render_image),
                hero: hero.map(|(image, scrim)| Background {
                    image: crate::images::render_image(image),
                    scrim,
                }),
            });
            cx.refresh_windows();
        });
    });
    cx.default_global::<SkinState>()._decode = Some(task);
    cx.refresh_windows();
}

/// Shows a changed description of the skin on screen: its images are prepared again
/// only when what they are made from changed (image, fit or blur), so a colour or a
/// slider doesn't decode anything.
pub fn update(entry: SkinEntry, store: SkinStore, cx: &mut App) {
    let current = look(cx);
    let same_images = current.id == entry.id
        && current.spec.sidebar.image == entry.spec.sidebar.image
        && current.spec.sidebar.fit == entry.spec.sidebar.fit
        && current.spec.sidebar.blur == entry.spec.sidebar.blur
        && current.spec.hero.image == entry.spec.hero.image;
    if !same_images {
        return apply_entry(Some(entry), store, None, cx);
    }
    let state = cx.default_global::<SkinState>();
    state.look = Arc::new(Look {
        id: entry.id,
        spec: entry.spec,
        sidebar: current.sidebar.clone(),
        hero: current.hero.clone(),
    });
    cx.refresh_windows();
}

/// The skin's images, ready to draw: the sidebar's and the hero's with its scrim.
fn prepare(
    id: &str,
    spec: &SkinSpec,
    store: &SkinStore,
) -> (
    Option<RgbaImage>,
    Option<(RgbaImage, crate::instances::appearance::Scrim)>,
) {
    let sidebar = spec.sidebar.image.as_deref().and_then(|name| {
        let image = load(id, name, store, ImageRole::Sidebar)?;
        // the sidebar's shape at 2×; a blurred image is made at a quarter of that,
        // blurred, and scaled up once
        let (w, h) = (480, 1800);
        let blur = spec.sidebar.blur;
        let scale = if blur > 0 { 4 } else { 1 };
        let image = match spec.sidebar.fit {
            Fit::Tile if blur == 0 => return Some(tile(&image, w, h)),
            Fit::Tile => image::imageops::resize(
                &tile(&image, w, h),
                w / scale,
                h / scale,
                image::imageops::FilterType::Triangle,
            ),
            Fit::Cover => image::DynamicImage::ImageRgba8(image)
                .resize_to_fill(w / scale, h / scale, image::imageops::FilterType::Triangle)
                .to_rgba8(),
        };
        Some(match blur {
            0 => image,
            blur => image::imageops::resize(
                &image::imageops::blur(&image, (blur as f32 / 4.0).max(0.5)),
                w,
                h,
                image::imageops::FilterType::Triangle,
            ),
        })
    });
    let hero = spec.hero.image.as_deref().and_then(|name| {
        let image = load(id, name, store, ImageRole::Hero)?;
        let image = if image.width() > 1920 || image.height() > 1080 {
            image::DynamicImage::ImageRgba8(image)
                .resize(1920, 1080, image::imageops::FilterType::Triangle)
                .to_rgba8()
        } else {
            image
        };
        let scrim = scrim_for(&image);
        Some((image, scrim))
    });
    (sidebar, hero)
}

/// Image `name` of skin `id`: drawn for a built-in image, read from the pack else.
fn load(id: &str, name: &str, store: &SkinStore, _role: ImageRole) -> Option<RgbaImage> {
    if let Some(builtin) = name.strip_prefix("builtin:") {
        return art::draw(builtin);
    }
    let path = store.image_path(id, name)?;
    let bytes = std::fs::read(path).ok()?;
    crate::images::decode_large(&bytes).or_else(|| {
        image::load_from_memory(&bytes)
            .ok()
            .map(|image| image.to_rgba8())
    })
}

/// `image` repeated over a `width` × `height` canvas.
fn tile(image: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return image.clone();
    }
    RgbaImage::from_fn(width, height, |x, y| *image.get_pixel(x % w, y % h))
}

/// The colours the window uses for an instance: its own over the skin's.
#[derive(Clone, Debug, PartialEq)]
pub struct Resolved {
    pub accent: Option<Rgb>,
    pub play: PlaySpec,
    pub progress: Option<Rgb>,
}

/// What the window shows for `instance` with `spec`: the instance's colours win.
pub fn resolve(spec: &SkinSpec, instance: Option<&Instance>) -> Resolved {
    let mut play = spec.play.clone();
    if let Some(color) = instance.and_then(|i| i.play_color) {
        play.color = Some(color);
        play.gradient = None;
    }
    Resolved {
        accent: instance.and_then(|i| i.accent).or(spec.accent),
        play,
        progress: spec.progress,
    }
}

/// The built-in skins' images, drawn in code so they cost nothing to ship.
pub mod art {
    use image::{Rgba, RgbaImage};

    /// A small, repeatable hash, for the pixel art's speckles.
    fn noise(x: u32, y: u32, seed: u32) -> u32 {
        let mut n = x
            .wrapping_mul(374_761_393)
            .wrapping_add(y.wrapping_mul(668_265_263))
            .wrapping_add(seed.wrapping_mul(2_147_483_647));
        n = (n ^ (n >> 13)).wrapping_mul(1_274_126_177);
        n ^ (n >> 16)
    }

    fn rgb(hex: u32) -> Rgba<u8> {
        Rgba([(hex >> 16) as u8, (hex >> 8) as u8, hex as u8, 255])
    }

    fn mix(a: Rgba<u8>, b: Rgba<u8>, t: f32) -> Rgba<u8> {
        let t = t.clamp(0.0, 1.0);
        let c = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8;
        Rgba([c(0), c(1), c(2), 255])
    }

    /// Built-in image `name` ("dirt", "sky", "glass", "grid").
    pub fn draw(name: &str) -> Option<RgbaImage> {
        Some(match name {
            "dirt" => dirt(),
            "sky" => sky(),
            "glass" => glass(),
            "grid" => grid(),
            _ => return None,
        })
    }

    /// A 16 × 16 dirt block, each pixel 4 × 4.
    fn dirt() -> RgbaImage {
        let palette = [0x7A5536, 0x6B4A2F, 0x8B623F, 0x5C3F28, 0x966B47].map(rgb);
        RgbaImage::from_fn(64, 64, |x, y| {
            let n = noise(x / 4, y / 4, 7) as usize;
            palette[n % palette.len()]
        })
    }

    /// Blocky hills of grass and dirt under a pale sky with square clouds.
    fn sky() -> RgbaImage {
        let (w, h) = (1600u32, 640u32);
        let block = 40;
        let (top, bottom) = (rgb(0x5FA8F0), rgb(0xCBE6FF));
        let heights: Vec<u32> = (0..w / block + 1)
            .map(|col| {
                let wave =
                    ((col as f32 * 0.35).sin() * 1.6 + (col as f32 * 0.11).cos() * 1.2) as i32;
                (5 + wave + (noise(col, 0, 3) % 2) as i32).clamp(3, 8) as u32
            })
            .collect();
        RgbaImage::from_fn(w, h, |x, y| {
            let col = (x / block) as usize;
            let ground = h - heights[col] * block;
            let px = (x / 8, y / 8);
            if y >= ground {
                let depth = (y - ground) / block;
                let speck = noise(px.0, px.1, 11) % 5;
                let base = match depth {
                    0 if y - ground < 14 => [0x5DB83B, 0x4FA332, 0x6CC748, 0x58B036, 0x4C9C30],
                    0..=2 => [0x7A5536, 0x6B4A2F, 0x8B623F, 0x5C3F28, 0x966B47],
                    _ => [0x7D7D7D, 0x6E6E6E, 0x8A8A8A, 0x747474, 0x666666],
                };
                rgb(base[speck as usize])
            } else {
                let sky = mix(top, bottom, y as f32 / ground.max(1) as f32);
                // square clouds
                let (cx, cy) = (x / 32, y / 32);
                let cloud = (2..=4).contains(&cy)
                    && noise(cx / 3, cy / 2, 5).is_multiple_of(7)
                    && y < h / 2;
                if cloud {
                    mix(sky, rgb(0xFFFFFF), 0.85)
                } else {
                    sky
                }
            }
        })
    }

    /// Soft colour blobs on dark blue, the Glass skin's backdrop. Drawn small: it is
    /// soft anyway, and scaled up when shown.
    fn glass() -> RgbaImage {
        let (w, h) = (600u32, 400u32);
        let blobs = [
            (0.25, 0.30, 0.45, rgb(0x6E56CF)),
            (0.75, 0.25, 0.40, rgb(0x2BB3C0)),
            (0.60, 0.80, 0.50, rgb(0xE0607E)),
            (0.10, 0.85, 0.35, rgb(0x3B6FD8)),
        ];
        let base = rgb(0x121624);
        RgbaImage::from_fn(w, h, |x, y| {
            let (fx, fy) = (x as f32 / w as f32, y as f32 / h as f32);
            blobs.iter().fold(base, |color, (bx, by, r, c)| {
                let d = ((fx - bx).powi(2) + (fy - by).powi(2) * 0.6).sqrt();
                let t = (1.0 - d / r).max(0.0);
                mix(color, *c, t * t * 0.85)
            })
        })
    }

    /// A neon grid going to the horizon, on black.
    fn grid() -> RgbaImage {
        let (w, h) = (1600u32, 640u32);
        let horizon = (h as f32 * 0.45) as u32;
        let ground = (h - horizon) as f32;
        let (black, pink, cyan) = (rgb(0x07070C), rgb(0xFF2BD6), rgb(0x22D3EE));
        // the rows' heights on screen: closer together towards the horizon
        let rows: Vec<f32> = (0..24)
            .map(|n| horizon as f32 + ground / (1.0 + n as f32 * 0.55))
            .collect();
        let centre = w as f32 / 2.0;
        RgbaImage::from_fn(w, h, |x, y| {
            if y < horizon {
                // a glow rising from the horizon
                let t = 1.0 - (horizon - y) as f32 / horizon as f32;
                return mix(black, rgb(0x2A0A36), t.powi(3));
            }
            // 0 at the horizon, 1 at the bottom
            let d = (y - horizon) as f32 / ground;
            let floor = mix(rgb(0x14051C), black, d);
            if y <= horizon + 1 {
                return pink;
            }
            let width = 0.8 + d * 1.6;
            let on_row = rows.iter().any(|r| (y as f32 - r).abs() < width);
            // columns meeting at the middle of the horizon, 180 px apart at the bottom
            let spacing = 180.0 * d;
            let offset = x as f32 - centre;
            let column = (offset / spacing).round();
            let on_column = (offset - column * spacing).abs() < width;
            if on_row {
                mix(floor, pink, 0.55 + d * 0.4)
            } else if on_column {
                mix(floor, cyan, 0.45 + d * 0.45)
            } else {
                floor
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_core::ui_skin::built_in;

    #[test]
    fn instances_override_the_skin() {
        let spec = built_in("neon").unwrap();
        let none = resolve(&spec, None);
        assert_eq!(none.accent, spec.accent);
        assert_eq!(none.play.color, spec.play.color);
        let instance = Instance {
            play_color: Some(Rgb(0x123456)),
            accent: Some(Rgb(0xABCDEF)),
            ..Instance::default()
        };
        let resolved = resolve(&spec, Some(&instance));
        assert_eq!(resolved.accent, Some(Rgb(0xABCDEF)));
        assert_eq!(resolved.play.color, Some(Rgb(0x123456)));
        // the instance's colour replaces the skin's gradient
        assert_eq!(resolved.play.gradient, None);
        // the rest of the button stays the skin's
        assert_eq!(resolved.play.glow, spec.play.glow);
        // default look and plain instance: nothing set
        let plain = resolve(&SkinSpec::default(), Some(&Instance::default()));
        assert_eq!(plain.accent, None);
        assert_eq!(plain.play, PlaySpec::default());
    }

    #[test]
    fn built_in_skins_prepare_their_images() {
        // built-in skins read nothing from the folder
        let store = SkinStore::at(std::path::PathBuf::from("/nonexistent"));
        for id in ["grassblock", "glass", "neon"] {
            let spec = launcher_core::ui_skin::built_in(id).unwrap();
            let start = std::time::Instant::now();
            let (sidebar, hero) = prepare(id, &spec, &store);
            eprintln!("{id}: {:?}", start.elapsed());
            assert_eq!(sidebar.is_some(), spec.sidebar.image.is_some(), "{id}");
            assert!(hero.is_some(), "{id}");
        }
    }

    #[test]
    fn built_in_art_is_drawn() {
        for name in ["dirt", "sky", "glass", "grid"] {
            let image = art::draw(name).unwrap();
            assert!(image.width() > 0, "{name}");
        }
        assert!(art::draw("nope").is_none());
        let tiled = tile(&art::draw("dirt").unwrap(), 100, 130);
        assert_eq!(tiled.dimensions(), (100, 130));
    }
}
