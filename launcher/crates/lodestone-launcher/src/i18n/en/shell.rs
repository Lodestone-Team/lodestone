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
        /// A game that has started but whose window hasn't opened yet.
        Launching = "Launching",
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
        Servers = "Lodestone Host",
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
        /// The automatic sidebar group of instances linked to a server: on a Lodestone
        /// core, or a friend's shared server. It comes first.
        GroupServers = "Servers",

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
        LinkedToRemoved = "Linked to {server} on {core} (that core was removed from the Lodestone Host page)",
        NoInstances = "No instances",
        /// Under PLAY, for an instance made for a server.
        AndJoin = "and join {server}",
        PlayWithoutJoining = "Play without joining",
        /// The Play button over an instance's icon in the sidebar, for assistive technology.
        PlayNamed = "Play {name}",

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
        CauseModPatch = "A mod couldn't load its changes into the game. It's probably made for a different Minecraft version, or it needs a newer version of another mod. Update the mod or turn it off.",
        /// A heading in the crash dialog (shown in capitals), above the one mod the
        /// crash points at.
        ProbablyCausedBy = "Probably caused by",
        /// A heading in the crash dialog (shown in capitals), above several mods.
        ModsInvolved = "Mods involved",
        ModsInvolvedHint = "Any of these could be the cause. Try turning them off one at a time.",
        /// Why the crash dialog names a mod.
        EvidenceFailedToPatch = "It failed to load, so it may be made for a different Minecraft version.",
        EvidenceCrashedInItsCode = "The crash happened inside this mod.",
        EvidencePatchedCrashingCode = "It changes the part of the game that crashed.",
        EvidenceInStack = "It was running when the game crashed.",
        /// The crash dialog's buttons on a mod: rename it to .disabled, and back.
        TurnOffMod = "Turn off",
        TurnOnMod = "Turn on",
        ModTurnedOff = "Turned off. Play again to see if that fixed it. You can turn it back on here or in the Content tab.",
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
        SharedLead = "Keep your settings in sync across instances.",
        NoAccount = "No account",
        ManageAccounts = "Manage accounts",
        GroupAutomatically = "Group instances automatically",
        GroupAutomaticallyHint = "Sorts ungrouped instances by server or loader.",
        /// Settings > Instance defaults: a toggle, on by default.
        AutoConfigure = "Set up new instances automatically",
        AutoConfigureHint = "Before a new instance's first game, chooses its memory, Java arguments and video settings for this computer and its mods, and copies your mouse sensitivity and key bindings when shared settings don't. Anything you change yourself is kept.",
        /// Settings > General: what the launcher does once a game's window opens.
        WhenAGameStarts = "When a game starts",
        AfterLaunchStay = "Stay open",
        AfterLaunchMinimize = "Minimize",
        AfterLaunchQuit = "Quit",
        AfterLaunchStayHint = "The launcher stays open beside the game.",
        AfterLaunchMinimizeHint = "The launcher minimizes once the game's window opens, and comes back when the game closes.",
        AfterLaunchQuitHint = "The launcher quits once the game's window opens. The game keeps running, but its log, crash report and play time aren't recorded.",
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
        SyncedFrom = "{name} is now your main instance.",
        SyncedFromNone = "No main instance now: edit the shared settings here.",
        SharedCleared = "The shared values are cleared. Instances keep their own options.txt.",
        NoInstance = "No main instance",
        /// Beside "No main instance" in the Main instance list.
        EditHereOnly = "edit here only",
        /// The placeholder of a shared value being edited.
        Value = "Value",
        SyncFrom = "Main instance",
        SyncFromHint = "Settings you change in-game in your main instance are copied to your other instances when they start.",
        SyncFromNoneHint = "Without a main instance, the shared settings change only here.",
        /// The offer to make a new instance the main one; also asked before another
        /// instance takes over as main.
        MainOfferTitle = "Make {name} your main instance?",
        MainOfferBody = "Shared game settings keep your controls, video, sound and other options the same in every instance. Settings you change in-game in your main instance are copied to the others when they start.",
        MainChangeLater = "You can change this later in Settings › Shared game settings.",
        MakeItMain = "Make it main",
        MainSwitchBody = "{current} is your main instance now. Its in-game changes will stop updating your shared settings, and {name}'s will update them instead.",
        MakeNameMain = "Make {name} main",
        /// Settings > Developer.
        FirstTimeSetupSection = "First-time setup",
        ResetFirstTimeSetup = "Reset first-time setup",
        ResetFirstTimeSetupHint = "The next new instance is offered as your main instance again, keeping the narrator off is offered at the next start, and the Lodestone Host page offers Lodestone Host again. This clears your main instance; the shared values stay.",
        FirstTimeSetupReset = "First-time setup is reset. Create an instance to see it.",
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
        /// The notification at start that offers "Keep the narrator off".
        NarratorOfferTitle = "Keep the narrator off?",
        NarratorOfferBody = "Lodestone Launcher can keep Minecraft's narrator off in all your instances, new ones included, and skip the first-start accessibility screen.",
        NarratorOfferAccept = "Keep it off",
        NarratorOfferDismiss = "Dismiss",

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
        /// Settings > Developer: the section of "Sign in as the Minecraft Launcher".
        SignInSection = "Microsoft sign-in",
        /// Settings > Developer: the toggle for offline accounts.
        OfflineAccountsSetting = "Offline accounts",
        /// {add} is "Add offline account".
        OfflineAccountsSettingHint = "Shows “{add}” in the account menu. For testing only.",
        /// The hint while offline accounts are on whatever the setting says.
        OfflineAccountsAlwaysOn = "Always on in debug builds and with LODESTONE_LAUNCHER_OFFLINE=1.",
        /// Settings > Developer: the section of the Lodestone Connect service setting.
        ConnectSection = "Lodestone Connect",
        /// Settings > Developer: the Lodestone Connect introduction service to use.
        ConnectService = "Connect service (relay)",
        ConnectServiceHint = "For testing. The friend list uses this service. Invite links always use the service they name.",
        /// Under the setting, and next to Friends' servers, while the service in use isn't
        /// connect.lodestone.cc: {service} is its address, "connect-dev.lodestone.cc".
        ConnectServiceInUse = "Dev relay: {service}",
        /// While the environment variable {var} overrides the setting; {service} is its URL.
        ConnectServiceEnv = "{var} is set, so the launcher uses {service}. The setting applies once it is unset.",
        /// Errors for a typed service URL.
        ConnectServiceScheme = "Start the address with https://.",
        ConnectServiceInsecure = "Use https://. Only a service on this computer (127.0.0.1 or localhost) can use http://.",
        ConnectServiceExtra = "Type only the address, without a path, query or user name.",
        ConnectServiceAddress = "That isn't a valid address.",
        /// Settings > Developer: what lodestone-launcher-cli may do.
        /// Settings > Developer > Coding agents.
        CodingAgentsSection = "Coding agents",
        CodingAgentsLead = "Claude Code, Codex and other coding agents can drive the launcher through lodestone-launcher-cli. The launcher keeps guides for them in its data folder.",
        AgentSkills = "Skills",
        AgentPrompt = "Set up a coding agent",
        AgentPromptHint = "Paste this into your agent. It installs lodestone-launcher-cli, reads the skills, checks the connection and starts debugging with you.",
        CopyPrompt = "Copy prompt",
        CommandLineSection = "Command line",
        CommandLineLead = "lodestone-launcher-cli and other programs you run reach the launcher through a socket in its data folder. Only your user account can connect. These settings change only here, never from the command line.",
        AllowCli = "Allow command-line connections",
        AllowCliHint = "Off, the socket is closed and connected programs are dropped.",
        /// What command-line programs may do.
        CliAccess = "Access",
        CliAccessHint = "Read only answers questions but refuses anything that changes instances, settings, accounts or files.",
        CliFullControl = "Full control",
        CliReadOnly = "Read only",
        DebugTraces = "Debug traces",
        DebugTracesHint = "Records what you click and what the launcher does, for lodestone-launcher-cli trace. Off, nothing is recorded.",
        /// Whether command-line programs may read game output.
        CliGameLogs = "Game logs",
        CliGameLogsHint = "Programs may read the game’s console output.",
        /// The control socket's file.
        SocketPath = "Socket",
        CopyPath = "Copy path",
        OpenDataFolder = "Open data folder",
        /// The socket row while connections are off.
        SocketClosed = "Closed",
        /// {error} says why.
        SocketError = "The socket couldn’t open: {error}",
        ConnectedClients = "Connected programs",
        NoClients = "No programs are connected.",
        /// A connected program that didn't say what it is.
        UnnamedClient = "Unnamed program",
        /// {time} is "Today, 14:30".
        ClientSince = "Connected {time}",
        /// {pid} is a process number.
        ClientPid = "Process {pid}",
        Disconnect = "Disconnect",
        /// Settings > Developer: the section of the performance monitor.
        DiagnosticsSection = "Diagnostics",
        /// The performance monitor window, and its row in Settings > Developer.
        PerfMonitor = "Performance monitor",
        PerfMonitorHint = "A small window with the launcher’s memory, CPU and frame times, and a running game’s, updated every second.",
        PerfOpen = "Open monitor",
        /// The launcher's own process.
        PerfLauncher = "Launcher",
        PerfMemory = "Memory",
        PerfCpu = "CPU",
        /// {percent} is "12.5%", {cores} the number of cores.
        PerfCpuValue = "{percent} of one core · {cores} cores",
        /// Windows, counted as Task Manager does: {percent} is the share of the whole
        /// processor, {one} the share of one core, {cores} the number of cores.
        PerfCpuValueAll = "{percent} of all {cores} cores · {one} of one core",
        PerfThreads = "Threads",
        PerfOpenFiles = "Open files",
        /// Windows: open handles of every kind, as Task Manager calls them.
        PerfHandles = "Handles",
        /// How the main window draws.
        PerfRendering = "Rendering",
        PerfFps = "Frames per second",
        /// How long each frame took to draw.
        PerfFrameTime = "Frame time",
        /// {avg}, {p95} and {max} are times such as "16.7 ms".
        PerfFrameStats = "avg {avg} · p95 {p95} · max {max}",
        PerfIdle = "Idle: nothing redrew in the last 2 seconds",
        /// The main window isn't the focused one (this monitor usually is).
        PerfBackground = "The launcher’s window is in the background, so it animates at no more than 30 frames per second.",
        /// Under the frame stats.
        PerfFramesNote = "The launcher redraws only when something on screen changes. Frame time is how long each redraw took.",
        PerfGame = "Game",
        PerfNoGame = "No game is running.",
        PerfInternals = "Inside the launcher",
        PerfImages = "Decoded images",
        /// Modrinth answers kept in memory.
        PerfContentCache = "Content cache",
        /// Command-line jobs running.
        PerfJobs = "Jobs",
        PerfDownloads = "Downloads",
        PerfCliClients = "CLI clients",
        PerfTraceBus = "Trace bus",
        /// {subscribers} is a number, {rate} "3.5".
        PerfTraceValue = "{subscribers} listening · {rate} events/s",
        /// {rate} is a size such as "2.1 MB".
        PerfRate = "{rate}/s",
        PerfSelfNote = "Sampled once a second while this window is open. It measures itself too: its own redraws add a little CPU.",
        PerfCopy = "Copy snapshot",
        PerfCopied = "Copied",
        /// Under the small charts.
        PerfLastMinute = "Last 60 s",
        /// A number the system doesn't give.
        PerfUnknown = "–",
        PerfOff = "Off",

        // Signing in with Microsoft.
        SignInWithMicrosoft = "Sign in with Microsoft",
        LauncherIdentityNote = "Developer option: signing in as the Minecraft Launcher, not as Lodestone Launcher. Microsoft's pages may show Minecraft's name instead of Lodestone's.",
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
        /// The button beside the sign-in code.
        CopySignInCode = "Copy code",
        /// Asked on Back or closing once the sign-in code has been copied.
        StopSigningInTitle = "Stop signing in?",
        StopSigningInBody = "Microsoft is still waiting for the code you copied. If you stop now, entering it won't sign you in.",
        StopSigningIn = "Stop signing in",
        KeepWaiting = "Keep waiting",
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
        /// Settings > General > Updates: update to beta builds too.
        BetaBuilds = "Beta builds",
        BetaBuildsHint = "Get new features before everyone else. Beta builds can have bugs. Turning this off keeps your build until a stable one is newer.",
        DeveloperBuilds = "Developer builds",
        DeveloperBuildsHint = "For Lodestone's developers: updates to builds of work in progress, which may not work. Turning this off keeps your build until a release is newer.",
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
        /// Beside the top bar's logo in a development build (`cargo build`).
        DevBadge = "DEV",
        /// The top bar's arrows: the page shown before, and back again.
        GoBack = "Back",
        GoForward = "Forward",

        // Shared components.
        /// Under a Modrinth description that is cut short.
        RestOnModrinth = "The rest of the description is on Modrinth.",
        // Settings > Storage.
        Storage = "Storage",
        /// Settings > Skin: the look of the launcher's window.
        Skin = "Skin",
        SkinGallery = "Skins",
        SkinImport = "Import…",
        /// On Linux and Windows, whose file dialogs show files or folders but not
        /// both, "Import…" splits in two: this one imports a skin pack's .zip...
        SkinImportZip = "Import skin as zip",
        /// ...and this one a skin pack that isn't zipped.
        SkinImportFolder = "Import skin as folder",
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
        /// While a skin pack is being imported or exported, in the background.
        SkinImporting = "Importing…",
        SkinExporting = "Exporting…",
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
        SkinMaterial = "Material",
        SkinMaterialHint = "Lets what is behind the panel show through: the window's background, the page or a dialog. Blur and Frost show it blurred.",
        SkinMaterialTranslucent = "Translucent",
        SkinMaterialBlur = "Blur",
        SkinMaterialFrost = "Frost",
        SkinMaterialOpacity = "Fill shows",
        SkinMaterialBlurAmount = "Background blur",
        SkinMaterialFrostVeil = "Frost veil",
        SkinMaterialGrain = "Grain",
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
        /// Settings > Storage: the section with the data folder's path.
        StorageDataFolder = "Data folder",
        StorageDataFolderHint = "Your instances and worlds, accounts, settings and game files. Move it to keep them on another drive.",
        StorageMoveEllipsis = "Move…",
        /// Moves the data folder back to the default folder.
        StorageMoveBackEllipsis = "Move back…",
        /// The folder dialog's button when choosing where to move the data folder.
        StorageMoveHere = "Move here",
        StorageMoveCantUse = "Can't move the data there: {error}",
        StorageMoveStopGame = "Stop the game before moving the data folder.",
        StorageMoveTitle = "Move your data?",
        StorageMoveBody = "Everything in the data folder moves to {to}: your instances and worlds, accounts, settings and game files. The launcher restarts when it's done.",
        StorageMoveBackTitle = "Move your data back?",
        StorageMoveBackBody = "Everything moves back to the default folder, {to}. The launcher restarts when it's done.",
        StorageMoveSize = "It's {size}.",
        StorageMoveConfirm = "Move and restart",
        StorageMoving = "Moving your data…",
        StorageMovingTo = "To {to}",
        StorageMovingPreparing = "Getting ready…",
        StorageMovingProgress = "{done} of {total}",
        StorageMovingNote = "Cancel leaves it where it was.",
        StorageMoveFailedTitle = "Couldn't move your data",
        StorageMoveFailedBody = "Your data is still in {from}. {error}",
        StorageMoveRestartFailed = "Your data moved. Quit the launcher and open it again to use the new folder.",
        /// At start, when the moved data folder isn't there.
        DataFolderMissingTitle = "Can't find your data folder",
        DataFolderMissingBody = "Your instances and worlds are in {path}, which isn't there. If it's on a drive that isn't connected, connect it and try again. The default folder starts with nothing in it, and your data stays where it is.",
        DataFolderUseDefault = "Use the default folder",
        DataFolderQuit = "Quit",
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
        /// Event and trace subscriptions a connected program has.
        Subscriptions = ("{count} subscription", "{count} subscriptions"),
        PerfEntries = ("{count} entry", "{count} entries"),
    }
}
