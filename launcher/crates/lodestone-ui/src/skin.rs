//! Painting elements as a skin paints its surfaces: the part of the launcher's skins
//! the shared components need.
//!
//! An app resolves its skin (the launcher's `skin` module) into a [`Painted`] for
//! each [`SurfaceId`] it paints, and hands it to [`set_frame`] before each window
//! draws. Elements then paint themselves with [`Skinned::surface`]. An app without
//! skins (Lodestone Host) sets no frame, and every surface keeps its own look.

use std::{cell::RefCell, sync::Arc};

use gpui::RenderImage;
pub use launcher_core::ui_skin::SurfaceId;

use crate::theme::Color;

pub mod glass;
pub use glass::Glass;

/// How a surface is filled, ready to draw.
#[derive(Clone)]
pub enum Paint {
    Solid(Color),
    /// A two-colour linear gradient GPUI draws itself.
    Gradient(gpui::Background),
    /// An image prepared at the surface's shape (tiled, blurred, tinted), or a
    /// gradient GPUI can't draw, drawn once into an image.
    Image(Arc<RenderImage>),
}

/// A surface as the skin paints it. What is `None` stays the launcher's.
#[derive(Clone, Default)]
pub struct Painted {
    /// `None` also while its image is being prepared.
    pub paint: Option<Paint>,
    pub border: Option<(Color, f32)>,
    pub radius: Option<f32>,
    pub text: Option<Color>,
    pub muted: Option<Color>,
    /// What is behind it, blurred or frosted, drawn under its fill (a material).
    pub glass: Option<Glass>,
    /// How much of an image fill shows, 0 to 1, when a material makes it
    /// see-through; `None` for all of it.
    pub see_through: Option<f32>,
    /// Whether it is a see-through material at all: the element's own background
    /// is cleared, so what is behind shows.
    pub material: bool,
    /// The layer it is, when blurred surfaces may show it ([`glass::recorder`]).
    pub records: Option<SurfaceId>,
}

/// What a frame paints: the app's skin, resolved.
pub trait SkinFrame {
    /// How the skin paints `surface`, if it does.
    fn painted(&self, surface: SurfaceId) -> Option<Painted>;
}

thread_local! {
    /// The skin as this frame draws it, set by each window's root view.
    static FRAME: RefCell<Option<Arc<dyn SkinFrame>>> = const { RefCell::new(None) };
}

/// Sets the skin this frame draws (`None` for none). Each window's root view calls it
/// before it draws.
pub fn set_frame(frame: Option<Arc<dyn SkinFrame>>) {
    FRAME.with(|cell| *cell.borrow_mut() = frame);
    glass::start_frame();
}

/// How the skin paints `surface` in this frame, if it does.
pub fn painted(surface: SurfaceId) -> Option<Painted> {
    FRAME.with(|frame| {
        frame
            .borrow()
            .as_ref()
            .and_then(|look| look.painted(surface))
    })
}

/// Painting an element as the skin paints one of its surfaces.
///
/// Call it before adding the element's children: an image fill goes in as the first
/// child, under everything else.
pub trait Skinned: gpui::Styled + gpui::ParentElement + Sized {
    /// Paints the element as `surface`, when the skin paints it: its fill, border,
    /// corners and text colour. Otherwise leaves it as it is.
    fn surface(self, surface: SurfaceId) -> Self {
        match painted(surface) {
            Some(painted) => paint(self, &painted),
            None => self,
        }
    }
}

impl<T: gpui::Styled + gpui::ParentElement + Sized> Skinned for T {}

/// `element` painted as `painted` says.
pub fn paint<T: gpui::Styled + gpui::ParentElement>(mut element: T, painted: &Painted) -> T {
    use gpui::{IntoElement, ObjectFit, ParentElement as _, Styled as _, StyledImage, img};
    if let Some(radius) = painted.radius {
        element = element.rounded(crate::theme::px(radius));
    }
    if let Some((color, width)) = painted.border {
        let width: gpui::AbsoluteLength = crate::theme::px(width).into();
        let style = element.style();
        style.border_widths.top = Some(width);
        style.border_widths.right = Some(width);
        style.border_widths.bottom = Some(width);
        style.border_widths.left = Some(width);
        element = element.border_color(color);
    }
    if let Some(text) = painted.text {
        element = element.text_color(text);
    }
    let corners = element.style().corner_radii.clone();
    if let Some(layer) = painted.records {
        element = element.child(glass::recorder(layer));
    }
    // a see-through surface drops the launcher's own background, so what is behind
    // shows (or its blurred copy, drawn first)
    if painted.material {
        element.style().background = None;
    }
    if let Some(glass) = &painted.glass {
        element = element.child(glass::layer(glass.clone(), corners.clone()));
    }
    // over the glass, the fill is a layer of its own: the element's background
    // would be drawn under it
    let layered = painted.glass.is_some();
    let image_layer = |image: &Arc<RenderImage>| {
        let mut layer = img(image.clone())
            .absolute()
            .inset_0()
            .size_full()
            .object_fit(ObjectFit::Cover);
        // `overflow_hidden` clips to a rectangle, not to rounded corners, so the
        // image takes the element's corners (set before this is called).
        layer.style().corner_radii = corners.clone();
        match painted.see_through {
            Some(alpha) => gpui::div()
                .absolute()
                .inset_0()
                .size_full()
                .opacity(alpha)
                .child(layer)
                .into_any_element(),
            None => layer.into_any_element(),
        }
    };
    let color_layer = |background: gpui::Background| {
        let mut layer = gpui::div().absolute().inset_0().size_full().bg(background);
        layer.style().corner_radii = corners.clone();
        layer.into_any_element()
    };
    match &painted.paint {
        Some(Paint::Solid(color)) if layered => element.child(color_layer((*color).into())),
        Some(Paint::Gradient(gradient)) if layered => element.child(color_layer(*gradient)),
        Some(Paint::Solid(color)) => element.bg(*color),
        Some(Paint::Gradient(gradient)) => element.bg(*gradient),
        Some(Paint::Image(image)) => element.overflow_hidden().child(image_layer(image)),
        None => element,
    }
}
