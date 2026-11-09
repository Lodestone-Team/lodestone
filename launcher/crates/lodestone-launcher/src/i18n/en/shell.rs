//! The window's frame: top bar, sidebar, Play tab, the launcher's Settings dialog, accounts and sign-in, and the shared components.

crate::i18n::messages! {
    pub enum ShellMsg {
        /// The app menu's Quit; {app} is "Lodestone Launcher".
        QuitApp = "Quit {app}",
        /// Settings > General.
        Language = "Language",
        /// The Language setting's first choice.
        LanguageSystem = "System",
        /// The setting's button while it is System: {language} is the language it uses.
        LanguageSystemIs = "System ({language})",
        /// Settings > General: the interface's size.
        Zoom = "Zoom",
        /// The menu bar's View menu.
        ViewMenu = "View",
        ZoomIn = "Zoom In",
        ZoomOut = "Zoom Out",
        /// Back to 100% zoom.
        ActualSize = "Actual Size",
        /// The menu bar's Edit menu, and its items.
        EditMenu = "Edit",
        Undo = "Undo",
        Redo = "Redo",
        /// A toast after Undo: {change} says what the setting is now, "Language → Français".
        Undid = "Undid: {change}",
        Redid = "Redid: {change}",
        /// {setting} is a setting's name, {value} its value now.
        ChangeTo = "{setting} → {value}",
        /// Several settings of an instance at once.
        SettingsOf = "Settings of {name}",

        // The instance's tabs, and the Play button.
        /// The Play tab, and the large Play button (shown in capitals there).
        Play = "Play",
        /// Shown in capitals on the Play button while the game runs.
        Playing = "Playing",
        TabContent = "Content",
        TabWorlds = "Worlds",
        /// The instance's Settings tab, the sidebar's Settings and the dialog.
        Settings = "Settings",
        TabLogs = "Logs",

        // The sidebar.
        /// A heading over the instance played last (shown in capitals).
        LastPlayed = "Last played",
        /// A heading over the instance list, and the Settings group of its pages
        /// (shown in capitals in the sidebar).
        Instances = "Instances",
        NoInstancesYet = "No instances yet.",
        NewInstance = "New instance…",
        ImportInstances = "Import instances…",
        Servers = "Servers",
        /// Under the sidebar and the Settings pages.
        AppVersion = "Lodestone Launcher v{version}",
        /// A row's tooltip: {when} is "Played 2 hours ago", {time} "2 h 5 min".
        PlayedInTotal = "{when} · {time} in total",
        /// A modpack being imported: "45% · Downloading mods".
        ImportProgress = "{percent} · {label}",
        /// An automatic sidebar group of instances with a mod loader: "Fabric (modded)".
        GroupModded = "{loader} (modded)",
        /// The automatic sidebar group of instances without a mod loader, and the
        /// "loader" of such an instance.
        Vanilla = "Vanilla",
        /// The automatic sidebar group of instances imported from other launchers.
        GroupImported = "Imported",

        // The Play tab.
        NoInstanceSelected = "No instance selected",
        NoInstanceSelectedHint = "Create an instance to choose a Minecraft version and loader.",
        MinecraftVersion = "Minecraft {version}",
        /// {loader} is the loader with its version: "Fabric 0.16.14".
        MinecraftWith = "Minecraft {version} with {loader}",
        /// {game} is "Minecraft 1.20.1 with Fabric 0.16.14", {time} "2 h 5 min".
        HeroPlayed = "{game}. Played for {time}.",
        HeroNotPlayed = "{game}. Press Play to download it and start the game.",
        CreateFirstInstance = "Create your first instance",
        InstanceExplainer = "An instance is a Minecraft version with its own mods, worlds and settings.",
        ContentPlaceholder = "Content appears here once you have an instance.",
        WorldsPlaceholder = "Worlds appear here once you have an instance.",
        SettingsPlaceholder = "Settings appear here once you have an instance.",
        LogsPlaceholder = "Logs appear here once you have an instance.",
        /// Under the instance's name: {server} is a server, {core} a Lodestone Core.
        LinkedTo = "Linked to {server} on {core}",
        LinkedToRemoved = "Linked to {server} on {core} (that core was removed from the Servers page)",
        /// The bottom bar's pickers (shown in capitals).
        PickerInstance = "Instance",
        PickerVersion = "Version",
        NoInstances = "No instances",
        /// Under PLAY, for an instance made for a server.
        AndJoin = "and join {server}",
        PlayWithoutJoining = "Play without joining",

        // Dialogs over the window.
        /// The title of a dialog that deletes an instance or a world.
        DeleteNamed = "Delete {name}?",
        DeleteWorldBody = "The world's folder moves to {trash}, so you can put it back from there. Its backups stay.",
        DeleteInstanceBody = "The instance's folder, with its worlds, mods and settings, moves to {trash}. You can put it back from there.",
        ReplaceWorldTitle = "Replace {name} with this backup?",
        /// {when} is when the backup was made: "today, 14:05".
        ReplaceWorldBody = "{name} goes back to how it was {when}. Everything since is lost from the world, but it is backed up first and the current folder goes to {trash}.",
        BackUpAndReplace = "Back up and replace",
        /// {loader} is a mod loader's name.
        SwitchToLoader = "Switch to {loader}?",
        SwitchToVanilla = "Switch to vanilla?",
        ModsWontLoadOn = "These enabled mods are made for another loader and won't load on {loader}:",
        ModsWontLoadVanilla = "Vanilla Minecraft loads no mods, so these enabled mods won't load:",
        /// What a mod is made for: "Forge mod".
        ModMadeFor = "{loader} mod",
        TurningOffRenames = "Turning them off renames them to .disabled, so you can turn them back on in Content.",
        WorldsAtRisk = "Worlds that used blocks or items from these mods can lose them. Back up your worlds first.",
        TurnOffAndSave = "Turn them off and save",
        SaveAndKeep = "Save and keep them",
        RemoveAccountTitle = "Remove {name}?",
        RemoveAccountBody = "Lodestone Launcher forgets this account and deletes its saved sign-in from your {keychain}. You can add it again later.",
        RemoveAccount = "Remove account",
        /// {description} is the crash's own text, in English.
        MinecraftCrashed = "Minecraft crashed: {description}.",
        MinecraftExitCode = "Minecraft stopped with exit code {code}.",
        MinecraftStopped = "Minecraft stopped unexpectedly.",
        CrashTitle = "{name} crashed",
        /// A heading in the crash dialog (shown in capitals).
        LikelyCause = "Likely cause",
        OpenCrashReport = "Open crash report",
        /// The likely causes of a crash.
        CauseWrongJava = "Wrong Java version. This Minecraft version (or a mod) needs a different Java. Clear the Java override in Settings to use the Java Mojang provides.",
        CauseOutOfMemory = "Out of memory. Raise the maximum memory in Settings, or close other apps.",
        CauseNatives = "Native libraries are missing or are for the wrong processor. On Apple Silicon, versions before 1.19 run in an Intel Java under Rosetta 2; check that Rosetta is installed and that the Java override isn't an arm64 Java.",
        CauseJvmOptions = "Java refused one of the settings. Check the JVM arguments and memory in Settings.",
        CauseGraphics = if cfg!(target_os = "macos") {
            "The game couldn't set up its window or OpenGL. Update macOS, or try without a custom window size or fullscreen."
        } else {
            "The game couldn't set up its window or OpenGL. Update your graphics drivers, or try without a custom window size or fullscreen."
        },
        CopyLog = "Copy log",

        // A modpack update.
        UpdateTitle = "Update {name}?",
        /// {pack} is the modpack, {from} and {to} its versions, {size} "38 MB".
        ModpackSummary = "{pack} {from} → {to}. Minecraft {game} with {loader}. {size} to download.",
        NoFilesChange = "No files change; only the version is recorded.",
        UpdatedFiles = "Updated ({count})",
        AddedFiles = "Added ({count})",
        RemovedFiles = "Removed ({count})",
        ReplacedOwnFiles = "Your files the pack now provides ({count})",
        KeptFiles = "Kept as you changed them ({count})",
        WrittenFiles = "Configs and other files written ({count})",
        ModpackUpdateSafety = "Your own mods, worlds and changed configs aren't touched. Every file that is replaced or removed is saved in the instance's backups folder first.",
        BackUpEveryWorld = "Back up every world first",

        // Unsaved settings.
        SaveChangesTitle = "Save your changes?",
        UnsavedDefaultsBody = "You changed Instance defaults. Save them, or discard them?",
        UnsavedInstanceBody = "You changed the settings of {name}. Save them, or discard them?",
        UnsavedThisInstanceBody = "You changed the settings of this instance. Save them, or discard them?",
        UnsavedInDefaults = "You have unsaved changes in Instance defaults",

        // The Settings dialog.
        /// A group of Settings pages.
        GroupLauncher = "Launcher",
        InstanceDefaults = "Instance defaults",
        SharedGameSettings = "Shared game settings",
        General = "General",
        Accounts = "Accounts",
        Developer = "Developer",
        DefaultsLead = "Each instance can change these in its own settings.",
        SharedLead = "The same game options in every instance.",
        NoAccount = "No account",
        ManageAccounts = "Manage accounts",
        GroupAutomatically = "Group instances automatically",
        GroupAutomaticallyHint = "Sorts ungrouped instances by loader.",
        OfflineAccount = "Offline account",
        MicrosoftAccount = "Microsoft account",
        SignInAgain = "Sign in again",
        /// A tag on the account Play uses.
        InUse = "In use",
        /// Makes an account the one in use.
        Use = "Use",
        AddAnAccount = "Add an account",
        /// Adds a Microsoft account.
        AddMicrosoftEllipsis = "Microsoft…",
        /// Adds an offline account.
        AddOfflineEllipsis = "Offline…",

        // Settings > Instance defaults.
        Memory = "Memory",
        MemoryError = "Memory: {error}",
        /// {total} is "This Mac has 16 GB of memory." (or nothing), {computer}
        /// "this Mac", {min} and {max} "512 MB", {limit} "4 GB".
        MemoryDefaultHint = "Default: {min} – {max}",
        /// The unit beside the memory fields.
        MegabytesUnit = "MB",
        Window = "Window",
        Fullscreen = "Fullscreen",
        JvmArguments = "JVM arguments",
        GameArguments = "Game arguments",
        EnvironmentVariables = "Environment variables",

        // Settings > Shared game settings.
        SyncedFrom = "Settings changed in-game in {name} now update every instance.",
        SyncedFromNone = "No instance updates the shared settings now; edit them here.",
        SharedCleared = "The shared values are cleared. Instances keep their own options.txt.",
        NoInstance = "No instance",
        /// Beside "No instance" in the Sync from list.
        EditHereOnly = "edit here only",
        /// The placeholder of a shared value being edited.
        Value = "Value",
        SyncFrom = "Sync from",
        SyncFromHint = "In-game changes in this instance update the others.",
        SyncFromNoneHint = "Shared values change only here.",
        StartFromInstance = "Start from an instance",
        CopyFromInstance = "Copy from instance…",
        ClearShared = "Clear the shared values",
        ClearSharedHint = "Instances keep their own options.txt.",
        ClearEveryShared = "Clear every shared value",
        ClearEllipsis = "Clear…",
        NoValuesYet = "No values yet",
        Hide = "Hide",
        Show = "Show",
        ServerList = "Server list",
        ServerListHint = "Coming soon.",
        /// A section of Shared game settings: where the values come from.
        Source = "Source",
        WhatsShared = "What's shared",
        WhatsSharedLead = "Categories that are off stay per instance.",
        CategoryControls = "Controls",
        CategoryVideo = "Video",
        CategorySound = "Sound",
        CategoryAccessibility = "Accessibility & chat",
        CategoryLanguage = "Language",
        CategoryControlsHint = "Mouse, key bindings",
        CategoryVideoHint = "FOV, render distance, GUI scale, graphics",
        CategorySoundHint = "Volume, subtitles",
        CategoryAccessibilityHint = "Narrator, chat, text background",
        CategoryLanguageHint = "Game language and font",
        /// Shared game settings > Narrator.
        NarratorSection = "Narrator",
        KeepNarratorOff = "Keep the narrator off",
        KeepNarratorOffHint = "Also skips the first-start accessibility screen.",
        /// {category} is "Accessibility & chat".
        NarratorNotShared = "{category} isn't shared, so each instance keeps its own narrator setting.",
        NarratorKeptOff = "The narrator stays off in every instance, new ones included.",
        NarratorPerInstance = "Each instance keeps its own narrator setting again.",

        // The account box and its menu.
        SigningIn = "Signing in…",
        OfflineCouldntConnect = "Offline – couldn't connect",
        AddMinecraftAccount = "Add a Minecraft account",
        /// Short, in the account menu: an offline account.
        Offline = "Offline",
        AddMicrosoftAccount = "Add Microsoft account",
        AddOfflineAccount = "Add offline account",
        /// The account menu's last row.
        RemoveNamed = "Remove {name}",
        /// Shown wherever an offline account appears.
        OfflineNote = "Offline – single-player and offline-mode servers only",

        // "Add offline account".
        OfflineDialogBody = "For development and testing. The account has no Microsoft sign-in, and its UUID is the one offline-mode servers give this name.",
        Username = "Username",
        UsernameRule = "3 to 16 letters, numbers or underscores.",
        AddAccount = "Add account",

        // Settings > Developer.
        /// The badge on accounts signed in as the Minecraft Launcher.
        LauncherIdentity = "Launcher identity",
        SignInAsLauncher = "Sign in as the Minecraft Launcher",
        SignInAsLauncherHint = "Until Lodestone's own app is approved. For testing only.",
        SignInAsLauncherTitle = "Sign in as the Minecraft Launcher?",
        WhatThisDoes = "What this does",
        WhatThisDoesBody = "Lodestone Launcher will tell Microsoft that it is Mojang's official Minecraft Launcher, using that launcher's public client ID, instead of Lodestone's own registered app. Microsoft and Mojang haven't approved this, Lodestone isn't affiliated with them, and it may be against Microsoft's terms for their apps.",
        Risks = "Risks",
        RisksBody = "Microsoft can block this at any time. If it does, accounts signed in this way stop working. Use it only for personal testing until Lodestone's app is approved.",
        WhyHere = "Why it's here",
        WhyHereBody = "Lodestone's own app is still waiting for Mojang's approval, so normal Microsoft sign-in can't finish yet.",
        WhenSignIn = "When you sign in",
        /// {add} is "Add Microsoft account".
        WhenSignInBody = "{add} gives you a code. Enter it at microsoft.com/link and sign in to your Microsoft account there. Microsoft's pages may show Minecraft's name instead of Lodestone's. If the code doesn't work, you can sign in with a link instead: your browser opens login.live.com, and you paste back the address of the blank page it ends on.",
        WhatLauncherGets = "What the launcher gets",
        /// {keychain} is "macOS Keychain", {badge} "Launcher identity".
        WhatLauncherGetsBody = "The same as a normal sign-in: tokens that let it play Minecraft as that account. The long-lived one is kept in your {keychain} and the rest only in memory. These accounts are marked \"{badge}\" in the account menu.",
        HowToUndo = "How to undo it",
        HowToUndoBody = "Turn this setting off and sign those accounts out. Lodestone Launcher offers to sign them out when you turn it off.",
        TurnItOn = "I understand, turn it on",
        TurnOffLauncherTitle = "Turn off the Minecraft Launcher sign-in?",
        TurnOffLauncherBody = "New Microsoft sign-ins will use Lodestone's own app again.",
        OneLauncherAccount = "{name} is signed in as the Minecraft Launcher.",
        /// {accounts} is the sentence that says which accounts.
        TurnOffLauncherAccounts = "{accounts} Kept accounts go on working until Microsoft blocks this. Signing out removes them from Lodestone Launcher and deletes their saved sign-ins from your {keychain}.",
        TurnOffAndSignOut = "Turn off and sign out",
        TurnOffOnly = "Turn off only",

        // Signing in with Microsoft.
        SignInWithMicrosoft = "Sign in with Microsoft",
        LauncherIdentityNote = "Developer option: signing in as the Minecraft Launcher, not as Lodestone Launcher. Microsoft's pages may show Minecraft's name instead of Lodestone's.",
        UseLinkInstead = "Use a sign-in link instead",
        UseCodeInstead = "Use a code instead",
        SignInCodeBody = "Sign in with the Microsoft account you play Minecraft: Java Edition with. You'll get a code to enter on Microsoft's website.",
        SignInBrowserBody = "Sign in with the Microsoft account you play Minecraft: Java Edition with. Your browser opens to finish signing in.",
        /// {keychain} is "macOS Keychain".
        KeepsSignIn = "Lodestone Launcher keeps your sign-in in the {keychain}.",
        GetCode = "Get a code",
        WaitingForBrowser = "Waiting for your browser…",
        FinishInBrowser = "Finish signing in in the browser window that opened, then come back here.",
        OpenBrowserAgain = "Open the browser again",
        GettingCode = "Getting a code…",
        /// {host} is "microsoft.com/link".
        GoToHost = "Go to {host} on any device and enter this code:",
        CodeCopiedWaiting = "Code copied. Waiting for you to enter it…",
        WaitingForCode = "Waiting for you to enter the code…",
        CopyCodeAndOpen = "Copy code and open {host}",
        LinkInstructions = "Sign in in the browser window that opened. Microsoft then shows a page saying it is not normally shown. Copy the full address from the browser's address bar and paste it only into this launcher.",
        /// {url} is the start of the address to paste.
        LinkStartsWith = "It starts with {url}",
        SigningInToMinecraft = "Signing in to Minecraft…",
        ExpectedUntilApproved = "This is expected until Mojang approves the app. Nothing is wrong with your account.",

        // Sign-in errors: a title, then what happened and what to do.
        AuthCancelled = "Sign-in cancelled",
        AuthCancelledBody = "Sign-in was cancelled.",
        AuthTimedOut = "Sign-in timed out",
        AuthTimedOutBody = "The browser sign-in wasn't finished in time. Try again, or use a code instead.",
        AuthStateMismatch = "Sign-in didn't match",
        AuthStateMismatchBody = "The browser came back with a different sign-in request, so Lodestone Launcher ignored it. Try again.",
        AuthDeclined = "Sign-in declined",
        AuthDeclinedBody = "The sign-in was declined in the browser. Try again and accept the permissions to continue.",
        AuthMicrosoft = "Microsoft sign-in failed",
        /// {code} is Microsoft's error code.
        AuthMicrosoftBody = "Microsoft couldn't sign you in ({code}). Try again in a moment.",
        /// {description} is Microsoft's own text, {code} its error code.
        AuthMicrosoftSaid = "Microsoft said: {description} ({code})",
        AuthCodeExpired = "The code expired",
        AuthCodeExpiredBody = "The code expired before sign-in finished. Get a new code and try again.",
        AuthSignInExpiredBody = "Your saved Microsoft sign-in has expired or was revoked. Sign in again to keep using this account.",
        AuthNoXbox = "No Xbox profile",
        AuthNoXboxBody = "This Microsoft account doesn't have an Xbox profile yet. Sign in once at xbox.com to create one, then try again.",
        AuthRegion = "Xbox Live isn't available",
        AuthRegionBody = "Xbox Live isn't available in this account's country or region, so it can't sign in to Minecraft.",
        AuthAdult = "Adult verification needed",
        AuthAdultBody = "This account needs adult verification on the Xbox website before it can play. This is required in South Korea.",
        AuthChild = "Child account",
        AuthChildBody = "This is a child account. An adult needs to add it to a Microsoft family group before it can sign in.",
        AuthXbox = "Xbox Live sign-in failed",
        AuthXboxBody = "Xbox Live refused the sign-in (error {code}). Try again later.",
        AuthNotApproved = "Not approved by Minecraft yet",
        AuthNotApprovedBody = "Microsoft sign-in works, but Minecraft hasn't approved Lodestone Launcher yet. Use offline mode for now.",
        AuthNotOwned = "Minecraft not found",
        AuthNotOwnedBody = "This Microsoft account doesn't own Minecraft: Java Edition. If you play through Xbox Game Pass, start Minecraft once in the official launcher, then try again.",
        AuthNoProfile = "No Minecraft profile",
        AuthNoProfileBody = "This account has no Minecraft profile yet. If you play through Xbox Game Pass, start Minecraft once in the official launcher to pick a name, then try again.",
        AuthRateLimited = "Too many attempts",
        AuthRateLimitedBody = "Too many sign-in attempts. Wait a few minutes and try again.",
        AuthNetwork = "Couldn't connect",
        AuthNetworkBody = "Lodestone Launcher couldn't reach the sign-in servers. Check your internet connection and try again.",
        AuthUnexpected = "Sign-in failed",
        /// {step} is a step of signing in, such as "Xbox Live sign-in".
        AuthUnexpectedBody = "Something went wrong during {step} (HTTP {status}). Try again in a moment.",
        AuthListener = "Couldn't start sign-in",
        AuthListenerBody = "Lodestone Launcher couldn't open the local page the browser returns to. Use a code instead.",
        AuthKeychain = "Couldn't save the sign-in",
        /// {keychain} is "Keychain", "Credential Manager" or "keyring".
        AuthKeychainBody = "Lodestone Launcher couldn't save the sign-in to your {keychain}. Try again.",
        AuthInvalidLink = "That's not the right address",
        AuthNotAUrlBody = "That isn't a web address. Copy the whole address from your browser's address bar after signing in.",
        /// {url} is the start of the address to paste.
        AuthWrongPageBody = "Only an address starting with {url} is accepted. Copy it from the address bar of the blank page Microsoft shows after you sign in.",
        AuthNoCodeBody = "That address has no sign-in code in it. Finish signing in first, then copy the address of the page you end up on.",
        StepMicrosoftToken = "Microsoft sign-in",
        StepDeviceCode = "Microsoft device code sign-in",
        StepXboxLive = "Xbox Live sign-in",
        StepXsts = "Xbox Live authorization",
        StepMinecraftLogin = "Minecraft sign-in",
        StepEntitlements = "the Minecraft ownership check",
        StepProfile = "loading the Minecraft profile",

        // Problems with the saved accounts.
        AccountsTooNew = "Your accounts were saved by a newer Lodestone Launcher. Changes made here won't be saved.",
        AccountsCorrupt = "accounts.json couldn't be read, so Lodestone Launcher started without accounts.",
        /// {path} is where the unreadable file was moved.
        AccountsCorruptMoved = "accounts.json couldn't be read, so Lodestone Launcher started without accounts. The old file is at {path}.",
        CouldntReadAccounts = "Couldn't read your accounts: {error}",
        CouldntSaveSettings = "Couldn't save your settings: {error}",
        CouldntSaveAccounts = "Couldn't save your accounts: {error}",
        CouldntRemoveSignIn = "Couldn't remove {name}'s saved sign-in: {error}",
        AddAccountFirst = "Add an account first: sign in with Microsoft, or add an offline account.",
        SignInExpired = "Sign in to {name} again: its saved sign-in has expired.",
        LauncherClosing = "The launcher is closing.",

        // Settings > General > Updates, and the top bar's notice.
        /// Settings > General: the section about updating the launcher.
        UpdatesSection = "Updates",
        UpdateAutomatically = "Update automatically",
        UpdateAutomaticallyHint = "Downloads new builds from GitHub in the background.",
        /// The running build: the number of commits it was built from.
        BuildNumber = "Build {build}",
        /// {commit} is the first seven characters of the commit.
        BuildNumberCommit = "Build {build} · {commit}",
        DevelopmentBuild = "Development build",
        UpdatesUnavailable = "This build wasn't published on GitHub, so it doesn't update itself.",
        UpdateNotChecked = "Checks GitHub for new builds every few hours.",
        UpdateChecking = "Looking for a newer build…",
        UpToDate = "This is the newest build.",
        UpdateAvailable = "Build {build} is available.",
        /// {percent} is how much is done: "42%".
        UpdateDownloading = "Downloading build {build}… {percent}",
        UpdateReady = "Build {build} is installed. Restart to use it.",
        UpdateReadyPlaying = "Build {build} is installed. You can restart once the game is closed.",
        /// {error} is in English, from the updater.
        UpdateFailed = "Couldn't update: {error}",
        CheckForUpdates = "Check now",
        DownloadUpdate = "Download",
        RestartToUpdate = "Restart",
        /// The top bar's notice, beside its Restart button.
        UpdateReadyBadge = "Update ready",
        UpdateAvailableBadge = "Update available",
        /// The window's own buttons in the top bar, on Linux desktops that leave them
        /// to apps (GNOME): their tooltips and accessible names. Close is Common's.
        MinimizeWindow = "Minimize",
        MaximizeWindow = "Maximize",
        /// Un-maximizes the window, back to the size it had.
        RestoreWindow = "Restore",

        // Shared components.
        /// Under a Modrinth description that is cut short.
        RestOnModrinth = "The rest of the description is on Modrinth.",
        // Settings > Storage.
        Storage = "Storage",
        /// Settings > Skin: the look of the launcher's window.
        Skin = "Skin",
        SkinGallery = "Skins",
        SkinImport = "Import…",
        /// The file dialog's button that imports a skin.
        SkinImportButton = "Import",
        SkinExport = "Export…",
        SkinNew = "New skin…",
        SkinDelete = "Delete",
        /// {name} is the skin's name.
        SkinDeleteConfirm = "Delete {name}",
        /// The name of a copy of skin {name}.
        SkinCopyName = "{name} copy",
        /// Built-in skins can't change, so a change makes a copy.
        SkinCopied = "Made a copy of {name} to change it.",
        SkinImported = "Imported {name}.",
        SkinExported = "Exported to {path}.",
        SkinSafeMode = "Started without a skin (--no-skin).",
        /// {error} is the problem, in English.
        SkinCouldntLoad = "Couldn't load the skin: {error}",
        SkinSidebar = "Sidebar",
        SkinImage = "Image",
        SkinChoose = "Choose…",
        /// The file dialog's button that picks an image.
        SkinUseImage = "Use image",
        SkinRemove = "Remove",
        SkinFit = "Fit",
        SkinFitCover = "Cover",
        SkinFitTile = "Tile",
        SkinTint = "Tint",
        SkinTintStrength = "Tint strength",
        SkinBlur = "Blur",
        SkinBanner = "Instance banner",
        SkinBannerInSidebar = "Show behind sidebar rows",
        /// The large image on the Play tab.
        SkinHero = "Play hero",
        SkinHeroHint = "For instances without their own.",
        SkinPlayButton = "Play button",
        SkinColor = "Colour",
        SkinFadeTo = "Fade to",
        SkinShape = "Shape",
        SkinShapeRounded = "Rounded",
        SkinShapePill = "Pill",
        SkinShapePixel = "Pixel",
        SkinLabel = "Label",
        SkinGlow = "Glow",
        SkinColors = "Colours",
        SkinAccent = "Accent",
        SkinProgress = "Progress bar",
        SkinDefault = "Default",
        SkinNone = "None",
        SkinAdjusted = "The accent was adjusted to stay readable.",
        SkinReset = "Reset to Default",
        /// An instance's own colour, over the skin's.
        SkinPlayColor = "Play button colour",
        /// Use the skin's colour, not the instance's own.
        SkinUseSkins = "Skin's",
        /// A part of the window a skin can paint.
        SkinSurfaceWindow = "Window",
        SkinSurfaceTopBar = "Top bar",
        SkinSurfaceSidebar = "Sidebar",
        SkinSurfaceMain = "Page",
        SkinSurfaceHero = "Play picture",
        SkinSurfacePlayBar = "Play bar",
        SkinSurfaceCard = "Cards",
        SkinSurfaceSettingCard = "Settings groups",
        SkinSurfaceRow = "List rows",
        SkinSurfaceDialog = "Dialogs",
        SkinSurfaceSettingsNav = "Settings pages",
        SkinSurfacePopover = "Menus",
        SkinSurfaceInput = "Text fields",
        SkinSurfaceButtonPrimary = "Primary buttons",
        SkinSurfaceButtonSecondary = "Secondary buttons",
        SkinSurfaceButtonGhost = "Quiet buttons",
        SkinSurfaceButtonDanger = "Danger buttons",
        SkinSurfaceToggle = "Switches",
        SkinSurfaceTab = "Tabs",
        SkinSurfacePill = "Pills",
        SkinSurfacePlayButton = "Play button",
        SkinSurfaceProgress = "Progress bars",
        SkinSurfaceToast = "Notices",
        /// Which text a readability check is about.
        SkinRoleText = "text",
        SkinRoleMuted = "muted text",
        SkinRoleFaint = "hints and icons",
        SkinRoleLabel = "label",
        /// {surface} is a part of the window, {role} which text, {ratio} and {target} contrast ratios ("3.1", "4.5").
        SkinCheckShort = "{surface}, {role}: {ratio}:1, needs {target}:1",
        SkinCheckNoFix = "No text colour gets there on this fill; change the fill.",
        SkinApplyFix = "Apply suggested fix",
        SkinFixAll = "Fix all",
        /// {count} is a number.
        SkinChecksPass = "All text is readable ({count} checks).",
        SkinChecksNote = "Readability is up to the skin: these are suggestions.",
        /// Banners a skin offers, in an instance's background picker.
        SkinBanners = "From the skin",
        SkinBannerMissing = "This banner's skin is gone; the default is shown.",
        /// The pop-out window for editing a skin.
        SkinEditorTitle = "Skin editor",
        SkinPopOut = "Pop out editor",
        SkinEditorLead = "Edit every surface, font and banner in the editor. Changes show in the window as you make them, and Undo takes them back.",
        /// Each placeholder is a number.
        SkinSummary = "{surfaces} surfaces painted · {fonts} fonts · {banners} banners",
        SkinSurfaces = "Surfaces",
        SkinTokens = "Text colours",
        SkinFonts = "Fonts",
        SkinBannerSet = "Banners",
        SkinPlayButtonSection = "Play button",
        SkinReadability = "Readability",
        SkinFill = "Fill",
        SkinFillColour = "Colour",
        SkinFillGradient = "Gradient",
        SkinOpacity = "Opacity",
        SkinGradientLinear = "Linear",
        SkinGradientRadial = "Radial",
        SkinAngle = "Angle",
        /// The colours along a gradient.
        SkinStops = "Colours",
        SkinStopAt = "Position",
        SkinAddStop = "Add colour",
        SkinRemoveStop = "Remove colour",
        SkinBuiltinImages = "Built-in images",
        SkinBorder = "Border",
        SkinBorderWidth = "Border width",
        SkinRadius = "Corners",
        SkinTextColour = "Text",
        SkinMutedColour = "Muted text",
        SkinTextElsewhere = "Text on this surface follows the text colours.",
        /// The theme's text colours.
        SkinTokenText = "Text",
        SkinTokenSecondary = "Labels",
        SkinTokenMuted = "Descriptions",
        SkinTokenFaint = "Hints",
        SkinTokenBorder = "Lines",
        /// Parts of the window with a font of their own.
        SkinFontHeading = "Headings",
        SkinFontBody = "Body",
        SkinFontSidebar = "Sidebar",
        SkinFontButton = "Buttons",
        SkinFontPlay = "Play button and title",
        SkinFontTab = "Tabs",
        SkinFontMono = "Code and logs",
        SkinFontAdd = "Add font file…",
        SkinBannerAdd = "Add banner…",
        SkinBannersNone = "No banners yet. New instances keep the default Play picture.",
        SkinBannersLead = "A new instance takes one at random; each can switch in its Play background.",
        SkinSaveAsNew = "Save as new skin",
        SkinUseLaunchers = "Use the launcher’s",
        SkinPreviewIn = "Show in the window",
        SkinPreviewSettings = "Settings",
        SkinPreviewMenu = "A menu",
        SkinPreviewNotice = "A notice",
        SkinPreviewNoticeText = "This is how notices look.",
        SkinSampleText = "Sample text",
        SkinSampleMuted = "Quieter text",
        SkinScrim = "Shade under the title",
        StorageInstances = "Instances",
        StorageGameFiles = "Game files",
        StorageRuntimes = "Java runtimes",
        StorageCache = "Cache",
        StorageLeftovers = "Unfinished downloads and imports",
        StorageCleanLeftoversTitle = "Remove unfinished downloads?",
        StorageCleanLeftoversBody = "This frees {size}. These are files of modpack downloads and imports that didn't finish.",
        StorageSnapshots = "Snapshots",
        StorageSnapshotsHint = "{count} snapshots · {size} can be freed",
        StorageCleanSnapshotsTitle = "Remove automatic snapshots?",
        StorageCleanSnapshotsBody = "This frees {size}. It removes the snapshots taken before going back, those of deleted instances, and files made to share.",
        StorageOther = "Settings and other files",
        StorageCacheAndLogs = "Cache and logs",
        StorageLogs = "Logs and crash reports",
        StorageOldLogs = "{size} older than 30 days",
        StorageVersions = "Versions",
        StorageLibraries = "Libraries",
        StorageAssets = "Assets",
        StorageUnused = "Not used by any instance",
        StorageUnusedTag = "Unused",
        StorageJava = "Java {major}",
        StorageWorlds = "Worlds {size}",
        StorageContent = "Mods and packs {size}",
        StorageOtherFiles = "Other {size}",
        StorageNoInstances = "No instances",
        StorageNoRuntimes = "No Java runtimes",
        StorageCounting = "Counting…",
        StorageCouldntCount = "Couldn't count the files: {error}",
        StorageStopGame = "Stop the game to remove game files, Java runtimes or the cache.",
        StorageOpenFolder = "Open folder",
        StorageOpen = "Open",
        StorageDelete = "Delete…",
        StorageRemoveEllipsis = "Remove…",
        StorageClearEllipsis = "Clear…",
        StorageRemove = "Remove",
        StorageClear = "Clear",
        StorageDeleteOld = "Delete old…",
        StorageDeleteAll = "Delete all…",
        StorageDeleteConfirm = "Delete",
        StorageFreed = "Freed {size}",
        StorageCouldntRemove = "Couldn't remove everything: {error}",
        StorageCleanGameFilesTitle = "Remove unused game files?",
        StorageCleanGameFilesBody = "This frees {size}. Versions, libraries and assets no instance uses are deleted, and download again if an instance needs them.",
        StorageCleanRuntimesTitle = "Remove unused Java runtimes?",
        StorageCleanRuntimesBody = "This frees {size}. They download again when a Minecraft version needs them.",
        StorageClearCacheTitle = "Clear the cache?",
        StorageClearCacheBody = "This frees {size}. The launcher downloads what it needs again.",
        StorageDeleteOldLogsTitle = "Delete old logs?",
        StorageDeleteOldLogsBody = "This frees {size}. Logs and crash reports older than 30 days are deleted for good.",
        StorageDeleteAllLogsTitle = "Delete all logs?",
        StorageDeleteAllLogsBody = "This frees {size}. Every log and crash report is deleted for good, except a running game's.",
    }

    pub enum ShellPlural {
        /// Under a shortened list: "and 3 more".
        AndMore = ("and {count} more", "and {count} more"),
        /// How many shared values a category has.
        Values = ("{count} value", "{count} values"),
        CopiedValues = ("Copied {count} value from {name}.", "Copied {count} values from {name}."),
        /// {names} is the list of accounts.
        LauncherAccounts = (
            "{count} account is signed in as the Minecraft Launcher: {names}.",
            "{count} accounts are signed in as the Minecraft Launcher: {names}."
        ),
    }
}
