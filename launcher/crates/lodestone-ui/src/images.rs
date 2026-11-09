//! Images drawn from pixels.

use std::sync::Arc;

use gpui::RenderImage;

/// A decoded RGBA image, in the BGRA layout GPUI draws.
pub fn render_image(mut image: image::RgbaImage) -> Arc<RenderImage> {
    for pixel in image.pixels_mut() {
        pixel.0.swap(0, 2);
    }
    Arc::new(RenderImage::new([image::Frame::new(image)]))
}
