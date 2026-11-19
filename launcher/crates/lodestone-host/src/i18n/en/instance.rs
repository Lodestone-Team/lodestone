//! Work package C: an instance's Settings, Mods and Event logs tabs.

crate::i18n::messages! {
    pub enum InstanceMsg {
        /// The Settings tab (`InstanceSettings.tsx`).
        Automation = "Automation",
        AutoStart = "Auto start",
        AutoStartHint = "Start this server when Lodestone Core starts",
        RestartOnCrash = "Restart on crash",
        /// A section of the core's settings that has none yet.
        NotAvailable = "Not available at this moment",
        NotAvailableHint = "Try to start this server at least once",
        NoPermission = "No permission",
        SettingsFailed = "Couldn’t load the settings: {error}",
        DangerZone = "Danger zone",
        DeleteServer = "Delete server",
        /// The title of the confirmation dialog; {name} is the server's name.
        DeleteTitle = "Delete server?",
        DeleteConfirm = "I understand, delete this server",
        DeleteCannotUndo = "This action cannot be undone.",
        DeleteWarning = "Its settings and worlds will be permanently deleted. Please back up any important data before proceeding.",
        /// After DeleteWarning: Lodestone's own snapshots and world backups of the server go
        /// with it.
        DeleteBackupsToo = "Its snapshots and world backups are deleted too.",
        Deleting = "Deleting {name}…",
        /// Why a setting can't be changed.
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
        PackUrlHint = "A direct link to the pack’s .zip.",
        PackSha1 = "SHA-1",
        PackSha1Hint = "Lets players check the download and cache the pack.",
        PackSha1Error = "The SHA-1 is 40 hexadecimal characters.",
        /// Computes the SHA-1 of the pack.
        PackCompute = "Compute",
        /// {size} is the size of the pack, such as "1.5 MB".
        PackDownloaded = "Downloaded",
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
        EulaAccepted = "Accepted",
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
        EulaDialogBody = "This server only starts after you agree to the Minecraft EULA. Do you agree to it?",
        /// Accepts the EULA and starts the server.
        EulaAcceptAndStart = "I agree, start the server",
        /// The Event logs tab: what happened to this server.
        EventLogsLead = "Only recent events are kept.",
        EventLogsEmpty = "Nothing has happened yet",
        EventLogsFailed = "Couldn’t load the events: {error}",
        /// {state} is the server's new state, such as Running.
        EventState = "The server is now {state}",
        /// {names} is a list of player names.
        EventJoined = "Joined",
        EventLeft = "Left",
        /// {user} is who did it.
        EventBy = "By",
        /// The Mods (or Plugins) tab (`ModsTab.tsx`, `InstalledMods.tsx`, `ModrinthSearch.tsx`, `ModrinthProjectView.tsx`).
        ModsUnsupported = "This server doesn’t support mods or plugins. Fabric, Forge, Paper and Spigot servers do.",
        ModsNoRead = "You don’t have permission to read this server’s files.",
        ModsNoWrite = "You don’t have permission to change this server’s files",
        ModsInstalledView = "Installed",
        ModsBrowseView = "Browse",
        ModsRestartMod = "Restart the server to apply mod changes",
        ModsRestartPlugin = "Restart the server to apply plugin changes",
        ModsSearchMod = "Search mods on Modrinth, or paste a Modrinth link",
        ModsSearchPlugin = "Search plugins on Modrinth, or paste a Modrinth link",
        /// The same, in a copy of the Host that also searches CurseForge.
        ModsSearchModBoth = "Search mods, or paste a Modrinth or CurseForge link",
        ModsSearchPluginBoth = "Search plugins, or paste a Modrinth or CurseForge link",
        ModsSearchLabel = "Search Modrinth",
        /// {gameVersion} is a Minecraft version, {loader} a loader such as fabric.
        ModsOnlyCompatible = "Only compatible versions",
        ModsPlayersNeedToo = "Players need this too",
        ModsServerOnly = "Server only",
        ModsPlayersNeed = "Players need this",
        ModsUnknown = "Unknown",
        ModsUnknownModrinth = "Modrinth doesn’t say whether players need it. Check the mod’s page, and tick For players if they do.",
        ModsUnknownNotOnModrinth = "Not on Modrinth. Check the mod’s page, and tick For players if players need it.",
        /// A mod row's box (Fabric): players get the mod in the client pack when they join.
        ModsForPlayers = "For players",
        ModsForPlayersHint = "Players get this mod, and the mods it needs, when they join. Tick it for mods players need that Lodestone can't tell, such as ones you uploaded.",
        ModsForPlayersAlready = "Players get this mod already: Modrinth or the mod says they need it.",
        ModsForPlayersOptional = "Modrinth says players can join without this mod, but on Fabric mods like it often stop them joining, so players get it. Untick it to leave it out.",
        ModsMarkedForPlayers = "Players get it the next time they join",
        ModsUnmarkedForPlayers = "Players no longer get it when they join",
        ModsMarkFailed = "Couldn’t change it: {error}",
        ModsRelease = "Release",
        ModsBeta = "Beta",
        ModsAlpha = "Alpha",
        ModsInstalledTag = "Installed",
        ModsSortBy = "Sort by",
        ModsSortRelevance = "Relevance",
        ModsSortDownloads = "Downloads",
        ModsSortFollows = "Follows",
        ModsSortNewest = "Newest",
        ModsSortUpdated = "Recently updated",
        ModsOpening = "Opening the link…",
        ModsLinkFailed = "Couldn’t open the link: {error}",
        ModsOpenLink = "Open {slug}",
        ModsSearchFailed = "Search failed: {error}",
        ModsNoResults = "No results.",
        ModsPage = "Page {page} of {total}",
        ModsPreviousPage = "Previous page",
        ModsNextPage = "Next page",
        ModsEnableFailed = "Couldn’t turn it on: {error}",
        ModsDisableFailed = "Couldn’t turn it off: {error}",
        ModsUpdated = "Updated",
        ModsUpdateFailed = "Couldn’t update it: {error}",
        ModsUpdateTo = "Update to {version}",
        ModsViewOnModrinth = "View on Modrinth",
        ModsViewOnCurseForge = "View on CurseForge",
        /// The card for files only CurseForge's website has: the Host watches the Downloads folder for them.
        ModsByHandTitle = "Download these from CurseForge",
        /// {folder} is the folder watched for the downloads.
        ModsByHandLead = "Their authors don’t let other apps download them. Download each one in your browser; Lodestone adds it to the server as soon as it appears in {folder}.",
        ModsByHandNoFolder = "Their authors don’t let other apps download them. Download each one in your browser, then choose the folder it was saved to.",
        ModsByHandOpen = "Open download page",
        /// Opens the first page; each next one opens when a file arrives.
        ModsByHandOpenAll = "Open the pages one by one",
        /// Under the list, once the pages are being opened one by one.
        ModsByHandOneByOne = "The next page opens when the file before it has arrived.",
        ModsByHandAdded = "Added",
        /// {file} is the file name to download.
        ModsByHandWaiting = "Waiting for {file}",
        ModsByHandChangeFolder = "Watch another folder…",
        ModsByHandSkip = "Skip the rest",
        /// The folder dialog's button.
        ModsByHandFolderPrompt = "Watch this folder",
        ModsByHandFailed = "Couldn’t add {name}: {error}",
        /// The toast when a file the user downloaded was found and put on the server.
        ModsByHandAddedToast = "Added from your downloads",
        /// Why a mod from CurseForge shows as unknown: CurseForge has no such field.
        ModsUnknownCurseForge = "CurseForge doesn’t say whether players need it. Check the mod’s page, and tick For players if they do.",
        /// The empty Installed view, in a copy of the Host that also searches CurseForge.
        ModsEmptyModBoth = "No mods yet. Browse Modrinth or CurseForge, or upload a .jar file.",
        ModsEmptyPluginBoth = "No plugins yet. Browse Modrinth or CurseForge, or upload a .jar file.",
        ModsDeleteLabel = "Delete {name}",
        ModsUploaded = "Uploaded",
        ModsUploadFailed = "Upload failed: {error}",
        ModsOnlyJars = "Only .jar files can be uploaded. Not uploaded:",
        ModsUpdatesInstalled = "Updates installed",
        ModsDeleted = "Deleted",
        ModsDeleteFailed = "Couldn’t delete: {error}",
        ModsDeleteTitleMod = "Delete mod?",
        ModsDeleteTitlePlugin = "Delete plugin?",
        ModsDeleteBody = "Its file will be deleted from the server. This can’t be undone.",
        ModsOverwriteTitle = "Replace the files that are there?",
        ModsOverwrite = "Replace",
        /// {message} is the core's message.
        ModsOverwriteBody = "{message} Replace it with the uploaded file?",
        ModsOverwriteDefault = "A file with the same name is already installed.",
        ModsUpdateAll = "Update all",
        ModsCheckUpdates = "Check for updates",
        ModsUpload = "Upload",
        ModsUploadPrompt = "Upload",
        ModsLoadFailedMod = "Couldn’t load the mods: {error}",
        ModsLoadFailedPlugin = "Couldn’t load the plugins: {error}",
        ModsEmptyMod = "No mods yet. Browse Modrinth or upload a .jar file.",
        ModsEmptyPlugin = "No plugins yet. Browse Modrinth or upload a .jar file.",
        ModsPlainFabric = "Players can join with Fabric alone, at these versions.",
        ModsDownloadPack = "Download client modpack (.mrpack)",
        /// Folds the client pack's list of mods again, after ModsPackShowAll.
        ModsPackShowFewer = "Show fewer",
        ModsDownloadPackFailed = "Couldn’t download the client modpack: {error}",
        /// {name} is the file's name.
        ModsPackSaved = "Saved {name}",
        ModsSavePack = "Save",
        ModsImportHint = "Import it into Prism Launcher, the Modrinth App or ATLauncher to get everything at the right versions.",
        /// Updating the server's modpack. {pack} is the modpack's name, {version},
        /// {from} and {to} its versions, {software} a loader and version ("NeoForge 21.1.209").
        ModpackFrom = "This server runs this modpack.",
        ModpackCheck = "Check for an update",
        ModpackCheckFailed = "Couldn’t check for an update: {error}",
        ModpackUpToDate = "It’s the newest version.",
        ModpackUpdateTo = "Update to {version}",
        ModpackFromFile = "Update from a file",
        ModpackFilePrompt = "Update",
        ModpackNotMrpack = "Not a .mrpack file",
        ModpackPlanFailed = "Couldn’t prepare the update: {error}",
        ModpackDialogTitle = "Update the modpack?",
        /// {change} is what made the entry a restore goes back to ("Added JEI").
        ModpackDialogRestoreIntro = "The mods and configs go back to how they were after this change.",
        ModpackDialogReinstall = "The server moves to this game and loader and installs it again.",
        ModpackDialogKept = "Stay as you changed them",
        ModpackDialogNothing = "No files change.",
        ModpackDialogStop = "Stop the server first: it can’t be updated while it runs.",
        ModpackDialogUnsupported = "This version can’t be used: {reason}",
        ModpackUpdate = "Update",
        ModpackUpdated = "Updated to {version}",
        ModpackUpdateFailed = "Couldn’t update the modpack: {error}",
        ModpackUnknownVersion = "unknown version",
        /// The modpack's history (Mods tab): {version} is a modpack version, {change} what
        /// made a history entry ("Added JEI"), {date} when.
        ModpackWhatsNew = "What’s new",
        ModpackHistory = "History",
        ModpackHistoryFailed = "Couldn’t load the modpack’s history: {error}",
        ModpackHistoryEmpty = "Nothing is recorded yet. Changes from now on can be undone.",
        ModpackCurrent = "Current",
        ModpackRestore = "Restore",
        ModpackRestoreMissing = "Lodestone no longer has some of this version’s files.",
        ModpackRestoreTitle = "Restore this version?",
        ModpackRestored = "Restored",
        ModpackRestoreFailed = "Couldn’t restore that version: {error}",
        ModpackSinceTooltip = "Unchanged since: {change}",
        /// A Modrinth project.
        ModsProjectFailed = "Couldn’t load the project: {error}",
        ModsSource = "Source",
        ModsIssues = "Issues",
        ModsWiki = "Wiki",
        /// {value} is one of the side values (required, optional…).
        ModsServerSide = "Server side",
        ModsSideRequired = "Required",
        ModsSideOptional = "Optional",
        ModsSideUnsupported = "Unsupported",
        ModsSideUnknown = "Unknown",
        ModsCreateFromModpack = "Create a server from this modpack",
        ModsModpackNoPermission = "This is a modpack. You don’t have permission to create servers.",
        ModsNoServerSupport = "This project says it doesn’t run on servers.",
        ModsWrongTypeMod = "This is a {type}. Lodestone only installs mods here.",
        ModsWrongTypePlugin = "This is a {type}. Lodestone only installs plugins here.",
        ModsDescription = "Description",
        ModsRestOnModrinth = "The rest of the description is on Modrinth.",
        ModsRestOnCurseForge = "The rest of the description is on CurseForge.",
        ModsVersions = "Versions",
        ModsWithDependencies = "Also install required dependencies",
        ModsShowAll = "Show all versions",
        ModsVersionsFailed = "Couldn’t load the versions: {error}",
        ModsNoVersions = "No versions for the game and loader this server runs.",
        ModsNoVersionsShowAll = "No versions for the game and loader this server runs. Turn on “Show all versions” to see the others.",
        ModsNotFor = "Not for this server",
        ModsInstalledVersion = "Installed",
        ModsInstallFailed = "Couldn’t install it: {error}",
        ModsCurseForgeFailed = "Couldn’t ask CurseForge about the installed files: {error}",
        ModsCurseForgeUnavailable = "CurseForge isn’t available in this copy of Lodestone Host.",
        ModsIncompatibleTitle = "Install a version made for something else?",
        ModsInstallAnyway = "Install anyway",
        ModsIncompatibleBody = "It may crash the server.",
        /// The Settings tab's Friends section (launcher/docs/friends.md).
        FriendsTitle = "Friends",
        FriendsLead = "Friends you added in Lodestone Launcher see the servers you share with them, and join without a link.",
        FriendsAddInLauncher = "Add friends in Lodestone Launcher",
        FriendsAddInLauncherHint = "Lodestone Launcher on this computer keeps your friend list.",
        FriendsLoading = "Loading your friends…",
        /// A checkbox; {name} is the friend's name.
        FriendsShareWith = "Share this server",
        /// The two haven't compared safety codes in Lodestone Launcher.
        FriendsNotVerified = "Not verified",
        /// Under a friend to share with: {names} are their Minecraft accounts.
        FriendsMinecraft = "Added to the whitelist when you share, if the server has it on.",
        /// A toast after sharing: {names} are Minecraft player names.
        FriendsWhitelisted = "Added to the server’s whitelist",
        /// {name} is a friend's name, {error} what went wrong.
        FriendsWhitelistFailed = "Couldn’t add them to the server’s whitelist: {error}",
        FriendsConnected = "Connected now",
        FriendsSyncFailed = "Couldn’t sync your friend list to this core: {error}",
        FriendsOpenFailed = "Couldn’t open your friend list: {error}",
        FriendsPublishFailed = "Couldn’t tell {name} about this core: {error}",
        FriendsPublishFailedAll = "Couldn’t tell your friends about this core: {error}",
        FriendsRetry = "Try again",
        /// Under a failure: the Host tries again by itself.
        FriendsRetrying = "Lodestone Host tries again every minute.",
        /// This core has the friend list as it is now.
        FriendsSynced = "Friend list synced",
        /// The person retired their friend identity in Lodestone Launcher.
        FriendsRetired = "Your friend identity was retired",
        FriendsRetiredHint = "This core no longer shares with anyone. To start again, choose a new name in Lodestone Launcher's Friends section, then add your friends again.",
        /// A test copy of the Host (a developer's), whose core or friend list is a test one.
        FriendsOff = "Sharing with friends is off in this test copy",
        FriendsOffHint = "Its core or its friend list is in a test folder, and the other isn't, so your real friends never reach a test core. To test sharing, set both LODESTONE_PATH and LODESTONE_LAUNCHER_DATA_DIR to test folders.",
        /// Before the first share with a friend whose safety code wasn't compared; {name} is their name.
        FriendsConfirmUnverified = "You haven’t compared safety codes with this friend yet, so you can’t be sure it’s really them: someone else could have answered your friend link. Compare codes in Lodestone Launcher first, or share anyway.",
        FriendsShareAnyway = "Share anyway",
        /// The core's friend certificate: what friends know this core by.
        FriendsCertificate = "This core’s friend certificate",
        FriendsCertificateHint = "Friends know this core by it. Retire it if this computer’s data may have been copied: friends drop it, and the core gets a new one they find instead.",
        FriendsCertificateRetire = "Retire certificate…",
        FriendsCertificateRetireBody = "Friends drop this core’s current certificate and stop trusting its servers’ mods. The core gets a new certificate, and friends find it under that one the next time they check. Everyone connected now is disconnected.",
        FriendsCertificateRetireConfirm = "Retire and get a new one",
        /// The core's certificate was retired (here, or in Lodestone Launcher).
        FriendsCertificateRetired = "This core’s friend certificate was retired",
        FriendsCertificateRetiredHint = "Friends don’t see this core now.",
        FriendsCertificateNew = "Get a new certificate",
        /// For a core too old to make a new certificate; {path} is the certificate's file.
        FriendsCertificateManual = "Friends don’t see this core now. This core can’t make a new certificate itself: quit Lodestone Host, delete this file, and open it again.",
        FriendsCancel = "Cancel",
        /// More words a Settings search finds the row by (never shown).
        MemoryKeywords = "memory RAM heap",
        /// More words a Settings search finds the row by (never shown).
        JvmArgsKeywords = "JVM Java arguments flags options",
        /// More words a Settings search finds the row by (never shown).
        WhitelistKeywords = "whitelist allowlist players",
        /// More words a Settings search finds the row by (never shown).
        MotdKeywords = "MOTD message of the day server description",
        /// More words a Settings search finds the row by (never shown).
        AutoStartKeywords = "boot startup launch automatically",
        /// Over the Plugins tab's list, before its count.
        ModsPluginsLabel = "Plugins",
        /// A badge by the modpack's name when a newer version is out.
        ModpackUpdateAvailable = "Update available",
        /// A key: how many files a modpack change replaced.
        ModpackReplacedKey = "Replaced",
        /// A key: when a modpack history entry was recorded.
        ModpackRecorded = "Recorded",
        /// A column: what a mod's version is made for.
        ModsThisVersion = "This version",
        /// A column: what the server runs.
        ModsThisServer = "This server",
        /// A key before a friend's Minecraft accounts.
        FriendsMinecraftKey = "Minecraft accounts",
        /// A badge: the server is shared with the friend.
        FriendsShared = "Shared",
        /// Before the server's new state, a badge.
        EventStateLead = "The server is now",
    }

    pub enum InstancePlural {
        ModsPackTitle = ("Players need {count} mod to join", "Players need {count} mods to join"),
        ModsImportHintJars = ("Import it into Prism Launcher, the Modrinth App or ATLauncher to get everything at the right versions. {count} mod isn’t on Modrinth, so its file is in the pack.", "Import it into Prism Launcher, the Modrinth App or ATLauncher to get everything at the right versions. {count} mods aren’t on Modrinth, so their files are in the pack."),
        /// Under the first few of a big client pack's mods: shows them all.
        ModsPackShowAll = ("Show all {count} mod", "Show all {count} mods"),
        ModsShowMore = ("Show more ({count} left)", "Show more ({count} left)"),
        ModpackAdded = ("{count} file added", "{count} files added"),
        ModpackReplaced = ("{count} file replaced", "{count} files replaced"),
        ModpackRemoved = ("{count} file removed", "{count} files removed"),
        ModpackMoreChanges = ("and {count} more", "and {count} more"),
        FriendsConnectedCount = ("{count} friend connected now", "{count} friends connected now"),
        /// A tooltip's heading, over the names of the mods that need this one, one a line.
        ModsNeededBy = ("Needed by {count} mod", "Needed by {count} mods"),
        /// A toast's heading over mod names, one a line.
        ModsResultInstalled = ("Installed {count} mod", "Installed {count} mods"),
        /// A toast's heading over file names, one a line.
        ModsResultRemoved = ("Removed {count} file", "Removed {count} files"),
        /// A toast's heading over projects and why, one a line.
        ModsResultSkipped = ("Skipped {count} project", "Skipped {count} projects"),
        /// A toast's heading over mod names, one a line.
        ModsResultPlayersNeed = ("Players will need {count} of them too", "Players will need {count} of them too"),
        /// A toast's heading over each mod and its error, one a line.
        ModsSomeUpdatesFailed = ("{count} update failed", "{count} updates failed"),
        /// A toast's heading over mod names, one a line.
        ModsUploadedCount = ("Uploaded {count} mod", "Uploaded {count} mods"),
    }
}
