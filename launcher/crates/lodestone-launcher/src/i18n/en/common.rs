//! Words most screens use. Prefer these to a message of your own for the same word.

use launcher_core::wording;

crate::i18n::messages! {
    pub enum CommonMsg {
        Add = "Add",
        Apply = "Apply",
        Back = "Back",
        Browse = "Browse…",
        Cancel = "Cancel",
        Change = "Change…",
        Close = "Close",
        Continue = "Continue",
        Copy = "Copy",
        Copied = "Copied",
        Default = "Default",
        Delete = "Delete",
        Discard = "Discard",
        Done = "Done",
        Edit = "Edit",
        Install = "Install",
        Loading = "Loading…",
        Next = "Next",
        No = "No",
        None = "None",
        NotNow = "Not now",
        Off = "Off",
        Ok = "OK",
        On = "On",
        Open = "Open",
        OpenFolder = "Open folder",
        Previous = "Previous",
        Refresh = "Refresh",
        Remove = "Remove",
        Rename = "Rename",
        Reset = "Reset",
        Save = "Save",
        Saved = "Saved",
        Search = "Search",
        SignIn = "Sign in",
        SignOut = "Sign out",
        Start = "Start",
        Stop = "Stop",
        TryAgain = "Try again",
        Update = "Update",
        Menu = "Menu",
        MoreActions = "More actions",
        MoveDown = "Move down",
        MoveUp = "Move up",
        Yes = "Yes",
        /// "Save your changes?" and the like, when leaving with unsaved changes.
        KeepEditing = "Keep editing",
        UnsavedChanges = "You have unsaved changes",
        /// The platform's file manager: Finder, File Explorer, or a plain word.
        ShowInFileManager = wording::SHOW_IN_FILE_MANAGER,
        /// The button that deletes a file to the Trash (the Recycle Bin on Windows).
        MoveToTrash = wording::MOVE_TO_TRASH,
        /// Where deleted files go, with its article, in a sentence: "moves to
        /// {trash}".
        TheTrash = wording::TRASH,
        /// Opens an image in the default app: Preview, on macOS.
        OpenImage = wording::OPEN_IMAGE,
        /// The computer in a sentence: "this Mac", "this PC", "this computer".
        ThisComputer = if cfg!(target_os = "macos") {
            "this Mac"
        } else if cfg!(windows) {
            "this PC"
        } else {
            "this computer"
        },
        /// The same, at the start of a sentence.
        ThisComputerStart = if cfg!(target_os = "macos") {
            "This Mac"
        } else if cfg!(windows) {
            "This PC"
        } else {
            "This computer"
        },
        /// Where secrets are kept, once it is clear which: "your Keychain".
        Keychain = wording::KEYCHAIN,
        /// The same, in full: "the macOS Keychain".
        KeychainInFull = wording::KEYCHAIN_IN_FULL,
        /// A "Browse…", "Import…" or "Export…" button's file dialog didn't open. On
        /// Linux the dialog is the desktop portal's, which some desktops don't run;
        /// the package names stay as they are in every language.
        FileDialogUnavailable = if cfg!(any(target_os = "linux", target_os = "freebsd")) {
            "The system's file dialog isn't available. It needs xdg-desktop-portal and the portal for your desktop, such as xdg-desktop-portal-kde or xdg-desktop-portal-gnome."
        } else {
            "The system's file dialog couldn't open. Try again."
        },
    }

    pub enum CommonPlural {
        Instances = ("{count} instance", "{count} instances"),
        Files = ("{count} file", "{count} files"),
    }
}
