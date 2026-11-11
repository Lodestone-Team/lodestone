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
        Username = "Username",
        Password = "Password",
        NoUsername = "Enter a username.",
        NoPassword = "Enter a password.",
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
        TabEventLogs = "Event logs",
        /// The settings and their pages.
        Settings = "Settings",
        /// The app menu's item that opens the Settings dialog.
        SettingsMenu = "Settings…",
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
        /// The selected instance's players pill, as the dashboard's card: "3/20 Online".
        PlayersOnline = "{online}/{max} Online",
        /// The selected instance's power button, which opens Start, Stop, Restart or Kill; {state} is its state.
        PowerMenu = "Power: {state}",
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
        InstanceNotFound = "Server not found",
        /// The menu bar icon.
        ShowHost = "Show Lodestone Host",
        QuitHost = "Quit Lodestone Host",
        /// Beside the logo in development builds.
        DevBadge = "DEV",
        CoreVersion = "Version {version}",
        /// The footer and the banner when the core answers but its live updates stopped.
        Degraded = "Live updates stopped",
        DegradedBanner = "Lodestone Host reaches the core, but live updates stopped. What you see may be out of date.",
        Reload = "Reload",
        /// The footer's tooltip.
        CoreAddress = "Address: {address}",
        CoreHost = "Computer: {host}",
        CoreSystem = "System: {system}",
        CoreUptime = "Running for {uptime}",
    }

    pub enum ShellPlural {
    }
}
