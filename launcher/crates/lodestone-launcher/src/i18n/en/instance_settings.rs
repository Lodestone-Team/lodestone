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
        GameVersionHint = "Set when the instance was made.",
        /// {loader} is a loader and its version, "Fabric 0.16.9".
        LoaderInstallsNextStart = "{loader} installs the next time the game starts.",
        LoaderInstallsNextPlay = "{loader} installs the next time you press Play.",
        SavedNextStart = "Saved. It applies the next time the game starts.",
        /// {error} is the problem, in English.
        CouldntTurnModsOff = "Couldn't turn the mods off: {error}",

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
        WhatToInclude = "What to include",
        WhatToRestore = "What to put back",
        PartMods = "Mods",
        PartResourcePacks = "Resource packs",
        PartShaderPacks = "Shader packs",
        PartConfigs = "Configs and options",
        PartWorlds = "Worlds",
        PartScreenshots = "Screenshots",
        PartSettings = "Instance settings",
        PartOther = "Other files",
        NoSnapshots = "No snapshots yet.",
        SnapshotSizes = "{total}, {new} new",
        BeforeGoingBack = "Before going back to “{name}”",
        RevertEllipsis = "Revert…",
        RevertTitle = "Go back to {name}?",
        RevertBody = "The chosen parts are replaced by the snapshot’s. A snapshot of them is taken first, so you can come back.",
        Revert = "Revert",
        ShareEllipsis = "Share…",
        ShareSnapshotTitle = "Share {name}",
        PreparingFile = "Preparing the file…",
        DragToShare = "Drag the file into another app, or:",
        SaveAsEllipsis = "Save as…",
        ShowInFolder = "Show in folder",
        DeleteSnapshotTitle = "Delete {name}?",
        DeleteSnapshotBody = "Files no other snapshot uses are removed. This can’t be undone.",
        TakingSnapshot = "Taking the snapshot…",
        RevertingSnapshot = "Putting the snapshot back…",
        ImportingSnapshot = "Creating the instance…",
        ReadingSnapshot = "Reading the snapshot…",
        ImportSnapshotTitle = "New instance from a snapshot",
        SnapshotFrom = "{name} · Minecraft {version}",
        SnapshotModsWarning = "It has {count} mods: programs that run when the game starts. Only use snapshots from people you trust.",
        CreateInstanceButton = "Create instance",
        SnapshotGameRunning = "Stop the game first.",
        ChooseSomething = "Choose at least one part.",
        SnapshotTaken = "Snapshot taken.",
        SnapshotReverted = "Snapshot put back.",
        SnapshotsHint = "Copies of this instance to go back to or share.",

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
    }

    pub enum InstanceSettingsPlural {
        /// The share dialog's warning title.
        ShareNotShared = ("{count} file isn't shared", "{count} files aren't shared"),
        /// Parts of "Your changes: …" in the share dialog.
        ShareAddedCount = ("{count} added", "{count} added"),
        ShareRemovedCount = ("{count} removed", "{count} removed"),
        ShareChangedCount = ("{count} changed", "{count} changed"),
        ShareToggledCount = ("{count} turned off or on", "{count} turned off or on"),
        /// After "Gaycraft 5.0 from Modrinth" in the New instance dialog.
        CodePackChanges = ("+ {count} change", "+ {count} changes"),
        /// After a loader change; {loader} is "Fabric 0.16.9".
        LoaderNextStartModsOff = (
            "{loader} installs the next time the game starts. {count} mod was turned off.",
            "{loader} installs the next time the game starts. {count} mods were turned off."
        ),
        LoaderNextPlayModsOff = (
            "{loader} installs the next time you press Play. {count} mod was turned off.",
            "{loader} installs the next time you press Play. {count} mods were turned off."
        ),
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
    }
}
