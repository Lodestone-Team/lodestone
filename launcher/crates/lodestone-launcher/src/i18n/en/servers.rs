//! The Servers page, a server's page, hosts and their dialogs.
//!
//! A "server" is a game server a host runs; a "host" is the Lodestone Core itself
//! (the code calls it a core). "Add host" adds one, and lists its servers.

crate::i18n::messages! {
    pub enum ServersMsg {
        // ===== The page =====
        /// The page's title, and the back button on a server's page.
        ServersTitle = "Lodestone Host",
        /// The Friends page's title.
        FriendsPageTitle = "Friends",
        /// "On this computer": hosts found here, and "Connect as owner".
        OnThisComputer = "On this computer",
        Rescan = "Rescan",
        ScanningLocal = "Looking for hosts on this computer…",
        NoLocalCores = "No host is running on this computer.",
        /// The offer under "On this computer" while nothing runs here.
        HostOfferTitle = "Host any server and play with your friends right away",
        HostOfferBody = "No setup, no port forwarding.",
        HostOfferDownload = "Download Lodestone Host",
        /// A small link under the download button: the server program without the desktop app.
        HostOfferCoreOnly = "Download the server program only",
        ConnectAsOwner = "Connect as owner",
        ChooseDataFolder = "Choose folder…",
        ChooseDataFolderPrompt = "Choose",
        ProductHostHint = "the desktop app, with the dashboard in a window",
        ProductCoreHint = "the server program on its own, or Docker",
        /// After Connect as owner; the host is in a chip over it.
        ConnectedAsOwner = "Connected as owner",
        /// Its user name is in a field to copy under it, keyed OwnerAccount.
        ConnectedMadeOwner = "It had no owner yet, so Lodestone Launcher made an owner account for you.",
        ConnectedOwnerKey = "This launcher has an owner key for it now. You can revoke it from the host's dashboard.",
        /// The key of the user name of the owner account the launcher made.
        OwnerAccount = "Owner account",
        StartedBy = "Started by {who}",
        StartedDirectly = "Started directly",
        DirectNote = "This host was started directly. It is meant to be run with lodestone_cli, which installs it and keeps it updated.",
        GetLodestoneCli = "Get lodestone_cli",
        CredentialsReadFailed = "Couldn't read the owner keys of hosts on this computer",
        CredentialsSaveFailed = "Couldn't save the owner key",
        ClaimNoCore = "No host is running on this computer.",
        ClaimRefused = "The host refused to give owner access. Turn on \"Let apps on this computer claim owner access\" in its settings, or update it.",
        ClaimFolderNotFound = "Couldn't find this host's data folder. Choose the folder it keeps its data in (its LODESTONE_PATH). A host in Docker needs its data folder mounted from this computer.",
        ClaimNotThisCore = "The host that answered isn't the one whose folder was found, so nothing was saved.",
        ClaimNoResult = "The host didn't finish the claim. Try again.",
        ClaimNoCertificate = "This host has no HTTPS certificate, so Lodestone Launcher can't make sure its owner key only ever goes to it. Turn on HTTPS in the host's settings, then connect again.",
        ClaimFiles = "Couldn't use the host's data folder: {error}",
        ClaimInvalid = "The host sent something Lodestone Launcher doesn't understand: {error}",
        /// The button (and dialog title) that adds a host by its address.
        AddServer = "Add host",
        /// Closes the message at the top of the page.
        Dismiss = "Dismiss",
        /// "Other hosts": hosts added by address, on other computers.
        OtherCoresTitle = "Other hosts and servers",
        OtherCoresHint = "Hosts on other computers that you added by their address, and servers you joined with an invite link.",
        NoOtherCores = "No other hosts or servers yet",
        NoServersHint = "Add a host by its address, or add a server with the invite link someone sent you.",

        // ===== A host's card =====
        /// The "…" button on a host's card.
        CoreMoreActions = "More actions for {name}",
        /// In a host's "…" menu: copies the owner key that claiming the host here gave.
        CopyOwnerApiKey = "Copy owner API key",
        /// The host is in a chip over it.
        OwnerKeyCopied = "Owner API key copied",
        OwnerKeyCopiedHint = "Keep it private: anyone who has it can control this host as its owner.",
        /// At the bottom of a host's "…" menu: the launcher only accepts the host's own
        /// HTTPS certificate, the one the user trusted.
        CertificatePinned = "Certificate pinned",
        /// After the host's address: "192.168.1.20:16662 · Signed in as peter".
        SignedInAs = "Signed in as {username}",
        SignedIn = "Signed in",
        NotSignedIn = "Not signed in: public servers only",
        /// The toggle on a host's card.
        TrustedForMods = "Trusted for mods",
        Offline = "Offline: couldn't reach this host. Check that it is running and on the same network.",
        /// "List servers publicly" and "Core Settings" are the dashboard's words (it still
        /// says Core).
        NotListedPublicly = "The owner hasn't turned on “List servers publicly” in Core Settings, so there's nothing to show without signing in.",
        NoPublicServers = "No servers are listed publicly. Sign in to see the ones you have access to.",
        NoVisibleServers = "No servers you can see on this host.",
        CertChangedWarning = "This host's HTTPS certificate changed since you trusted it, so Lodestone Launcher refuses to connect. The host makes a new certificate when its built-in one is about to expire or its address changes; it can also mean someone is intercepting the connection. Check the new fingerprint before you trust it.",
        /// {computer} is "this Mac", "this PC" or "this computer".
        CertUntrustedWarning = "This host's HTTPS certificate isn't trusted by {computer}. Hosts make their own certificate, so this is expected: check its fingerprint, then trust it.",
        ReviewNewCertificate = "Trust the new certificate…",
        ReviewCertificate = "Trust certificate…",

        // ===== A server's row =====
        CreateInstance = "Create instance",
        StartAndJoin = "Start and join",
        Play = "Play and join",
        /// A tag next to a server's name: it is shared with friends through an invite
        /// link.
        SharedTag = "Shared",
        /// The copy button in the Shared tag, and what it says once pressed.
        CopyInviteLink = "Copy invite link",
        /// The server is in a chip over it.
        InviteLinkCopied = "Invite link copied",
        InviteLinkCopiedHint = "Anyone who has it can join while the server is shared.",
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
        /// The host is in a chip over it, with a NotTrusted badge.
        NoLongerTrusted = "No longer trusted for mods",
        /// The address is in a field under it.
        NoLongerTrustedIdChanged = "Another host answers at this address now (its id changed). Trust it again on its card only if you trust its owner.",
        NoLongerTrustedOtherCore = "Another host answers at this address now.",
        NoLongerTrustedNewCertificate = "You trusted a new certificate for it. Trust it for mods again on its card if you're sure it is the same host.",
        NoLongerTrustedCertificateChanged = "Its certificate changed. Trust it for mods again on its card if you're sure it is the same host.",
        /// The badge beside a host that is no longer trusted for mods.
        NotTrusted = "Not trusted",
        /// The host is in a chip over it.
        SignedOut = "Signed out",
        /// "Settings → API keys" is where the dashboard lists keys. The key's prefix is in a field under it, to find it there.
        KeyStillValid = "This device's key still works until it expires. To end it now, revoke it in the dashboard under Settings → API keys.",
        /// {keychain} is "Keychain", "Credential Manager" or "keyring".
        KeySaveFailed = "Couldn't save the key in the {keychain}",
        KeySaveFailedError = "Couldn't save the key in the {keychain}: {error}",
        CheckCertificate = "Check the host's certificate to continue.",
        CertificateNotTrusted = "The certificate wasn't trusted, so your sign-in wasn't sent to the host.",
        CreateFailed = "Couldn't create the instance",
        /// The new instance is in a chip over it, with a NoServerMods badge.
        CreatedWithoutMods = "Created without the server's mods",
        CreatedWithoutModsBody = "The owner hasn't shared this server's client mods, so the instance has only its Minecraft version and loader. Add any mods it needs yourself.",
        /// The badge beside an instance made without the server's mods.
        NoServerMods = "No server mods",
        LinkFailed = "Couldn't link the instance to the server",
        /// An instance made for a server is ready; it is in a chip over it.
        ReadyTitle = "Instance ready",
        /// Under ReadyTitle.
        ReadyBody = "It has the server's mods and configs. Press Play to join.",
        CheckModsFailed = "Couldn't check the server's mods",
        /// The server is in a chip over it, with a Failed badge.
        JoinFailed = "Couldn't join",
        /// Its address is in a field under it.
        CoreRemoved = "The host this instance was made for isn't in Lodestone Launcher any more. Add it on the Lodestone Host page, or press Play without joining.",
        ServerNotShown = "The host doesn't show this server to you any more: it was deleted, or it isn't listed publicly and you aren't signed in.",
        /// The server is in a chip over it.
        NotRunning = "Not running",
        AskOwnerToStart = "Ask its owner to start it, or sign in to the host on the Lodestone Host page to start it yourself.",
        UpdateModsFailed = "Couldn't update the mods",
        StopGameFirst = "Stop the game first.",
        FinishFinderFirst = "Finish the Find it for me search first: it has some of the mods turned off.",
        /// Another job is changing the instance's files (an install, a restore, a copy).
        FilesBusy = "Wait for the instance's files to finish changing, then try again.",
        UpdateInstanceFailed = "Couldn't update the instance",
        InstanceNotSaved = "The mods were updated, but the instance couldn't be saved: {error}",
        BadJoinAddress = "The server's address or port isn't one Minecraft can join.",
        StartCrashed = "The server crashed while starting. Check its console in the dashboard.",
        StartStopped = "The server stopped while starting. Check its console in the dashboard.",
        ServerGone = "The server is gone.",
        StartTimedOut = "The server didn't finish starting within 5 minutes.",
        /// The server is in a chip over it, with a Failed badge.
        StartFailed = "Couldn't start",
        StopFailed = "Couldn't stop",
        UnlinkFailed = "Couldn't unlink the instance: {error}",
        Cancelled = "Cancelled",
        /// {computer} is "this Mac", "this PC" or "this computer".
        TrustExplanation = "Mods from a host run as programs on {computer}, with the same access to your files and accounts as you. Only trust a host whose owner you trust.",
        /// The dashboard's menus and label (it still says Core Settings); the host's log line
        /// stays in English.
        FingerprintHelp = "Compare it with the fingerprint the host shows: in the dashboard under Settings → Core Settings → Information (“Certificate fingerprint (SHA-256)”), or in the host's log when it starts (“TLS certificate SHA-256 fingerprint”).",

        // ===== Why a request to a host failed =====
        /// {detail} is the system's error, in English.
        CoreUnreachable = "Couldn't reach the host. Check the address and that the host is running. ({detail})",
        CoreUntrusted = "The HTTPS certificate of {host} isn't trusted by {computer}. Check its fingerprint, then trust it to connect.",
        CoreCertificateChanged = "The HTTPS certificate of {host} changed since you trusted it, so Lodestone Launcher refuses to connect. Check the new fingerprint before you trust it.",
        CoreSignedOut = "Signed out: this device's key was revoked or has expired. Sign in again.",
        CoreBadCredentials = "Wrong username or password.",
        CoreForbidden = "The host refused: you don't have permission to do that.",
        CoreNotFound = "Not found",
        CoreTooOld = "This host is too old for Lodestone Launcher. Update it to a version with server listing.",
        CoreTooManyRequests = "Too many attempts. Try again in a few minutes.",
        /// {status} is an HTTP status, {message} the host's own text.
        CoreAnswered = "The host answered {status}: {message}",
        CoreInvalidReply = "The host sent a reply Lodestone Launcher doesn't understand: {detail}",

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
        CertTrustTitle = "Trust this host's certificate?",
        CertChangedIntro = "The host at {address} presents another HTTPS certificate than the one you trusted. The host makes a new certificate when its built-in one is about to expire or its address changes, but this can also mean someone is intercepting the connection.",
        /// {os} is "macOS", "Windows" or "Linux".
        CertUntrustedIntro = "{os} doesn't trust the HTTPS certificate of {name} at {address}. Hosts make their own certificate, so this is expected the first time you connect.",
        /// A caption over a fingerprint, in capitals where the language has them.
        FingerprintTrustedBefore = "TRUSTED UNTIL NOW (SHA-256)",
        FingerprintPresented = "PRESENTED NOW (SHA-256)",
        FingerprintCaption = "CERTIFICATE FINGERPRINT (SHA-256)",
        CertIfMatch = "If they match, trust it: Lodestone Launcher then accepts exactly this certificate, only for {host}, and warns you if it changes.",
        CertIfMatchChanged = "If they match, trust it: Lodestone Launcher then accepts exactly this certificate, only for {host}, and warns you if it changes. A host trusted for mods stops being trusted until you trust it again.",
        TrustNewCertificate = "Trust the new certificate",
        TrustCertificate = "Trust certificate",

        // ===== "Do you trust <host>?" =====
        TrustTitle = "Do you trust {name}?",
        /// "Peter's Lodestone Core at 192.168.1.20:16662."
        CoreAt = "{name} at {address}.",
        /// {setting} is the "Trusted for mods" toggle.
        TrustAfterAdd = "Until you trust it, Lodestone Launcher shows you what a server's mods are before it installs them. You can change this later with “{setting}” on the host's card.",
        TrustLater = "Mods from this host will then be installed without asking. You can turn this off again on the host's card.",
        TrustThisCore = "Trust this host",

        // ===== Before installing mods from a host that isn't trusted =====
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
        /// {button} is the "Trust <host> and install" button; {explanation} is
        /// TrustExplanation.
        AdvisoryTrustNote = "“{button}” also installs its mods without asking from now on. {explanation}",
        InstallThisTime = "Install this time",
        TrustAndInstall = "Trust {core} and install",

        // ===== A server's image: what it installs, and what went wrong =====
        /// The server records its files the first time a player asks: minutes for a big server.
        BusyPreparing = "Waiting for the server to prepare its files…",
        /// While the server's modpack downloads, before anything is installed: sizes.
        BusyBasePack = "Getting the server's modpack, {done} of {total}…",
        /// Asked before playing when the server's files are the instance's, but it has
        /// files it can't share that the player would need.
        LacksFiles = "{name} has files you won't get",
        LacksFilesBody = "Your instance has the server's files, but the server has some it can't share. You may not be able to join without them.",
        InstallTitle = "Install {name}'s files?",
        /// {loader} is the loader and its version: "Fabric 0.16.10", or "Vanilla".
        InstallIntro = "A new instance for {server} on {core} gets Minecraft {version} with {loader}, and these files from the server.",
        /// The modpack a server is built on, and its version.
        SummaryBase = "Modpack: {pack} {version}",
        SummaryAddedMods = "Mods the server added ({count})",
        /// Files the server uploaded or changed, which only it can send.
        SummaryServerOnly = "Files only the server has ({count})",
        /// What installing downloads, like "Download: 24.5 MiB".
        SummaryDownload = "Download: {size}",
        MissingTitle = "Files the server couldn't share ({count})",
        MissingNote = "You won't have these, and may not be able to join without them. Ask the server's owner.",
        /// A file the server couldn't share, and why: "mods/a.jar (it is a link)".
        MissingLine = "{name} ({reason})",
        /// Why a file couldn't be shared: parts of MissingLine.
        WithheldSymlink = "it is a link",
        WithheldUnreadable = "it couldn't be read",
        WithheldBadName = "its name can't be shared",
        WithheldKeptChanging = "it kept changing",
        WithheldCurseForge = "CurseForge doesn't allow it",
        WithheldOther = "not shared",
        CurseForgeTitle = "Download these from CurseForge yourself ({count})",
        CurseForgeNote = "Their authors don't let other apps download them.",
        Install = "Install",
        /// The loader version an update brings: "Fabric 0.16.11".
        ChangeLoader = "Loader: {loader}",
        /// Why a server's files couldn't be installed.
        ErrUpdateLodestone = "This server runs a newer Lodestone. Update Lodestone Launcher to join it.",
        ErrPrepareFailed = "The server couldn't prepare its files. Try again in a few minutes.",
        ErrStillPreparing = "The server is still preparing its files. Try again in a few minutes.",
        ErrFileFailed = "The server couldn't send {path}. Try again later.",
        ErrViewChanged = "The server's files changed while they downloaded. Try again.",
        /// {path} is a file in the instance, like "config/sodium.toml".
        ErrFilesChangedSince = "{path} changed after the update was worked out, so nothing was changed. Press Play again.",
        ErrBasePackGone = "The server's modpack is no longer on Modrinth, so its files can't be downloaded. Ask the server's owner to update the modpack.",
        /// {game} and {instance} are "Minecraft 1.21.1 with Fabric 0.16.10".
        ErrGameChanged = "The server now runs {game}, and this instance is for {instance}. Make a new instance for the server.",
        ErrCurseForgeBase = "The server is built on a CurseForge modpack, which Lodestone Launcher can't install from a server yet.",
        ErrCurseForgeUnreachable = "CurseForge couldn't be reached, and the server has files from it. Check your connection, then try again.",
        /// {detail} says what, in English.
        ErrRefused = "The server sent files Lodestone Launcher won't install: {detail}",
        ErrBaseFileMissing = "The server's modpack has no file {path}, which the server says comes from it. Ask the server's owner to update the modpack.",
        /// A host whose Lodestone is too old to serve its servers' files this way.
        ErrOlderCore = "The host runs an older Lodestone, which doesn't share its servers' files this way.",
        /// {detail} says what went wrong, in English.
        ErrInstallFiles = "Couldn't install the server's files: {detail}",
        /// An instance that has the server's files, whose server stopped sharing them.
        ErrStoppedSharing = "The server stopped sharing its files, so they couldn't be checked. Ask the server's owner.",
        /// {needed} and {free} are sizes, like "24.5 GiB".
        ErrNotEnoughSpace = "There isn't enough free space for the server's files: they need {needed}, and {free} is free.",
        /// The instance was unlinked while the update waited for an answer.
        ErrNotLinked = "This instance isn't linked to the server any more, so nothing was installed.",

        // ===== "Add host" and signing in =====
        /// How long the device's key lasts.
        KeyDays90 = "90 days",
        KeyYear = "1 year",
        KeyNever = "Never",
        AddressPlaceholder = "192.168.1.20 or localhost:16662",
        UsernameOptional = "Username (optional)",
        Username = "Username",
        Password = "Password",
        EnterAddress = "Enter the host's address.",
        EnterUsernamePassword = "Enter your username and password.",
        EnterPassword = "Enter your password, or leave the username empty.",
        ConsoleAccess = "Console access (optional)",
        ReadConsole = "Read the console",
        SendChat = "Send chat and commands",
        ConsoleAccessHint = "Only where your user may use the console. Commands can do anything an operator can, so allow them only if you trust everyone who uses {computer}.",
        /// The dialog's title when signing in to a host that is added already.
        SignInTitle = "Sign in to server",
        SignInIntro = "Sign in to start and stop servers and see the ones that aren't listed publicly. Lodestone Launcher makes its own key for {computer}; your password isn't kept.",
        AddIntro = "The address of a host. Without signing in you see the servers its owner lists publicly.",
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
        MinecraftAccount = "Minecraft account",
        CoreTooOldToLink = "This host is too old to link Minecraft accounts.",
        Unlink = "Unlink",
        LinkAccount = "Link {account} to {user}",
        LinkCheckHint = "The host checks with Microsoft that the account is yours, using this sign-in once. It doesn't keep it.",
        SignInMicrosoftToLink = "Sign in with a Microsoft account to link it to your Lodestone user.",

        // ===== "Host on Lodestone", from an instance's Settings =====
        HostTitle = "Host on Lodestone",
        ShowOnServersPage = "Show on the Lodestone Host page",
        /// The label over the choice of host.
        CoreLabel = "Host",
        PasswordNotKept = "Your password is sent once to sign in, and isn't kept.",
        ServerName = "Server name",
        Memory = "Memory",
        EulaAgree = "I agree to the Minecraft EULA ({url}). The host accepts it for the new server.",
        /// Opens https://aka.ms/MinecraftEULA in the browser.
        ReadEula = "Read the EULA",
        CreateServer = "Create server",
        NameTheServer = "Give the server a name.",
        HostNeedsPassword = "Enter your username and password for this host. Creating a server needs more than the key on {computer} may do.",
        AgreeEulaFirst = "Agree to the Minecraft EULA first: the host accepts it for the new server.",
        StageExporting = "Exporting the instance…",
        StageCreating = "Creating the server…",
        CoreGone = "That host is gone.",
        InstanceGone = "The instance is gone.",
        SignedOutRightAway = "The host signed you out right after signing in. Try again.",
        AddCoreFirst = "Add your host on the Lodestone Host page first.",
        HostIntro = "Makes a server on a host from {name}: its mods and configs, and the worlds you pick. Mods only players need, your accounts and your own settings stay here. The server is created stopped.",
        HostWorlds = "Worlds to send",
        HostWorldsHint = "You pick which one becomes the server’s world next. Send none for a new world.",
        HostNoWorlds = "This instance has no worlds: the server makes a new one.",
        HostWorldsLoading = "Looking for the instance’s worlds…",
        /// {done} and {total} are sizes ("90 MB of 180 MB").
        HostStageSending = "Sending it to the host ({done} of {total})…",
        HostStageReading = "The host is reading the file…",
        HostPort = "Port",
        HostPortInvalid = "Enter a port from 1 to 65535.",
        HostNameForbidden = "A server’s name can’t have / \\ : * ? \" < > |",
        HostNameTooLong = "A server’s name can have at most 100 characters.",
        HostNameControl = "A server’s name can’t have control characters.",
        HostCreatingHint = "You can close this window: the host goes on setting up the server, and this instance is linked to it once it’s ready.",
        /// The server was made: {name} on the host {core}, on {port}.
        HostCreated = "{name} is on {core}, on port {port}. It’s stopped: start it from the Lodestone Host page or the dashboard when you’re ready. This instance is linked to it, so Play joins it.",
        /// The server was made, but its instance was deleted meanwhile.
        HostCreatedUnlinked = "{name} is on {core}, on port {port}. It’s stopped: start it from the Lodestone Host page or the dashboard when you’re ready. The instance was deleted meanwhile, so nothing is linked to it.",
        HostCreatedRelinked = "This instance was linked to {server}; it now belongs to the new server.",
        /// Over the lines that say what the new server didn't get.
        HostLeftOut = "Not on the server",
        HostCreationFailed = "The host couldn’t make the server: {error}",
        HostCreationFailedPlain = "The host couldn’t make the server.",
        HostStillCreating = "The host is still setting up the server. It shows on the Lodestone Host page once it’s ready; this instance isn’t linked to it yet.",
        /// {core} is the host's name.
        HostTooOld = "{core}’s Lodestone is too old to make servers from files. Update it on that computer, then try again.",
        HostNoPermission = "Your user on {core} can’t create servers. Ask its owner.",
        HostGameRunning = "Quit the game first: its worlds can’t be sent while it runs.",
        DownloadWorldAction = "Download this world…",
        DownloadWorldTitle = "Download {server}’s world",
        DownloadWorldIntro = "Makes a new instance on this computer from a backup of {server}: its world, with the mods its players get. The server’s settings, operators and plugins stay on it.",
        DownloadWorldSignIn = "Sign in with a user who can read this server’s files.",
        DownloadWorldNeedsPassword = "Enter your username and password for this host.",
        DownloadWorldBackups = "Backup",
        DownloadWorldMidWrite = "taken while it ran, may be caught mid-write",
        DownloadWorldBackUpNow = "Back up now",
        DownloadWorldBackUpNowHint = "A backup of the server as it is now",
        DownloadWorldButton = "Download",
        DownloadWorldBackingUp = "The host is backing the server up…",
        DownloadWorldPreparing = "The host is preparing the file…",
        DownloadWorldDownloading = "Downloading ({done} of {total})…",
        DownloadWorldTooOld = "{core}’s Lodestone is too old to send a world. Update it on that computer, then try again.",
        DownloadWorldForbidden = "Your user can’t read this server’s files, so its world can’t be downloaded. Ask its owner.",
        DownloadWorldNoBackups = "The server has no backups yet. Back it up first.",
        DownloadWorldNothingToDownload = "This server has no backups, and your user can’t back it up. Ask its owner to back it up, then try again.",
        DownloadWorldBackupForbidden = "Your user can’t back this server up. Pick one of its backups instead.",
        DownloadWorldBackupFailed = "The backup failed: {error}",
        DownloadWorldCancelled = "The download was cancelled.",

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
        /// In the paste dialog: saves the server without joining it.
        AddInvite = "Add",
        AddAndJoin = "Add and join",
        /// A server added from a link whose host hasn't answered yet; {link} is the
        /// link without its secret part.
        ServerFromLink = "Server from {link}",
        AddedNotAnswered = "Added. Its name and version show once its host answers.",
        /// "Add" with a link to a server that is saved already (its row is highlighted).
        AlreadyAddedTitle = "Already added",
        AlreadyAddedBody = "This link is for a server on this page. Its row is highlighted.",
        OnYourNetwork = "On your network",
        OnYourNetworkHint = "Worlds opened to LAN on this network.",
        JoinWorld = "Join {world}",
        PickInstance = "Pick the instance to play with. It needs the same Minecraft version and mods as the world at {address}.",
        NoInstancesYet = "You have no instances yet. Create one first.",

        // ===== Friends' servers (Lodestone Connect) =====
        FriendsTitle = "Servers from invite links",
        FriendsHint = "Servers people shared with you through a Lodestone Connect invite link.",
        JoinAFriend = "Add server",
        /// {name} is the name of the friend's host.
        SharedBy = "Shared by {name}",
        /// Under a host in "On this computer": servers it shares with an invite link that
        /// the user joined here.
        SharedWithFriends = "Shared with friends",
        SharedWithFriendsHint = "You joined these with their invite links.",
        /// Under a server shared from this computer, while the launcher isn't the host's
        /// owner (as owner, the server shows in the host's own list).
        ConnectToJoinDirectly = "Connect as owner to join it directly, without a tunnel.",
        /// The connection fact in the join dialog, for a server shared from this computer.
        DirectConnection = "Directly on this computer, without a tunnel",
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
        JoinFriendTitle = "Join a server",
        /// The paste dialog's title, from "Add server".
        AddServerTitle = "Add a server",
        JoinFriendIntro = "Paste the invite link someone sent you. It starts with https://connect.lodestone.cc/j/. Add saves the server even if it's offline right now.",
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
        FriendServerStoppedBody = "Ask your friend to start it. Play turns on once it runs.",
        FriendServerStarting = "The server is starting",
        FriendServerStartingBody = "Play turns on once it's running.",
        FriendStarting = "Getting the game ready…",
        FriendInstanceBusy = "That instance is already running. Quit the game first.",
        FriendLinkEnded = "Your friend stopped sharing this server",
        FriendLinkEndedBody = "Ask them for a new link.",
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
        /// The server is in a chip over it.
        FriendTunnelEnded = "Connection ended",
        FriendTunnelEndedBody = "Your friend stopped sharing, or their computer went offline. Minecraft says it lost the connection. Play again when they're back.",
        FriendLinkMissing = "Lodestone Launcher couldn't read this server's invite link. Open the link from your friend again.",
        /// Its invite link (without its secret part) is in a field under it, keyed InviteLinkKey.
        FriendRemoved = "This instance was made for a friend's server that isn't in Lodestone Launcher any more. Open its invite link again to join it.",
        /// The key of a server's invite link.
        InviteLinkKey = "Invite link",
        FriendsReadFailed = "Couldn't read your friends' servers",
        FriendsSaveFailed = "Couldn't save your friends' servers",
        /// On a friend's server's row. {address} is 127.0.0.1:<port>, the same every
        /// time; it works while the launcher is connected to the server.
        /// "Magic localhost" names the feature: a tunnelled server always answers at the
        /// same address on this computer.
        FriendAddress = "Magic localhost: {address} (while connected)",
        /// The address to use this time and the usual port are in fields under it.
        FriendPortTaken = "Port was in use",
        /// {address} is this session's 127.0.0.1:<port>; {port} the usual one.
        FriendPortTakenBody = "Something else on this computer is using the usual port, so use this address in Minecraft this time. Next time, Lodestone Launcher tries the usual port first.",
        /// The key of the address a friend's server has this time, its usual port being taken.
        AddressNow = "Address now",
        /// The key of the port a friend's server usually has here.
        UsualPort = "Usual port",

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
            "It has the server's mods and configs. {count} file of the pack was left out. Press Play to join.",
            "It has the server's mods and configs. {count} files of the pack were left out. Press Play to join."
        ),
        /// A new instance from a server's files, some of which the server couldn't share.
        ReadyBodyMissing = (
            "It has the server's files. {count} file the server couldn't share is missing. Press Play to join.",
            "It has the server's files. {count} files the server couldn't share are missing. Press Play to join."
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
    }
}
