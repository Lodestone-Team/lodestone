//! Materials: surfaces that let what is behind them show through, as it is,
//! blurred, or frosted ([`Material`]).
//!
//! GPUI has no backdrop filter, so a blurred surface draws what is behind it
//! itself. What is behind is worked out from the skin ([`under`]): the nearest layer
//! that hides what is under it (the window's, the page's or a dialog's picture or
//! gradient, or a plain colour), and the see-through layers between (a page at 40%,
//! the dimmer behind a dialog), drawn over it as colours.
//!
//! A layer's picture is blurred once, small, when the skin is prepared
//! ([`blur_source`]). Each frame, the layers that pictures are taken from record
//! where they are ([`recorder`]), and a blurred surface draws its part of the
//! blurred copy lined up with the layer ([`layer`]), so it stays right while the
//! page scrolls, a dialog slides in or the window is resized. Nothing is blurred
//! while the window is drawn.

use std::{cell::RefCell, collections::BTreeSet, sync::Arc};

use gpui::{
    AbsoluteLength, AnyElement, Bounds, Corners, CornersRefinement, Hsla, IntoElement,
    ParentElement, Pixels, RenderImage, Styled, Window, canvas, div, point, px, size,
};
use image::RgbaImage;
use launcher_core::ui_skin::{Fill, Material, Rgb, SkinSpec, SurfaceId, readable::DIALOG_SCRIM};

use crate::theme::Color;

/// What a see-through layer stands on: a colour, or a layer's picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Under {
    Color(Rgb),
    /// The picture (image or gradient) of a layer.
    Layer(SurfaceId),
}

/// What is visible at `layer`: the layer that hides what is under it, and the
/// see-through colours over it, bottom first, pushed onto `overlays`.
pub fn under(spec: &SkinSpec, layer: SurfaceId, overlays: &mut Vec<(Rgb, f32)>) -> Under {
    let own = spec.surface(layer).unwrap_or_default();
    // what is behind `layer`, with the dimmer between a dialog and the window
    let through = |overlays: &mut Vec<(Rgb, f32)>| -> Option<Under> {
        let behind = layer.behind()?;
        let found = under(spec, behind, overlays);
        if layer == SurfaceId::Dialog {
            overlays.push(DIALOG_SCRIM);
        }
        Some(found)
    };
    let see_through =
        |color: Rgb, alpha: f32, overlays: &mut Vec<(Rgb, f32)>| match through(overlays) {
            Some(found) => {
                overlays.push((color, alpha));
                found
            }
            None => Under::Color(color),
        };
    match (&own.fill, own.material) {
        // the page paints nothing of its own: the window shows
        (None, None) if layer == SurfaceId::Main => {
            through(overlays).unwrap_or(Under::Color(layer.base()))
        }
        (None, None) => Under::Color(layer.base()),
        (Some(Fill::Solid { color, opacity }), None) if *opacity < 100 => {
            see_through(*color, f32::from(*opacity) / 100.0, overlays)
        }
        (Some(Fill::Solid { color, .. }), None) => Under::Color(*color),
        (Some(_), None) => Under::Layer(layer),
        // a see-through layer: what is behind it, under its fill (a picture is taken
        // as it is; its own blur isn't repeated)
        (fill, Some(material)) => {
            let alpha = f32::from(material.opacity(fill.as_ref())) / 100.0;
            match fill {
                Some(Fill::Solid { color, .. }) => see_through(*color, alpha, overlays),
                None => see_through(layer.base(), alpha, overlays),
                Some(_) => Under::Layer(layer),
            }
        }
    }
}

/// A material, what it stands on, and the see-through colours between.
type Behind = (Material, Under, Vec<(Rgb, f32)>);

/// What is behind `surface` for its material: `None` when it has none, or one that
/// lets the window show through as it is.
fn behind(spec: &SkinSpec, surface: SurfaceId) -> Option<Behind> {
    let material = spec.surface(surface)?.material?;
    if !material.samples_behind() {
        return None;
    }
    let mut overlays = Vec::new();
    let found = under(spec, surface.behind()?, &mut overlays);
    if surface == SurfaceId::Dialog {
        overlays.push(DIALOG_SCRIM);
    }
    Some((material, found, overlays))
}

/// The layers' pictures the skin's materials need, each with its blur.
pub fn sources(spec: &SkinSpec) -> BTreeSet<(SurfaceId, u8)> {
    SurfaceId::ALL
        .into_iter()
        .filter_map(|surface| match behind(spec, surface)? {
            (material, Under::Layer(layer), _) => Some((layer, material.blur())),
            _ => None,
        })
        .collect()
}

/// How wide each layer usually is on screen, in pixels, to turn a material's blur
/// into pixels of the picture.
fn span(layer: SurfaceId) -> f32 {
    match layer {
        SurfaceId::Dialog => 900.0,
        SurfaceId::Main => 1100.0,
        _ => 1400.0,
    }
}

/// `image`, the picture of `layer` as it is drawn, blurred by `blur` screen pixels.
/// It is made small (blurred pictures are smooth) and stretched when drawn.
pub fn blur_source(image: &RgbaImage, blur: u8, layer: SurfaceId) -> RgbaImage {
    let width = (image.width() / 4).clamp(16, 480);
    let height = ((image.height() as f32 * width as f32 / image.width().max(1) as f32).round()
        as u32)
        .max(16);
    let small =
        image::imageops::resize(image, width, height, image::imageops::FilterType::Triangle);
    let sigma = f32::from(blur) * width as f32 / span(layer);
    if sigma < 0.3 {
        return small;
    }
    image::imageops::blur(&small, sigma)
}

/// A see-through surface's backdrop, ready to draw.
#[derive(Clone)]
pub struct Glass {
    under: Base,
    /// See-through colours over it, bottom first.
    overlays: Vec<Hsla>,
    /// The pale veil, 0 to 1.
    frost: f32,
    /// The grain, 0 to 1.
    grain: f32,
}

#[derive(Clone)]
enum Base {
    Color(Hsla),
    Layer {
        layer: SurfaceId,
        blur: u8,
        /// `None` while it is being prepared.
        image: Option<Arc<RenderImage>>,
        /// Whether the layer's picture is stretched over it (a gradient GPUI draws)
        /// rather than covering it.
        stretch: bool,
        /// Shown while the picture is being prepared.
        stand_in: Hsla,
    },
}

fn hsla(color: Rgb, alpha: f32) -> Hsla {
    Hsla::from(Color::rgb(color.0)).opacity(alpha)
}

impl Glass {
    /// The backdrop of `surface`, if its material blurs what is behind it. Its
    /// picture is added when it is prepared ([`Glass::with_image`]).
    pub fn of(spec: &SkinSpec, surface: SurfaceId) -> Option<Glass> {
        let (material, found, overlays) = behind(spec, surface)?;
        let under = match found {
            Under::Color(color) => Base::Color(hsla(color, 1.0)),
            Under::Layer(layer) => {
                let fill = spec.surface(layer).and_then(|s| s.fill);
                Base::Layer {
                    layer,
                    blur: material.blur(),
                    image: None,
                    stretch: matches!(&fill, Some(Fill::Gradient(g)) if g.is_simple()),
                    stand_in: hsla(
                        launcher_core::ui_skin::readable::backdrop_of(
                            fill.as_ref(),
                            layer.base(),
                            None,
                        )
                        .mean,
                        1.0,
                    ),
                }
            }
        };
        Some(Glass {
            under,
            overlays: overlays
                .into_iter()
                .map(|(color, alpha)| hsla(color, alpha))
                .collect(),
            frost: f32::from(material.frost()) / 100.0,
            grain: f32::from(material.grain()) / 100.0,
        })
    }

    /// The layer picture it needs, with its blur, if it needs one.
    pub fn source(&self) -> Option<(SurfaceId, u8)> {
        match &self.under {
            Base::Layer { layer, blur, .. } => Some((*layer, *blur)),
            Base::Color(_) => None,
        }
    }

    /// Gives it its prepared picture.
    pub fn with_image(&mut self, prepared: Arc<RenderImage>) {
        if let Base::Layer { image, .. } = &mut self.under {
            *image = Some(prepared);
        }
    }
}

thread_local! {
    /// Where the layers pictures are taken from are in this frame, as they were laid
    /// out.
    static LAYERS: RefCell<Vec<(SurfaceId, Bounds<Pixels>)>> = const { RefCell::new(Vec::new()) };
    static GRAIN: Arc<RenderImage> = crate::images::render_image(grain_texture());
}

/// Forgets the layers of the last frame; each window's root view calls it first.
pub fn start_frame() {
    LAYERS.with(|layers| layers.borrow_mut().clear());
}

/// An element that records where `layer` is in this frame, for the surfaces that
/// show it through themselves.
pub fn recorder(layer: SurfaceId) -> AnyElement {
    canvas(
        move |bounds, _, _| LAYERS.with(|layers| layers.borrow_mut().push((layer, bounds))),
        |_, _, _, _| {},
    )
    .absolute()
    .inset_0()
    .size_full()
    .into_any_element()
}

/// Where `layer` is around `at`: the last of its kind recorded there, else the
/// window, else the whole window's area.
fn layer_bounds(layer: SurfaceId, at: gpui::Point<Pixels>, window: &Window) -> Bounds<Pixels> {
    let found = LAYERS.with(|layers| {
        let layers = layers.borrow();
        let find = |id: SurfaceId| {
            layers
                .iter()
                .rev()
                .find(|(l, b)| *l == id && b.contains(&at))
                .map(|(_, b)| *b)
        };
        find(layer).or_else(|| find(SurfaceId::Window))
    });
    found.unwrap_or(Bounds {
        origin: point(px(0.), px(0.)),
        size: window.viewport_size(),
    })
}

/// Where a picture of `image_size` lands when it covers `area`, as GPUI's
/// `ObjectFit::Cover` draws it.
fn cover(image_size: gpui::Size<gpui::DevicePixels>, area: Bounds<Pixels>) -> Bounds<Pixels> {
    let (iw, ih) = (
        image_size.width.0.max(1) as f32,
        image_size.height.0.max(1) as f32,
    );
    let (aw, ah) = (f32::from(area.size.width), f32::from(area.size.height));
    let scale = (aw / iw).max(ah / ih);
    let (w, h) = (iw * scale, ih * scale);
    Bounds {
        origin: point(
            area.origin.x + px((aw - w) / 2.0),
            area.origin.y + px((ah - h) / 2.0),
        ),
        size: size(px(w), px(h)),
    }
}

fn radii(corners: &CornersRefinement<AbsoluteLength>, window: &Window) -> Corners<Pixels> {
    let rem = window.rem_size();
    let at = |c: Option<AbsoluteLength>| c.map_or(px(0.), |c| c.to_pixels(rem));
    Corners {
        top_left: at(corners.top_left),
        top_right: at(corners.top_right),
        bottom_right: at(corners.bottom_right),
        bottom_left: at(corners.bottom_left),
    }
}

/// The layer that draws `glass` under an element with `corners`: what is behind,
/// blurred, the see-through colours, the veil and the grain. It goes in first,
/// under the element's fill and children.
pub fn layer(glass: Glass, corners: CornersRefinement<AbsoluteLength>) -> AnyElement {
    let grain = glass.grain;
    let grain_corners = corners.clone();
    div()
        .absolute()
        .inset_0()
        .size_full()
        .child(
            canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    let radii = radii(&corners, window);
                    match &glass.under {
                        Base::Color(color) => {
                            window.paint_quad(gpui::fill(bounds, *color).corner_radii(radii))
                        }
                        Base::Layer {
                            layer,
                            image: Some(image),
                            stretch,
                            ..
                        } => {
                            let area = layer_bounds(*layer, bounds.center(), window);
                            let image_bounds = if *stretch {
                                area
                            } else {
                                cover(image.size(0), area)
                            };
                            if let Err(err) = window.paint_image(
                                bounds,
                                image_bounds,
                                radii,
                                image.clone(),
                                0,
                                false,
                            ) {
                                log::debug!("glass backdrop not drawn: {err}");
                            }
                        }
                        Base::Layer { stand_in, .. } => {
                            window.paint_quad(gpui::fill(bounds, *stand_in).corner_radii(radii))
                        }
                    }
                    for overlay in &glass.overlays {
                        window.paint_quad(gpui::fill(bounds, *overlay).corner_radii(radii));
                    }
                    if glass.frost > 0.0 {
                        let veil = gpui::white().opacity(glass.frost);
                        window.paint_quad(gpui::fill(bounds, veil).corner_radii(radii));
                    }
                },
            )
            .absolute()
            .inset_0()
            .size_full(),
        )
        .when(grain > 0.0, |this| {
            this.child(
                div().absolute().inset_0().size_full().opacity(grain).child(
                    canvas(
                        |_, _, _| {},
                        move |bounds, _, window, _| {
                            let radii = radii(&grain_corners, window);
                            // fixed to the window, so it doesn't swim as things move
                            let area = layer_bounds(SurfaceId::Window, bounds.center(), window);
                            let texture = GRAIN.with(Arc::clone);
                            let image_bounds = cover(texture.size(0), area);
                            if let Err(err) =
                                window.paint_image(bounds, image_bounds, radii, texture, 0, false)
                            {
                                log::debug!("glass grain not drawn: {err}");
                            }
                        },
                    )
                    .absolute()
                    .inset_0()
                    .size_full(),
                ),
            )
        })
        .into_any_element()
}

/// Fine grain: light and dark specks, faint, over a window-sized area. Made once.
fn grain_texture() -> RgbaImage {
    let (width, height) = (1200u32, 750u32);
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    RgbaImage::from_fn(width, height, |_, _| {
        // xorshift: the same specks every time
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let light = state & 1 == 0;
        let alpha = ((state >> 8) & 0x3F) as u8;
        let v = if light { 255 } else { 0 };
        image::Rgba([v, v, v, alpha])
    })
}

use gpui::prelude::FluentBuilder as _;

#[cfg(test)]
mod tests {
    use launcher_core::ui_skin::{MaterialKind, SurfaceSpec};

    use super::*;

    fn spec(surfaces: &[(SurfaceId, SurfaceSpec)]) -> SkinSpec {
        SkinSpec {
            surfaces: surfaces.iter().cloned().collect(),
            ..SkinSpec::default()
        }
    }

    fn image_fill() -> Fill {
        Fill::Image(launcher_core::ui_skin::ImageFill {
            image: "back.png".into(),
            fit: launcher_core::ui_skin::Fit::Cover,
            tint: None,
            tint_opacity: 0,
            blur: 0,
        })
    }

    fn with_material(kind: MaterialKind) -> SurfaceSpec {
        SurfaceSpec {
            material: Some(Material::of(kind)),
            ..SurfaceSpec::default()
        }
    }

    #[test]
    fn a_card_sees_the_window_through_a_see_through_page() {
        let spec = spec(&[
            (
                SurfaceId::Window,
                SurfaceSpec {
                    fill: Some(image_fill()),
                    ..SurfaceSpec::default()
                },
            ),
            (
                SurfaceId::Main,
                SurfaceSpec {
                    fill: Some(Fill::Solid {
                        color: Rgb(0x101020),
                        opacity: 40,
                    }),
                    ..SurfaceSpec::default()
                },
            ),
            (SurfaceId::Card, with_material(MaterialKind::Frost)),
        ]);
        let (_, found, overlays) = behind(&spec, SurfaceId::Card).unwrap();
        assert_eq!(found, Under::Layer(SurfaceId::Window));
        assert_eq!(overlays, vec![(Rgb(0x101020), 0.4)]);
        assert_eq!(
            sources(&spec),
            BTreeSet::from([(SurfaceId::Window, MaterialKind::Frost.default_blur())])
        );
    }

    #[test]
    fn an_opaque_page_hides_the_window() {
        let spec = spec(&[
            (
                SurfaceId::Window,
                SurfaceSpec {
                    fill: Some(image_fill()),
                    ..SurfaceSpec::default()
                },
            ),
            (
                SurfaceId::Main,
                SurfaceSpec {
                    fill: Some(Fill::solid(Rgb(0x202020))),
                    ..SurfaceSpec::default()
                },
            ),
            (SurfaceId::Card, with_material(MaterialKind::Blur)),
        ]);
        let (_, found, overlays) = behind(&spec, SurfaceId::Card).unwrap();
        assert_eq!(found, Under::Color(Rgb(0x202020)));
        assert!(overlays.is_empty());
        assert!(sources(&spec).is_empty());
    }

    #[test]
    fn dialogs_and_their_cards_are_dimmed() {
        let spec = spec(&[
            (
                SurfaceId::Window,
                SurfaceSpec {
                    fill: Some(image_fill()),
                    ..SurfaceSpec::default()
                },
            ),
            (SurfaceId::Dialog, with_material(MaterialKind::Blur)),
            (SurfaceId::SettingCard, with_material(MaterialKind::Frost)),
        ]);
        let (_, found, overlays) = behind(&spec, SurfaceId::Dialog).unwrap();
        assert_eq!(found, Under::Layer(SurfaceId::Window));
        assert_eq!(overlays, vec![DIALOG_SCRIM]);
        // the card sees the window, dimmed, under the dialog's own colour
        let (_, found, overlays) = behind(&spec, SurfaceId::SettingCard).unwrap();
        assert_eq!(found, Under::Layer(SurfaceId::Window));
        assert_eq!(overlays.len(), 2);
        assert_eq!(overlays[0], DIALOG_SCRIM);
    }

    #[test]
    fn translucent_draws_nothing_behind() {
        let spec = spec(&[(SurfaceId::TopBar, with_material(MaterialKind::Translucent))]);
        assert!(Glass::of(&spec, SurfaceId::TopBar).is_none());
    }

    #[test]
    fn blurred_copies_are_small() {
        let image = RgbaImage::from_pixel(1600, 1000, image::Rgba([10, 20, 30, 255]));
        let blurred = blur_source(&image, 20, SurfaceId::Window);
        assert_eq!((blurred.width(), blurred.height()), (400, 250));
        assert_eq!(blurred.get_pixel(200, 120).0, [10, 20, 30, 255]);
    }
}
