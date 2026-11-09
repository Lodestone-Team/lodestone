use gpui::{Hsla, Pixels, Styled, Svg, svg};

/// The icons embedded from `launcher/assets/icons` (Font Awesome Free, as in the dashboard).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    ArrowDown,
    ArrowLeft,
    ArrowUp,
    ArrowUpRightFromSquare,
    Bars,
    CaretDown,
    Check,
    CircleUp,
    Copy,
    Ellipsis,
    Cube,
    Download,
    Expand,
    Gear,
    HardDrive,
    MagnifyingGlass,
    Plus,
    RightFromBracket,
    SquarePlus,
    TrashCan,
    TriangleExclamation,
    Upload,
    User,
    Xmark,
}

impl Icon {
    pub const ALL: [Icon; 24] = [
        Icon::ArrowDown,
        Icon::ArrowLeft,
        Icon::ArrowUp,
        Icon::ArrowUpRightFromSquare,
        Icon::Bars,
        Icon::CaretDown,
        Icon::Check,
        Icon::CircleUp,
        Icon::Copy,
        Icon::Ellipsis,
        Icon::Cube,
        Icon::Download,
        Icon::Expand,
        Icon::Gear,
        Icon::HardDrive,
        Icon::MagnifyingGlass,
        Icon::Plus,
        Icon::RightFromBracket,
        Icon::SquarePlus,
        Icon::TrashCan,
        Icon::TriangleExclamation,
        Icon::Upload,
        Icon::User,
        Icon::Xmark,
    ];

    /// The asset path of the icon's SVG.
    pub fn path(self) -> &'static str {
        match self {
            Icon::ArrowDown => "icons/arrow-down.svg",
            Icon::ArrowLeft => "icons/arrow-left.svg",
            Icon::ArrowUp => "icons/arrow-up.svg",
            Icon::ArrowUpRightFromSquare => "icons/arrow-up-right-from-square.svg",
            Icon::Bars => "icons/bars.svg",
            Icon::CaretDown => "icons/caret-down.svg",
            Icon::Check => "icons/check.svg",
            Icon::CircleUp => "icons/circle-up.svg",
            Icon::Copy => "icons/copy.svg",
            Icon::Ellipsis => "icons/ellipsis.svg",
            Icon::Cube => "icons/cube.svg",
            Icon::Download => "icons/download.svg",
            Icon::Expand => "icons/expand.svg",
            Icon::Gear => "icons/gear.svg",
            Icon::HardDrive => "icons/hard-drive.svg",
            Icon::MagnifyingGlass => "icons/magnifying-glass.svg",
            Icon::Plus => "icons/plus.svg",
            Icon::RightFromBracket => "icons/right-from-bracket.svg",
            Icon::SquarePlus => "icons/square-plus.svg",
            Icon::TrashCan => "icons/trash-can.svg",
            Icon::TriangleExclamation => "icons/triangle-exclamation.svg",
            Icon::Upload => "icons/upload.svg",
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
