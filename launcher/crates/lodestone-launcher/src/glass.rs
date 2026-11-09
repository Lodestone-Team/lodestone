//! See-through windows: Settings > General > Transparency, or a skin's `transparency`.
//!
//! The desktop behind the window shows through it: sharp ([`Transparency::Clear`]) or
//! blurred by the system's compositor ([`Transparency::Frosted`], [`Transparency::Glass`]),
//! which costs the launcher nothing per frame. The surfaces drawn over the window
//! ([`SurfaceId`]) lose some of their fill so it shows through them too: the window
//! as much as Opacity says, the panels on it (sidebar, top bar, Play bar) a little
//! more, and dialogs and popovers, which sit over the launcher's own content, little.
//! Glass makes the floating layer Liquid Glass ([`crate::liquid`]).
//!
//! GPUI can't blur what the window itself draws under a panel, so nothing blurs
//! the launcher's content. Where the skin fills the window with an image, a blurred
//! copy of it, made once when the image is prepared, lies under the panels and
//! dialogs instead ([`backdrop`]): they look frosted over the skin's image.
//!
//! Each window's root view resolves the material for its frame ([`set_frame`]) and
//! asks the system for the matching background ([`apply_to_window`]).

use std::{cell::Cell, cell::RefCell, collections::HashMap};

use gpui::{
    AbsoluteLength, AnyElement, CornersRefinement, IntoElement, ParentElement, Styled, Window,
    WindowBackgroundAppearance, WindowId, div,
};
use launcher_core::ui_skin::{
    DEFAULT_OPACITY, MIN_OPACITY, SkinSpec, SurfaceId, Transparency,
};

use crate::theme::colors;

/// What shows through the window in this frame, and how opaque it is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Glass {
    pub material: Transparency,
    /// 0.3 to 1.0.
    pub opacity: f32,
}

impl Glass {
    pub const OPAQUE: Glass = Glass {
        material: Transparency::Opaque,
        opacity: 1.0,
    };

    pub fn is_see_through(self) -> bool {
        self.material.is_see_through()
    }
}

thread_local! {
    /// Settings > General > Transparency and Opacity; `None` follows the skin.
    static USER: Cell<(Option<Transparency>, Option<u8>)> = const { Cell::new((None, None)) };
    /// The material as the window being drawn uses it.
    static FRAME: Cell<Glass> = const { Cell::new(Glass::OPAQUE) };
    /// What each window was last asked to show behind it.
    static APPEARANCE: RefCell<HashMap<WindowId, WindowBackgroundAppearance>> =
        RefCell::new(HashMap::new());
}

/// Takes the settings' Transparency and Opacity (`None` follows the skin).
pub fn set_user(material: Option<Transparency>, opacity: Option<u8>) {
    USER.with(|user| user.set((material, opacity)));
}

/// The settings' Transparency and Opacity, as [`set_user`] last took them.
pub fn user() -> (Option<Transparency>, Option<u8>) {
    USER.with(Cell::get)
}

/// The material for a window showing skin `spec`: the settings' choice, else the
/// skin's, else opaque.
pub fn resolve(spec: &SkinSpec) -> Glass {
    let (material, opacity) = user();
    let material = material.or(spec.transparency).unwrap_or_default();
    if !material.is_see_through() {
        return Glass::OPAQUE;
    }
    let percent = opacity
        .or(spec.window_opacity)
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
pub fn appearance(material: Transparency) -> WindowBackgroundAppearance {
    match material {
        Transparency::Opaque => WindowBackgroundAppearance::Opaque,
        Transparency::Clear => WindowBackgroundAppearance::Transparent,
        Transparency::Frosted | Transparency::Glass => WindowBackgroundAppearance::Blurred,
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

pub use launcher_core::ui_skin::transparency::{Role, role};

/// How much of `surface`'s fill is kept under `glass`: `None` keeps all of it
/// (see [`launcher_core::ui_skin::material::alpha`]).
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
    launcher_core::ui_skin::transparency::alpha(glass.opacity, surface, own, image, backdrop)
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

/// A selected or hovered row's fill: `color`, or while see-through a light veil
/// (white at `percent`) that lets the panel under it show.
pub fn row_fill(color: crate::theme::Color, percent: u32) -> crate::theme::Color {
    if frame().is_see_through() && color.0 & 0xff == 0xff {
        colors::WHITE.opacity(percent)
    } else {
        color
    }
}

/// How far the window's picture is blurred under a see-through window's panels, in
/// screen pixels ([`crate::skin::glass::blur_source`]).
pub const WINDOW_BLUR: u8 = 24;

/// The window's picture, blurred, as a see-through window's panel shows it (as its
/// first child): `opacity` is how much of it covers the desktop behind, and `radii`
/// are the panel's corners. Drawn by the skins' materials
/// ([`crate::skin::glass::layer`]), lined up with the window.
pub fn backdrop(
    behind: crate::skin::Glass,
    opacity: f32,
    radii: CornersRefinement<AbsoluteLength>,
) -> AnyElement {
    div()
        .absolute()
        .inset_0()
        .opacity(opacity)
        .child(crate::skin::glass::layer(behind, radii))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glass(material: Transparency, opacity: f32) -> Glass {
        Glass { material, opacity }
    }

    #[test]
    fn the_setting_wins_over_the_skin() {
        let mut spec = SkinSpec::default();
        set_user(None, None);
        assert_eq!(resolve(&spec), Glass::OPAQUE);
        spec.transparency = Some(Transparency::Frosted);
        spec.window_opacity = Some(40);
        assert_eq!(resolve(&spec), glass(Transparency::Frosted, 0.4));
        set_user(Some(Transparency::Opaque), None);
        assert_eq!(resolve(&spec), Glass::OPAQUE);
        set_user(Some(Transparency::Glass), Some(75));
        assert_eq!(resolve(&spec), glass(Transparency::Glass, 0.75));
        set_user(Some(Transparency::Clear), Some(5));
        assert_eq!(resolve(&spec).opacity, f32::from(MIN_OPACITY) / 100.0);
        set_user(None, None);
    }

    #[test]
    fn nothing_fades_while_opaque() {
        assert_eq!(
            alpha(Glass::OPAQUE, SurfaceId::Window, None, false, false),
            None
        );
        assert!(
            alpha(
                glass(Transparency::Clear, 0.6),
                SurfaceId::Window,
                None,
                false,
                false
            )
            .is_some()
        );
    }

    #[test]
    fn appearance_matches_the_material() {
        assert_eq!(
            appearance(Transparency::Opaque),
            WindowBackgroundAppearance::Opaque
        );
        assert_eq!(
            appearance(Transparency::Clear),
            WindowBackgroundAppearance::Transparent
        );
        assert_eq!(
            appearance(Transparency::Glass),
            WindowBackgroundAppearance::Blurred
        );
    }


}
