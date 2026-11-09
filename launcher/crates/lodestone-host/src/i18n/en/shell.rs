//! The window: starting the core, the first run, signing in, the top bar, the sidebar, the routes' names and the home page. Owned by the foundation.

crate::i18n::messages! {
    pub enum ShellMsg {
        /// While the core starts.
        StartingCore = "Starting Lodestone Core…",
        RestartingCore = "Lodestone Core stopped. Starting it again…",
        CoreFailed = "Lodestone Core couldn’t start",
        CoreLog = "The end of its log:",
        OpenLogFolder = "Open the log folder",
        /// While quitting: the servers stop first.
        StoppingCore = "Stopping your servers…",
        SigningIn = "Signing in…",
        /// The first run: the owner account.
        SetupTitle = "Set up Lodestone Core",
        SetupIntro = "Create the owner account. You sign in with it from the web dashboard and your other devices.",
        Username = "Username",
        Password = "Password",
        ConfirmPassword = "Confirm password",
        CreateOwner = "Create owner account",
        NoUsername = "Enter a username.",
        NoPassword = "Enter a password.",
        PasswordsDiffer = "The passwords don’t match.",
        /// The first run, after the owner account.
        FirstConfigTitle = "Name your core",
        CoreName = "Core name",
        PublicAddress = "Public domain or IP address",
        PublicAddressHint = "Players join your servers with this address. You can leave it empty.",
        SignInTitle = "Sign in",
        SignInIntro = "This core doesn’t let apps on this computer claim owner access. Sign in with your Lodestone account.",
        /// The sidebar.
        SelectedInstance = "Selected instance",
        AllInstances = "All instances",
        NoInstanceSelected = "No instance selected",
        NoInstances = "No instances yet",
        NewServer = "New server",
        NoPermission = "No permission",
        /// An instance being made.
        SettingUp = "Setting up {name}…",
        /// An instance's tabs.
        TabOverview = "Overview",
        TabSettings = "Settings",
        TabConsole = "Console",
        TabFiles = "Files",
        TabMods = "Mods",
        TabPlugins = "Plugins",
        TabMacros = "Macros",
        TabEventLogs = "Event logs",
        TabPlayit = "Playit.gg",
        /// The settings and their pages.
        Settings = "Settings",
        SettingsCore = "Core",
        SettingsGeneral = "General",
        SettingsUsers = "Users",
        SettingsAccount = "Account",
        SettingsProfile = "Profile",
        /// An instance's state.
        StateStarting = "Starting",
        StateRunning = "Running",
        StateStopping = "Stopping",
        StateStopped = "Stopped",
        StateError = "Crashed",
        StateUnknown = "Unknown",
        /// The power menu (Start and Stop are Common).
        Restart = "Restart",
        Kill = "Kill",
        PlayersOnline = "{online}/{max} players",
        /// The connection to the core, in the footer.
        Connected = "Connected",
        Connecting = "Connecting…",
        Reconnecting = "Reconnecting…",
        SignedOut = "Signed out",
        SignedInAs = "Signed in as {name}",
        /// The page without a selected instance.
        HomeTitle = "Welcome to Lodestone Host",
        HomeSubtitle = "Select a server in the sidebar, or create a new one.",
        CreateServer = "Create a new server",
        /// A page still to be built (development builds).
        NotBuilt = "This page isn’t in Lodestone Host yet.",
        NeedsScripting = "Macros need a core with scripting. Lodestone Host’s core runs without it.",
        InstanceNotFound = "Server not found",
        /// The menu bar icon.
        ShowHost = "Show Lodestone Host",
        QuitHost = "Quit Lodestone Host",
        /// Beside the logo in development builds.
        DevBadge = "DEV",
        CoreVersion = "Version {version}",
    }

    pub enum ShellPlural {
    }
}
