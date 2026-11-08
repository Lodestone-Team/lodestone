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
        /// {group} is an automatic group: "Fabric (modded)", "Vanilla".
        GroupHintAuto = "Instances in the same group are listed together. Without one it's listed under {group}.",
        GroupHint = "Instances in the same group are listed together.",
        NameMissing = "Give the instance a name.",
        Icon = "Icon",
        IconHint = "Shown in the sidebar and around the launcher.",
        PlayBackground = "Play background",
        PlayBackgroundHint = "Shown behind the name on the Play tab. The launcher darkens it under the text as much as the image needs.",
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
        ModLoaderHint = "A change installs the next time you press Play.",
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
        UpdatesHint = "An update replaces the pack's own files and keeps your mods, configs and worlds. You see what changes first.",
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
        LinkedServerHint = "On {core}. Play joins it and keeps the mods in step.",
        LinkedServerCoreRemoved = "On {core}, which was removed from the Servers page. Add the core again there to join the server.",
        CoreRemoved = "Core removed",
        Unlink = "Unlink",
        Unlinked = "Unlinked",
        UnlinkNote = "Unlinking keeps the instance and its mods. Play won't join the server or update its mods.",

        // Java & memory, Launch options.
        /// Sets a value for this instance instead of the launcher's.
        ChangeSetting = "Change",
        JavaMemoryLead = "Values marked Default come from the launcher's Settings. Change one to set it for this instance only.",
        Memory = "Memory",
        Checking = "Checking…",
        /// {major} is "21", {version} is "21.0.5".
        JavaFound = "Java {major} (version {version})",
        JavaTooOld = "Java {major} (version {version}). Minecraft {game} needs Java {needed} or newer.",
        /// The Java Mojang provides for the game, "Mojang's Java 21".
        MojangJavaVersion = "Mojang's Java {major}",
        MojangJava = "Mojang's Java",
        JavaOverrideHint = "A java binary to use instead of the one Mojang provides.",
        JavaHint = "Mojang provides the right Java for each Minecraft version.",
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
        ArgumentsHint = "Separated by spaces; quote values with spaces.",
        Window = "Window",
        Fullscreen = "Fullscreen",
        WindowHint = "Leave the size empty for the game's default.",
        EnvironmentVariables = "Environment variables",
        EnvHint = "NAME=value, separated by spaces.",

        // Maintenance.
        VerifyGameFiles = "Verify game files",
        VerifyHint = "Checks every file and downloads any that are missing or damaged.",
        Verify = "Verify",
        VerifyStarted = "Checking every file. The progress is on the Play button.",
        Export = "Export",
        ExportHint = "A .mrpack file that other launchers can import.",
        ExportButton = "Export…",
        Exporting = "Exporting…",
        Duplicate = "Duplicate",
        DuplicateHint = "A copy with its own folder, mods and worlds.",
        HostOnCore = "Host on a Lodestone core",
        HostHint = "Makes a server on one of your cores with this instance's mods.",
        HostButton = "Host…",
        DeleteInstance = "Delete instance",
        DeleteHint = "Its folder, with its worlds and mods. You confirm first.",
        DeleteButton = "Delete…",

        // Shared game settings.
        SharedLead = "Mouse, key bindings, video, sound and more from options.txt, kept the same in every instance.",
        UseSharedSettings = "Use shared settings",
        SharingOff = "Off: this instance keeps its own options.txt, and nothing is read back from it.",
        SharingPreferred = "This is the preferred instance: settings you change in-game here update every instance.",
        /// {name} is the preferred instance's name.
        SharingFrom = "Settings changed in-game in {name} reach this instance at its next launch.",
        SharingNoPreferred = "No instance is preferred, so shared values change only in the launcher's Settings.",
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
        CategoryControlsDescription = "Mouse sensitivity, invert, raw input, key bindings, toggle crouch and sprint",
        CategoryVideoDescription = "FOV, render and simulation distance, GUI scale, brightness, fullscreen, VSync, max FPS, particles, graphics",
        CategorySoundDescription = "Every volume slider, subtitles, directional audio",
        CategoryAccessibilityDescription = "Narrator, text background, chat size and opacity, effects",
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
