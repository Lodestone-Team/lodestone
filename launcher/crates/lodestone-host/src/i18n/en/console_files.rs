//! Work package B: the Console and Files tabs.

crate::i18n::messages! {
    pub enum ConsoleFilesMsg {
        /// The console's search field.
        ConsoleSearch = "Search console…",
        /// A checkbox: the search text is a regular expression.
        ConsoleRegex = "Regex",
        /// The status dot in the console's corner (tooltip).
        ConsoleStatusNoPermission = "No permission to access console",
        ConsoleStatusLoading = "Loading console…",
        ConsoleStatusBuffered = "History messages. No live updates",
        ConsoleStatusLive = "Console is live",
        ConsoleStatusLiveNoBuffer = "Console is live but failed to fetch history. Your internet connection may be unstable",
        ConsoleStatusClosed = "Console is closed",
        ConsoleStatusError = "Connection lost or error",
        /// The status tooltip and the command line's placeholder when the server is stopped.
        ConsoleServerStopped = "Server is stopped",
        /// The command line's placeholder when it can't be used.
        ConsoleInputNoPermission = "No permission",
        ConsoleInputPlaceholder = "Enter command…",
        ConsoleNoPermission = "You don’t have permission to access this console",
        /// A screen reader label for the placeholder shown while the console loads.
        ConsoleLoading = "Loading console",
        ConsoleEmpty = "No console messages yet",
        /// A toast; {error} is the core's reason.
        ConsoleSendFailed = "Failed to send command: {error}",
        /// The tooltip of the name after a console line about a linked player.
        ConsoleLinkedUser = "The Lodestone user who linked this Minecraft account",
        /// A button over the console when it has been scrolled up.
        ConsoleJumpToLatest = "Jump to latest",
        /// The console's name for screen readers.
        Console = "Console",
        /// The command line's name for screen readers.
        ConsoleCommand = "Command",
        /// Over the tab while files are dragged onto it.
        /// The toolbar's menu.
        FilesAddRemove = "Add/Remove",
        FilesSelectAll = "Select all",
        FilesDeselectAll = "Deselect all",
        FilesCutSelected = "Cut selected",
        FilesDownloadSelected = "Download selected",
        FilesUnarchiveSelected = "Unarchive selected",
        FilesDeleteSelected = "Delete selected",
        FilesDiscardChanges = "Discard changes",
        /// The editor's find bar (⌘F / Ctrl+F).
        FilesFind = "Find",
        FilesReplaceWith = "Replace with",
        FilesFindCount = "{current} of {total}",
        FilesFindNone = "No results",
        FilesFindPrevious = "Previous match",
        FilesFindNext = "Next match",
        FilesReplace = "Replace",
        FilesReplaceAll = "Replace all",
        FilesFindClose = "Close find",
        FilesUpload = "Upload",
        FilesZippingForDownload = "Zipping the folder for download…",
        FilesDownloadFailed = "Failed to download {name}: {error}",
        /// A toast with a button that shows the file in its folder.
        FilesDownloaded = "Downloaded {name}",
        FilesShowInFolder = "Show in folder",
        FilesOnlyFilesUnzip = "Only files can be unzipped",
        FilesOnlyFilesUpload = "Only files can be uploaded",
        FilesDeleteMessage = "Are you sure you want to delete the following?",
        /// After a folder's name in the list of what is deleted.
        FilesDeleteDirectory = "(folder)",
        FilesDeleteUnknownType = "(unknown type)",
        /// A screen reader label for the placeholder shown while a file loads.
        FilesLoadingFile = "Loading file",
        FilesEditorTitle = "File editor",
        FilesNoReadPermission = "You don’t have permission to read this folder",
        FilesLoading = "Loading files",
        FilesEmpty = "No files here…",
        FilesUnknownTime = "Unknown time",
        FilesCut = "Cut",
        FilesPaste = "Paste",
        FilesPasteInto = "Paste into {name}",
        FilesZip = "Zip",
        FilesZipTo = "Zip to {archive}",
        FilesZipFile = "Zip {name} to {archive}",
        FilesUnzipHere = "Unzip here",
        FilesUnzipHereSmart = "Unzip here (smart)",
        FilesUnzipTo = "Unzip to {name}",
        FilesUnsupportedZip = "Unsupported archive type",
        FilesNewFolder = "New folder",
        FilesNewFile = "New file",
        FilesNameFile = "Name your file",
        FilesNameFilePlaceholder = "Untitled",
        FilesCreateFile = "Create file",
        FilesNameFolder = "Name your folder",
        FilesNameFolderPlaceholder = "Untitled folder",
        FilesCreateFolder = "Create folder",
        FilesRenameFile = "Rename file",
        FilesNameRequired = "Enter a name.",
        /// The New file or New folder dialog, when the folder already has something by that name.
        FilesNameTaken = "A file or folder named {name} already exists here.",
        FilesDeletedFile = "Deleted file: {name}",
        FilesDeletedFolder = "Deleted folder: {path}",
        FilesMoveFailed = "Failed to move {name}: {error}",
        FilesZipFailed = "Failed to start zipping: {error}",
        FilesUnzipFailed = "Failed to unzip {name}: {error}",
        FilesUnzipping = "Unzipping {name}…",
        FilesSaveFailed = "Failed to save {name}: {error}",
        /// The unsaved-changes dialog, before another file or folder opens.
        FilesSaveChanges = "Save your changes to {name}?",
        FilesTooLarge = "This file is too large to open here (over {size}). Download it instead.",
        /// Next to the file's name when the user may not change files.
        FilesReadOnly = "Read only",
        /// A checkbox under the editor: long lines wrap instead of scrolling sideways (⌥Z / Alt+Z).
        FilesWrapLines = "Wrap lines",
        /// A row's checkbox, for screen readers.
        FilesSelect = "Select {name}",
        /// The file list's name for screen readers.
        FilesList = "Files",
        FilesResize = "Resize the file list",
    }

    pub enum ConsoleFilesPlural {
        FilesPasteCount = ("Paste {count} file", "Paste {count} files"),
        FilesDeleteTitle = ("Delete {count} file", "Delete {count} files"),
        FilesSelected = ("{count} item selected", "{count} items selected"),
        FilesInClipboard = ("{count} item in clipboard", "{count} items in clipboard"),
        FilesZipFiles = ("Zip {count} file to {archive}", "Zip {count} files to {archive}"),
        FilesCopiedToClipboard = ("{count} item copied to clipboard", "{count} items copied to clipboard"),
        FilesCutToClipboard = ("{count} item cut to clipboard", "{count} items cut to clipboard"),
        FilesUploading = ("Uploading {count} file", "Uploading {count} files"),
        FilesZipping = ("Zipping {count} item…", "Zipping {count} items…"),
    }
}
