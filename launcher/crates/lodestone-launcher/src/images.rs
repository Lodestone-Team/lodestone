//! Images decoded before the window opens, so the first frame already has them.
//!
//! GPUI's `img()` loads an image on its background executor the first time it is
//! drawn, so the window appears without it and the image pops in once it has
//! been decoded. The bundled images are decoded here instead, and the window is
//! opened once they are ready. Their rendered pixels are kept in the data folder's
//! cache, so only the first start rasterizes them. Skin heads, which are small, are
//! decoded from the on-disk cache when the account list is opened.
//!
//! On Windows the hero, the Play banner's artwork, is rasterized to fit the banner as
//! drawn ([`hero_fit`]) rather than at one size for every screen. Its pixels are held
//! twice, in memory and on the GPU, where some drivers (AMD's) also count about twice
//! the texture's size against the process's committed memory: at 2400 × 960 that came
//! to about 27 MB, for a banner that a 1200 × 760 window draws about 1400 pixels wide.
//! The render fitted at startup is cached for the next start, which uses it while the
//! display is no denser.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use gpui::{
    App, AppContext as _, Global, ImageSource, IntoElement, RenderImage, Styled as _, Task, canvas,
};

use crate::assets::Assets;

/// The bundled images that `img()` draws.
pub struct BundledImages {
    pub logo: Arc<RenderImage>,
    pub hero: Arc<RenderImage>,
    /// The scale `hero` was rasterized at.
    hero_scale: f32,
    /// Where renders are cached, when the hero is fitted to the banner: only with a
    /// cache (not in the offscreen scenes, whose screenshots mustn't depend on timing)
    /// and where the display's scale factor is known before the window opens.
    fit_cache: Option<PathBuf>,
    /// Whether the hero was fitted to the banner since it was loaded. After that it
    /// only grows (a larger window, a denser screen): a smaller texture wouldn't give
    /// back the GPU memory the driver set aside for the larger one.
    hero_settled: bool,
    /// A render for the banner is under way.
    hero_rendering: bool,
    /// The green Lodestone mark at icon size, for the sidebar's Lodestone Host.
    pub mark_small: Arc<RenderImage>,
    /// The mark larger, for the Lodestone Host page's offer.
    pub mark_large: Arc<RenderImage>,
}

impl Global for BundledImages {}

/// Adds the bundled images, for the performance monitor.
pub fn tally(tally: &mut crate::perf::ImageTally, cx: &App) {
    if let Some(images) = cx.try_global::<BundledImages>() {
        tally.add(&images.logo);
        tally.add(&images.hero);
        tally.add(&images.mark_small);
        tally.add(&images.mark_large);
    }
}

/// The scale passed to GPUI's SVG renderer, as its own `img()` loader does. The
/// renderer doubles it.
const SVG_SCALE: f32 = 1.0;
/// The hero's largest scale: its 1600×640 viewBox is rasterized at 2400×960 pixels
/// (the renderer doubles the scale), sharp at 2× in the banner of a typical window
/// (about 1000 points wide) while taking half the memory of 3200×1280. The first start
/// renders it at this scale; then [`hero_fit`] fits it to the banner. Without fitting it
/// is always drawn at this scale.
const HERO_MAX_SCALE: f32 = 0.75;
/// The hero's smallest scale, 400×160: smaller than the smallest window's banner.
const HERO_MIN_SCALE: f32 = 0.125;
/// The hero is fitted in steps of 200×80 pixels, so resizing the window a little
/// doesn't rasterize it again.
const HERO_SCALE_STEP: f32 = 0.0625;
/// The mark's scales: its 184-point square is rasterized at 64 pixels for icons (14 to
/// 18 points) and 160 for the offer (40 points), so neither is shrunk much on screen.
/// A picture drawn far smaller than it was rasterized looks jagged.
const MARK_SMALL_SCALE: f32 = 64. / 368.;
const MARK_LARGE_SCALE: f32 = 160. / 368.;

/// Starts decoding the bundled images on the background executor, in parallel.
/// With a `cache` folder, images rendered by an earlier start are read from it
/// instead of rasterizing their SVGs again, and new renders are saved there.
pub fn decode(cx: &App, cache: Option<PathBuf>) -> Task<anyhow::Result<BundledImages>> {
    let render = |path: &'static str, scale: f32| {
        let renderer = cx.svg_renderer();
        let cache = cache.clone();
        cx.background_spawn(async move {
            render_cached(&renderer, cache.as_deref(), path, scale, Saved::Yes)
        })
    };
    let logo = render(LOGO_PATH, SVG_SCALE);
    let fit = cache.clone().zip(startup_scale_factor());
    // The hero as fitted to the banner at the last start, if the display is no denser
    // now; otherwise at its largest scale.
    let hero = {
        let renderer = cx.svg_renderer();
        let cache = cache.clone();
        let fit = fit.clone();
        cx.background_spawn(async move {
            if let Some((dir, scale_factor)) = &fit
                && let Some(fitted) = fitted_hero(dir, *scale_factor)
            {
                return anyhow::Ok(fitted);
            }
            let scale = HERO_MAX_SCALE;
            let image = render_cached(&renderer, cache.as_deref(), HERO_PATH, scale, Saved::Yes)?;
            Ok((scale, image))
        })
    };
    let mark_small = render(MARK_PATH, MARK_SMALL_SCALE);
    let mark_large = render(MARK_PATH, MARK_LARGE_SCALE);
    cx.background_spawn(async move {
        let (hero_scale, hero) = hero.await?;
        Ok(BundledImages {
            logo: logo.await?,
            hero,
            hero_scale,
            fit_cache: fit.map(|(dir, _)| dir),
            hero_settled: false,
            hero_rendering: false,
            mark_small: mark_small.await?,
            mark_large: mark_large.await?,
        })
    })
}

/// Whether [`render_cached`] saves a new render in the cache.
#[derive(Clone, Copy, PartialEq)]
enum Saved {
    /// Yes, replacing the renders of an older version of the picture at this scale.
    Yes,
    /// No: the hero fitted to a banner that grew, which the next start (with the
    /// window at its usual size) shouldn't load.
    No,
}

/// `path` rendered at `scale`: read from `cache` if it was rendered before, otherwise
/// rendered (and saved there, with [`Saved::Yes`]).
fn render_cached(
    renderer: &gpui::SvgRenderer,
    cache: Option<&Path>,
    path: &'static str,
    scale: f32,
    saved: Saved,
) -> anyhow::Result<Arc<RenderImage>> {
    let bytes = Assets::embedded(path).ok_or_else(|| anyhow::anyhow!("{path} is not embedded"))?;
    let file = cache.map(|dir| cache_file(dir, path, bytes, scale));
    if let Some(image) = file.as_deref().and_then(read_cached) {
        return Ok(image);
    }
    let image = renderer
        .render_single_frame(bytes, scale)
        .map_err(|err| anyhow::anyhow!("rendering {path}: {err}"))?;
    if let Some(file) = file
        && saved == Saved::Yes
    {
        write_cached(&file, &image);
    }
    Ok(image)
}

/// The file that names the hero render fitted to the banner at the last start: its
/// scale and the display's scale factor then, as text (`0.4375 1`).
const HERO_FIT_FILE: &str = "hero.fit";

/// The hero render fitted at the last start, and its scale, unless it was fitted for a
/// display less dense than one with `scale_factor` (it would be drawn blurry).
fn fitted_hero(dir: &Path, scale_factor: f32) -> Option<(f32, Arc<RenderImage>)> {
    let text = std::fs::read_to_string(dir.join(HERO_FIT_FILE)).ok()?;
    let (scale, fitted_for) = text.trim().split_once(' ')?;
    let (scale, fitted_for): (f32, f32) = (scale.parse().ok()?, fitted_for.parse().ok()?);
    if !(HERO_MIN_SCALE..=HERO_MAX_SCALE).contains(&scale) || fitted_for + 0.01 < scale_factor {
        return None;
    }
    let bytes = Assets::embedded(HERO_PATH)?;
    Some((
        scale,
        read_cached(&cache_file(dir, HERO_PATH, bytes, scale))?,
    ))
}

/// Records the hero render at `scale` (already cached) as fitted at this start, on a
/// display with `scale_factor`, and removes renders at other scales but the largest
/// (the fallback for a denser display).
fn save_hero_fit(dir: &Path, scale: f32, scale_factor: f32) {
    let Some(bytes) = Assets::embedded(HERO_PATH) else {
        return;
    };
    let keep = [scale, HERO_MAX_SCALE].map(|scale| cache_file(dir, HERO_PATH, bytes, scale));
    let prefix = format!("{}-", cache_stem(HERO_PATH));
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(&prefix) && name.ends_with(".bgra") && !keep.contains(&entry.path()) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
    let _ = std::fs::write(dir.join(HERO_FIT_FILE), format!("{scale} {scale_factor}"));
}

/// The scale factor of the display the window opens on, before it opens: the primary
/// display's, as `Bounds::centered` picks it, read as GPUI reads it. `None` where it
/// isn't known, and then the hero isn't fitted.
#[cfg(windows)]
fn startup_scale_factor() -> Option<f32> {
    use windows_sys::Win32::{
        Foundation::POINT,
        Graphics::Gdi::{MONITOR_DEFAULTTOPRIMARY, MonitorFromPoint},
        UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI},
    };
    // SAFETY: plain calls with valid arguments; (0, 0) is on the primary display.
    unsafe {
        let monitor = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        if monitor.is_null() {
            return None;
        }
        let (mut dpi_x, mut dpi_y) = (0, 0);
        let read = GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        (read == 0 && dpi_x > 0).then(|| dpi_x as f32 / 96.)
    }
}

#[cfg(not(windows))]
fn startup_scale_factor() -> Option<f32> {
    None
}

/// The scale that rasterizes the hero to cover a banner `width` × `height` device
/// pixels, as `ObjectFit::Cover` draws it, in [`HERO_SCALE_STEP`]s.
fn hero_scale_for(width: f32, height: f32) -> f32 {
    // The renderer doubles the scale: the 1600×640 viewBox comes out 3200 × scale wide.
    let cover = (width / 3200.).max(height / 1280.);
    // a hair over a step stays on it
    let steps = (cover / HERO_SCALE_STEP - 0.01).ceil();
    (steps * HERO_SCALE_STEP).clamp(HERO_MIN_SCALE, HERO_MAX_SCALE)
}

/// Fits the hero to the banner it is drawn in: put it in the banner beside the
/// artwork, filling it. Once the banner has been drawn, the hero is rasterized again
/// at its size, in the background; until then the same picture is drawn, scaled.
pub fn hero_fit() -> impl IntoElement {
    canvas(
        |_, _, _| {},
        |bounds, (), window, cx| {
            let scale_factor = window.scale_factor();
            let width = f32::from(bounds.size.width) * scale_factor;
            let height = f32::from(bounds.size.height) * scale_factor;
            cx.defer(move |cx| fit_hero(width, height, scale_factor, cx));
        },
    )
    .absolute()
    .inset_0()
    .size_full()
}

/// Rasterizes the hero again for a banner `width` × `height` device pixels on a display
/// with `scale_factor`, if it should be: to the banner's size the first time (which
/// the next start loads), then only larger.
fn fit_hero(width: f32, height: f32, scale_factor: f32, cx: &mut App) {
    let Some(images) = cx.try_global::<BundledImages>() else {
        return;
    };
    let Some(dir) = images.fit_cache.clone() else {
        return;
    };
    if images.hero_rendering || width <= 0. || height <= 0. {
        return;
    }
    let scale = hero_scale_for(width, height);
    let settling = !images.hero_settled;
    let render = scale > images.hero_scale || (settling && scale < images.hero_scale);
    if !settling && !render {
        return;
    }
    let images = cx.global_mut::<BundledImages>();
    images.hero_settled = true;
    images.hero_rendering = render;
    if !render {
        // The size it has fits: the next start loads it too.
        let scale = images.hero_scale;
        cx.background_spawn(async move { save_hero_fit(&dir, scale, scale_factor) })
            .detach();
        return;
    }
    let renderer = cx.svg_renderer();
    let rendering = cx.background_spawn(async move {
        let saved = if settling { Saved::Yes } else { Saved::No };
        let image = render_cached(&renderer, Some(&dir), HERO_PATH, scale, saved)?;
        if settling {
            save_hero_fit(&dir, scale, scale_factor);
        }
        anyhow::Ok(image)
    });
    cx.spawn(async move |cx| {
        let rendered = rendering.await;
        cx.update(|cx| {
            let images = cx.global_mut::<BundledImages>();
            images.hero_rendering = false;
            let replaced = match rendered {
                Ok(image) => {
                    images.hero_scale = scale;
                    Some(std::mem::replace(&mut images.hero, image))
                }
                Err(err) => {
                    eprintln!("lodestone-launcher: fitting the artwork to the banner: {err:#}");
                    None
                }
            };
            if let Some(old) = replaced {
                release([old], cx);
                cx.refresh_windows();
            }
        });
    })
    .detach();
}

/// Where rendered bundled images are kept between starts: rasterizing the hero
/// takes most of the time before the window opens (about 190 ms), and reading its
/// pixels back takes a few. `None` without a data folder, or when it was moved to a
/// folder that isn't there (a drive not connected), which this mustn't create.
pub fn cache_dir() -> Option<PathBuf> {
    let location = launcher_core::paths::locate_data_dir()?;
    if location.source == launcher_core::paths::DataDirSource::Moved && !location.dir.is_dir() {
        return None;
    }
    Some(
        launcher_core::paths::DataDir::at(location.dir)
            .cache_dir()
            .join("images"),
    )
}

/// Bumped when the rendering changes in a way the SVG's bytes and scale don't show
/// (a new resvg, a different colour conversion), so older renders aren't used.
const CACHE_VERSION: u32 = 1;
const CACHE_MAGIC: &[u8; 4] = b"LSIM";
/// The magic, the version, the width and the height, before the BGRA pixels.
const CACHE_HEADER: usize = 16;

/// `<cache>/<name>-<hash>.bgra`, the hash covering the SVG, the scale and
/// [`CACHE_VERSION`].
fn cache_file(dir: &Path, path: &str, bytes: &[u8], scale: f32) -> PathBuf {
    use std::hash::{Hash, Hasher};
    // Keyed the same in every run (unlike `RandomState`).
    let mut hasher = std::hash::DefaultHasher::new();
    (CACHE_VERSION, bytes, scale.to_bits()).hash(&mut hasher);
    dir.join(format!(
        "{}-{}-{:016x}.bgra",
        cache_stem(path),
        scale.to_bits(),
        hasher.finish()
    ))
}

fn cache_stem(path: &str) -> &str {
    Path::new(path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("image")
}

/// The image saved at `file`, or `None` if there isn't a whole one.
fn read_cached(file: &Path) -> Option<Arc<RenderImage>> {
    let mut bytes = std::fs::read(file).ok()?;
    let header = bytes.get(..CACHE_HEADER)?;
    let word = |at: usize| u32::from_le_bytes(header[at..at + 4].try_into().unwrap());
    if &header[..4] != CACHE_MAGIC || word(4) != CACHE_VERSION {
        return None;
    }
    let (width, height) = (word(8), word(12));
    if bytes.len() != CACHE_HEADER + width as usize * height as usize * 4 {
        return None;
    }
    bytes.drain(..CACHE_HEADER);
    // Already BGRA, as GPUI draws it.
    let buffer = image::RgbaImage::from_raw(width, height, bytes)?;
    Some(Arc::new(RenderImage::new([image::Frame::new(buffer)])))
}

/// Saves `image` at `file`, replacing older renders of the same picture. A failure
/// (a moved data folder that isn't connected) only means rasterizing next time.
fn write_cached(file: &Path, image: &RenderImage) {
    let (Some(dir), Some(name), Some(pixels)) =
        (file.parent(), file.file_name(), image.as_bytes(0))
    else {
        return;
    };
    let size = image.size(0);
    let mut bytes = Vec::with_capacity(CACHE_HEADER + pixels.len());
    bytes.extend_from_slice(CACHE_MAGIC);
    for word in [CACHE_VERSION, size.width.0 as u32, size.height.0 as u32] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    bytes.extend_from_slice(pixels);
    let partial = file.with_extension("part");
    if std::fs::create_dir_all(dir).is_err()
        || std::fs::write(&partial, &bytes).is_err()
        || std::fs::rename(&partial, file).is_err()
    {
        let _ = std::fs::remove_file(&partial);
        return;
    }
    // Renders of an older version of this picture at this scale.
    let name = name.to_string_lossy();
    let Some((prefix, _)) = name.rsplit_once('-') else {
        return;
    };
    let prefix = format!("{prefix}-");
    for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
        let other = entry.file_name();
        let other = other.to_string_lossy();
        if other != name && other.starts_with(&prefix) && other.ends_with(".bgra") {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

const LOGO_PATH: &str = "images/logo.svg";
const HERO_PATH: &str = "images/hero.svg";
const MARK_PATH: &str = "images/lodestone_mark.svg";

/// Makes the decoded images available to [`logo`] and [`hero`]. If decoding
/// failed, they fall back to GPUI's loader.
pub fn install(images: anyhow::Result<BundledImages>, cx: &mut App) {
    match images {
        Ok(images) => cx.set_global(images),
        Err(err) => eprintln!("lodestone-launcher: decoding the bundled images: {err:#}"),
    }
}

pub fn logo(cx: &App) -> ImageSource {
    match cx.try_global::<BundledImages>() {
        Some(images) => images.logo.clone().into(),
        None => LOGO_PATH.into(),
    }
}

pub fn hero(cx: &App) -> ImageSource {
    match cx.try_global::<BundledImages>() {
        Some(images) => images.hero.clone().into(),
        None => HERO_PATH.into(),
    }
}

/// The green Lodestone mark, which stands for Lodestone Host: `large` for the
/// offer, otherwise at icon size.
pub fn mark(large: bool, cx: &App) -> ImageSource {
    match cx.try_global::<BundledImages>() {
        Some(images) if large => images.mark_large.clone().into(),
        Some(images) => images.mark_small.clone().into(),
        None => MARK_PATH.into(),
    }
}

pub use lodestone_ui::images::render_image;

/// Frees the GPU copies of images the launcher no longer shows. GPUI uploads each
/// image it draws to the window's sprite atlas and keeps it there until it is told
/// to drop it, so images that are replaced (slideshows, backgrounds, banners) would
/// otherwise pile up in GPU memory. Deferred, so a window being drawn right now
/// (which GPUI holds apart while it does) is included. An image drawn again later is
/// simply uploaded again.
pub fn release(images: impl IntoIterator<Item = Arc<RenderImage>>, cx: &mut App) {
    let images: Vec<_> = images.into_iter().collect();
    if images.is_empty() {
        return;
    }
    cx.defer(move |cx| {
        for image in images {
            cx.drop_image(image, None);
        }
    });
}

/// Decodes a downloaded image (PNG, JPEG, WebP or GIF's first frame) for a project
/// description, at most 1280 pixels on a side.
pub fn decode_large(bytes: &[u8]) -> Option<image::RgbaImage> {
    const MAX: u32 = 1280;
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    let image = reader.decode().ok()?;
    let image = if image.width() > MAX || image.height() > MAX {
        image.thumbnail(MAX, MAX)
    } else {
        image
    };
    Some(image.to_rgba8())
}

/// Decodes a small PNG, such as a cached skin head.
pub fn decode_png(bytes: &[u8]) -> anyhow::Result<Arc<RenderImage>> {
    let image = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)?;
    Ok(render_image(image.to_rgba8()))
}

/// Prints how long startup took, when `LODESTONE_LAUNCHER_TRACE_STARTUP` is set.
pub struct StartupTrace(Option<Instant>);

impl StartupTrace {
    pub fn start() -> Self {
        Self(std::env::var_os("LODESTONE_LAUNCHER_TRACE_STARTUP").map(|_| Instant::now()))
    }

    pub fn mark(&self, event: &str) {
        if let Some(start) = self.0 {
            eprintln!(
                "startup: {event} at {:.1} ms",
                start.elapsed().as_secs_f64() * 1000.0
            );
        }
    }
}

impl Clone for StartupTrace {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_to_bgra() {
        let image = image::RgbaImage::from_pixel(2, 1, image::Rgba([10, 20, 30, 40]));
        let rendered = render_image(image);
        assert_eq!(rendered.as_bytes(0).unwrap()[..4], [30, 20, 10, 40]);
    }

    #[test]
    fn bundled_images_render() {
        let renderer = gpui::SvgRenderer::new(Arc::new(Assets));
        let hero = renderer
            .render_single_frame(Assets::embedded(HERO_PATH).unwrap(), HERO_MAX_SCALE)
            .unwrap();
        assert_eq!(hero.size(0).width.0, 2400);
        assert_eq!(hero.size(0).height.0, 960);
        let logo = renderer
            .render_single_frame(Assets::embedded(LOGO_PATH).unwrap(), SVG_SCALE)
            .unwrap();
        assert_eq!(logo.size(0).width.0, 212);
        for (scale, side) in [(MARK_SMALL_SCALE, 64), (MARK_LARGE_SCALE, 160)] {
            let mark = renderer
                .render_single_frame(Assets::embedded(MARK_PATH).unwrap(), scale)
                .unwrap();
            assert_eq!(mark.size(0).width.0, side);
        }
    }

    #[test]
    fn cached_renders_read_back() {
        let dir =
            std::env::temp_dir().join(format!("lodestone-image-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let renderer = gpui::SvgRenderer::new(Arc::new(Assets));
        let bytes = Assets::embedded(MARK_PATH).unwrap();
        let file = cache_file(&dir, MARK_PATH, bytes, MARK_SMALL_SCALE);
        assert!(read_cached(&file).is_none());
        let image = renderer
            .render_single_frame(bytes, MARK_SMALL_SCALE)
            .unwrap();
        write_cached(&file, &image);
        let cached = read_cached(&file).unwrap();
        assert_eq!(cached.size(0), image.size(0));
        assert_eq!(cached.as_bytes(0), image.as_bytes(0));

        // Another scale is another file; a changed picture replaces the old render.
        assert_ne!(cache_file(&dir, MARK_PATH, bytes, MARK_LARGE_SCALE), file);
        let changed = cache_file(&dir, MARK_PATH, b"<svg/>", MARK_SMALL_SCALE);
        assert_ne!(changed, file);
        write_cached(&changed, &image);
        assert!(!file.exists());
        assert!(read_cached(&changed).is_some());

        // A cut-off file is rendered again.
        let bytes = std::fs::read(&changed).unwrap();
        std::fs::write(&changed, &bytes[..bytes.len() - 1]).unwrap();
        assert!(read_cached(&changed).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hero_scale_covers_the_banner() {
        // The default window's banner at 1× and 2×, filled as `ObjectFit::Cover` does.
        assert_eq!(hero_scale_for(930., 560.), 0.4375);
        assert_eq!(hero_scale_for(1860., 1120.), HERO_MAX_SCALE);
        // A wide banner is covered across its width; exactly on a step stays on it.
        assert_eq!(hero_scale_for(1600., 300.), 0.5);
        assert_eq!(hero_scale_for(1400., 560.), 0.4375);
        assert_eq!(hero_scale_for(10., 10.), HERO_MIN_SCALE);
        assert_eq!(hero_scale_for(8000., 4000.), HERO_MAX_SCALE);
        let renderer = gpui::SvgRenderer::new(Arc::new(Assets));
        let hero = renderer
            .render_single_frame(Assets::embedded(HERO_PATH).unwrap(), 0.4375)
            .unwrap();
        assert_eq!(hero.size(0).width.0, 1400);
        assert_eq!(hero.size(0).height.0, 560);
    }

    #[test]
    fn the_hero_fitted_at_startup_is_loaded_next_time() {
        let dir = std::env::temp_dir().join(format!("lodestone-hero-cache-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let count = || std::fs::read_dir(&dir).unwrap().flatten().count();
        assert!(fitted_hero(&dir, 1.0).is_none());
        let renderer = gpui::SvgRenderer::new(Arc::new(Assets));
        let hero = |scale, saved| render_cached(&renderer, Some(&dir), HERO_PATH, scale, saved);
        // The first start renders the largest; nothing was fitted yet.
        let large = hero(HERO_MAX_SCALE, Saved::Yes).unwrap();
        assert!(fitted_hero(&dir, 1.0).is_none());
        // Fitted at 1×, and grown later in that session, which isn't kept.
        let small = hero(0.25, Saved::Yes).unwrap();
        save_hero_fit(&dir, 0.25, 1.0);
        hero(0.5, Saved::No).unwrap();
        assert_ne!(large.size(0), small.size(0));
        let (scale, cached) = fitted_hero(&dir, 1.0).unwrap();
        assert_eq!(scale, 0.25);
        assert_eq!(cached.size(0), small.size(0));
        // The largest render stays, for a denser display, whose start doesn't use the
        // 1× fit; a less dense one does.
        assert!(fitted_hero(&dir, 1.5).is_none());
        assert!(fitted_hero(&dir, 0.75).is_some());
        assert_eq!(count(), 3);
        // Fitted again at another size: the old fit goes, the largest stays.
        hero(0.375, Saved::Yes).unwrap();
        save_hero_fit(&dir, 0.375, 1.5);
        assert_eq!(fitted_hero(&dir, 1.5).unwrap().0, 0.375);
        assert_eq!(count(), 3);
        // Other pictures' renders aren't touched.
        render_cached(
            &renderer,
            Some(&dir),
            MARK_PATH,
            MARK_SMALL_SCALE,
            Saved::Yes,
        )
        .unwrap();
        save_hero_fit(&dir, 0.375, 1.5);
        assert_eq!(count(), 4);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn decodes_a_skin_head() {
        use launcher_core::skin;
        let skin_png = skin::encode_png(&skin::sample_skin(true)).unwrap();
        let head = skin::render_head(&skin_png, skin::HEAD_SCALE).unwrap();
        let image = decode_png(&skin::encode_png(&head).unwrap()).unwrap();
        assert_eq!(image.size(0).width.0, 64);
        assert!(decode_png(b"not a png").is_err());
    }
}
