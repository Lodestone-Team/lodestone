//! An instance's Settings tab, the new-instance dialog, the loader picker, appearance and the memory and window fields.

crate::i18n::messages! {
    pub enum InstanceSettingsMsg {
        // The memory, window and environment fields, here and in the launcher's
        // Settings.
        MemoryFormat = "Enter memory in MB (such as 4096) or GB (such as 4G), at least 256 MB.",
        MemoryMinAboveMax = "The minimum is more than the maximum.",
        /// {computer} is "this Mac", "this PC" or "this computer"; {os} is macOS,
        /// Windows or Linux.
        MemoryTooHigh = "That is more than three quarters of {computer}'s memory, which leaves too little for {os} and your other apps.",
        /// {computer} is "This Mac", "This PC" or "This computer"; {size} is "16 GB".
        TotalMemory = "{computer} has {size} of memory.",
        /// The memory a game gets, "512 MB – 4 GB".
        MemoryRange = "{min} – {max}",
        /// The unit beside the memory fields.
        MemoryUnit = "MB",
        /// Under the memory while automatic setup chose it.
        MemoryChosenAutomatically = "Chosen for this computer and the instance's mods. It follows the mods until you change it.",
        WindowDefaultSize = "The game's default size",
        /// {size} is "854 × 480" or "The game's default size".
        WindowFullscreen = "{size}, fullscreen",
        WindowSizeInvalid = "The window size needs a width and a height in pixels.",
        /// The name of the list of window sizes, for assistive technology.
        WindowSize = "Window size",
        /// In the window size list: the size the game picks itself (854 × 480).
        WindowSizeGameDefault = "Game default",
        /// After a size in the window size list that is the size of this screen.
        WindowSizeThisScreen = "This screen",
        /// In the window size list: type a width and height.
        WindowSizeCustom = "Custom",
        /// The names of the width and height fields, for assistive technology.
        WindowWidth = "Window width",
        WindowHeight = "Window height",
        /// {error} is the problem, in English.
        EnvInvalid = "Environment variables: {error}",
        /// An example in the environment variables field.
        EnvPlaceholder = "NAME=value",

        // The Settings tab's sections.
        General = "General",
        GameAndLoader = "Game & loader",
        JavaAndMemory = "Java & memory",
        LaunchOptions = "Launch options",
        SharedGameSettings = "Shared game settings",
        Maintenance = "Maintenance",
        Modpack = "Modpack",
        LinkedServer = "Linked server",

        // General.
        Name = "Name",
        Group = "Group",
        /// The group field's placeholder.
        NoGroup = "No group",
        NameMissing = "Give the instance a name.",
        Icon = "Icon",
        PlayBackground = "Play background",
        /// The button of the file dialog that picks an icon.
        UseAsIcon = "Use as icon",
        /// The button of the file dialog that picks a background.
        UseAsBackground = "Use as background",
        SavedSeePlayTab = "Saved. See it on the Play tab.",
        YourScreenshots = "Your screenshots",
        FindingScreenshots = "Finding your screenshots",
        NoScreenshots = "No screenshots yet. Press F2 in the game to take one, or choose a file.",
        ChooseFile = "Choose a file…",
        UseDefault = "Use the default",
        /// The Play background picker's tabs, and the slideshow's sources.
        BackgroundDefault = "Default",
        BackgroundImage = "Image",
        BackgroundSlideshow = "Slideshow",
        SlideshowThisInstance = "This instance",
        SlideshowAllInstances = "All instances",
        SlideshowOwnImages = "My images",
        AddImages = "Add images…",
        AddToSlideshow = "Add to slideshow",
        SlideshowNoScreenshots = "No screenshots yet. The default artwork shows until there are.",
        SlideshowNoImages = "No images yet. Add some, or pick screenshots below.",
        /// The icon picker's tabs.
        BlocksAndItems = "Blocks & items",
        Emoji = "Emoji",
        UploadImage = "Upload an image…",

        // The automatic sidebar groups, as the group hint names them.
        AutoGroupServers = "Servers",
        AutoGroupFabric = "Fabric (modded)",
        AutoGroupForge = "Forge (modded)",
        AutoGroupNeoForge = "NeoForge (modded)",
        AutoGroupQuilt = "Quilt (modded)",
        AutoGroupVanilla = "Vanilla",
        AutoGroupImported = "Imported",

        // Game & loader.
        /// Under the Minecraft version and the mod loader, which never change.
        GameVersionHint = "Set when the instance was made.",
        /// The same, for an instance a modpack's updates can move to another version.
        GameVersionFollowsModpack = "Follows its modpack's updates.",
        /// The same, for an instance a linked server's pack can move to another version.
        GameVersionFollowsServer = "Follows the linked server's pack.",
        /// {loader} is a loader and its version, "Fabric 0.16.9".
        LoaderInstallsNextStart = "{loader} installs the next time the game starts.",
        LoaderInstallsNextPlay = "{loader} installs the next time you press Play.",
        SavedNextStart = "Saved. It applies the next time the game starts.",

        // The loader picker.
        /// No mod loader: the game as Mojang ships it.
        Vanilla = "Vanilla",
        ModLoader = "Mod loader",
        /// What each loader is for, on its card in "New instance".
        AboutVanilla = "Minecraft as Mojang ships it.",
        AboutFabric = "Light and quick to update. Most performance mods.",
        AboutQuilt = "Fabric-compatible, with its own extras.",
        AboutForge = "The classic. Big content mods and older packs.",
        AboutNeoForge = "Forge's successor, for newer versions.",
        /// Whether Modrinth content updates to beta and alpha versions.
        PrereleaseUpdates = "Beta and alpha updates",
        PrereleaseUpdatesHint = "Off, mods already on a beta still get betas.",
        LoaderVersion = "Loader version",
        /// {loader} is Fabric, Quilt, Forge or NeoForge.
        ChooseLoaderVersion = "Choose a {loader} version.",
        ChooseVersion = "Choose a version",
        LoadingLoaderVersions = "Loading {loader} versions",
        /// {error} is the problem, in English.
        CouldntLoadVersions = "Couldn't load the versions: {error}",
        NoVersionsFor = "No versions for Minecraft {game}.",
        /// A tag on a loader version.
        Recommended = "Recommended",
        Beta = "Beta",

        // Modpack.
        FromModrinth = "From Modrinth",
        /// {server} is a server's name.
        FromServerMods = "From the mods of {server}: Play keeps them in step with the server.",
        FromMrpack = "From a .mrpack file that isn't on Modrinth, so it can't be updated here.",
        ViewOnModrinth = "View on Modrinth",
        DownloadingNewVersion = "Downloading the new version…",
        CheckingModrinth = "Checking Modrinth…",
        /// {version} is the pack's version, "1.4.2".
        UpdateTo = "Update to {version}…",
        UpToDate = "Up to date",
        CheckForUpdates = "Check for updates",
        ModrinthUnreachable = "Modrinth can't be reached.",
        StopGameFirst = "Stop the game first, or wait for the update to finish.",
        /// {error} is the problem, in English.
        CouldntPrepareUpdate = "Couldn't prepare the update: {error}",

        // Linked server.
        /// {core} is a host's name.
        LinkedServerHint = "On {core}. Play joins it and syncs its mods.",
        LinkedServerCoreRemoved = "On {core}, which was removed from the Lodestone Host page. Add the host again there to join the server.",
        CoreRemoved = "Host removed",
        Unlink = "Unlink",
        Unlinked = "Unlinked",
        UnlinkNote = "Play won't join the server anymore. The mods stay.",

        // Java & memory, Launch options.
        /// Sets a value for this instance instead of the launcher's.
        ChangeSetting = "Change",
        JavaMemoryLead = "Defaults come from the launcher's settings.",
        Memory = "Memory",
        Checking = "Checking…",
        /// {major} is "21", {version} is "21.0.5".
        JavaFound = "Java {major} (version {version})",
        JavaTooOld = "Java {major} (version {version}). Minecraft {game} needs Java {needed} or newer.",
        /// The Java Mojang provides for the game, "Mojang's Java 21".
        MojangJavaVersion = "Mojang's Java {major}",
        MojangJava = "Mojang's Java",
        /// The Java a Java 8 version runs in on an Apple silicon Mac, where Mojang has
        /// no Java 8.
        AzulZulu8 = "Azul Zulu 8, made for Apple silicon",
        /// The Intel Java an old version runs in on an Apple silicon Mac, "Mojang's
        /// Intel Java 8, through Rosetta 2".
        MojangIntelJavaVersion = "Mojang's Intel Java {major}, through Rosetta 2",
        /// The default Java on an Apple silicon Mac before the version is installed.
        JavaTheVersionNeeds = "The Java this version needs",
        /// The button of the file dialog that picks a java binary.
        UseThisJava = "Use this Java",
        /// Checks the chosen java binary.
        Check = "Check",
        JvmArguments = "JVM arguments",
        GameArguments = "Game arguments",
        /// No JVM or game arguments.
        NoArguments = "None",
        /// No environment variables.
        NoVariables = "None",
        Window = "Window",
        Fullscreen = "Fullscreen",
        EnvironmentVariables = "Environment variables",

        // Snapshots.
        InstanceSnapshots = "Snapshots",
        TakeSnapshotEllipsis = "Take snapshot…",
        TakeSnapshotTitle = "Take a snapshot",
        TakeSnapshot = "Take snapshot",
        SnapshotName = "Name",
        NoSnapshots = "No snapshots yet.",
        SnapshotSizes = "{total}, {new} new",
        SnapshotNothingNew = "{total}, nothing new",
        PreparingFile = "Preparing the file…",
        DragToShare = "Drag the file into another app, or:",
        SaveAsEllipsis = "Save as…",
        ShowInFolder = "Show in folder",
        DeleteSnapshotTitle = "Delete {name}?",
        DeleteSnapshotBody = "The snapshot is deleted. Its files go at the next clean-up, unless another snapshot uses them. This can’t be undone.",
        TakingSnapshot = "Taking the snapshot…",
        ImportingSnapshot = "Creating the instance…",
        ReadingSnapshot = "Reading the snapshot…",
        ImportSnapshotTitle = "New instance from a snapshot",
        ImportServerTitle = "New instance from a server’s world",
        CreateInstanceButton = "Create instance",
        SnapshotGameRunning = "Stop the game first.",
        SnapshotsHint = "Copies of the whole instance folder to go back to or share.",
        /// The Settings tab's toggle; also Settings › Instance defaults' default.
        SnapshotAfterPlay = "Take a snapshot after each play session",
        SnapshotAfterPlayHint = "Copies the whole instance folder after you play. Follows Settings › Instance defaults, where it is off unless you turn it on.",
        /// Settings › Instance defaults' toggle.
        SnapshotAfterPlayDefaultHint = "Copies the whole instance folder after each play session, for instances that don't set it themselves. Off unless you turn it on.",
        /// {last}, {days} and {weeks} are numbers.
        SnapshotKeepHint = "Snapshots the Launcher takes on its own are kept for a while: the last {last}, one a day for {days} days and one a week for {weeks} weeks. Yours stay until you delete them.",
        /// Badges on a snapshot's row.
        BadgeAutomatic = "Automatic",
        BadgeAfterPlaying = "After playing",
        BadgeImported = "Imported",
        BadgePinned = "Pinned",
        /// A snapshot row's buttons.
        RestoreEllipsis = "Restore…",
        ExportEllipsis = "Export…",
        PinSnapshot = "Pin",
        UnpinSnapshot = "Unpin",
        UnreadableImagesHint = "Old content history isn't cleaned up while one can't be read.",
        DeleteUnreadable = "Delete them",
        /// The restore dialog's title; {name} is the snapshot.
        RestoreTitle = "Restore {name}",
        /// The restore dialog's choices.
        RestoreWorldsOnly = "Only the worlds",
        RestoreWorldsAndMods = "The worlds and their mods",
        RestoreEverything = "Everything",
        RestoreAsNew = "As a new instance",
        RestoreAsNewName = "New instance's name",
        RestoreEverythingHint = "The whole game folder goes back to the snapshot: worlds, mods, configs and options. Worlds made since are removed.",
        /// A world in the snapshot that the instance no longer has; {size} is "120 MB".
        WorldGoneNow = "{size} · not in the instance now",
        ModsDiffer = "These worlds last ran with other mods.",
        /// Like ModsDiffer, when only configs and other files differ, not mods.
        FilesDiffer = "These worlds last ran with other files.",
        RestoreModsToo = "Put their mods back too",
        /// Before the list of what "Put their mods back too" changes, when it is off.
        RestoreModsTooChanges = "Putting their mods back too:",
        /// {version} is a loader version.
        RestoreLoaderChange = "The loader goes back to {version}.",
        RestoreSafety = "A snapshot of what is replaced is taken first and kept for 30 days.",
        RestoreOtherGame = "This snapshot is of another Minecraft version or loader, so it can only become a new instance.",
        RestoreButton = "Restore",
        RestoringSnapshot = "Restoring the snapshot…",
        DeletingSnapshot = "Deleting the snapshot…",
        ChooseWorld = "Choose at least one world.",
        /// The export dialog's title; {name} is the snapshot.
        ExportTitle = "Export {name}",
        ExportNote = "Mods and packs from Modrinth and CurseForge are listed, not copied: they download again when the file is imported, where their authors allow it.",
        /// {files} is a list of files.
        ExportLeftOut = "Left out, as they can hold passwords and sign-ins: {files}.",
        /// {name} is the snapshot, {version} a Minecraft version, {loader} "Fabric 0.16.10".
        ImportFrom = "{name} · Minecraft {version} · {loader}",
        /// {worlds} is a list of worlds.
        ImportWorlds = "Worlds: {worlds}",
        /// {size} is "38 MB".
        ImportDownloads = "{size} to download from Modrinth and CurseForge.",
        /// {files} is a list of files.
        ImportNotDownloadable = "Their authors don't allow downloads, so these are left out: {files}.",
        ImportServerOnlyMods = "Mods only the server runs, left out: {names}",
        ImportServerPack = "The files only the server’s players use come from its modpack on Modrinth, and may include mods: programs that run when the game starts. Only download worlds from servers you trust.",
        ImportServerPackCurseforge = "The server’s modpack is from CurseForge, which can’t be downloaded from a server yet, so the files only its players use won’t come.",
        /// {files} is a list of files.
        ImportHostLeftOut = "The host left these out of the file: {files}",
        /// {names} is a list of "<file>: <setting>".
        ImportHostBlanked = "The host emptied these secrets: {names}",
        /// {size} is "1.2 GB".
        ImportSpace = "Needs {size} of disk space, 1 GB of it to spare.",
        ImportNeedsModrinth = "Modrinth can't be reached, so the file's mods can't be downloaded.",

        // Maintenance.
        VerifyGameFiles = "Verify game files",
        VerifyHint = "Redownloads missing or damaged files.",
        Verify = "Verify",
        VerifyStarted = "Checking every file. The progress is on the Play button.",
        /// Starts the mod finder (the crash dialog's "Find it for me") without a crash.
        FindCrashingMod = "Find a mod that crashes the game",
        FindCrashingModHint = "Starts the game a few times with half of the mods off, until one is left.",
        FindCrashingModButton = "Find it…",
        /// Search words for the row (Settings search), comma-free.
        FindCrashingModKeywords = "crash conflict broken incompatible troubleshoot",
        Export = "Export",
        ExportHint = "As a .mrpack file.",
        ExportButton = "Export…",
        Exporting = "Exporting…",
        Duplicate = "Duplicate",
        /// The Duplicate button while the copy is being made.
        Duplicating = "Duplicating…",
        /// The Game & loader row that leads to a copy: an instance's version and loader
        /// never change.
        ChangeVersionOrLoader = "Another version or loader",
        ChangeVersionOrLoaderHint = "An instance keeps the version and loader it was made with. A copy can have others.",
        /// The Maintenance row that opens "Copy with another version or loader".
        CopyAs = "Copy with another version or loader",
        /// Opens "Copy with another version or loader".
        CopyAsButton = "Copy…",
        /// Search words for the copy rows (Settings search), comma-free.
        CopyAsKeywords = "change switch update upgrade downgrade version loader Fabric Forge NeoForge Quilt",

        // "Copy with another version or loader": a new instance from this one.
        /// The dialog's title.
        CopyAsTitle = "Copy with another version or loader",
        /// {name} is the instance copied; {version} is its version, "1.20.1 · Fabric 0.16.9".
        CopyFrom = "A new instance from {name} ({version}), which stays as it is.",
        /// The heading over the choices of what goes into the copy.
        CopyWhat = "What to copy",
        CopyWorlds = "Worlds",
        CopyNoWorlds = "This instance has no worlds.",
        CopyWorldsOlder = "Worlds can't go to an older version of Minecraft: it can't open them.",
        /// {version} is the Minecraft version chosen.
        CopyWorldsUnknown = "Worlds stay: Lodestone Launcher can't tell whether {version} is newer.",
        /// {version} is the Minecraft version chosen; {name} is the instance copied.
        CopyWorldsUpgraded = "Minecraft {version} upgrades the worlds when it opens them, and they can't go back. The originals stay in {name}.",
        /// Under Worlds, for another loader; {name} is the instance copied.
        CopyWorldsModsGone = "Blocks and items from mods that don't come along are removed when the copy opens the worlds. The originals stay in {name}.",
        CopyPacks = "Resource packs, shaders and options",
        CopyMods = "Mods",
        /// While the instance's mods are read.
        CopyReading = "Reading the instance's mods…",
        CopyNoMods = "This instance has no mods.",
        CopyModsVanilla = "Vanilla Minecraft runs no mods.",
        CopyModsOffline = "Modrinth can't be reached, so no mods can be copied.",
        /// {error} is the problem, in English.
        CopyModsFailed = "Couldn't check the mods: {error}",
        /// {target} is "1.21.1 Fabric"; {names} is a list of mods.
        CopyModsNoBuild = "No build for {target}, so these stay behind: {names}.",
        /// {names} is a list of mods.
        CopyModsNotOnModrinth = "Not on Modrinth, so these stay behind: {names}.",
        /// {names} is a list of mods whose Modrinth lookup failed.
        CopyModsUnchecked = "Couldn't check these on Modrinth, so they stay behind: {names}.",
        /// After "Try again", when the mods couldn't be checked.
        CopyOrUncheckMods = "or uncheck Mods to copy without them.",
        /// While the version and loader chosen are the instance's own.
        CopySameTarget = "Choose another Minecraft version or loader. Duplicate, in Maintenance, copies the instance as it is.",
        /// Starts the copy.
        CopyAsStart = "Make the copy",
        HostOnCore = "Host on Lodestone",
        HostHint = "Creates a server with this instance's mods.",
        HostButton = "Host…",
        ShareCodeRow = "Share code",
        /// Opens the share code dialog.
        ShareButton = "Share…",
        /// {name} is the instance's name.
        ShareTitle = "Share {name}",
        ShareIntro = "Another Lodestone Launcher can make this instance from the code: the same Minecraft version, loader and Modrinth content.",
        ShareReading = "Reading the content…",
        ShareNotSharedBody = "Only content from Modrinth goes into a share code. Send these separately if they're needed.",
        ShareFromCurseForge = "From CurseForge",
        ShareAddedByHand = "Added by hand",
        /// A file of the share dialog's list that Modrinth doesn't know.
        ShareNotOnModrinth = "Not on Modrinth",
        /// One of the modpack's own files, changed since the pack wrote it.
        ShareChangedSincePack = "Changed since the modpack",
        /// Modrinth couldn't be asked about the file (offline).
        ShareUnchecked = "Not checked with Modrinth",
        /// {pack} is the modpack and its version, like "Gaycraft 5.0".
        ShareBasedOnPack = "Based on the modpack {pack}",
        ShareIntroPack = "Another Lodestone Launcher can make this instance from the code: the modpack, then your changes to it.",
        ShareNotSharedBodyPack = "The modpack brings its own files, but these won't arrive as they are here. Send them separately if they're needed.",
        ShareNoChanges = "No changes to the modpack",
        /// {changes} lists them, like "2 added, 1 removed".
        ShareChanges = "Your changes: {changes}",
        /// The New instance dialog's preview of a code based on a modpack; {pack} is like "Gaycraft 5.0".
        CodePackPreview = "{pack} from Modrinth",
        ShareMore = "And {count} more",
        CopyCode = "Copy code",
        CodeCopied = "Copied",
        /// The New instance dialog's third mode: from a share code.
        ShareCodeMode = "Share code",
        PasteShareCode = "Paste a share code",
        CodeNotACode = "That isn't a Lodestone share code.",
        CodeNewer = "This code is from a newer Lodestone Launcher.",
        CodeTooLarge = "The code is too long.",
        CodeDamaged = "The code is damaged. Copy it again.",
        DeleteInstance = "Delete instance",
        DeleteHint = "Including its worlds and mods.",
        DeleteButton = "Delete…",

        // Shared game settings.
        SharedLead = "Keep your settings in sync across instances.",
        UseSharedSettings = "Use shared settings",
        SharingOff = "This instance keeps its own options.",
        /// {name} is the main instance's name.
        SharingFrom = "Gets in-game changes from {name}.",
        SharingMain = "In-game changes here update every instance.",
        SharingNoMain = "No main instance: shared values change only in Settings.",
        MainInstance = "Main instance",
        MainInstanceHint = "Settings you change in-game in your main instance are copied to every instance that shares them, the next time it starts. Only one instance is the main one.",
        IsMainInstance = "This is your main instance",
        MakeMain = "Make main",
        StopGameToChange = "Stop the game to change these: it rewrites options.txt when it quits.",
        /// A category the launcher's Settings doesn't share.
        NotShared = "Not shared (launcher Settings)",
        ShareAll = "Share all",
        OverrideAll = "Override all",
        Show = "Show",
        Hide = "Hide",
        NotSharedYet = "Not shared yet",
        NotInOptions = "Not in options.txt",
        /// A tag on a value this instance sets itself.
        ThisInstance = "This instance",
        /// {value} is the shared value.
        UseShared = "Use shared ({value})",
        /// A tag on a value that comes from the shared settings.
        Shared = "Shared",
        Override = "Override",
        /// The placeholder of a shared setting's field.
        Value = "Value",
        KeepForInstance = "Keep them for this instance",
        Dismiss = "Dismiss",
        /// A line in the game's console; {error} is the problem, in English.
        SharedNotApplied = "Shared game settings weren't applied: {error}",
        /// A line in the game's console after it exits; {error} is the problem, in
        /// English.
        SharedNotReadBack = "Shared game settings couldn't be read back: {error}",
        CategoryControls = "Controls",
        CategoryVideo = "Video",
        CategorySound = "Sound",
        CategoryAccessibility = "Accessibility & chat",
        CategoryLanguage = "Language",
        CategoryControlsDescription = "Mouse, key bindings",
        CategoryVideoDescription = "FOV, render distance, GUI scale, graphics",
        CategorySoundDescription = "Volume, subtitles",
        CategoryAccessibilityDescription = "Narrator, chat, text background",
        CategoryLanguageDescription = "Game language and font",

        // New instance.
        NewInstance = "New instance",
        /// The dialog's two modes.
        MinecraftVersion = "Minecraft version",
        /// The name field's placeholder.
        MyInstance = "My instance",
        SearchVersions = "Search versions",
        /// Filters of the version list.
        Releases = "Releases",
        Snapshots = "Snapshots",
        OldVersions = "Old versions",
        /// Kinds of Minecraft versions, in a narrow column.
        KindRelease = "Release",
        KindSnapshot = "Snapshot",
        KindBeta = "Beta",
        KindAlpha = "Alpha",
        KindOther = "Other",
        LoadingVersionList = "Loading Mojang's version list",
        NoVersionsMatch = "No versions match.",
        PickModpack = "Pick a modpack and a version.",
        ChooseGameVersion = "Choose a Minecraft version.",
        CreateInstance = "Create instance",
        InstallModpack = "Install modpack",
        ImportMrpack = "Import .mrpack…",
        /// The button of the file dialog that picks .mrpack files.
        Import = "Import",
        /// Words a search of the settings finds Memory by, besides its name; never
        /// shown.
        MemoryKeywords = "RAM heap Xmx Xms",
        /// Words a search of the settings finds Java by, besides its name; never
        /// shown.
        JavaKeywords = "JDK JRE runtime path",
        /// Words a search of the settings finds JVM arguments by, besides its name; never
        /// shown.
        JvmArgumentsKeywords = "Java flags options",
        /// Words a search of the settings finds Window by, besides its name; never
        /// shown.
        WindowKeywords = "resolution width height fullscreen size",
        /// Words a search of the settings finds Environment variables by, besides its name; never
        /// shown.
        EnvironmentKeywords = "env PATH",
        /// Words a search of the settings finds Play background by, besides its name; never
        /// shown.
        PlayBackgroundKeywords = "wallpaper image picture slideshow",
        /// Words a search of the settings finds Delete instance by, besides its name; never
        /// shown.
        DeleteKeywords = "remove uninstall",
        /// Words a search of the settings finds the loader by, besides its name; never
        /// shown.
        LoaderKeywords = "mod loader Fabric Forge NeoForge Quilt",

        // "Add a modpack": a modpack's files added to an instance that exists.
        /// The dialog's title; {name} is the instance.
        AddModpackTitle = "Add a modpack to {name}",
        /// Above the list: "Modpacks for Minecraft 1.21.1 with Fabric. {note}".
        AddModpackFilter = "Modpacks for Minecraft {version} with {loader}. {note}",
        /// For an instance made for a server.
        AddModpackServerNote = "The pack's mods go next to {server}'s, which stay as the server has them, so you can still join.",
        AddModpackNote = "The pack's files go next to the instance's own. Nothing the instance has is replaced.",
        /// The button that downloads the picked pack and shows what adding it does.
        AddModpackButton = "Add modpack",
        CheckingModpack = "Checking the modpack",
        /// "Fabulously Optimized 6.2.0: 12.4 MiB to download."
        AddModpackSummary = "{pack} {version}: {size} to download.",
        AddModpackNothing = "The instance has every file of this pack already.",
        /// Groups of files in the review.
        AddGroupFiles = "To add ({count})",
        AddGroupConfigs = "Configs to add ({count})",
        /// Files the server's pack has, kept as the server has them.
        AddGroupServer = "Kept as {server} has them ({count})",
        /// The same for an instance made from a modpack.
        AddGroupPack = "Kept as the instance's modpack has them ({count})",
        AddGroupThere = "Already in the instance ({count})",
        /// The Settings section of the packs added to an instance.
        AddedModpacks = "Added modpacks",
        AddModpackRow = "Add a modpack",
        AddModpackHint = "Add a modpack's mods and configs to this instance, next to the ones it has.",
        AddModpackServerHint = "Add a modpack's mods, such as client optimizations, next to {server}'s. The server's mods stay as they are.",
        AddModpackEllipsis = "Add a modpack…",
        RemoveAddedModpackNote = "Removes the files the pack added. Files you changed stay.",
        SnapshotsNotChanged = "The snapshots couldn't be changed",
        RestoreKeptAsideTitle = "Some files were in the restore's way",
        /// {name} is the snapshot's name.
        SnapshotRestored = "Restored “{name}”.",
        ImportLeftOutTitle = "Some files weren't imported",
        /// {files} is a list of files.
        ImportLeftOut = "They couldn't be downloaded, so the new instance doesn't have them: {files}.",
        ImportPackSkippedTitle = "The modpack’s files are missing",
        ImportPackSkipped = "The server’s modpack couldn’t be downloaded or placed (no connection, not enough free space, or a file that failed its check), so the files only its players use aren’t in the new instance.",
        ImportPackSkippedGone = "The server’s modpack is no longer on Modrinth, so the files only its players use aren’t in the new instance. Ask the server’s owner to update the modpack.",
        ImportPackSkippedCurseforge = "The server is built on a CurseForge modpack, which can’t be installed from a server yet, so the files only its players use aren’t in the new instance.",
        RestoreKeptAside = "Files written while the restore ran are kept in {folder}.",
        ImportNoRoom = "The disk doesn't have room for it: it needs {size}, 1 GB of it to spare, and {free} is free.",
        ImportSpaceFree = "Needs {size} of disk space, 1 GB of it to spare; {free} is free.",
        ExportNotDownloadable = "These can't be downloaded, so whoever imports the file won't get them: {files}.",
        SnapshotAfterPlayOwnHint = "Copies the whole instance folder after you play. Set for this instance.",
        DeleteUnreadableNote = "They are deleted for good.",
        /// Settings › Snapshots: the section of zips of single worlds.
        WorldBackups = "World backups",
        WorldBackupsHint = "Zip copies of one world, kept until you delete them. They open anywhere.",
        /// Settings › Snapshots: the heading of the snapshots, below the world backups.
        SnapshotsGroup = "Snapshots of the whole instance",
        /// The row, and the dialog's title.
        BackUpWorldTitle = "Back up a world",
        BackUpWorldEllipsis = "Back up a world…",
        NoWorldBackups = "No world backups yet.",
        /// Beside a world's name: its backups are kept, but the world was deleted or renamed.
        WorldNotHere = "Not in this instance any more",
        RestoreAsNewWorld = "Restore as a new world",
        /// Back up a world: the label over the worlds to pick from.
        WorldBackupWorld = "World",
        /// The name field's placeholder: the name a world backup gets when none is given.
        WorldBackupDefaultName = "World backup",
        /// The name of the world backup made before another is restored; {name} is that
        /// backup's name.
        WorldBackupBeforeRestoring = "Before restoring “{name}”",
        BackUpButton = "Back up",
        /// {name} is the world's name.
        BackingUpWorld = "Backing up {name}…",
        WorldBackedUp = "Backed up {name}.",
        NoWorldsToBackUp = "This instance has no worlds yet.",
        /// {world} is the world's name. A title, not a question: it stays once the restore ends.
        RestoreWorldTitle = "Restore {world}",
        /// {name} is the world backup's name; {when} is when it was made: "today, 14:05".
        RestoreWorldBody = "{world} goes back to how it was in “{name}” ({when}). The world as it is now is saved first, as another world backup.",
        RestoreWorldGoneBody = "{world} isn't in this instance any more. It comes back as it was in “{name}” ({when}).",
        SaveAndRestore = "Save and restore",
        SavingWorld = "Saving the world as it is now…",
        UnpackingWorldBackup = "Unpacking the world backup…",
        WorldRestored = "Restored {world}.",
        /// Said after WorldRestored. {name} is the world backup made of the world just before.
        WorldRestoredSafety = "The world as it was is in the world backup “{name}”.",
        /// {folder} is the new world's folder.
        WorldRestoredAsNew = "Restored as a new world, {folder}.",
        DeleteWorldBackupTitle = "Delete this world backup?",
        /// {name} is the backup's name; {trash} is the Trash, with its article.
        DeleteWorldBackupBody = "“{name}” moves to {trash}.",
    }

    pub enum InstanceSettingsPlural {
        /// The share dialog's warning title.
        ShareNotShared = ("{count} file isn't shared", "{count} files aren't shared"),
        /// Parts of "Your changes: …" in the share dialog.
        ShareAddedCount = ("{count} added", "{count} added"),
        ShareRemovedCount = ("{count} removed", "{count} removed"),
        ShareChangedCount = ("{count} changed", "{count} changed"),
        /// The Snapshots list: images of the instance that can't be read.
        UnreadableImages = ("{count} image can't be read", "{count} images can't be read"),
        /// The restore dialog: files the image store lost.
        RestoreMissing = (
            "The image store lost {count} of its files, so it can't be put back whole.",
            "The image store lost {count} of its files, so it can't be put back whole."
        ),
        /// The import dialog's warning.
        ImportModsWarning = (
            "It has {count} mod: a program that runs when the game starts. Only import files from people you trust.",
            "It has {count} mods: programs that run when the game starts. Only import files from people you trust."
        ),
        ShareToggledCount = ("{count} turned off or on", "{count} turned off or on"),
        /// After "Gaycraft 5.0 from Modrinth" in the New instance dialog.
        CodePackChanges = ("+ {count} change", "+ {count} changes"),
        /// How many of a category's settings are shared.
        SharedCount = ("{count} shared", "{count} shared"),
        /// How many of a category's settings this instance sets itself.
        OverriddenCount = ("{count} overridden", "{count} overridden"),
        /// The names of a few settings and how many more there are: "FOV, Brightness,
        /// Max FPS and 2 more".
        AndMore = ("{names} and {count} more", "{names} and {count} more"),
        /// {names} lists the settings.
        ChangedInGame = (
            "{count} setting you changed in-game ({names}) will be replaced by the shared one next launch.",
            "{count} settings you changed in-game ({names}) will be replaced by the shared ones next launch."
        ),
        /// Lines in the game's console before it starts.
        SharedWritten = (
            "Shared game settings: {count} value written to options.txt",
            "Shared game settings: {count} values written to options.txt"
        ),
        SharedWrittenNewFile = (
            "Shared game settings: {count} value written to options.txt (new file)",
            "Shared game settings: {count} values written to options.txt (new file)"
        ),
        /// Lines in the game's console after it exits.
        SharedEmitted = (
            "Shared game settings: {count} value from this instance now applies to every instance",
            "Shared game settings: {count} values from this instance now apply to every instance"
        ),
        SharedChangedNote = (
            "Shared game settings: {count} setting changed in-game will be replaced by the shared one next launch (see Settings)",
            "Shared game settings: {count} settings changed in-game will be replaced by the shared ones next launch (see Settings)"
        ),

        /// Below a review's list of files.
        AndMoreFiles = ("and {count} more", "and {count} more"),
        /// An added modpack's files in the instance.
        AddedModpackFiles = ("{count} file added", "{count} files added"),
        /// Beside the worlds' box in "Copy with another version or loader".
        CopyWorldsCount = ("{count} world", "{count} worlds"),
        CopyCheckingMods = ("Checking {count} mod on Modrinth…", "Checking {count} mods on Modrinth…"),
        /// {found} of the {count} mods have a build; {target} is "1.21.1 Fabric".
        CopyModsFound = (
            "{found} of {count} mod has a build for {target}.",
            "{found} of {count} mods have a build for {target}."
        ),
        RestorePutsBack = ("Puts back {count} file: {names}", "Puts back {count} files: {names}"),
        RestoreRemoves = ("Removes {count} file: {names}", "Removes {count} files: {names}"),
        RestoreChanges = ("Changes {count} file: {names}", "Changes {count} files: {names}"),
        ImportCarriedMods = ("{count} of them is in the file itself, not downloaded from Modrinth or CurseForge: {names}", "{count} of them are in the file itself, not downloaded from Modrinth or CurseForge: {names}"),
        /// A server's backup (Lodestone Host's export) opened as an instance.
        ImportFromServer = ("A server’s backup from {server}: its world, with the mods its players get. {count} server file isn’t taken (the server’s settings, operators and plugins).", "A server’s backup from {server}: its world, with the mods its players get. {count} server files aren’t taken (the server’s settings, operators and plugins)."),
        /// Files of the world the zip couldn't hold: links, or names some disks can't keep.
        WorldBackupLeftOut = ("{count} file left out", "{count} files left out"),
    }
}
