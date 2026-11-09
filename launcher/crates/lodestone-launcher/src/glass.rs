//! See-through windows: Settings > General > Transparency, or a skin's `material`.
//!
//! The desktop behind the window shows through it: sharp ([`Material::Clear`]) or
//! blurred by the system's compositor ([`Material::Frosted`], [`Material::Glass`]),
//! which costs the launcher nothing per frame. The surfaces drawn over the window
//! ([`SurfaceId`]) lose some of their fill so it shows through them too: the window
//! as much as Opacity says, the panels on it (sidebar, top bar, Play bar) a little
//! more, and dialogs and popovers, which sit over the launcher's own content, little.
//! Glass also lights the top edge of panels and cards.
//!
//! GPUI can't blur what the window itself draws under a panel, so nothing blurs
//! the launcher's content. Where the skin fills the window with an image, a blurred
//! copy of it, made once when the image is prepared, lies under the panels and
//! dialogs instead ([`backdrop`]): they look frosted over the skin's image.
//!
//! Each window's root view resolves the material for its frame ([`set_frame`]) and
//! asks the system for the matching background ([`apply_to_window`]).

use std::{cell::Cell, cell::RefCell, collections::HashMap, sync::Arc};

use gpui::{
    AbsoluteLength, Corners, CornersRefinement, Hsla, IntoElement, ParentElement, RenderImage,
    Styled, Window, WindowBackgroundAppearance, WindowId, canvas, div,
};
use launcher_core::ui_skin::{DEFAULT_OPACITY, MIN_OPACITY, Material, SkinSpec, SurfaceId};

use crate::theme::colors;

/// What shows through the window in this frame, and how opaque it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glass {
    pub material: Material,
    /// 0.2 to 1.0.
    pub opacity: f32,
}

impl Glass {
    pub const OPAQUE: Glass = Glass {
        material: Material::Opaque,
        opacity: 1.0,
    };

    pub fn is_see_through(self) -> bool {
        self.material.is_see_through()
    }
}

thread_local! {
    /// Settings > General > Transparency and Opacity; `None` follows the skin.
    static USER: Cell<(Option<Material>, Option<u8>)> = const { Cell::new((None, None)) };
    /// The material as the window being drawn uses it.
    static FRAME: Cell<Glass> = const { Cell::new(Glass::OPAQUE) };
    /// What each window was last asked to show behind it.
    static APPEARANCE: RefCell<HashMap<WindowId, WindowBackgroundAppearance>> =
        RefCell::new(HashMap::new());
}

/// Takes the settings' Transparency and Opacity (`None` follows the skin).
pub fn set_user(material: Option<Material>, opacity: Option<u8>) {
    USER.with(|user| user.set((material, opacity)));
}

/// The settings' Transparency and Opacity, as [`set_user`] last took them.
pub fn user() -> (Option<Material>, Option<u8>) {
    USER.with(Cell::get)
}

/// The material for a window showing skin `spec`: the settings' choice, else the
/// skin's, else opaque.
pub fn resolve(spec: &SkinSpec) -> Glass {
    let (material, opacity) = user();
    let material = material.or(spec.material).unwrap_or_default();
    if !material.is_see_through() {
        return Glass::OPAQUE;
    }
    let percent = opacity
        .or(spec.opacity)
        .unwrap_or(DEFAULT_OPACITY)
        .clamp(MIN_OPACITY, 100);
    Glass {
        material,
        opacity: f32::from(percent) / 100.0,
    }
}

/// Sets the material for the frame being drawn. Each window's root view calls it,
/// after [`crate::skin::set_frame`].
pub fn set_frame(glass: Glass) {
    FRAME.with(|frame| frame.set(glass));
}

/// The material of the frame being drawn.
pub fn frame() -> Glass {
    FRAME.with(Cell::get)
}

/// The system background a window takes for `material`.
pub fn appearance(material: Material) -> WindowBackgroundAppearance {
    match material {
        Material::Opaque => WindowBackgroundAppearance::Opaque,
        Material::Clear => WindowBackgroundAppearance::Transparent,
        Material::Frosted | Material::Glass => WindowBackgroundAppearance::Blurred,
    }
}

/// Asks the system for `window`'s background under `glass`, when it changed. It
/// takes effect right away; nothing is drawn per frame for it.
pub fn apply_to_window(window: &mut Window, glass: Glass) {
    let wanted = appearance(glass.material);
    let id = window.window_handle().window_id();
    let changed = APPEARANCE.with(|shown| {
        let mut shown = shown.borrow_mut();
        let before = shown
            .get(&id)
            .copied()
            .unwrap_or(WindowBackgroundAppearance::Opaque);
        shown.insert(id, wanted);
        before != wanted
    });
    if changed {
        window.set_background_appearance(wanted);
    }
}

/// How a surface sits in the window, which decides how much of it shows through.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The window itself.
    Base,
    /// Panels laid on the window: the sidebar, the top bar, the Play bar.
    Pane,
    /// Cards, rows and fields on a page.
    Content,
    /// Dialogs, popovers and notices over the launcher's own content.
    Floating,
}

/// The role of `surface`; `None` for controls (buttons, toggles, the Play button),
/// which keep their fill.
pub fn role(surface: SurfaceId) -> Option<Role> {
    match surface {
        SurfaceId::Window => Some(Role::Base),
        SurfaceId::Main
        | SurfaceId::Sidebar
        | SurfaceId::TopBar
        | SurfaceId::PlayBar
        | SurfaceId::SettingsNav => Some(Role::Pane),
        SurfaceId::Card
        | SurfaceId::SettingCard
        | SurfaceId::Row
        | SurfaceId::Input
        | SurfaceId::Hero
        | SurfaceId::Pill
        | SurfaceId::Tab => Some(Role::Content),
        SurfaceId::Dialog | SurfaceId::Popover | SurfaceId::Toast => Some(Role::Floating),
        SurfaceId::ButtonPrimary
        | SurfaceId::ButtonSecondary
        | SurfaceId::ButtonGhost
        | SurfaceId::ButtonDanger
        | SurfaceId::Toggle
        | SurfaceId::PlayButton
        | SurfaceId::Progress => None,
    }
}

/// How much of `surface`'s fill is kept under `glass`: `None` keeps all of it.
/// `own` is the skin's `opacity` for the surface, in percent; `image` says the fill
/// is an image, which fades with the window rather than with its role, and
/// `backdrop` that a blurred copy of the skin's image lies under it.
pub fn alpha(
    glass: Glass,
    surface: SurfaceId,
    own: Option<u8>,
    image: bool,
    backdrop: bool,
) -> Option<f32> {
    if !glass.is_see_through() {
        return None;
    }
    if let Some(own) = own {
        return Some(f32::from(own.min(100)) / 100.0);
    }
    let f = glass.opacity;
    let role = role(surface)?;
    Some(match role {
        Role::Base => f,
        Role::Pane if image => f,
        // over the window's own tint, so they read as a shade darker
        Role::Pane => 0.4 * f,
        Role::Content => 0.6 * f,
        // over the launcher's content, which isn't blurred: mostly solid
        Role::Floating if backdrop => 0.55 + 0.35 * f,
        Role::Floating => 0.85 + 0.1 * f,
    })
}

/// Whether Glass lights `surface`'s top edge.
pub fn lit(glass: Glass, surface: SurfaceId) -> bool {
    glass.material == Material::Glass
        && match role(surface) {
            Some(Role::Pane) => surface != SurfaceId::Main,
            Some(Role::Content) => matches!(
                surface,
                SurfaceId::Card | SurfaceId::SettingCard | SurfaceId::Input | SurfaceId::Hero
            ),
            Some(Role::Floating) => true,
            _ => false,
        }
}

/// Whether a blurred copy of the skin's window image goes under `surface`.
pub fn takes_backdrop(glass: Glass, surface: SurfaceId) -> bool {
    glass.material.is_blurred()
        && match role(surface) {
            Some(Role::Pane) => surface != SurfaceId::Main,
            Some(Role::Floating) => true,
            _ => false,
        }
}

/// The light along a glass surface's top edge, with a faint sheen fading down from
/// it. Rounded as `radii` (the surface's corners), since children are clipped to a
/// rectangle.
pub fn highlight(radii: CornersRefinement<AbsoluteLength>) -> impl IntoElement {
    let white: Hsla = colors::WHITE.into();
    let mut layer = div()
        .absolute()
        .inset_0()
        .border_t_1()
        .border_color(white.opacity(0.16))
        .bg(gpui::linear_gradient(
            180.,
            gpui::linear_color_stop(white.opacity(0.06), 0.),
            gpui::linear_color_stop(white.opacity(0.0), 0.6),
        ));
    layer.style().corner_radii = radii;
    layer
}

/// The edge Glass gives a surface's own border.
pub fn edge() -> Hsla {
    Hsla::from(colors::WHITE).opacity(0.12)
}

/// A blurred copy of the skin's window image, laid where the window shows it, seen
/// through the element it is put in (as its first child). `opacity` is how much of
/// it covers the desktop behind; `radii` are the element's corners.
pub fn backdrop(
    image: Arc<RenderImage>,
    opacity: f32,
    radii: CornersRefinement<AbsoluteLength>,
) -> impl IntoElement {
    let paint = move |bounds: gpui::Bounds<gpui::Pixels>,
                      _: (),
                      window: &mut Window,
                      _: &mut gpui::App| {
        let rem = window.rem_size();
        let corner = |r: Option<AbsoluteLength>| r.map_or(gpui::Pixels::ZERO, |r| r.to_pixels(rem));
        let radii = Corners {
            top_left: corner(radii.top_left),
            top_right: corner(radii.top_right),
            bottom_right: corner(radii.bottom_right),
            bottom_left: corner(radii.bottom_left),
        };
        let content = crate::components::window_frame::content_bounds(window);
        let size = image.size(0);
        let image_bounds = cover(
            content,
            gpui::size(
                gpui::px(size.width.0 as f32),
                gpui::px(size.height.0 as f32),
            ),
        );
        window
            .paint_image(bounds, image_bounds, radii, image.clone(), 0, false)
            .ok();
    };
    // A div's opacity carries to what it paints; a canvas has none of its own.
    div()
        .absolute()
        .inset_0()
        .opacity(opacity)
        .child(canvas(|_, _, _| (), paint).size_full())
}

/// Where an image of `image` size lies when it covers `area`, centred.
pub fn cover(
    area: gpui::Bounds<gpui::Pixels>,
    image: gpui::Size<gpui::Pixels>,
) -> gpui::Bounds<gpui::Pixels> {
    if image.width <= gpui::Pixels::ZERO || image.height <= gpui::Pixels::ZERO {
        return area;
    }
    let scale = (area.size.width / image.width).max(area.size.height / image.height);
    let size = gpui::size(image.width * scale, image.height * scale);
    gpui::Bounds::new(
        gpui::point(
            area.origin.x + (area.size.width - size.width) / 2.,
            area.origin.y + (area.size.height - size.height) / 2.,
        ),
        size,
    )
}

/// The blurred copy [`backdrop`] shows: small (it is soft anyway) and blurred once,
/// in the background, when the skin's image is prepared.
pub fn blurred_copy(image: &image::RgbaImage) -> image::RgbaImage {
    let (w, h) = image.dimensions();
    if w == 0 || h == 0 {
        return image.clone();
    }
    // a quarter of the window's image at most: the blur hides the difference
    let scale = (400.0 / w as f32).min(1.0);
    let small = image::imageops::resize(
        image,
        ((w as f32 * scale).round() as u32).max(1),
        ((h as f32 * scale).round() as u32).max(1),
        image::imageops::FilterType::Triangle,
    );
    image::imageops::fast_blur(&small, 8.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glass(material: Material, opacity: f32) -> Glass {
        Glass { material, opacity }
    }

    #[test]
    fn the_setting_wins_over_the_skin() {
        let mut spec = SkinSpec::default();
        set_user(None, None);
        assert_eq!(resolve(&spec), Glass::OPAQUE);
        spec.material = Some(Material::Frosted);
        spec.opacity = Some(40);
        assert_eq!(resolve(&spec), glass(Material::Frosted, 0.4));
        set_user(Some(Material::Opaque), None);
        assert_eq!(resolve(&spec), Glass::OPAQUE);
        set_user(Some(Material::Glass), Some(75));
        assert_eq!(resolve(&spec), glass(Material::Glass, 0.75));
        set_user(Some(Material::Clear), Some(5));
        assert_eq!(resolve(&spec).opacity, f32::from(MIN_OPACITY) / 100.0);
        set_user(None, None);
    }

    #[test]
    fn panels_show_more_than_dialogs() {
        let g = glass(Material::Frosted, 0.6);
        let of = |s| alpha(g, s, None, false, false).unwrap();
        assert!(of(SurfaceId::Sidebar) < of(SurfaceId::Window));
        assert!(of(SurfaceId::Card) < of(SurfaceId::Dialog));
        assert!(of(SurfaceId::Dialog) >= 0.85);
        assert_eq!(alpha(g, SurfaceId::ButtonPrimary, None, false, false), None);
        assert_eq!(alpha(g, SurfaceId::Card, Some(30), false, false), Some(0.3));
        assert_eq!(alpha(Glass::OPAQUE, SurfaceId::Window, None, false, false), None);
    }

    #[test]
    fn appearance_matches_the_material() {
        assert_eq!(
            appearance(Material::Opaque),
            WindowBackgroundAppearance::Opaque
        );
        assert_eq!(
            appearance(Material::Clear),
            WindowBackgroundAppearance::Transparent
        );
        assert_eq!(
            appearance(Material::Glass),
            WindowBackgroundAppearance::Blurred
        );
    }

    #[test]
    fn cover_fills_and_centres() {
        let area = gpui::Bounds::new(
            gpui::point(gpui::px(0.), gpui::px(0.)),
            gpui::size(gpui::px(1000.), gpui::px(500.)),
        );
        let laid = cover(area, gpui::size(gpui::px(400.), gpui::px(400.)));
        assert_eq!(laid.size.width, gpui::px(1000.));
        assert_eq!(laid.origin.y, gpui::px(-250.));
    }

    #[test]
    fn the_blurred_copy_is_small() {
        let image = image::RgbaImage::from_pixel(1600, 1000, image::Rgba([10, 20, 30, 255]));
        let copy = blurred_copy(&image);
        assert_eq!(copy.dimensions(), (400, 250));
    }
}
