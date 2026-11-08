use gpui::{Hsla, Pixels, Styled, Svg, svg};

/// The icons embedded from `launcher/assets/icons` (Font Awesome Free, as in the dashboard).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    ArrowUpRightFromSquare,
    CaretDown,
    Check,
    Copy,
    Expand,
    Gear,
    Plus,
    RightFromBracket,
    SquarePlus,
    TriangleExclamation,
    User,
    Xmark,
}

impl Icon {
    pub const ALL: [Icon; 12] = [
        Icon::ArrowUpRightFromSquare,
        Icon::CaretDown,
        Icon::Check,
        Icon::Copy,
        Icon::Expand,
        Icon::Gear,
        Icon::Plus,
        Icon::RightFromBracket,
        Icon::SquarePlus,
        Icon::TriangleExclamation,
        Icon::User,
        Icon::Xmark,
    ];

    /// The asset path of the icon's SVG.
    pub fn path(self) -> &'static str {
        match self {
            Icon::ArrowUpRightFromSquare => "icons/arrow-up-right-from-square.svg",
            Icon::CaretDown => "icons/caret-down.svg",
            Icon::Check => "icons/check.svg",
            Icon::Copy => "icons/copy.svg",
            Icon::Expand => "icons/expand.svg",
            Icon::Gear => "icons/gear.svg",
            Icon::Plus => "icons/plus.svg",
            Icon::RightFromBracket => "icons/right-from-bracket.svg",
            Icon::SquarePlus => "icons/square-plus.svg",
            Icon::TriangleExclamation => "icons/triangle-exclamation.svg",
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
