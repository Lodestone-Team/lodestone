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
        /// Java is running; the game's window hasn't opened yet.
        StageLaunching = "Launching Minecraft",
        /// Under the Play button while downloading: files done of the total, then
        /// megabytes done of the total.
        ProgressDetail = "{files_done}/{files_total} · {done} MB/{total} MB",

        // Lines the launcher writes in the console.
        PreparingNote = "Preparing Minecraft {version} ({loader})",
        JoiningNote = "Joining {address} once the game starts",
        VerifyingNote = "Verifying the game files of Minecraft {version} ({loader})",
        RosettaNote = "The game runs in an Intel (x86_64) Java under Rosetta 2.",
        /// Followed by the Java command line.
        StartingJavaNote = "Starting Java: {command}",
        /// Automatic setup gave the instance memory: {memory} is a range, "2 GB – 4 GB".
        AutoMemoryNote = "Automatic setup: memory set to {memory} for this computer and the instance's mods",
        /// The mods want more memory than the computer can spare.
        AutoMemoryLimitedNote = "The mods would like {wanted} of memory, more than this computer can spare; close other apps while playing.",
        AutoJvmArgsNote = "Automatic setup: Java's garbage collector tuned for modded play",
        /// The memory the user chose is well below what the mods need.
        AutoLowMemoryNote = "This instance has {have} of memory; with its mods, {recommended} is recommended. Change it in the instance's Settings.",
        /// {distance} is the render distance, in chunks.
        AutoVideoNote = "Automatic setup: video settings chosen for this computer (render distance {distance})",
        AutoVideoNoDistanceNote = "Automatic setup: video settings chosen for this computer",
        /// {source} is the name of the instance they were copied from.
        AutoControlsNote = "Automatic setup: mouse sensitivity, key bindings and language copied from {source}",
        AutoControlsLauncherNote = "Automatic setup: mouse sensitivity, key bindings and language copied from the Minecraft Launcher",
        /// {instance} is the name of an instance in another launcher; {launcher} is that
        /// launcher's name ("Prism Launcher", "Modrinth App").
        AutoControlsOtherLauncherNote = "Automatic setup: mouse sensitivity, key bindings and language copied from {instance} in {launcher}",
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
        RosettaMissing = "This Minecraft version needs an Intel (x86_64) Java, which runs on Apple silicon through Rosetta 2. Rosetta isn't installed. Install it by running this in Terminal, then press Play again:\n\nsoftwareupdate --install-rosetta",
        /// Asked once per instance, before its first game under Rosetta 2 on an Apple
        /// silicon Mac, with Play and Cancel. {version} is the Minecraft version, such
        /// as 1.12.2.
        RosettaNoticeTitle = "Minecraft {version} runs under Rosetta 2",
        /// Minecraft 1.13.2 and older.
        RosettaNoticeBody = "Mojang made no Apple silicon build of some libraries this version needs, so it runs in an Intel Java through Rosetta 2. It takes longer to start, then plays normally. Minecraft 1.14 and later run natively.",
        /// A version that could run natively, with an Intel Java chosen in the
        /// instance's Settings.
        RosettaNoticeBodyOverride = "This instance uses an Intel Java, chosen in its Settings, so the game runs through Rosetta 2. It takes longer to start, then plays normally. To run it natively, choose a Java made for Apple silicon, or go back to the default Java.",
        /// Minecraft 1.14 to 1.16.5 on Apple silicon, when Azul lists no Java 8 for it.
        NoNativeJava8 = "This Minecraft version needs Java 8, and no Java 8 for Apple silicon is available right now: Mojang makes none, and Azul lists none. Try again later, or choose a Java 8 made for Apple silicon (arm64) in the instance's Settings.",
        /// Minecraft 1.13.2 and older on Apple silicon, with a Java chosen in the
        /// instance's Settings that is made for Apple silicon.
        NeedsIntelJava = "On Apple silicon, Minecraft {version} runs only in an Intel (x86_64) Java, through Rosetta 2, and the Java chosen in the instance's Settings is made for Apple silicon. Choose an Intel Java there, or Mojang's Java.",
        CouldntStartMinecraft = "Couldn't start Minecraft",
        CouldntStartJava = "Couldn't start Java",
        CouldntVerify = "Couldn't verify the game files",
        GameFilesVerified = "Game files verified",
        /// Followed by the check's summary ("Checked 4,091 game files…").
        EveryFileIntact = "Every file was intact. {summary}",
        ModpackUpdating = "The modpack is updating",
        WaitForUpdate = "Wait for the update to finish, then press Play.",
        StopBeforeDeleting = "Stop the game before deleting its instance.",
        /// Another change of the instance's files (a copy, a move to the Trash, an image
        /// being imported) is still running.
        FilesBusy = "Wait for the instance's files to finish changing, then try again.",
        CouldntSavePlayed = "Couldn't save when it was played: {error}",
        CouldntSavePlayTime = "Couldn't save the play time: {error}",
        ModpackExported = "Modpack exported",
        CouldntExportModpack = "Couldn't export the modpack",

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
        /// The list beside the search: what the Logs tab shows. "This session" is the live console; the rest are files the game left on disk.
        LogSource = "Log to show",
        LogThisSession = "This session",
        LogNoFiles = "No earlier logs or crash reports yet.",
        LogKindLog = "Log",
        LogKindCrash = "Crash report",
        LogKindJvm = "Java crash",
        LogReading = "Reading {name}",
        LogReadFailed = "Couldn’t read {name}: {error}",
        LogFileEmpty = "This file is empty.",

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
        /// A copy with another version or loader: the notice when it is made.
        CopyReady = "{name} is ready",
        CouldntCopy = "Couldn't make the copy",
        /// What a copy with another version or loader is doing.
        CopyingWorlds = "Copying the worlds",
        CopyingPacksAndOptions = "Copying the resource packs, shaders and options",
        InstallingMods = "Installing the mods",
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

        // Finding the mod that crashes the game.
        /// The crash dialog's button that starts the search.
        FindModButton = "Find it for me",
        FinderTitle = "Find the mod that crashes {name}",
        FinderIntro = "Lodestone Launcher will start the game a few times, turning off half of the mods that might be causing the crash each time, until only one is left. Afterwards, your mods go back to how they were.",
        FinderAutomatic = "The game crashed during startup, so you don't need to do anything. Each try ends when the game crashes or gets to the main menu.",
        FinderManual = "The game crashed while you were playing, so you'll need to play each try. Do whatever made it crash before. If it crashes again, the next try starts on its own. If it doesn't, press It didn't crash.",
        /// The finder started from Settings, not after a crash.
        FinderByHand = "Play each try and do whatever made the game crash. If it crashes, the next try starts on its own. If it doesn't, press It didn't crash.",
        FinderStart = "Start",
        FinderReading = "Checking your mods",
        /// A heading: the try being played, 1, 2, 3…
        FinderTry = "Try {number}",
        FinderStarting = "Starting the game",
        FinderWaiting = "Waiting to see if the game crashes",
        FinderSettling = "Made it to the main menu. Checking that it stays open",
        FinderPlay = "Play until it crashes. If it doesn't, press It didn't crash.",
        FinderAsk = "The game closed. Did the problem happen?",
        FinderDidntCrash = "It didn't crash",
        FinderItCrashed = "It crashed",
        FinderStopping = "Stopping the game",
        FinderFound = "Found it",
        FinderFoundOne = "The game doesn't crash when {name} is turned off.",
        FinderFoundGroup = "The game doesn't crash with these turned off: {names}. They depend on each other, so they can only be turned off together.",
        FinderAlsoOff = "The mods that need it stay off too: {names}.",
        FinderKeepOff = "Keep it off",
        FinderPutBack = "Turn everything back on",
        FinderNotFound = "The game kept crashing even with those mods turned off, so the cause probably isn't a mod, or the crash only happens sometimes. Your mods are back to how they were.",
        FinderRefused = "Some mods wouldn't load without the ones that were turned off, so the search had to stop. Your mods are back to how they were.",
        FinderFailed = "Something went wrong and the search stopped. Your mods are back to how they were.",
        FinderStopped = "Search stopped. Your mods are back to how they were.",
        FinderStop = "Stop",
        FinderInterruptedTitle = "Finish finding the mod in {name}?",
        FinderInterrupted = "Lodestone Launcher closed in the middle of a search, so some of {name}'s mods are still turned off.",
        /// {error} says what went wrong.
        FinderPutBackFailed = "Some mods couldn't be turned back on: {error}",

        // Adding a modpack's files to an instance that exists.
        /// Progress: the pack's files going into the instance.
        AddingName = "Adding {name}",
        ModrinthUnreachable = "Modrinth can't be reached.",
        /// The pack, or another version of it, was added to the instance before.
        ModpackAddedAlready = "{pack} is added to this instance already. Remove it in Settings first to add another version.",
        CouldntAddModpack = "Couldn't add the modpack",
        ModpackAdded = "Modpack added",
        /// "Fabulously Optimized was added to Survival. Press Play to start it."
        ModpackAddedMessage = "{pack} was added to {name}. Press Play to start it.",
        CouldntRemoveModpack = "Couldn't remove the modpack",
        ModpackRemoved = "Modpack removed",
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
        /// After the footer's count, for a very long file: only its last lines are shown.
        LogLinesSkipped = (
            "the first {count} line isn’t shown",
            "the first {count} lines aren’t shown"
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
        /// After a copy's "ready" message; {names} is a list of mods.
        CopyModsLeftBehind = ("{count} mod stayed behind: {names}.", "{count} mods stayed behind: {names}."),
        /// The mod finder's progress.
        FinderModsLeft = ("{count} mod left to check", "{count} mods left to check"),
        FinderModsOff = ("{count} mod turned off", "{count} mods turned off"),
        FinderTriesLeft = ("About {count} more try", "About {count} more tries"),

        /// After removing a pack added to an instance.
        AddedPackFilesRemoved = (
            "{count} file of {pack} was removed.",
            "{count} files of {pack} were removed."
        ),
        AddedPackFilesKept = (
            "{count} file you changed was kept.",
            "{count} files you changed were kept."
        ),
    }
}
