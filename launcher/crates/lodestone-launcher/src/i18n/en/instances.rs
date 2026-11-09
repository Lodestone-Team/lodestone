//! Instances: playing and its progress, the console, imports from other launchers and modpacks.

crate::i18n::messages! {
    pub enum InstancesMsg {
        /// Two sentences one after the other, as one message: "{first} {second}".
        TwoSentences = "{first} {second}",
        /// After an instance was made from a share code; {name} is its name.
        ShareCodeReady = "{name} is ready",
        /// {names} lists what couldn't be installed.
        ShareCodeNotInstalled = "Not installed: {names}.",
        ShareCodeContentFailed = "Couldn't install the share code's content",
        ShareCodeOffline = "Modrinth can't be reached, so the instance was made without its content.",
        /// An error's title and its text, in the console: "Couldn't start Java: …".
        TitleAndMessage = "{title}: {message}",
        /// An instance without a mod loader.
        Vanilla = "Vanilla",

        // Preparing the game: the Play button's progress.
        StagePreparing = "Preparing",
        StageJava = "Downloading Java",
        StageLibraries = "Downloading libraries",
        StageLoader = "Installing the mod loader",
        /// Running Forge's or NeoForge's installer.
        StageProcessors = "Patching Minecraft for the mod loader",
        /// The game's sounds, languages and textures.
        StageAssets = "Downloading assets",
        /// Unpacking the game's native libraries.
        StageNatives = "Extracting natives",
        StageStarting = "Starting Java",
        /// Under the Play button while downloading: files done of the total, then
        /// megabytes done of the total.
        ProgressDetail = "{files_done}/{files_total} · {done} MB/{total} MB",

        // Lines the launcher writes in the console.
        PreparingNote = "Preparing Minecraft {version} ({loader})",
        JoiningNote = "Joining {address} once the game starts",
        VerifyingNote = "Verifying the game files of Minecraft {version} ({loader})",
        RosettaNote = "This version runs in an Intel (x86_64) Java under Rosetta 2.",
        /// Followed by the Java command line.
        StartingJavaNote = "Starting Java: {command}",
        StoppedNote = "Stopped",
        ExitedWithCode = "Minecraft exited with code {code}",
        EndedBySignal = "Minecraft was ended by a signal",
        CancellingNote = "Cancelling…",
        StoppingNote = "Stopping Minecraft",
        /// An install, a check or an import that was cancelled.
        Cancelled = "Cancelled",

        // Launch errors and notices.
        CouldntSignIn = "Couldn't sign in",
        RosettaNeeded = "Rosetta 2 is needed",
        RosettaMissing = "This Minecraft version needs an Intel (x86_64) Java, which runs on Apple Silicon through Rosetta 2. Rosetta isn't installed. Install it by running this in Terminal, then press Play again:\n\nsoftwareupdate --install-rosetta",
        CouldntStartMinecraft = "Couldn't start Minecraft",
        CouldntStartJava = "Couldn't start Java",
        CouldntVerify = "Couldn't verify the game files",
        GameFilesVerified = "Game files verified",
        /// Followed by the check's summary ("Checked 4,091 game files…").
        EveryFileIntact = "Every file was intact. {summary}",
        ModpackUpdating = "The modpack is updating",
        WaitForUpdate = "Wait for the update to finish, then press Play.",
        StopBeforeDeleting = "Stop the game before deleting its instance.",
        CouldntSavePlayed = "Couldn't save when it was played: {error}",
        CouldntSavePlayTime = "Couldn't save the play time: {error}",
        ModpackExported = "Modpack exported",
        CouldntExportModpack = "Couldn't export the modpack",
        CouldntSwitchLoader = "Couldn't switch to a mod loader",
        NoModLoaderFor = "No mod loader Lodestone Launcher supports runs on Minecraft {version}: Fabric starts at 1.14, and Forge has no build for it.",
        /// After "Switch to Fabric": the dialog's title.
        SwitchedTo = "Switched to {loader}",
        SwitchedToMessage = "{name} now uses {loader} {version}. It installs the next time you press Play.",

        // The Logs tab.
        SearchLog = "Search the log",
        /// Log level filters.
        LevelDebug = "Debug",
        LevelInfo = "Info",
        LevelWarn = "Warn",
        LevelError = "Error",
        LogEmpty = "The game's output appears here when you press Play.",
        NoLinesMatch = "No lines match the filter.",
        JumpToLatest = "Jump to latest",
        CopyLog = "Copy log",

        // Import from another launcher.
        ImportTitle = "Import from another launcher",
        /// The folder picker's button.
        ImportFromHere = "Import from here",
        LinksLeftOut = "Links to outside the game folder were left out.",
        /// An instance that asks for whatever Minecraft release is newest.
        NewestRelease = "The newest release",
        NewestSnapshot = "The newest snapshot",
        /// A launcher's folder with no instances in it.
        NoInstances = "No instances.",
        LookingForLaunchers = "Looking for other launchers",
        NoLaunchersFound = "No other launchers were found in their usual places. Choose a launcher's folder, or an instance's, to import from it.",
        /// An import in the queue that hasn't started.
        Waiting = "Waiting",
        FindingVersion = "Finding the version and loader",
        Imported = "Imported",
        /// Followed by a note about the import.
        ImportedNote = "Imported. {note}",
        /// Bytes copied of the total: "Copying 120 MB of 380 MB".
        Copying = "Copying {done} of {total}",
        MatchingModrinth = "Matching mods and packs to Modrinth",
        CheckToImport = "Check the instances to import. Their files are copied; the other launcher keeps its own.",
        LeaveWorldsOut = "Leave worlds out",
        /// With the size the worlds take.
        LeaveWorldsOutSize = "Leave worlds out ({size})",
        Import = "Import",
        ChooseFolder = "Choose a folder…",
        /// Asked when the import dialog closes while imports are running.
        StopImportingTitle = "Stop importing?",
        StopImportingBody = "The imports that haven't finished stop, and what they copied so far is removed. Instances already imported stay.",
        StopImporting = "Stop importing",
        KeepImporting = "Keep importing",

        // Modpacks.
        NotAModpack = "Not a modpack",
        NotMrpack = "{name} isn't a modpack. Modrinth modpacks end in .mrpack, and CurseForge modpacks are .zip files with a manifest.json in them.",
        /// A CurseForge modpack while this build can't reach CurseForge (no API key).
        CurseForgeUnreachable = "This is a CurseForge modpack, and this copy of Lodestone Launcher can't reach CurseForge.",
        CurseForgePackOnWebsite = "This modpack can only be downloaded from CurseForge's website. Download it there, then drop the .zip on Lodestone Launcher.",
        /// The name of a modpack whose file has no name.
        Modpack = "Modpack",
        ReadingModpack = "Reading the modpack",
        /// A modpack import finished: the dialog's title.
        ModpackReady = "{name} is ready",
        ModpackReadyMessage = "Minecraft {version} with {loader}. Press Play to download the game and start it.",
        CouldntImportModpack = "Couldn't import the modpack",
        CouldntUpdateModpack = "Couldn't update the modpack",
        StopGameFirst = "Stop the game first.",
        ModpackUpdated = "Modpack updated",
        /// "Survival now runs Fabulously Optimized 6.1.0. The files it replaced are saved in …".
        ModpackUpdatedMessage = "{name} now runs {pack} {version}. The files it replaced are saved in {path}.",
        UpdatedButNotSaved = "The files were updated, but the instance couldn't be saved: {error}",
        // A modpack import's or update's progress.
        DownloadingName = "Downloading {name}",
        InstallingName = "Installing {name}",
        CopyingConfigs = "Copying the pack's configs",
        RecordingFiles = "Recording the pack's files",
        SavingSnapshot = "Saving a snapshot of the instance's files",
        /// Backing up a world before an update.
        BackingUp = "Backing up {name}",
        UpdatingTo = "Updating to {version}",

        // New instance > Modpack.
        SearchModpacks = "Search modpacks on Modrinth, or paste a Modrinth link",
        SearchModpacksCurseForge = "Search modpacks on CurseForge",
        /// How search results are sorted.
        SortRelevance = "Relevance",
        SortDownloads = "Downloads",
        SortFollows = "Follows",
        SortUpdated = "Updated",
        /// A pasted Modrinth link to something else: "That link is a mod, not a modpack."
        LinkNotModpack = "That link is a {kind}, not a modpack. Add it from an instance's Content tab.",
        KindMod = "mod",
        KindResourcePack = "resource pack",
        KindShaderPack = "shader pack",
        KindDataPack = "data pack",
        KindPlugin = "plugin",
        SearchingModrinth = "Searching Modrinth",
        NoModpacksMatch = "No modpacks match.",
        /// Under a search result: its author and download count ("1.2M").
        ByAuthor = "by {author} · {downloads} downloads",
        PickModpack = "Pick a modpack to see its versions.",
        LoadingModpack = "Loading the modpack",
        /// The picked modpack's tabs.
        Versions = "Versions",
        Description = "Description",
        NoVersions = "This modpack has no versions.",
        LoadingVersions = "Loading versions",
    }

    pub enum InstancesPlural {
        ShareCodeInstalled = (
            "Installed {count} item from the code.",
            "Installed {count} items from the code."
        ),
        /// "Checked 4,091 game files in 45 ms: 4,091 unchanged since the last check, 0 hashed, 0 downloaded."
        CheckSummary = (
            "Checked {count} game file in {ms} ms: {unchanged} unchanged since the last check, {hashed} hashed, {downloaded} downloaded.",
            "Checked {count} game files in {ms} ms: {unchanged} unchanged since the last check, {hashed} hashed, {downloaded} downloaded."
        ),
        /// Followed by the check's summary.
        FilesRepaired = (
            "{count} missing or damaged file was downloaded again. {summary}",
            "{count} missing or damaged files were downloaded again. {summary}"
        ),
        BrokenFolders = (
            "{count} instance folder couldn't be read: {path}.",
            "{count} instance folders couldn't be read, such as {path}."
        ),
        /// An export finished.
        FilesSaved = ("{count} file saved to {path}.", "{count} files saved to {path}."),
        /// After an export: mods from CurseForge left out of the Modrinth pack.
        CurseForgeLeftOut = (
            "{count} file was left out because it is only on CurseForge, whose files can't go into a Modrinth pack: {names}.",
            "{count} files were left out because they are only on CurseForge, whose files can't go into a Modrinth pack: {names}."
        ),
        FilesLeftOut = (
            "{count} file was left out: {name}.",
            "{count} files were left out, such as {name}."
        ),
        /// The console's footer: rows shown of the total.
        LinesShown = ("{shown} of {count} line", "{shown} of {count} lines"),
        /// The same while it follows new output.
        LinesShownFollowing = (
            "{shown} of {count} line · following",
            "{shown} of {count} lines · following"
        ),
        /// A launcher's first problem, and how many more it has.
        ProblemAndMore = ("{problem} (and {count} more)", "{problem} (and {count} more)"),
        /// The import's summary: how many instances are checked, and their size.
        InstancesToCopy = ("{count} instance to copy, {size}.", "{count} instances to copy, {size}."),
        /// The import button, for two instances or more.
        ImportInstances = ("Import {count} instance", "Import {count} instances"),
        /// After importing a CurseForge pack: files to download from curseforge.com.
        PackFilesToDownload = (
            "{count} file has to be downloaded from CurseForge's website: the instance's Content tab shows it.",
            "{count} files have to be downloaded from CurseForge's website: the instance's Content tab shows them."
        ),
        PackFilesLeftOut = (
            "{count} file of the pack was left out: {path}.",
            "{count} files of the pack were left out, such as {path}."
        ),
        WorldsBackedUp = ("{count} world was backed up first.", "{count} worlds were backed up first."),
        /// A modpack's progress.
        DownloadingFiles = ("Downloading {count} file", "Downloading {count} files"),
    }
}
