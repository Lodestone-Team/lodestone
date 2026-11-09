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
    ui_skin::{
        Fit, ImageRole, PlaySpec, Readability, Rgb, SkinEntry, SkinSpec, SkinStore,
        readable::{self, Backdrop, Palette},
    },
};

use crate::theme::{Color, colors};

use crate::instances::appearance::{Background, scrim_for};

/// The skin the window shows.
pub struct Look {
    pub id: String,
    pub spec: SkinSpec,
    /// The sidebar's prepared image: tiled and blurred as the skin says.
    pub sidebar: Option<Arc<RenderImage>>,
    /// The Play tab's image for an instance without its own.
    pub hero: Option<Background>,
    /// How the sidebar's prepared image looks behind text, before the tint.
    pub sidebar_backdrop: Option<Backdrop>,
    /// How the hero's image looks behind text, before its scrim.
    pub hero_backdrop: Option<Backdrop>,
}

impl Look {
    fn plain(id: &str, spec: SkinSpec) -> Self {
        Self {
            id: id.into(),
            spec,
            sidebar: None,
            hero: None,
            sidebar_backdrop: None,
            hero_backdrop: None,
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
    /// Whether the skin's images are still being prepared.
    decoding: bool,
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
            decoding: false,
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

/// Whether the skin's images are still being prepared (the screenshots wait).
pub fn decoding(cx: &App) -> bool {
    cx.try_global::<SkinState>()
        .is_some_and(|state| state.decoding)
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
        state.decoding = false;
        cx.refresh_windows();
        return;
    }
    let work = cx.background_executor().spawn({
        let (id, spec) = (id.clone(), spec.clone());
        async move { prepare(&id, &spec, &store) }
    });
    let task = cx.spawn(async move |cx| {
        let ((sidebar, sidebar_backdrop), (hero, hero_backdrop)) = work.await;
        cx.update(|cx| {
            let state = cx.default_global::<SkinState>();
            if state.generation != generation {
                return;
            }
            state.decoding = false;
            state.look = Arc::new(Look {
                id,
                spec,
                sidebar: sidebar.map(crate::images::render_image),
                hero: hero.map(|(image, scrim)| Background {
                    image: crate::images::render_image(image),
                    scrim,
                }),
                sidebar_backdrop,
                hero_backdrop,
            });
            cx.refresh_windows();
        });
    });
    let state = cx.default_global::<SkinState>();
    state._decode = Some(task);
    state.decoding = true;
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
        && current.spec.hero.image == entry.spec.hero.image
        && current.spec.hero.readability == entry.spec.hero.readability;
    if !same_images {
        return apply_entry(Some(entry), store, None, cx);
    }
    let state = cx.default_global::<SkinState>();
    state.look = Arc::new(Look {
        id: entry.id,
        spec: entry.spec,
        sidebar: current.sidebar.clone(),
        hero: current.hero.clone(),
        sidebar_backdrop: current.sidebar_backdrop,
        hero_backdrop: current.hero_backdrop,
    });
    cx.refresh_windows();
}

/// An image ready to draw, and how it looks behind text.
type Prepared<T> = (Option<T>, Option<Backdrop>);

/// How `image` looks behind text: every fourth pixel each way is plenty.
fn measure(image: &RgbaImage) -> Option<Backdrop> {
    Backdrop::measure(
        image
            .enumerate_pixels()
            .filter(|(x, y, _)| x % 4 == 0 && y % 4 == 0)
            .map(|(_, _, p)| Rgb(((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32)),
    )
}

/// The skin's images, ready to draw: the sidebar's and the hero's with its scrim
/// (none when the hero's readability is off), and how each looks behind text.
fn prepare(
    id: &str,
    spec: &SkinSpec,
    store: &SkinStore,
) -> (
    Prepared<RgbaImage>,
    Prepared<(RgbaImage, crate::instances::appearance::Scrim)>,
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
        let scrim = match spec.hero.readability {
            Readability::Auto => scrim_for(&image),
            Readability::Off => crate::instances::appearance::Scrim {
                text: 0.0,
                dim: 0.0,
            },
        };
        Some((image, scrim))
    });
    let sidebar_backdrop = sidebar.as_ref().and_then(measure);
    let hero_backdrop = hero.as_ref().and_then(|(image, _)| measure(image));
    ((sidebar, sidebar_backdrop), (hero, hero_backdrop))
}

/// The panel colour the sidebar is drawn on.
const PANEL: Rgb = Rgb(0x26282C);

/// The sidebar's text colours for `look`: `None` for the launcher's own (no image,
/// no tint, no colours of its own), else worked out from the image under its tint.
pub fn sidebar_palette(look: &Look) -> Option<Palette> {
    let sidebar = &look.spec.sidebar;
    if sidebar.image.is_none() && sidebar.tint.is_none() && sidebar.text == Default::default() {
        return None;
    }
    let base = look
        .sidebar_backdrop
        .filter(|_| sidebar.image.is_some())
        .unwrap_or_else(|| Backdrop::solid(PANEL));
    let seen = match sidebar.tint {
        Some(tint) => base.under(tint, f32::from(sidebar.tint_opacity) / 100.0),
        None => base,
    };
    Some(readable::derive(seen, &sidebar.text, sidebar.readability))
}

/// The hero's text contrast: white over the image's light end, under its scrim.
pub fn hero_contrast(look: &Look) -> Option<f32> {
    let backdrop = look.hero_backdrop?;
    let scrim = look.hero.as_ref()?.scrim;
    let seen = backdrop.under(readable::DARK, scrim.text.max(scrim.dim));
    Some(Rgb::WHITE.contrast(seen.light))
}

/// The sidebar's text colours as the sidebar draws them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SidebarInks {
    pub primary: Color,
    pub secondary: Color,
    pub muted: Color,
    pub faint: Color,
    pub icon: Color,
    /// Behind the selected or hovered row.
    pub fill: Color,
    /// The quiet buttons' labels (New instance…, Import instances…).
    pub ghost: Color,
    /// A backing over the sidebar's image, under its text.
    pub backing: Option<Color>,
}

impl Default for SidebarInks {
    /// The launcher's own.
    fn default() -> Self {
        Self {
            primary: colors::GRAY_300,
            secondary: colors::GRAY_400,
            muted: colors::GRAY_500,
            faint: colors::GRAY_FADED.opacity(30),
            icon: colors::GRAY_FADED.opacity(50),
            fill: colors::GRAY_800,
            ghost: colors::WHITE.opacity(50),
            backing: None,
        }
    }
}

impl SidebarInks {
    fn of(palette: Option<Palette>) -> Self {
        let Some(palette) = palette else {
            return Self::default();
        };
        let color = |rgb: Rgb| Color::rgb(rgb.0);
        let layer = if palette.dark_text {
            readable::LIGHT
        } else {
            readable::DARK
        };
        Self {
            primary: color(palette.inks.primary),
            secondary: color(palette.inks.secondary),
            muted: color(palette.inks.muted),
            faint: color(palette.inks.faint),
            icon: color(palette.inks.icon),
            // a light row under dark text; the launcher's own under light text
            fill: if palette.dark_text {
                color(readable::LIGHT).opacity(70)
            } else {
                colors::GRAY_800
            },
            ghost: color(palette.inks.muted),
            backing: (palette.backing > 0.0)
                .then(|| color(layer).opacity((palette.backing * 100.0).round() as u32)),
        }
    }
}

thread_local! {
    /// The sidebar's colours for this frame, set by the window's root view.
    static SIDEBAR_INKS: std::cell::Cell<SidebarInks> =
        std::cell::Cell::new(SidebarInks::default());
}

/// Works out the sidebar's colours for this frame from `look`.
pub fn set_sidebar_inks(look: &Look) {
    let inks = SidebarInks::of(sidebar_palette(look));
    SIDEBAR_INKS.with(|cell| cell.set(inks));
}

/// The sidebar's colours for this frame.
pub fn sidebar_inks() -> SidebarInks {
    SIDEBAR_INKS.with(|cell| cell.get())
}

/// The backing of a sidebar row over an instance's banner: [`readable::DARK`] (or
/// [`readable::LIGHT`] under dark text) as strong as the row's text needs over
/// `backdrop`, from 55%. With readability off, a fixed 70%.
pub fn row_backing(backdrop: Backdrop, inks: &SidebarInks, readability: Readability) -> Color {
    let rgb = |c: Color| Rgb(c.0 >> 8);
    let dark_text = rgb(inks.primary).luminance() < 0.3;
    let layer = if dark_text {
        readable::LIGHT
    } else {
        readable::DARK
    };
    let alpha = match readability {
        Readability::Off => 0.7,
        Readability::Auto => {
            let texts = [inks.primary, inks.secondary, inks.muted, inks.faint].map(rgb);
            (11..=19)
                .map(|step| step as f32 * 0.05)
                .find(|&alpha| {
                    let seen = backdrop.under(layer, alpha);
                    let end = if dark_text { seen.dark } else { seen.light };
                    texts.iter().all(|t| t.contrast(end) >= readable::BODY)
                })
                .unwrap_or(0.95)
        }
    };
    Color::rgb(layer.0).opacity((alpha * 100.0).round() as u32)
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
            let ((sidebar, _), (hero, _)) = prepare(id, &spec, &store);
            eprintln!("{id}: {:?}", start.elapsed());
            assert_eq!(sidebar.is_some(), spec.sidebar.image.is_some(), "{id}");
            assert!(hero.is_some(), "{id}");
        }
    }

    #[test]
    fn built_in_skins_keep_text_readable() {
        let store = SkinStore::at(std::path::PathBuf::from("/nonexistent"));
        for id in launcher_core::ui_skin::BUILT_IN {
            let spec = launcher_core::ui_skin::built_in(id).unwrap();
            let ((_, sidebar_backdrop), (hero, hero_backdrop)) = prepare(id, &spec, &store);
            let look = Look {
                id: id.into(),
                spec: spec.clone(),
                sidebar: None,
                hero: hero.map(|(image, scrim)| Background {
                    image: crate::images::render_image(image),
                    scrim,
                }),
                sidebar_backdrop,
                hero_backdrop,
            };
            // every sidebar text style: primary, secondary, muted, faint
            if let Some(palette) = sidebar_palette(&look) {
                assert!(palette.passes(), "{id} sidebar: {palette:?}");
                eprintln!(
                    "{id}: sidebar {:.2}:1, backing {:.2}",
                    palette.contrast, palette.backing
                );
            } else {
                assert_eq!(id, "default");
            }
            if let Some(contrast) = hero_contrast(&look) {
                eprintln!("{id}: hero {contrast:.2}:1");
                assert!(contrast >= readable::BODY - 0.05, "{id} hero: {contrast}");
            }
        }
    }

    #[test]
    fn sidebar_inks_are_the_launchers_own_without_a_skin() {
        let look = Look::default_look();
        assert_eq!(
            SidebarInks::of(sidebar_palette(&look)),
            SidebarInks::default()
        );
        // a bright pack image takes dark text or a backing, and passes
        let mut spec = SkinSpec::named("Bright");
        spec.sidebar.image = Some("x.png".into());
        let look = Look {
            spec,
            sidebar_backdrop: Some(Backdrop::solid(Rgb(0xF2F2F2))),
            ..Look::default_look()
        };
        let palette = sidebar_palette(&look).unwrap();
        assert!(palette.passes(), "{palette:?}");
        // dark text sits on light rows, not the launcher's dark ones
        let inks = SidebarInks::of(Some(palette));
        assert!(palette.dark_text);
        assert_ne!(inks.fill, colors::GRAY_800);
        assert_ne!(inks.ghost, SidebarInks::default().ghost);
        // readability off: the colours as given, and it says so
        let mut off = look.spec.clone();
        off.sidebar.readability = Readability::Off;
        let look = Look {
            spec: off,
            sidebar_backdrop: Some(Backdrop::solid(Rgb(0xF2F2F2))),
            ..Look::default_look()
        };
        assert!(!sidebar_palette(&look).unwrap().passes());
    }

    #[test]
    fn banner_rows_back_their_text() {
        let inks = SidebarInks::default();
        let bright = Backdrop::solid(Rgb(0xF0F0F0));
        let backing = row_backing(bright, &inks, Readability::Auto);
        // strong enough that the faintest row text reads over a white banner
        assert!(backing.0 & 0xFF >= (0.8 * 255.0) as u32, "{backing:?}");
        let dark = row_backing(Backdrop::solid(Rgb(0x101010)), &inks, Readability::Auto);
        assert!(dark.0 & 0xFF <= backing.0 & 0xFF);
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
