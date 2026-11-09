//! The Worlds tab: worlds, backups and screenshots.

crate::i18n::messages! {
    pub enum WorldsMsg {
        /// The section switch.
        Worlds = "Worlds",
        Screenshots = "Screenshots",

        StopGameFirst = "Stop the game first: it has the world open and saves it when it exits.",
        WaitForTask = "Wait for the current task to finish.",
        Cancelled = "Cancelled.",
        GameRunning = "The game is running: worlds can be opened and backups shown, but changes wait until it stops.",

        // What is running, over its progress bar. {name} is a world's name; {trash} is
        // the Trash, with its article.
        BackingUp = "Backing up {name}",
        RestoringBackup = "Restoring the backup",
        ReplacingWorld = "Replacing the world with the backup",
        MovingToTrash = "Moving {name} to {trash}",
        MovingBackupToTrash = "Moving the backup to {trash}",
        Renaming = "Renaming",
        Exporting = "Exporting {name}",
        ImportingWorlds = "Importing worlds",

        // How it went.
        BackedUp = "Backed up {name} ({size}).",
        /// {folder} is the new world's folder.
        RestoredAsCopy = "Restored as a new world, {folder}.",
        Restored = "Restored {folder}.",
        /// {backup} is the file name of a backup of the world as it was.
        RestoredWithSafety = "Restored {folder}. The world as it was is backed up as {backup}, and in {trash}.",
        /// {name} is a world, a backup or a screenshot.
        InTrash = "{name} is in {trash}.",
        RenamedTo = "Renamed to {name}.",
        RenamedToFolder = "Renamed to {name} (folder {folder}).",
        Exported = "Exported {name} to {path} ({size}).",
        /// {worlds} is a list of folders.
        Imported = "Imported {worlds}.",
        /// A zip that couldn't be imported, and why.
        FileError = "{name}: {error}",
        ZipsOnly = "Worlds are imported from .zip files.",
        /// The file dialog's button.
        ImportWorldPrompt = "Import world",
        CopiedScreenshot = "Copied the screenshot.",
        CouldntCopy = "Couldn't copy it: {error}",

        // A world's row.
        Hardcore = "Hardcore",
        Survival = "Survival",
        Creative = "Creative",
        Adventure = "Adventure",
        Spectator = "Spectator",
        /// Commands are allowed in the world.
        Cheats = "Cheats",
        /// {version} is a game version that is a snapshot.
        Snapshot = "{version} (snapshot)",
        /// {time} is "Today, 14:30", "Yesterday, 09:12" or a date.
        PlayedAt = "Played {time}",
        BackUp = "Back up",
        Backups = "Backups",
        Export = "Export…",
        DeleteWorld = "Delete…",
        NoBackupsYet = "No backups yet. \"Back up\" saves the world as a zip in the instance's backups folder.",
        RestoreAsCopy = "Restore as copy",
        ReplaceWorld = "Replace world…",

        // The worlds list.
        ImportWorld = "Import world…",
        OpenSavesFolder = "Open saves folder",
        ReadingWorlds = "Reading the worlds",
        NoWorldsYet = "No worlds yet",
        NoWorldsHint = "Worlds you create in the game appear here. You can also import a world zip, or drop one here.",

        // Screenshots.
        ReadingScreenshots = "Reading the screenshots",
        NoScreenshotsYet = "No screenshots yet",
        NoScreenshotsHint = "Press F2 in the game to take one; they appear here.",
        /// In the large view: "3 of 12".
        Position = "{index} of {count}",
        CopyImage = "Copy image",
    }

    pub enum WorldsPlural {
        ScreenshotsSummary = ("{count} screenshot · F2 in the game takes one", "{count} screenshots · F2 in the game takes one"),
    }
}
