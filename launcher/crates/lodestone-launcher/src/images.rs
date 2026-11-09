//! Images decoded before the window opens, so the first frame already has them.
//!
//! GPUI's `img()` loads an image on its background executor the first time it is
//! drawn, so the window appears without it and the image pops in once it has
//! been decoded. The bundled images are decoded here instead, and the window is
//! opened once they are ready. Skin heads, which are small, are decoded from the
//! on-disk cache when the account list is opened.

use std::{sync::Arc, time::Instant};

use gpui::{App, AppContext as _, Global, ImageSource, RenderImage, Task};

use crate::assets::Assets;

/// The bundled images that `img()` draws.
pub struct BundledImages {
    pub logo: Arc<RenderImage>,
    pub hero: Arc<RenderImage>,
}

impl Global for BundledImages {}

/// Adds the bundled images, for the performance monitor.
pub fn tally(tally: &mut crate::perf::ImageTally, cx: &App) {
    if let Some(images) = cx.try_global::<BundledImages>() {
        tally.add(&images.logo);
        tally.add(&images.hero);
    }
}

/// The scale passed to GPUI's SVG renderer, as its own `img()` loader does. The
/// renderer doubles it.
const SVG_SCALE: f32 = 1.0;
/// The hero's scale: its 1600×640 viewBox is rasterized at 2400×960 pixels, sharp at
/// 2× in the banner of a typical window (about 1000 points wide) while taking half
/// the memory of 3200×1280.
const HERO_SCALE: f32 = 0.75;

/// Starts decoding the bundled images on the background executor, in parallel.
pub fn decode(cx: &App) -> Task<anyhow::Result<BundledImages>> {
    let render = |path: &'static str, scale: f32| {
        let renderer = cx.svg_renderer();
        cx.background_spawn(async move {
            let bytes =
                Assets::embedded(path).ok_or_else(|| anyhow::anyhow!("{path} is not embedded"))?;
            renderer
                .render_single_frame(bytes, scale)
                .map_err(|err| anyhow::anyhow!("rendering {path}: {err}"))
        })
    };
    let logo = render(LOGO_PATH, SVG_SCALE);
    let hero = render(HERO_PATH, HERO_SCALE);
    cx.background_spawn(async move {
        Ok(BundledImages {
            logo: logo.await?,
            hero: hero.await?,
        })
    })
}

const LOGO_PATH: &str = "images/logo.svg";
const HERO_PATH: &str = "images/hero.svg";

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

/// A decoded RGBA image, in the BGRA layout GPUI draws.
pub fn render_image(mut image: image::RgbaImage) -> Arc<RenderImage> {
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new([image::Frame::new(image)]))
}

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
