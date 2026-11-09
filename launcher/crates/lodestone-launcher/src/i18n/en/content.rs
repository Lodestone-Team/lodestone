//! The Content tab: mods, resource packs and shader packs, browsing Modrinth and a project's page.

crate::i18n::messages! {
    pub enum ContentMsg {
        /// The section switch.
        KindMods = "Mods",
        KindResourcePacks = "Resource packs",
        KindShaders = "Shaders",
        /// An instance without a mod loader, in "1.21.1 / Vanilla".
        Vanilla = "Vanilla",

        /// The search over the list: it filters what is installed and searches
        /// Modrinth too.
        SearchInstalledOrModrinth = "Search",
        /// Headings over the list, with how many rows follow.
        InstalledCount = "Installed · {count}",
        /// Resource packs in use; the top one wins where they overlap.
        EnabledCount = "Enabled · {count} · the top pack wins where packs overlap",
        AvailableCount = "Available · {count}",
        /// The heading of the Modrinth results below the installed ones.
        OnModrinthCount = "On Modrinth · {count}",
        OnModrinth = "On Modrinth",
        /// The heading over CurseForge's results in the list.
        OnCurseForgeCount = "On CurseForge · {count}",
        OnCurseForge = "On CurseForge",
        /// Nothing installed matches the search; {kind} is "mods", "resource packs" or
        /// "shaders".
        NoInstalledMatch = "Installed · {count} · No installed {kind} match.",
        /// The filter beside the Modrinth heading; {target} is "1.20.1 / Fabric".
        OnlyTarget = "{target} only",


        StopGameFirst = "Stop the game first: it rewrites these files when it exits.",
        WaitForInstall = "Wait for the install to finish.",
        GameRunning = "The game is running. Stop it to change its content: it rewrites options.txt and the shader settings when it exits.",

        AllModsUpToDate = "Every mod from Modrinth is up to date.",
        AllResourcePacksUpToDate = "Every resource pack from Modrinth is up to date.",
        AllShaderPacksUpToDate = "Every shader pack from Modrinth is up to date.",
        CouldntCheckUpdates = "Couldn't check for updates: {error}",

        /// The progress line of an install: {name} is a Modrinth project's title.
        CheckingProject = "Checking {name}",
        CheckingDependencies = "Checking the dependencies",
        /// Under the progress line: "3 of 5 files · 1.2 MB / 3.4 MB".
        FilesProgress = "{done} of {total} files",
        /// Bytes downloaded of the total: "1.2 MB / 3.4 MB".
        BytesProgress = "{done} / {total}",

        NothingInstalled = "Nothing was installed.",
        InstalledOne = "Installed {name}.",
        /// {names} is a list of the others: "Installed Iris, Sodium and Fabric API."
        InstalledMany = "Installed {names} and {last}.",
        /// {files} are file names.
        Replaced = "Replaced {files}.",
        /// {reason} comes from the installer, in English.
        Skipped = "Skipped {name}: {reason}.",

        /// {files} are file names.
        AddedFiles = "Added {files}.",
        /// A file that couldn't be added, and why.
        FileError = "{name}: {error}",
        /// {files} are "file: reason" items.
        NotAddedFiles = "Not added: {files}",
        AddedAndNotAdded = "Added {added}. Not added: {failed}",
        /// The file dialog's button.
        AddToMods = "Add to Mods",
        AddToResourcePacks = "Add to Resource packs",
        AddToShaders = "Add to Shaders",

        /// {kind} is Modrinth's word for the project's type: "modpack", "plugin".
        LinkNotInstallable = "That link is a {kind}, which Lodestone Launcher doesn't install yet.",

        // The installed list.
        CheckForUpdates = "Check for updates",
        /// {count} is how many updates were found.
        UpdateAll = "Update all ({count})",
        AddFile = "Add file…",
        /// An installed item that is a folder: "{name} · folder".
        ItemFolder = "{name} · folder",
        Compatible = "Compatible",
        OlderVersion = "Older version",
        NewerVersion = "Newer version",
        NotOnModrinth = "Not on Modrinth",
        UpdateTo = "Update to {version}",
        NoModsYet = "No mods yet. Browse Modrinth, or drop .jar files here.",
        NoResourcePacksYet = "No resource packs yet. Browse Modrinth, or drop .zip files or folders here.",
        NoShaderPacksYet = "No shader packs yet. Browse Modrinth, or drop .zip files or folders here.",
        /// {folder} is the folder's name: "mods".
        ReadingFolder = "Reading the {folder} folder",
        CouldntReadFolder = "Couldn't read the {folder} folder: {error}",
        SomeFilesNotMatched = "Some files couldn't be matched to Modrinth: {error}",
        ShadersOff = "Shaders off",
        NormalLook = "Minecraft's normal look.",

        // A vanilla instance: content that needs a mod loader.
        ModsNeedLoader = "Mods need a mod loader",
        ShadersNeedLoader = "Shaders need a mod loader",
        /// {loader} is Fabric or Forge.
        VanillaSwitchMods = "This instance is vanilla Minecraft. Mods run through a mod loader such as {loader}; switching keeps your worlds and settings, and {loader} installs the next time you press Play.",
        VanillaSwitchShaders = "This instance is vanilla Minecraft. Shaders run through a mod loader such as {loader}; switching keeps your worlds and settings, and {loader} installs the next time you press Play.",
        VanillaNoLoaderMods = "This instance is vanilla Minecraft {version}. Mods need a mod loader, and none that Lodestone Launcher supports runs on this version: Fabric starts at 1.14, and Forge has no build for it. A new instance on a newer version can use them.",
        VanillaNoLoaderShaders = "This instance is vanilla Minecraft {version}. Shaders need a mod loader, and none that Lodestone Launcher supports runs on this version: Fabric starts at 1.14, and Forge has no build for it. A new instance on a newer version can use them.",
        VanillaCheckFailedMods = "This instance is vanilla Minecraft. Mods need a mod loader, but checking which one fits didn't work: {error}",
        VanillaCheckFailedShaders = "This instance is vanilla Minecraft. Shaders need a mod loader, but checking which one fits didn't work: {error}",
        VanillaCheckingMods = "This instance is vanilla Minecraft. Mods run through a mod loader; checking which one runs on Minecraft {version}…",
        VanillaCheckingShaders = "This instance is vanilla Minecraft. Shaders run through a mod loader; checking which one runs on Minecraft {version}…",
        SwitchTo = "Switch to {loader}",
        NewFabricInstance = "New Fabric instance…",

        // The Shaders section's shader mod card. {shader_mod} is Iris, Oculus or OptiFine.
        ShaderModUsingPack = "{shader_mod} is using {pack}.",
        ShaderModUsingBuiltIn = "{shader_mod} is using its built-in shaders.",
        ShaderModInstalledOff = "{shader_mod} is installed; shaders are off.",
        /// {file} is a config file: "config/iris.properties".
        ShaderModPickPack = "Pick a pack below. {shader_mod} reads its choice from {file} when the game starts.",
        ShadersNeedShaderMod = "Shaders need a shader mod",
        /// {add_file} is the "Add file…" button, {mods} the Mods section.
        OptiFineNote = "OptiFine works too, but it isn't on Modrinth and its licence doesn't allow Lodestone to download it. If you prefer it, download it from optifine.net and add the jar with {add_file} in {mods} (Fabric also needs OptiFabric).",
        LookingForShaderMod = "Looking for a shader mod for this version on Modrinth",
        CouldntAskModrinth = "Couldn't ask Modrinth: {error}",
        InstallShaderMod = "Install {shader_mod} {version}",
        /// {loader} is the instance's mod loader: Fabric, NeoForge...
        ShaderModRunsOn = "Minecraft can't load shader packs by itself; a shader mod does that. {shader_mod} runs on this instance's {loader}.",
        /// Follows ShaderModRunsOn: "It comes with Sodium, which Iris needs."
        ShaderModComesWith = "It comes with {dependency}, which {shader_mod} needs.",
        /// {reason} comes from Modrinth's lookup, in English.
        ShaderModUnavailable = "Minecraft can't load shader packs by itself; a shader mod does that. {reason} Fabric instances can always use Iris.",

        // Browse.
        BrowsingNeedsNetwork = "Browsing Modrinth needs the network.",
        /// The sort order of the results.
        SortRelevance = "Relevance",
        SortDownloads = "Downloads",
        SortFollows = "Follows",
        SortNewest = "Newest",
        SortUpdated = "Updated",
        SearchingModrinth = "Searching Modrinth",
        NothingMatches = "Nothing on Modrinth matches.",
        PageOf = "Page {page} of {pages}",
        /// A project's author.
        ByAuthor = "by {author}",
        /// A badge: the project is installed.
        InstalledBadge = "Installed",
        OptionalOnServers = "Optional on servers",
        /// A large count, shortened: "1.2k downloads" ("1.2万" in Chinese, so no space
        /// is needed there). Smaller counts use the plural.
        DownloadsCompact = "{count} downloads",
        FollowsCompact = "{count} follows",
        FollowersCompact = "{count} followers",

        // A project's page.
        LoadingProject = "Loading the project",
        ViewOnModrinth = "View on Modrinth",
        Versions = "Versions",
        Description = "Description",
        LoadingVersions = "Loading versions",
        ShowAllVersions = "Show versions for other loaders and Minecraft versions",
        AlsoInstallDependencies = "Also install required dependencies",
        /// {show_all} is the ShowAllVersions switch, shortened: "Show versions…".
        NoVersionFits = "No version of {project} fits this instance. Turn on \"{show_all}\" to see them all.",
        ShowVersionsShort = "Show versions…",
        InstallAnyway = "Install anyway…",
        InstallItAnyway = "Install it anyway",
        /// A shader pack that also installs the shader mod: "Install with Iris".
        InstallWith = "Install with {shader_mod}",
        /// Modrinth's version types.
        Release = "Release",
        Beta = "Beta",
        Alpha = "Alpha",

        // CurseForge.
        /// The sub-view switch with CurseForge: Installed / Modrinth / CurseForge.
        ModrinthTab = "Modrinth",
        /// The switch, and the badge of a file that came from CurseForge.
        CurseForgeTab = "CurseForge",
        SearchingCurseForge = "Searching CurseForge",
        ViewOnCurseForge = "View on CurseForge",
        /// A file neither platform knows (added by hand).
        NotOnEitherPlatform = "Not on Modrinth or CurseForge",
        /// CurseForge's sort orders, with SortDownloads and SortUpdated.
        SortPopular = "Popular",
        SortName = "Name",
        /// A CurseForge file the author only lets people download from curseforge.com.
        WebsiteDownload = "Website download",
        /// An update that has to be downloaded from curseforge.com.
        GetOnCurseForge = "Get it on CurseForge",
        /// The card for files downloaded from curseforge.com by hand.
        BlockedTitle = "Download these from CurseForge",
        /// {folder} is the folder watched for the downloads.
        BlockedLead = "Their authors don't let launchers download them. Download each one in your browser; Lodestone adds it as soon as it appears in {folder}.",
        BlockedNoFolder = "Their authors don't let launchers download them. Download each one in your browser, then choose the folder it was saved to.",
        BlockedOpen = "Open download page",
        BlockedOpenAll = "Open all pages",
        BlockedAdded = "Added",
        /// {file} is the file name to download.
        BlockedWaiting = "Waiting for {file}",
        BlockedChangeFolder = "Watch another folder…",
        BlockedSkip = "Skip the rest",
        /// The folder dialog's button.
        WatchFolderPrompt = "Watch this folder",
        CouldntAddDownload = "Couldn't add {name}: {error}",
        /// {names} is the list of files the user downloaded.
        AddedDownloads = "Added {names}.",
    }

    pub enum ContentPlural {
        Downloads = ("{count} download", "{count} downloads"),
        Follows = ("{count} follow", "{count} follows"),
        Followers = ("{count} follower", "{count} followers"),
        MoreVersions = ("{count} more version is on Modrinth.", "{count} more versions are on Modrinth."),
    }
}
