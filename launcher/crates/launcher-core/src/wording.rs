//! The words for the user's computer and its parts, which differ by platform: "this
//! Mac", the Keychain and Finder on macOS, "this PC", Credential Manager and File
//! Explorer on Windows, and plain words on Linux, where they vary by desktop.

/// The computer, after "this" or "your": "this Mac", "your PC".
pub const COMPUTER: &str = if cfg!(target_os = "macos") {
    "Mac"
} else if cfg!(windows) {
    "PC"
} else {
    "computer"
};

/// The operating system's name.
pub const OS: &str = if cfg!(target_os = "macos") {
    "macOS"
} else if cfg!(windows) {
    "Windows"
} else {
    "Linux"
};

/// Where GPUI's credentials API keeps secrets, in full: "the macOS Keychain".
pub const KEYCHAIN_IN_FULL: &str = if cfg!(target_os = "macos") {
    "macOS Keychain"
} else if cfg!(windows) {
    "Windows Credential Manager"
} else {
    // The Secret Service: GNOME Keyring, KWallet and the like.
    "system keyring"
};

/// [`KEYCHAIN_IN_FULL`] once it is clear which: "your Keychain".
pub const KEYCHAIN: &str = if cfg!(target_os = "macos") {
    "Keychain"
} else if cfg!(windows) {
    "Credential Manager"
} else {
    "keyring"
};

/// Where deleted files go, with its article: "moves to the Trash".
pub const TRASH: &str = if cfg!(windows) {
    "the Recycle Bin"
} else if cfg!(target_os = "macos") {
    "the Trash"
} else {
    "the trash"
};

/// The button that deletes a file to [`TRASH`].
pub const MOVE_TO_TRASH: &str = if cfg!(windows) {
    "Move to Recycle Bin"
} else {
    "Move to Trash"
};

/// The button that shows a file in the file manager.
pub const SHOW_IN_FILE_MANAGER: &str = if cfg!(target_os = "macos") {
    "Show in Finder"
} else if cfg!(windows) {
    "Show in File Explorer"
} else {
    "Show in folder"
};

/// The button that opens an image in the default app: Preview, on macOS.
pub const OPEN_IMAGE: &str = if cfg!(target_os = "macos") {
    "Open in Preview"
} else {
    "Open"
};
