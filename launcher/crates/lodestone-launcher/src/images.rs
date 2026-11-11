//! Images decoded before the window opens, so the first frame already has them.
//!
//! GPUI's `img()` loads an image on its background executor the first time it is
//! drawn, so the window appears without it and the image pops in once it has
//! been decoded. The bundled images are decoded here instead, and the window is
//! opened once they are ready. Their rendered pixels are kept in the data folder's
//! cache, so only the first start rasterizes them. Skin heads, which are small, are
//! decoded from the on-disk cache when the account list is opened.

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Instant,
};

use gpui::{App, AppContext as _, Global, ImageSource, RenderImage, Task};

use crate::assets::Assets;

/// The bundled images that `img()` draws.
pub struct BundledImages {
    pub logo: Arc<RenderImage>,
    pub hero: Arc<RenderImage>,
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
/// The hero's scale: its 1600×640 viewBox is rasterized at 2400×960 pixels, sharp at
/// 2× in the banner of a typical window (about 1000 points wide) while taking half
/// the memory of 3200×1280.
const HERO_SCALE: f32 = 0.75;
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
            let bytes =
                Assets::embedded(path).ok_or_else(|| anyhow::anyhow!("{path} is not embedded"))?;
            let file = cache
                .as_deref()
                .map(|dir| cache_file(dir, path, bytes, scale));
            if let Some(image) = file.as_deref().and_then(read_cached) {
                return Ok(image);
            }
            let image = renderer
                .render_single_frame(bytes, scale)
                .map_err(|err| anyhow::anyhow!("rendering {path}: {err}"))?;
            if let Some(file) = file {
                write_cached(&file, &image);
            }
            anyhow::Ok(image)
        })
    };
    let logo = render(LOGO_PATH, SVG_SCALE);
    let hero = render(HERO_PATH, HERO_SCALE);
    let mark_small = render(MARK_PATH, MARK_SMALL_SCALE);
    let mark_large = render(MARK_PATH, MARK_LARGE_SCALE);
    cx.background_spawn(async move {
        Ok(BundledImages {
            logo: logo.await?,
            hero: hero.await?,
            mark_small: mark_small.await?,
            mark_large: mark_large.await?,
        })
    })
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
            .render_single_frame(Assets::embedded(HERO_PATH).unwrap(), HERO_SCALE)
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
    fn decodes_a_skin_head() {
        use launcher_core::skin;
        let skin_png = skin::encode_png(&skin::sample_skin(true)).unwrap();
        let head = skin::render_head(&skin_png, skin::HEAD_SCALE).unwrap();
        let image = decode_png(&skin::encode_png(&head).unwrap()).unwrap();
        assert_eq!(image.size(0).width.0, 64);
        assert!(decode_png(b"not a png").is_err());
    }
}
