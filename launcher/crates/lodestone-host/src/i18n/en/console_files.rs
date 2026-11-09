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
        ConsoleOwnerOnlyAtom = "Only the owner can control atom servers",
        /// A button over the console when it has been scrolled up.
        ConsoleJumpToLatest = "Jump to latest",
        /// The console's name for screen readers.
        Console = "Console",
        /// The command line's name for screen readers.
        ConsoleCommand = "Command",
    }

    pub enum ConsoleFilesPlural {
    }
}
