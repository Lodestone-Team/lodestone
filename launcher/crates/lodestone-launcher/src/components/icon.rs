use gpui::{Hsla, Pixels, Styled, Svg, svg};

/// The icons embedded from `launcher/assets/icons` (Font Awesome Free, as in the dashboard).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    CaretDown,
    Check,
    Expand,
    Gear,
    SquarePlus,
    User,
    Xmark,
}

impl Icon {
    pub const ALL: [Icon; 7] = [
        Icon::CaretDown,
        Icon::Check,
        Icon::Expand,
        Icon::Gear,
        Icon::SquarePlus,
        Icon::User,
        Icon::Xmark,
    ];

    /// The asset path of the icon's SVG.
    pub fn path(self) -> &'static str {
        match self {
            Icon::CaretDown => "icons/caret-down.svg",
            Icon::Check => "icons/check.svg",
            Icon::Expand => "icons/expand.svg",
            Icon::Gear => "icons/gear.svg",
            Icon::SquarePlus => "icons/square-plus.svg",
            Icon::User => "icons/user.svg",
            Icon::Xmark => "icons/xmark.svg",
        }
    }

    /// The icon as a square element of `size`, filled with `color`.
    pub fn render(self, size: Pixels, color: impl Into<Hsla>) -> Svg {
        svg()
            .path(self.path())
            .flex_none()
            .size(size)
            .text_color(color)
    }
}
