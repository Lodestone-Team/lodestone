//! The Worlds tab: worlds, their backups and data packs, and screenshots.

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

        // A world's data packs.
        DataPacks = "Data packs",
        ReadingDataPacks = "Reading the data packs",
        NoDataPacksYet = "No data packs in this world yet. Add a .zip or a folder, or find one on Modrinth below.",
        AddDataPack = "Add data pack…",
        /// The file dialog's button.
        AddDataPackPrompt = "Add data pack",
        SearchDataPacks = "Search Modrinth for data packs, or paste a Modrinth link",
        /// A data pack the game hasn't loaded yet.
        NewPack = "New",
        NewPackHint = "The game turns it on the next time it loads the world.",
        PacksNotSwitchable = "Open this world in Minecraft 1.13 or later once, then its data packs can be turned on and off here.",
        /// Over the progress bar.
        AddingDataPacks = "Adding data packs",
        SavingPackList = "Saving the world's data pack list",
        /// {packs} is a list of file names; {name} is a world's name.
        AddedDataPacks = "Added {packs} to {name}.",
        /// {pack} is a file name.
        InstalledDataPack = "Installed {pack} in {name}.",
        /// {old} is the older file of the same pack it replaced.
        UpdatedDataPack = "Installed {pack} in {name}, replacing {old}.",
        PackTurnedOn = "{pack} is on. It loads the next time the world opens.",
        PackTurnedOff = "{pack} is off. It stops loading the next time the world opens.",
        /// {name} is a Modrinth project; {version} a Minecraft version.
        NoDataPackVersion = "{name} has no data pack for Minecraft {version}.",
        SearchingModrinth = "Searching Modrinth",
        NoDataPacksFound = "No data packs found for Minecraft {version}.",
        LinkNotDataPack = "That link isn't a data pack.",
        ModrinthUnavailable = "Modrinth can't be reached now; data packs can still be added from files.",

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
