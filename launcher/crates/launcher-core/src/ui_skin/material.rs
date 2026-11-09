//! See-through windows: what shows through the window (a skin's `material`, or
//! the launcher's Transparency setting) and how much of each surface's fill is
//! kept over it.

use serde::{Deserialize, Serialize};

use super::SurfaceId;

/// How much of what is behind the window shows through it: Settings > General >
/// Transparency, or a skin's `material`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Material {
    /// Nothing shows through.
    #[default]
    Opaque,
    /// The desktop shows through, sharp.
    Clear,
    /// The desktop shows through, blurred.
    Frosted,
    /// Frosted, with lit edges on panels and cards.
    Glass,
}

impl Material {
    /// The choices, in the order Settings lists them.
    pub const ALL: [Material; 4] = [
        Material::Opaque,
        Material::Clear,
        Material::Frosted,
        Material::Glass,
    ];

    /// Whether anything shows through.
    pub fn is_see_through(self) -> bool {
        self != Material::Opaque
    }

    /// Whether what shows through is blurred.
    pub fn is_blurred(self) -> bool {
        matches!(self, Material::Frosted | Material::Glass)
    }
}

/// The lowest opacity a see-through window takes, in percent: below it text gets
/// hard to read over a busy desktop.
pub const MIN_OPACITY: u8 = 30;
/// The opacity a see-through window takes unless a setting or skin says otherwise.
pub const DEFAULT_OPACITY: u8 = 75;

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

/// How much of `surface`'s fill is kept while the window is see-through at
/// `opacity` (0.3 to 1.0): `None` keeps all of it. `own` is the skin's `opacity`
/// for the surface, in percent; `image` says the fill is an image, which fades with
/// the window rather than with its role, and `backdrop` that a blurred copy of the
/// skin's window image lies under it.
pub fn alpha(
    opacity: f32,
    surface: SurfaceId,
    own: Option<u8>,
    image: bool,
    backdrop: bool,
) -> Option<f32> {
    if let Some(own) = own {
        return Some(f32::from(own.min(100)) / 100.0);
    }
    let f = opacity;
    Some(match role(surface)? {
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

/// How much of the desktop `surface` hides while the window is see-through at
/// `opacity`: its fill over the window's. `None` for controls.
pub fn coverage(opacity: f32, surface: SurfaceId, own: Option<u8>) -> Option<f32> {
    let window = opacity.clamp(0.0, 1.0);
    if surface == SurfaceId::Window {
        return Some(window);
    }
    let own = alpha(opacity, surface, own, false, false)?;
    Some(1.0 - (1.0 - window) * (1.0 - own))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panels_show_more_than_dialogs() {
        let of = |s| alpha(0.6, s, None, false, false).unwrap();
        assert!(of(SurfaceId::Sidebar) < of(SurfaceId::Window));
        assert!(of(SurfaceId::Card) < of(SurfaceId::Dialog));
        assert!(of(SurfaceId::Dialog) >= 0.85);
        assert_eq!(
            alpha(0.6, SurfaceId::ButtonPrimary, None, false, false),
            None
        );
        assert_eq!(
            alpha(0.6, SurfaceId::Card, Some(30), false, false),
            Some(0.3)
        );
    }

    #[test]
    fn a_surface_covers_more_than_the_window_alone() {
        let window = coverage(0.6, SurfaceId::Window, None).unwrap();
        let card = coverage(0.6, SurfaceId::Card, None).unwrap();
        assert!((window - 0.6).abs() < 1e-6);
        assert!(card > window && card < 1.0);
        assert_eq!(coverage(1.0, SurfaceId::Sidebar, None), Some(1.0));
    }
}
