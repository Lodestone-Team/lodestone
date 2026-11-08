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
        WindowDefaultSize = "The game's default size",
        /// {size} is "854 × 480" or "The game's default size".
        WindowFullscreen = "{size}, fullscreen",
        WindowSizeInvalid = "The window size needs a width and a height in pixels.",
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
        /// The icon picker's tabs.
        BlocksAndItems = "Blocks & items",
        Emoji = "Emoji",
        UploadImage = "Upload an image…",

        // The automatic sidebar groups, as the group hint names them.
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
        CouldntLoadLoaderVersions = "Couldn't load the {loader} versions: {error}",
        NoVersionsFor = "No versions for Minecraft {game}.",
        LoaderHasNoVersions = "{loader} has no versions for Minecraft {game}.",
        PlainMinecraft = "Plain Minecraft, without mods.",
        ChooseGameVersionFirst = "Choose a Minecraft version first.",
        /// A tag on a loader version.
        Recommended = "Recommended",
        Beta = "Beta",

        // Modpack.
        FromModrinth = "From Modrinth",
        /// {server} is a server's name.
        FromServerMods = "From the mods of {server}: Play keeps them in step with the server.",
        FromMrpack = "From a .mrpack file that isn't on Modrinth, so it can't be updated here.",
        ViewOnModrinth = "View on Modrinth",
        Updates = "Updates",
        UpdatesHint = "Keeps your mods, configs and worlds.",
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
        /// {core} is a Lodestone Core's name.
        LinkedServerHint = "On {core}. Play joins it and syncs its mods.",
        LinkedServerCoreRemoved = "On {core}, which was removed from the Servers page. Add the core again there to join the server.",
        CoreRemoved = "Core removed",
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

        // Maintenance.
        VerifyGameFiles = "Verify game files",
        VerifyHint = "Redownloads missing or damaged files.",
        Verify = "Verify",
        VerifyStarted = "Checking every file. The progress is on the Play button.",
        Export = "Export",
        ExportHint = "As a .mrpack file.",
        ExportButton = "Export…",
        Exporting = "Exporting…",
        Duplicate = "Duplicate",
        HostOnCore = "Host on a Lodestone core",
        HostHint = "Creates a server with this instance's mods.",
        HostButton = "Host…",
        DeleteInstance = "Delete instance",
        DeleteHint = "Including its worlds and mods.",
        DeleteButton = "Delete…",

        // Shared game settings.
        SharedLead = "The same game options in every instance.",
        UseSharedSettings = "Use shared settings",
        SharingOff = "This instance keeps its own options.",
        SharingPreferred = "In-game changes here update every instance.",
        /// {name} is the preferred instance's name.
        SharingFrom = "Gets in-game changes from {name}.",
        SharingNoPreferred = "Shared values change only in Settings.",
        /// A tag on the preferred instance.
        Preferred = "Preferred",
        MakePreferred = "Make preferred",
        MadePreferred = "In-game changes in this instance now update every instance.",
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
        /// Labels above the fields, in capitals.
        NameLabel = "NAME",
        MinecraftVersionLabel = "MINECRAFT VERSION",
        ModLoaderLabel = "MOD LOADER",
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
        /// {version} is "1.21.4", {loader} "Fabric", {loader_version} "0.16.9".
        GameWithLoader = "Minecraft {version} with {loader} {loader_version}",
        NoVersionSelected = "No version selected",
        NoModpackPicked = "No modpack picked",
        PickModpack = "Pick a modpack and a version.",
        ChooseGameVersion = "Choose a Minecraft version.",
        CreateInstance = "Create instance",
        InstallModpack = "Install modpack",
        ImportMrpack = "Import .mrpack…",
        /// The button of the file dialog that picks .mrpack files.
        Import = "Import",
    }

    pub enum InstanceSettingsPlural {
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
    }
}
