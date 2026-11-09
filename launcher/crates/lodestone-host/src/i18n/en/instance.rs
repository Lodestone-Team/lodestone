//! Work package C: an instance's Settings, Mods, Event logs and Playit.gg tabs.

crate::i18n::messages! {
    pub enum InstanceMsg {
        /// The Settings tab (`InstanceSettings.tsx`).
        Automation = "Automation",
        AutomationLead = "When Lodestone starts this server by itself.",
        AutoStart = "Auto start",
        AutoStartHint = "Start this server when Lodestone Core starts",
        RestartOnCrash = "Restart on crash",
        RestartOnCrashHint = "Start this server again if it crashes",
        /// A section of the core's settings that has none yet.
        NotAvailable = "Not available at this moment",
        NotAvailableHint = "Try to start this server at least once",
        NoPermission = "No permission",
        SettingsFailed = "Couldn’t load the settings: {error}",
        DangerZone = "Danger zone",
        DangerZoneLead = "These settings can cause irreversible damage to your server.",
        DeleteServer = "Delete server",
        DeleteServerHint = "Permanently deletes this server and its data",
        /// The title of the confirmation dialog; {name} is the server's name.
        DeleteTitle = "Delete “{name}”",
        DeleteConfirm = "I understand, delete this server",
        DeleteCannotUndo = "This action cannot be undone.",
        DeleteWarning = "This instance’s settings, worlds and backups will be permanently deleted. Please backup any important data before proceeding.",
        Deleting = "Deleting {name}…",
        /// Why a setting can't be changed.
        OwnerOnlyAtom = "Only the owner can control atom servers.",
        CommandLineReadOnly = "Read-only while code execution is disabled on this core.",
        CommandLineOwnerOnly = "Only the owner can change this setting.",
        /// A setting's text field.
        CannotBeEmpty = "Cannot be empty",
        MustBeNumber = "Must be a number",
        MustBeGreaterThan = "Must be greater than {min}",
        MustBeLessThan = "Must be less than {max}",
        ShowPassword = "Show",
        HidePassword = "Hide",
        Revert = "Revert",
        /// The resource pack (server.properties).
        ResourcePack = "Resource pack",
        ResourcePackLead = "A pack players are asked to download when they join.",
        PackUrl = "URL",
        PackUrlHint = "A direct link to the pack zip. Leave it empty for none.",
        PackSha1 = "SHA-1",
        PackSha1Hint = "Lets players check the download and cache the pack.",
        PackSha1Error = "The SHA-1 is 40 hexadecimal characters.",
        /// Computes the SHA-1 of the pack.
        PackCompute = "Compute",
        /// {size} is the size of the pack, such as "1.5 MB".
        PackDownloaded = "Downloaded {size}.",
        PackRequired = "Require the pack",
        PackRequiredHint = "Players who decline it are disconnected.",
        PackPrompt = "Prompt",
        PackPromptHint = "The message shown when players are asked. Plain text, or a JSON text component.",
        PackRemove = "Remove pack",
        PackSaved = "Resource pack saved",
        PackSavedRestart = "Resource pack saved. Restart the server to use it.",
        PackUrlSpaces = "The URL can’t have spaces.",
        PackNotAUrl = "This isn’t a URL.",
        PackUrlScheme = "The URL must start with http:// or https://.",
        /// The Minecraft EULA: only the user accepts it, with their own click.
        Eula = "Minecraft EULA",
        EulaLead = "A Minecraft server only runs after you agree to the Minecraft EULA.",
        EulaAccepted = "Accepted",
        EulaAcceptedHint = "You agreed to the Minecraft EULA for this server.",
        EulaNotAccepted = "Not accepted",
        EulaNotAcceptedHint = "This server doesn’t start until you agree to the Minecraft EULA.",
        EulaRead = "Read the EULA",
        /// Accepts the EULA; the user clicks it after reading the link beside it.
        EulaAgree = "I agree to the Minecraft EULA",
        /// {error}: the core's error message.
        EulaAcceptFailed = "Could not accept the Minecraft EULA: {error}",
        EulaNoPermission = "You need permission to start this server to accept it.",
        /// The dialog when a server doesn't start because its EULA isn't accepted.
        EulaDialogTitle = "Accept the Minecraft EULA",
        /// {name} is the server's name.
        EulaDialogBody = "{name} only starts after you agree to the Minecraft EULA. Do you agree to it?",
        /// Accepts the EULA and starts the server.
        EulaAcceptAndStart = "I agree, start the server",
        /// The Playit.gg tab (`PlayitggOverview.tsx`). The runner and its tunnels are the core's.
        PlayitLoadFailed = "Couldn’t load the playit.gg status: {error}",
        PlayitSignupFailed = "Couldn’t make a sign-up link: {error}",
        PlayitVerifyFailed = "Couldn’t check the playit.gg account: {error}",
        PlayitStartFailed = "Couldn’t start the playit.gg runner: {error}",
        PlayitStopFailed = "Couldn’t stop the playit.gg runner: {error}",
        /// The sign-up dialog.
        PlayitSetupTitle = "Set up playit.gg",
        PlayitSetupIntro = "Sign up for playit.gg to use it. Make a sign-up link with the button below.",
        PlayitGenerateLink = "Make a sign-up link",
        PlayitFollowLink = "Open the link and make an account. When you see the playit.gg dashboard, come back and press Verify.",
        PlayitOpenLink = "Open the sign-up link",
        /// Checks the new account; named in PlayitFollowLink.
        PlayitVerify = "Verify",
        /// The program that runs the tunnels on this computer.
        PlayitRunner = "playit.gg runner",
        PlayitRunnerLead = "Runs your tunnels on this computer, so players can join without port forwarding.",
        PlayitStopped = "Stopped",
        PlayitLoading = "Starting",
        PlayitStarted = "Started",
        PlayitTrouble = "Having trouble? Check that the tunnel is set up on the playit.gg website and that your server is running.",
        PlayitSignupRequired = "Sign up for playit.gg before you use it.",
        PlayitSignUp = "Sign up",
        PlayitTunnels = "Tunnels",
        PlayitTunnelsLead = "Make more tunnels and change them on the playit.gg website.",
        PlayitWebsite = "Open playit.gg",
        /// {address} is an IP address and port, such as 127.0.0.1:25565.
        PlayitLocalServer = "Local server at {address}",
        PlayitCopyAddress = "Copy the address",
        /// {address} is the tunnel's public address.
        PlayitCopied = "Copied {address}",
        PlayitActive = "Active",
        PlayitInactive = "Not active",
        PlayitNoTunnels = "You haven’t made any tunnels yet",
        PlayitNotSignedUp = "You haven’t signed up yet",
        /// A link to the integration's documentation.
        PlayitHelp = "Need help?",
        /// The Event logs tab: what happened to this server.
        EventLogsLead = "What happened to this server, newest first. The core keeps the recent events.",
        EventLogsEmpty = "Nothing has happened yet",
        EventLogsFailed = "Couldn’t load the events: {error}",
        /// {state} is the server's new state, such as Running.
        EventState = "The server is now {state}",
        /// {names} is a list of player names.
        EventJoined = "{names} joined",
        EventLeft = "{names} left",
        /// {user} is who did it.
        EventBy = "by {user}",
    }

    pub enum InstancePlural {
    }
}
