//! An instance's Snapshots tab (the route and the message names still say backups): world
//! backups, plain zips of the world made when asked, with restoring, downloading and
//! deleting; and snapshots of the whole server folder, taken now and on a schedule, with
//! restoring, downloading, exporting and deleting.

crate::i18n::messages! {
    pub enum BackupsMsg {
        /// Under the Snapshots section's heading.
        Lead = "Each snapshot is a full copy of the server folder, but files that didn’t change are stored only once, so taking them often takes little space.",
        BackUpNow = "Take snapshot",
        /// While a snapshot is taken (above its progress bar).
        BackingUp = "Taking a snapshot…",
        /// While a snapshot is restored.
        Restoring = "Restoring…",
        /// Why "Take snapshot", "Back up world…" and the restore buttons are off.
        JobRunning = "A snapshot, world backup or restore of this server is running.",
        NoSettingsPermission = "You need permission to change this server’s settings.",
        NoWritePermission = "You need permission to change this server’s files.",
        NoSchedulePermission = "You need permission to change this server’s settings and files.",
        /// {error}: the core's error message.
        LoadFailed = "Couldn’t load the snapshots: {error}",
        /// The schedule's card.
        Schedule = "Schedule",
        Automatic = "Take snapshots automatically",
        AutomaticHint = "Nothing is saved when nothing changed since the last snapshot.",
        /// {time}: how long until then, such as "2 h 5 min".
        NextBackupIn = "Next snapshot in {time}",
        NextBackupSoon = "Next snapshot soon",
        /// A snapshot is due, but the schedule only takes one after players were on.
        NextBackupWhenPlayerJoins = "Next snapshot once a player joins",
        Interval = "How often",
        EveryHour = "Every hour",
        EveryDay = "Every day",
        OnlyWhenActive = "Only when players were on",
        OnlyWhenActiveHint = "Skips the snapshot when nobody played since the last one.",
        /// How many automatic snapshots are kept.
        Keep = "Keep",
        KeepHint = "How many automatic snapshots are kept. Manual and pinned snapshots are always kept.",
        KeepFewer = "Fewer",
        KeepNormal = "Normal",
        KeepMore = "More",
        /// The retention matches none of the choices (set through the API).
        KeepCustom = "Custom",
        ScheduleSaveFailed = "Couldn’t save the schedule: {error}",
        /// The storage line: "12 snapshots · 48.2 GB of files · 3.1 GB stored".
        StorageFiles = "{size} of files",
        StorageStored = "{size} stored",
        StorageFree = "{size} free",
        Empty = "No snapshots yet. Take one now, or turn on automatic snapshots.",
        /// A snapshot or world backup without a name is called by when it was taken: {date} such as
        /// "Nov 3", {time} such as "14:30".
        DateAt = "{date}, {time}",
        /// A snapshot's size: "1.2 GB · 34 MB new".
        SizeNew = "{size} · {new} new",
        /// Tags on a snapshot or a world backup.
        TagManual = "Manual",
        TagAutomatic = "Automatic",
        TagBeforeRestore = "Before restore",
        TagLive = "Live",
        TagLiveHint = "Taken while the server was running.",
        TagInconsistent = "May be inconsistent",
        TagInconsistentHint = "The server didn’t confirm that it saved the world before the copy, so the world may have been caught mid-save.",
        TagSkippedHint = "Files that couldn’t be read, such as files in use, were left out or kept from the previous snapshot.",
        TagPinned = "Pinned",
        TagPinnedHint = "Automatic cleanup never deletes it.",
        /// A snapshot's actions.
        RestoreAction = "Restore…",
        DownloadLabel = "Download as a zip",
        PinLabel = "Pin, so it’s never deleted automatically",
        UnpinLabel = "Unpin",
        RestoreAsNewLabel = "Restore as a new server",
        DeleteLabel = "Delete snapshot",
        /// The restore dialog; {name} is the snapshot's name or date.
        RestoreTitle = "Restore “{name}”",
        ScopeEverything = "Everything",
        ScopeWorld = "Only the world",
        ScopeEverythingHint = "Replaces the server’s files with the snapshot’s. Its name, port and Java settings stay.",
        ScopeWorldHint = "Replaces the world, its Nether and its End. Settings, mods and plugins stay as they are.",
        NoWorld = "This snapshot has no world folder, so only the whole server can be restored.",
        StopFirst = "Stop the server first",
        StartAfter = "Start it again after",
        MustStop = "The server must be stopped to restore a snapshot.",
        SafetyNote = "A snapshot of the server as it is now is taken first, so you can undo this.",
        Restore = "Restore",
        /// A toast; {name} is the snapshot's name or date.
        RestoreStarted = "Restoring “{name}”…",
        /// The delete confirmation.
        DeleteTitle = "Delete this snapshot?",
        DeleteBody = "“{name}” will be deleted. The server itself doesn’t change.",
        Deleted = "Snapshot deleted",
        /// The restore-as-new-server dialog.
        NewTitle = "Restore as a new server",
        NewBody = "Makes a new server from this snapshot, with its own port. This server doesn’t change.",
        NewName = "Name",
        /// The new server's name until changed; {name} is this server's.
        NewDefaultName = "{name} (restored)",
        NewCreate = "Create server",
        /// A toast; {name} is the new server's name.
        NewStarted = "Creating “{name}”…",
        /// Downloading a snapshot.
        Zipping = "Zipping the snapshot for download…",
        /// {name}: the file's name (a snapshot's or a world backup's zip).
        Downloaded = "Saved {name}",
        DownloadFailed = "Couldn’t download the snapshot: {error}",
        /// Toasts when a job ends without a message from the core.
        BackupDone = "Snapshot done",
        BackupFailed = "Snapshot failed",
        RestoreDone = "Restore done",
        RestoreFailed = "Restore failed",
        /// Exporting a snapshot as a file for another computer.
        ExportLabel = "Export as a file for another Host or a Launcher",
        /// The export dialog; {name} is the snapshot's name or date.
        ExportTitle = "Export “{name}”",
        ExportBody = "Saves this snapshot as a .lodeimg file. Another Lodestone Host can make a server from it, and Lodestone Launcher can make a singleplayer instance of its world. Passwords in server.properties are blanked, and files that hold a password or token are left out.",
        ExportPlugins = "Include plugin settings (they can hold passwords)",
        ExportPluginsHint = "Everything in the plugins’ folders: settings, databases and player data.",
        ExportAction = "Export…",
        /// While the core writes the file.
        ExportPreparing = "Preparing the file…",
        /// While the file downloads.
        ExportSaving = "Saving the file…",
        /// {name}: the file's name.
        Exported = "Saved {name}",
        /// {names}: secrets made empty, as "server.properties: rcon.password".
        ExportedBlanked = "Made empty: {names}",
        /// {names}: CurseForge files whose authors don't allow downloads elsewhere.
        ExportedNotDownloadable = "Not in the file, as their authors don’t allow downloads elsewhere: {names}",
        /// {names}: settings files left out because they hold a password, a token or a key.
        ExportedSecretsWithheld = "Left out, as they hold a password or token (the other server writes its defaults): {names}",
        ExportFailed = "Couldn’t export the snapshot: {error}",
        ExportCancelled = "The export was cancelled",
        /// The page's two sections.
        WorldSection = "World backups",
        SnapshotsSection = "Snapshots",
        /// Under the World backups section's heading.
        WorldLead = "A world backup is a plain .zip of the world, its Nether and its End, which also opens outside Lodestone. One is made only when you ask.",
        WorldBackUp = "Back up world…",
        /// The dialog that makes a world backup.
        WorldCreateTitle = "Back up the world",
        WorldCreateBody = "Saves the world as a .zip. A running server keeps running; saving pauses while the world is copied.",
        WorldName = "Name (optional)",
        WorldCreate = "Back up",
        /// While a world backup is made or restored (above its progress bar).
        WorldBackingUp = "Backing up the world…",
        WorldRestoring = "Restoring the world…",
        WorldEmpty = "No world backups yet.",
        /// {error}: the core's error message.
        WorldLoadFailed = "Couldn’t load the world backups: {error}",
        /// A world backup's size and game version: "182 MB · Minecraft 1.21.4".
        WorldSizeVersion = "{size} · Minecraft {version}",
        /// The Before restore tag's tooltip; {name} is the world backup restored then.
        WorldBeforeRestoreHint = "Taken before restoring “{name}”.",
        WorldTagInconsistentHint = "The server didn’t confirm that it saved the world before the copy.",
        WorldLeftOutHint = "Links, special files, files that couldn’t be read and names that clash on some disks were left out.",
        /// A world backup's actions.
        WorldDownloadLabel = "Download the .zip",
        WorldDeleteLabel = "Delete world backup",
        /// The restore dialog; {name} is the world backup's name or date.
        WorldRestoreTitle = "Restore the world from “{name}”",
        WorldRestoreBody = "Replaces this server’s world, its Nether and its End with the backup’s. Settings, mods and plugins stay as they are.",
        WorldMustStop = "The server must be stopped to restore a world backup.",
        WorldSafetyNote = "The current world is saved as a world backup first, so you can undo this.",
        /// A toast; {name} is the world backup's name or date.
        WorldRestoreStarted = "Restoring the world from “{name}”…",
        /// The delete confirmation.
        WorldDeleteTitle = "Delete this world backup?",
        WorldDeleteBody = "“{name}” will be deleted. The server’s world doesn’t change.",
        WorldDeleted = "World backup deleted",
        WorldDownloadFailed = "Couldn’t download the world backup: {error}",
        WorldDownloadCancelled = "The download was cancelled",
        /// Toasts when a world job ends without a message from the core.
        WorldBackupDone = "World backup done",
        WorldBackupFailed = "World backup failed",
        WorldRestoreDone = "World restored",
        WorldRestoreFailed = "Couldn’t restore the world",
        /// Cancelling a world job failed (a restore already putting the world in place).
        WorldCancelFailed = "Couldn’t cancel: {error}",
    }

    pub enum BackupsPlural {
        Count = ("{count} snapshot", "{count} snapshots"),
        Skipped = ("{count} file skipped", "{count} files skipped"),
        EveryMinutes = ("Every {count} minute", "Every {count} minutes"),
        EveryHours = ("Every {count} hour", "Every {count} hours"),
        EveryDays = ("Every {count} day", "Every {count} days"),
        /// After "Saved …": how many files the export took out (Lodestone's settings, the
        /// EULA, server software, crash logs, plugin settings).
        ExportedLeftOut = (
            "{count} file that mustn’t leave this computer was taken out",
            "{count} files that mustn’t leave this computer were taken out"
        ),
        /// The world backups' storage line: "3 world backups · 540 MB · 182 GB free".
        WorldCount = ("{count} world backup", "{count} world backups"),
        /// A tag on a world backup that left out links or files it couldn't read.
        WorldLeftOut = ("{count} file left out", "{count} files left out"),
    }
}
