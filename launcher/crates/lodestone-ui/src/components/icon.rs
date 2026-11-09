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
    Palette,
    MagnifyingGlass,
    Plus,
    RightFromBracket,
    SquarePlus,
    TrashCan,
    TriangleExclamation,
    Upload,
    User,
    /// The window buttons drawn on Linux desktops that leave them to apps (see
    /// `components::window_frame`). Thin outlines, like the desktops' own.
    WindowClose,
    WindowMaximize,
    WindowMinimize,
    WindowRestore,
    Xmark,
    Link,
    UserGroup,
    ChevronDown,
    ChevronRight,
    ChevronLeft,
    Pen,
    FloppyDisk,
    CircleUser,
    CircleInfo,
    CircleCheck,
    CircleExclamation,
    Server,
    Folder,
    FolderPlus,
    File,
    FilePen,
    FileCirclePlus,
    ArrowsRotate,
    Scissors,
    Paste,
    FileZipper,
    ListCheck,
    ClipboardQuestion,
    Bell,
}

impl Icon {
    pub const ALL: [Icon; 53] = [
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
        Icon::Palette,
        Icon::MagnifyingGlass,
        Icon::Plus,
        Icon::RightFromBracket,
        Icon::SquarePlus,
        Icon::TrashCan,
        Icon::TriangleExclamation,
        Icon::Upload,
        Icon::User,
        Icon::WindowClose,
        Icon::WindowMaximize,
        Icon::WindowMinimize,
        Icon::WindowRestore,
        Icon::Xmark,
        Icon::Link,
        Icon::UserGroup,
        Icon::ChevronDown,
        Icon::ChevronRight,
        Icon::ChevronLeft,
        Icon::Pen,
        Icon::FloppyDisk,
        Icon::CircleUser,
        Icon::CircleInfo,
        Icon::CircleCheck,
        Icon::CircleExclamation,
        Icon::Server,
        Icon::Folder,
        Icon::FolderPlus,
        Icon::File,
        Icon::FilePen,
        Icon::FileCirclePlus,
        Icon::ArrowsRotate,
        Icon::Scissors,
        Icon::Paste,
        Icon::FileZipper,
        Icon::ListCheck,
        Icon::ClipboardQuestion,
        Icon::Bell,
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
            Icon::Palette => "icons/palette.svg",
            Icon::MagnifyingGlass => "icons/magnifying-glass.svg",
            Icon::Plus => "icons/plus.svg",
            Icon::RightFromBracket => "icons/right-from-bracket.svg",
            Icon::SquarePlus => "icons/square-plus.svg",
            Icon::TrashCan => "icons/trash-can.svg",
            Icon::TriangleExclamation => "icons/triangle-exclamation.svg",
            Icon::Upload => "icons/upload.svg",
            Icon::User => "icons/user.svg",
            Icon::WindowClose => "icons/window-close.svg",
            Icon::WindowMaximize => "icons/window-maximize.svg",
            Icon::WindowMinimize => "icons/window-minimize.svg",
            Icon::WindowRestore => "icons/window-restore.svg",
            Icon::Xmark => "icons/xmark.svg",
            Icon::Link => "icons/link.svg",
            Icon::UserGroup => "icons/user-group.svg",
            Icon::ChevronDown => "icons/chevron-down.svg",
            Icon::ChevronRight => "icons/chevron-right.svg",
            Icon::ChevronLeft => "icons/chevron-left.svg",
            Icon::Pen => "icons/pen.svg",
            Icon::FloppyDisk => "icons/floppy-disk.svg",
            Icon::CircleUser => "icons/circle-user.svg",
            Icon::CircleInfo => "icons/circle-info.svg",
            Icon::CircleCheck => "icons/circle-check.svg",
            Icon::CircleExclamation => "icons/circle-exclamation.svg",
            Icon::Server => "icons/server.svg",
            Icon::Folder => "icons/folder.svg",
            Icon::FolderPlus => "icons/folder-plus.svg",
            Icon::File => "icons/file.svg",
            Icon::FilePen => "icons/file-pen.svg",
            Icon::FileCirclePlus => "icons/file-circle-plus.svg",
            Icon::ArrowsRotate => "icons/arrows-rotate.svg",
            Icon::Scissors => "icons/scissors.svg",
            Icon::Paste => "icons/paste.svg",
            Icon::FileZipper => "icons/file-zipper.svg",
            Icon::ListCheck => "icons/list-check.svg",
            Icon::ClipboardQuestion => "icons/clipboard-question.svg",
            Icon::Bell => "icons/bell.svg",
        }
    }

    /// A name for an icon-only control showing this icon, when the control has none
    /// of its own: what pressing it usually does.
    pub fn label(self) -> gpui::SharedString {
        use crate::i18n::{CommonMsg as M, t};
        let message = match self {
            Icon::ArrowDown => M::MoveDown,
            Icon::ArrowUp => M::MoveUp,
            Icon::ArrowLeft | Icon::ChevronLeft => M::Back,
            Icon::Pen => M::Edit,
            Icon::FloppyDisk => M::Save,
            Icon::Bars => M::Menu,
            Icon::Check => M::Done,
            Icon::Copy => M::Copy,
            Icon::Ellipsis => M::MoreActions,
            Icon::Download => M::Install,
            Icon::Gear => M::Edit,
            Icon::MagnifyingGlass => M::Search,
            Icon::Plus | Icon::SquarePlus => M::Add,
            Icon::RightFromBracket => M::SignOut,
            Icon::TrashCan => M::Delete,
            Icon::ArrowUpRightFromSquare => M::Open,
            Icon::CircleUp => M::Update,
            Icon::Xmark | Icon::WindowClose => M::Close,
            Icon::CaretDown
            | Icon::Cube
            | Icon::Expand
            | Icon::HardDrive
            | Icon::Palette
            | Icon::TriangleExclamation
            | Icon::Upload
            | Icon::User
            | Icon::WindowMaximize
            | Icon::WindowMinimize
            | Icon::WindowRestore
            | Icon::Link
            | Icon::UserGroup
            | Icon::ChevronDown
            | Icon::ChevronRight
            | Icon::CircleUser
            | Icon::CircleInfo
            | Icon::CircleCheck
            | Icon::CircleExclamation
            | Icon::Server
            | Icon::Folder
            | Icon::FolderPlus
            | Icon::File
            | Icon::FilePen
            | Icon::FileCirclePlus
            | Icon::ArrowsRotate
            | Icon::Scissors
            | Icon::Paste
            | Icon::FileZipper
            | Icon::ListCheck
            | Icon::ClipboardQuestion
            | Icon::Bell => return format!("{self:?}").into(),
        };
        t(message).into()
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
