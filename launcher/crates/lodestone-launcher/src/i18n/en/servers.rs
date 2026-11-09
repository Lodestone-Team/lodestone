//! The Servers page, a server's page, cores and their dialogs.
//!
//! A "server" is a game server a Lodestone Core runs; a "core" is the Lodestone Core
//! itself. "Add server" adds a core, and lists its servers.

crate::i18n::messages! {
    pub enum ServersMsg {
        // ===== The page =====
        /// The page's title, and the back button on a server's page.
        ServersTitle = "Lodestone Host",
        PageHint = "Servers on your Lodestone cores. Create an instance that matches one, then play on it.",
        /// "On this computer": cores found here, and "Connect as owner".
        OnThisComputer = "On this computer",
        OnThisComputerHint = "Lodestone Host and Lodestone Core running here. Connecting as owner proves this launcher can reach the core's files, so no password is needed.",
        Rescan = "Rescan",
        ScanningLocal = "Looking for cores on this computer…",
        NoLocalCores = "No Lodestone Host or Lodestone Core is running on this computer.",
        ConnectAsOwner = "Connect as owner",
        Connected = "Connected",
        ChooseDataFolder = "Choose folder…",
        ChooseDataFolderPrompt = "Choose",
        ProductHostHint = "the desktop app, with the dashboard in a window",
        ProductCoreHint = "a core on its own (the server program, or Docker)",
        ConnectedAsOwner = "Connected to {name} as its owner",
        ConnectedMadeOwner = "It had no owner yet, so Lodestone Launcher made the owner account {username}. Its password is on the core's card; keep it somewhere safe.",
        ConnectedOwnerKey = "This launcher has an owner key for it now. You can revoke it from the core's dashboard.",
        OwnerSignIn = "Owner sign-in",
        OwnerKey = "Owner key",
        ShowPassword = "Show",
        HidePassword = "Hide",
        CopyUsername = "Copy username",
        CopyPassword = "Copy password",
        CopyApiKey = "Copy API key",
        StartedBy = "Started by {who}",
        StartedDirectly = "Started directly",
        DirectNote = "This core was started directly. Lodestone Core is meant to be run with lodestone_cli, which installs it and keeps it updated.",
        GetLodestoneCli = "Get lodestone_cli",
        CredentialsReadFailed = "Couldn't read the owner keys of cores on this computer",
        CredentialsSaveFailed = "Couldn't save the owner key",
        ClaimNoCore = "No Lodestone Host or Lodestone Core is running on this computer.",
        ClaimRefused = "The core refused to give owner access. Turn on \"Let apps on this computer claim owner access\" in its settings, or update it.",
        ClaimFolderNotFound = "Couldn't find this core's data folder. Choose the folder it keeps its data in (its LODESTONE_PATH). A core in Docker needs its data folder mounted from this computer.",
        ClaimNotThisCore = "The core that answered isn't the one whose folder was found, so nothing was saved.",
        ClaimNoResult = "The core didn't finish the claim. Try again.",
        ClaimNoCertificate = "This core has no HTTPS certificate, so Lodestone Launcher can't make sure its owner key only ever goes to it. Turn on HTTPS in the core's settings, then connect again.",
        ClaimFiles = "Couldn't use the core's data folder: {error}",
        ClaimInvalid = "The core sent something Lodestone Launcher doesn't understand: {error}",
        /// The button (and dialog title) that adds a core by its address.
        AddServer = "Add server",
        /// Closes the message at the top of the page.
        Dismiss = "Dismiss",
        NoServersYet = "No servers yet",
        NoServersHint = "Add a Lodestone core by its address. Without signing in you see the servers its owner lists publicly.",

        // ===== A core's card =====
        /// A badge next to a core's name: the user trusts it for mods.
        BadgeTrusted = "Trusted",
        /// A badge next to a core's name: its own HTTPS certificate is trusted.
        BadgePinned = "Pinned certificate",
        /// After the core's address: "192.168.1.20:16662 · Signed in as peter".
        SignedInAs = "Signed in as {username}",
        SignedIn = "Signed in",
        NotSignedIn = "Not signed in: public servers only",
        /// The toggle on a core's card.
        TrustedForMods = "Trusted for mods",
        Offline = "Offline: couldn't reach this core. Check that it is running and on the same network.",
        /// "List servers publicly" and "Core Settings" are the dashboard's words.
        NotListedPublicly = "The owner hasn't turned on “List servers publicly” in Core Settings, so there's nothing to show without signing in.",
        NoPublicServers = "No servers are listed publicly. Sign in to see the ones you have access to.",
        NoVisibleServers = "No servers you can see on this core.",
        CertChangedWarning = "This core's HTTPS certificate changed since you trusted it, so Lodestone Launcher refuses to connect. The core makes a new certificate when its built-in one is about to expire or its address changes; it can also mean someone is intercepting the connection. Check the new fingerprint before you trust it.",
        /// {computer} is "this Mac", "this PC" or "this computer".
        CertUntrustedWarning = "This core's HTTPS certificate isn't trusted by {computer}. Lodestone cores make their own certificate, so this is expected: check its fingerprint, then trust it.",
        ReviewNewCertificate = "Trust the new certificate…",
        ReviewCertificate = "Trust certificate…",

        // ===== A server's row =====
        /// Opens a server's page.
        Details = "Details",
        CreateInstance = "Create instance",
        StartAndJoin = "Start and join",
        Play = "Play",
        /// Opens a server's world and resource pack.
        More = "More…",
        /// A badge next to a server's name: an instance was made for it.
        InstanceReady = "Instance ready",
        AddressHidden = "The owner hasn't shared this server's address.",

        // ===== A server's state =====
        StateStarting = "Starting",
        StateRunning = "Running",
        StateStopping = "Stopping",
        StateStopped = "Stopped",
        StateCrashed = "Crashed",
        StateUnknown = "Unknown",
        /// The owner doesn't share whether the server runs.
        StateHidden = "Status hidden",
        /// A server's loader that isn't Vanilla, Fabric, Forge, Paper or Spigot.
        LoaderOther = "Other",
        /// Players online, of the most the server takes: "3 / 20 players".
        PlayersOfMax = "{count} / {max} players",

        // ===== What a server's row is doing =====
        BusyFetching = "Getting the server's mods…",
        BusyCreating = "Creating instance, downloading…",
        BusyCreatingFiles = "Creating instance, downloading {done}/{total} files…",
        BusyChecking = "Checking the server's mods…",
        BusyUpdating = "Updating mods…",
        BusyUpdatingFiles = "Updating mods, {done}/{total} files…",
        BusyStarting = "Starting the server…",
        BusyStopping = "Stopping…",

        // ===== Messages at the top of the page =====
        ReadFailed = "Couldn't read your servers",
        SaveFailed = "Couldn't save your servers",
        NoLongerTrusted = "{name} is no longer trusted for mods",
        NoLongerTrustedIdChanged = "Another core answers at {address} now (its id changed). Trust it again on its card only if you trust its owner.",
        NoLongerTrustedOtherCore = "Another core answers at {address} now.",
        NoLongerTrustedNewCertificate = "You trusted a new certificate for it. Trust it for mods again on its card if you're sure it is the same core.",
        NoLongerTrustedCertificateChanged = "Its certificate changed. Trust it for mods again on its card if you're sure it is the same core.",
        SignedOutOf = "Signed out of {name}",
        /// "Settings → API keys" is where the dashboard lists keys. {id} is the key's id.
        KeyStillValid = "The key for this device (lsk_{id}_…) is still valid until it expires. Revoke it under Settings → API keys in the dashboard to end it now.",
        /// {keychain} is "Keychain", "Credential Manager" or "keyring".
        KeySaveFailed = "Couldn't save the key in the {keychain}",
        KeySaveFailedError = "Couldn't save the key in the {keychain}: {error}",
        CheckCertificate = "Check the core's certificate to continue.",
        CertificateNotTrusted = "The certificate wasn't trusted, so your sign-in wasn't sent to the core.",
        CreateFailed = "Couldn't create the instance",
        CreatedWithoutMods = "Created {name} without the server's mods",
        CreatedWithoutModsBody = "The owner hasn't shared this server's client mods, so the instance has only its Minecraft version and loader. Add any mods it needs yourself.",
        LinkFailed = "Couldn't link the instance to the server",
        ReadyTitle = "{name} is ready",
        /// {loader} is the instance's loader and its version: "Fabric 0.16.14".
        ReadyBody = "Minecraft {version} with {loader}, and the server's mods and configs. Press Play to join.",
        CheckModsFailed = "Couldn't check the server's mods",
        JoinFailed = "Couldn't join {name}",
        CoreRemoved = "The core this instance was made for ({address}) isn't in Lodestone Launcher any more. Add it on the Lodestone Host page, or press Play without joining.",
        ServerNotShown = "The core doesn't show this server to you any more: it was deleted, or it isn't listed publicly and you aren't signed in.",
        NotRunning = "{name} isn't running",
        AskOwnerToStart = "Ask its owner to start it, or sign in to the core on the Lodestone Host page to start it yourself.",
        UpdateModsFailed = "Couldn't update the mods",
        StopGameFirst = "Stop the game first.",
        UpdateInstanceFailed = "Couldn't update the instance",
        InstanceNotSaved = "The mods were updated, but the instance couldn't be saved: {error}",
        BadJoinAddress = "The server's address or port isn't one Minecraft can join.",
        StartCrashed = "The server crashed while starting. Check its console in the dashboard.",
        StartStopped = "The server stopped while starting. Check its console in the dashboard.",
        ServerGone = "The server is gone.",
        StartTimedOut = "The server didn't finish starting within 5 minutes.",
        StartFailed = "Couldn't start {name}",
        StopFailed = "Couldn't stop {name}",
        UnlinkFailed = "Couldn't unlink the instance: {error}",
        Cancelled = "Cancelled",
        /// {computer} is "this Mac", "this PC" or "this computer".
        TrustExplanation = "Mods from a Lodestone core run as programs on {computer}, with the same access to your files and accounts as you. Only trust a core whose owner you trust.",
        /// The dashboard's menus and label; the core's log line stays in English.
        FingerprintHelp = "Compare it with the fingerprint the core shows: in the dashboard under Settings → Core Settings → Information (“Certificate fingerprint (SHA-256)”), or in the core's log when it starts (“TLS certificate SHA-256 fingerprint”).",

        // ===== Why a request to a core failed =====
        /// {detail} is the system's error, in English.
        CoreUnreachable = "Couldn't reach the core. Check the address and that the core is running. ({detail})",
        CoreUntrusted = "The HTTPS certificate of {host} isn't trusted by {computer}. Check its fingerprint, then trust it to connect.",
        CoreCertificateChanged = "The HTTPS certificate of {host} changed since you trusted it, so Lodestone Launcher refuses to connect. Check the new fingerprint before you trust it.",
        CoreSignedOut = "Signed out: this device's key was revoked or has expired. Sign in again.",
        CoreBadCredentials = "Wrong username or password.",
        CoreForbidden = "The core refused: you don't have permission to do that.",
        CoreNotFound = "Not found",
        CoreTooOld = "This core is too old for Lodestone Launcher. Update Lodestone Core to a version with server listing.",
        CoreTooManyRequests = "Too many attempts. Try again in a few minutes.",
        /// {status} is an HTTP status, {message} the core's own text.
        CoreAnswered = "The core answered {status}: {message}",
        CoreInvalidReply = "The core sent a reply Lodestone Launcher doesn't understand: {detail}",

        // ===== "<server> has different mods" =====
        DifferentMods = "{name} has different mods",
        /// {changes} is a list such as "2 added, 1 updated, 3 other files".
        DifferentModsBody = "The server's mods changed since this instance got them ({changes}). Update first so you can join; your own mods stay as they are, and the files that are replaced are kept in the instance's backups.",
        /// A mod or file the update adds.
        ChangeNew = "New: {name}",
        ChangeChanged = "Changed: {name}",
        ChangeRemoved = "Removed: {name}",
        /// A config or other file the update writes.
        ChangeConfig = "Config: {name}",
        NoChanges = "no changes",
        UpdateAndPlay = "Update and play",
        PlayAnyway = "Play anyway",

        // ===== The certificate prompt =====
        CertChangedTitle = "The certificate of {name} changed",
        CertTrustTitle = "Trust this core's certificate?",
        CertChangedIntro = "The core at {address} presents another HTTPS certificate than the one you trusted. The core makes a new certificate when its built-in one is about to expire or its address changes, but this can also mean someone is intercepting the connection.",
        /// {os} is "macOS", "Windows" or "Linux".
        CertUntrustedIntro = "{os} doesn't trust the HTTPS certificate of {name} at {address}. Lodestone cores make their own certificate, so this is expected the first time you connect.",
        /// A caption over a fingerprint, in capitals where the language has them.
        FingerprintTrustedBefore = "TRUSTED UNTIL NOW (SHA-256)",
        FingerprintPresented = "PRESENTED NOW (SHA-256)",
        FingerprintCaption = "CERTIFICATE FINGERPRINT (SHA-256)",
        CertIfMatch = "If they match, trust it: Lodestone Launcher then accepts exactly this certificate, only for {host}, and warns you if it changes.",
        CertIfMatchChanged = "If they match, trust it: Lodestone Launcher then accepts exactly this certificate, only for {host}, and warns you if it changes. A core trusted for mods stops being trusted until you trust it again.",
        TrustNewCertificate = "Trust the new certificate",
        TrustCertificate = "Trust certificate",

        // ===== "Do you trust <core>?" =====
        TrustTitle = "Do you trust {name}?",
        /// "Peter's Lodestone Core at 192.168.1.20:16662."
        CoreAt = "{name} at {address}.",
        /// {setting} is the "Trusted for mods" toggle.
        TrustAfterAdd = "Until you trust it, Lodestone Launcher shows you what a server's mods are before it installs them. You can change this later with “{setting}” on the core's card.",
        TrustLater = "Mods from this core will then be installed without asking. You can turn this off again on the core's card.",
        TrustThisCore = "Trust this core",

        // ===== Before installing mods from a core that isn't trusted =====
        /// {computer} is "this Mac", "this PC" or "this computer".
        AdvisoryTitle = "This server's mods will run on {computer}",
        AdvisoryNew = "The instance for {server} on {core} gets these files. Mods are programs: they run with the same access to your files and accounts as you.",
        AdvisoryUpdate = "Updating to the mods of {server} on {core} installs these files. Mods are programs: they run with the same access to your files and accounts as you.",
        ModrinthUnreachable = "Modrinth couldn't be reached, so no file could be checked against it.",
        UploadedNotChecked = "Uploaded to the server — not checked by Modrinth ({count})",
        AlsoOnModrinth = "Also on Modrinth ({count})",
        /// {files} is a list such as "14 config files, 2 script files, 1 other file".
        AlsoFiles = "Also {files}.",
        AlsoFilesScripts = "Also {files}. Script files (KubeJS, CraftTweaker) can drive code too.",
        OnlyIfYouTrust = "Only continue if you trust the owner of {core}.",
        /// {button} is the "Trust <core> and install" button; {explanation} is
        /// TrustExplanation.
        AdvisoryTrustNote = "“{button}” also installs its mods without asking from now on. {explanation}",
        InstallThisTime = "Install this time",
        TrustAndInstall = "Trust {core} and install",

        // ===== "Add server" and signing in =====
        /// How long the device's key lasts.
        KeyDays90 = "90 days",
        KeyYear = "1 year",
        KeyNever = "Never",
        AddressPlaceholder = "192.168.1.20 or localhost:16662",
        UsernameOptional = "Username (optional)",
        Username = "Username",
        Password = "Password",
        EnterAddress = "Enter the core's address.",
        EnterUsernamePassword = "Enter your username and password.",
        EnterPassword = "Enter your password, or leave the username empty.",
        ConsoleAccess = "Console access (optional)",
        ReadConsole = "Read the console",
        SendChat = "Send chat and commands",
        ConsoleAccessHint = "Only where your user may use the console. Commands can do anything an operator can, so allow them only if you trust everyone who uses {computer}.",
        /// The dialog's title when signing in to a core that is added already.
        SignInTitle = "Sign in to server",
        SignInIntro = "Sign in to start and stop servers and see the ones that aren't listed publicly. Lodestone Launcher makes its own key for {computer}; your password isn't kept.",
        AddIntro = "The address of a Lodestone core. Without signing in you see the servers its owner lists publicly.",
        Address = "Address",
        SignInOptional = "Sign in (optional)",
        /// Before the choices "90 days", "1 year", "Never".
        KeyExpiresAfter = "The key on {computer} expires after",
        NeverExpiresWarning = "A key that never expires works until you revoke it in the dashboard.",

        // ===== A server's page =====
        ChatPlaceholder = "Say something, or /command to run one",
        ConsoleNotAllowedAtSignIn = "Reading the console wasn't allowed when {computer} signed in.",
        /// {option} is the "Read the console" choice when signing in.
        ConsoleKeyCantRead = "The key on {computer} can't read the console. Sign in again and allow “{option}”, if your user may use it.",
        ConsoleKeyCantReadShort = "The key on {computer} can't read the console.",
        /// {option} is the "Send chat and commands" choice when signing in.
        ConsoleKeyCantWrite = "The key on {computer} can't send chat or commands. Sign in again and allow “{option}”, if your user may use the console.",
        OfflineCantLink = "Offline accounts can't be linked: their UUID comes from the name, so it proves nothing.",
        /// {account} is a Minecraft account's name, {user} a Lodestone user's or
        /// YourLodestoneUser.
        LinkedTo = "Linked {account} to {user}.",
        YourLodestoneUser = "your Lodestone user",
        Connecting = "Connecting…",
        /// The live stream of a server's events or console is connected.
        Live = "Live",
        /// {reason} is the connection's error, in English.
        Reconnecting = "Reconnecting: {reason}",
        ConsoleNotShown = "The console isn't shown",
        OutputAppearsHere = "The server's output appears here.",
        Send = "Send",
        ChatHint = "Text is said to everyone with /say. Start with / to run a command.",
        Console = "Console",
        JumpToLatest = "Jump to latest",
        Players = "Players",
        NoOneOnline = "No one is online.",
        ServerNotRunning = "The server isn't running.",
        MinecraftAccount = "Minecraft account",
        CoreTooOldToLink = "This core is too old to link Minecraft accounts.",
        Unlink = "Unlink",
        LinkAccount = "Link {account} to {user}",
        LinkCheckHint = "The core checks with Microsoft that the account is yours, using this sign-in once. It doesn't keep it.",
        SignInMicrosoftToLink = "Sign in with a Microsoft account to link it to your Lodestone user.",

        // ===== "Host on a Lodestone core", from an instance's Settings =====
        HostTitle = "Host on a Lodestone core",
        ShowOnServersPage = "Show on the Lodestone Host page",
        HostIntro = "Makes a server from {name}'s mods and configs, as a Modrinth modpack. Client-only mods from Modrinth are left out. The server is created stopped.",
        /// The label over the choice of core.
        CoreLabel = "Core",
        PasswordNotKept = "Your password is sent once to sign in, and isn't kept.",
        ServerName = "Server name",
        Memory = "Memory",
        GamePortHint = "The game port is the first free one from {port} on the core's machine.",
        EulaAgree = "I agree to the Minecraft EULA ({url}). Lodestone Core accepts it for the new server.",
        /// Opens https://aka.ms/MinecraftEULA in the browser.
        ReadEula = "Read the EULA",
        CreateServer = "Create server",
        NameTheServer = "Give the server a name.",
        HostNeedsPassword = "Enter your username and password for this core. Creating a server needs more than the key on {computer} may do.",
        AgreeEulaFirst = "Agree to the Minecraft EULA first: the core accepts it for the new server.",
        StageExporting = "Exporting the instance…",
        StageSigningIn = "Signing in…",
        /// {size} is "3.2 MB".
        StageUploading = "Uploading the modpack ({size})…",
        StageCreating = "Creating the server…",
        CoreGone = "That core is gone.",
        InstanceGone = "The instance is gone.",
        PackNotKept = "The core didn't keep the modpack.",
        SignedOutRightAway = "The core signed you out right after signing in. Try again.",
        AddCoreFirst = "Add your Lodestone core on the Lodestone Host page first.",

        // ===== Worlds opened to LAN =====
        LanListenFailed = "Couldn't listen for LAN worlds: {error}",
        LanLooking = "Looking for worlds opened to LAN…",
        /// "Open to LAN" is Minecraft's button; the settings are macOS's.
        LanNoneMac = "No LAN worlds found. In Minecraft, open a world and choose Open to LAN. If macOS asked whether Lodestone Launcher may find devices on your local network, allow it in System Settings → Privacy & Security → Local Network, then reopen this page.",
        LanNoneWindows = "No LAN worlds found. In Minecraft, open a world and choose Open to LAN. If Windows asked whether to let Lodestone Launcher through its firewall, allow it on private networks, then reopen this page.",
        LanNoneOther = "No LAN worlds found. In Minecraft, open a world and choose Open to LAN, then reopen this page.",
        /// A world opened to LAN that gives no name.
        LanWorld = "LAN world",
        Join = "Join",
        OnYourNetwork = "On your network",
        OnYourNetworkHint = "Worlds opened to LAN on this network.",
        JoinWorld = "Join {world}",
        PickInstance = "Pick the instance to play with. It needs the same Minecraft version and mods as the world at {address}.",
        NoInstancesYet = "You have no instances yet. Create one first.",

        // ===== A server's world and resource pack ("More…") =====
        /// Where a download goes, in PackNote: "saving it in a new instance".
        ANewInstance = "a new instance",
        NewMatchingInstance = "New matching instance",
        NoResourcePack = "This server doesn't set a resource pack.",
        DownloadToInstance = "Download to instance",
        /// {target} is an instance's name, or ANewInstance.
        PackNote = "Minecraft downloads it when you join; saving it in {target} makes it ready offline too.",
        PackNoteRequired = "Players must accept it to join. Minecraft downloads it when you join; saving it in {target} makes it ready offline too.",
        PackNoteNoSha = "The server gives no SHA-1 for it. Minecraft downloads it when you join; saving it in {target} makes it ready offline too.",
        PackNoteRequiredNoSha = "Players must accept it to join. The server gives no SHA-1 for it. Minecraft downloads it when you join; saving it in {target} makes it ready offline too.",
        /// The label over the instances a download can go to.
        InstanceLabel = "Instance",
        ResourcePack = "Resource pack",
        World = "World",
        WorldCopyHint = "A singleplayer copy of the server's world, in a new world folder. Changes you make in it stay on {computer}.",
        StopToDownload = "Stop the server to download its world: a running server can be writing to it.",
        PasswordNotKeptDownload = "Your password is sent once to sign in for this download, and isn't kept.",
        DownloadWorld = "Download world",
        WorldNeedsPassword = "Enter your username and password for this core. Downloading a world needs permission to read the server's files, which the key on {computer} doesn't have.",
        SignInToSeePack = "Sign in to this core to see the server's resource pack.",
        CreatingMatching = "Creating the matching instance",
        /// {more} is the "More…" button.
        CreatingMatchingBody = "It is made once the server's mods are downloaded. Then open {more} on the server again, pick it, and download again.",
        CouldNotCreateInstance = "Couldn't create the instance.",
        QuitGameFirst = "Quit the game in that instance first.",
        WorldZipping = "The core is zipping the world…",
        WorldDownloading = "Downloading…",
        /// {done} and {total} are sizes: "12 MB of 40 MB".
        WorldDownloadingOf = "Downloading {done} of {total}",
        WorldDownloadingSize = "Downloading {done}",
        WorldImporting = "Importing…",
        WorldImportingOf = "Importing {done} of {total}",
        /// "Singleplayer" is Minecraft's menu.
        WorldImported = "Imported as the world “{folder}” in {instance}. Open it from Singleplayer; it's a copy, so changes don't go back to the server.",
        PackSaved = "Saved as {file} in the instance's resource packs. Turn it on there to use it on other servers too.",

        // ===== Friends' servers (Lodestone Connect) =====
        FriendsTitle = "Friends' servers",
        FriendsHint = "Servers friends share with you through Lodestone Connect invite links.",
        JoinAFriend = "Join a friend",
        NoFriendsYet = "When a friend shares a server from Lodestone Host, they get an invite link. Open it, or paste it here with Join a friend.",
        /// {name} is the host's Lodestone core.
        SharedBy = "Shared by {name}",
        /// Asks once whether the server is shared now.
        FriendCheck = "Check",
        FriendChecking = "Checking…",
        FriendPlayingNow = "Playing now",
        StopTrusting = "Stop trusting",
        /// {how} is one of the Via… messages; {ms} the round trip time.
        ConnectedVia = "Connected directly {how}, {ms} ms",
        ViaSameComputer = "on the same computer",
        ViaLocalNetwork = "over your local network",
        ViaIpv6 = "over IPv6",
        ViaInternet = "over the internet",
        JoinFriendTitle = "Join a friend's server",
        JoinFriendIntro = "Paste the invite link your friend sent you. It starts with https://connect.lodestone.cc/j/.",
        InvitePlaceholder = "Invite link (https://connect.lodestone.cc/j/…)",
        NotAnInvite = "That isn't a Lodestone invite link. Invite links start with https://connect.lodestone.cc/j/.",
        PasteInviteFirst = "Paste an invite link first.",
        ConnectingTo = "Connecting to {name}…",
        ConnectingToFriend = "Connecting to your friend's server…",
        StageFinding = "Finding a direct way between your network and your friend's. This takes a few seconds.",
        StageFindingSlow = "Still looking. Some networks take up to a minute.",
        StageReading = "Connected. Reading the server's details…",
        /// Labels in the server's card.
        FactVersion = "Version",
        FactMods = "Mods",
        FactStatus = "Status",
        FactConnection = "Connection",
        FactInstance = "Instance",
        FriendHasMods = "This server uses mods. Lodestone Launcher installs the same ones for you.",
        FriendNoMods = "None needed",
        /// {name} is an instance's name.
        PlaysWithLinked = "{name}, made for this server. Its mods are kept in step.",
        PlaysWithInstance = "Your instance {name}, which has the same version.",
        MakesInstance = "Play makes a new instance with the same version.",
        MakesInstanceWithMods = "Play makes a new instance with the server's version and mods.",
        FriendServerStopped = "The server is stopped",
        FriendServerStoppedBody = "Ask your friend to start it. Minecraft can't join until it runs.",
        FriendStarting = "Getting the game ready…",
        FriendInstanceBusy = "That instance is already running. Quit the game first.",
        FriendLinkEnded = "This invite link has ended",
        FriendLinkEndedBody = "Your friend stopped sharing the server, or made a new link. Ask them for the new one.",
        FriendNotSharing = "Your friend isn't sharing this server right now",
        FriendNotSharingBody = "Their computer or Lodestone Host may be off. Try again later, or ask your friend to start sharing.",
        FriendNoPath = "Your network and your friend's can't connect directly",
        FriendNoPathBody = "Lodestone Connect joins two computers directly, and these two networks block that. Try another network (such as a phone's hotspot), or ask your friend to forward a port on their router.",
        FriendWrongHost = "This isn't your friend's server",
        FriendWrongHostBody = "The computer that answered isn't the one this link was made for, so Lodestone Launcher stopped before sending anything. Someone may be pretending to be your friend's server. Ask your friend for a new link, and only use links you get from them directly.",
        FriendOffline = "This computer seems to be offline",
        FriendOfflineBody = "Check your internet connection, then try again.",
        FriendServiceUnreachable = "Couldn't reach Lodestone Connect",
        FriendServiceUnreachableBody = "connect.lodestone.cc didn't answer ({detail}). Check your internet connection, then try again.",
        FriendJoinFailed = "Couldn't join",
        FriendJoinFailedBody = "Something went wrong: {detail}",
        FriendTunnelEnded = "The connection to {name} ended",
        FriendTunnelEndedBody = "Your friend stopped sharing, or their computer went offline. Minecraft says it lost the connection. Play again when they're back.",
        FriendLinkMissing = "Lodestone Launcher couldn't read this server's invite link. Open the link from your friend again.",
        FriendRemoved = "This instance was made for a friend's server ({link}) that isn't in Lodestone Launcher any more. Open its invite link again to join it.",
        FriendsReadFailed = "Couldn't read your friends' servers",
        FriendsSaveFailed = "Couldn't save your friends' servers",

        // ===== The Minecraft EULA of a server =====
        EulaNotAccepted = "The Minecraft EULA isn't accepted for this server",
        EulaNotAcceptedBody = "The server can't start until someone accepts Mojang's Minecraft EULA ({url}). Read it, and accept it only if you agree to it.",
        AcceptEula = "Accept the Minecraft EULA",
        EulaAccepted = "You accepted the Minecraft EULA for this server. It can start now.",
        EulaAcceptFailed = "Couldn't accept the EULA: {error}",
    }

    pub enum ServersPlural {
        /// Players online, when the server doesn't say how many it takes.
        Players = ("{count} player", "{count} players"),
        ReadyBodySkipped = (
            "Minecraft {version} with {loader}, and the server's mods and configs. {count} file of the pack was left out. Press Play to join.",
            "Minecraft {version} with {loader}, and the server's mods and configs. {count} files of the pack were left out. Press Play to join."
        ),
        /// Parts of DifferentModsBody's {changes}: mods added to the instance.
        ModsAdded = ("{count} added", "{count} added"),
        ModsUpdated = ("{count} updated", "{count} updated"),
        ModsRemoved = ("{count} removed", "{count} removed"),
        OtherFiles = ("{count} other file", "{count} other files"),
        /// Parts of AlsoFiles's {files}.
        ConfigFiles = ("{count} config file", "{count} config files"),
        ScriptFiles = ("{count} script file", "{count} script files"),
        /// Under a list that shows the first few.
        AndMore = ("and {count} more", "and {count} more"),
        HostDone = (
            "{name} is on {core}, on port {port}. The core is setting it up now: it installs Fabric and downloads {count} mod file from Modrinth.\n\nThe server is stopped. Start it from the Lodestone Host page or the dashboard when you're ready; this instance is ready to join it.",
            "{name} is on {core}, on port {port}. The core is setting it up now: it installs Fabric and downloads {count} mod files from Modrinth.\n\nThe server is stopped. Start it from the Lodestone Host page or the dashboard when you're ready; this instance is ready to join it."
        ),
        /// The same, for an instance that was linked to another server: {server}.
        HostDoneRelinked = (
            "{name} is on {core}, on port {port}. The core is setting it up now: it installs Fabric and downloads {count} mod file from Modrinth.\n\nThe server is stopped. Start it from the Lodestone Host page or the dashboard when you're ready; this instance is ready to join it. This instance was linked to {server}; it now belongs to the new server.",
            "{name} is on {core}, on port {port}. The core is setting it up now: it installs Fabric and downloads {count} mod files from Modrinth.\n\nThe server is stopped. Start it from the Lodestone Host page or the dashboard when you're ready; this instance is ready to join it. This instance was linked to {server}; it now belongs to the new server."
        ),
    }
}
