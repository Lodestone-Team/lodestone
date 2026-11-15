//! An instance's Backups tab: backing up now and on a schedule, the list, restoring,
//! downloading and deleting.

crate::i18n::messages! {
    pub enum BackupsMsg {
        /// Under the tab's heading.
        Lead = "Each backup is a full copy of the server, but files that didn’t change are stored only once, so backing up often takes little space.",
        BackUpNow = "Back up now",
        /// While a backup runs (above its progress bar).
        BackingUp = "Backing up…",
        /// While a restore runs.
        Restoring = "Restoring…",
        /// Why "Back up now" and the restore buttons are off.
        JobRunning = "A backup or restore of this server is running.",
        NoSettingsPermission = "You need permission to change this server’s settings.",
        NoWritePermission = "You need permission to change this server’s files.",
        NoSchedulePermission = "You need permission to change this server’s settings and files.",
        /// {error}: the core's error message.
        LoadFailed = "Couldn’t load the backups: {error}",
        /// The schedule's card.
        Schedule = "Schedule",
        Automatic = "Back up automatically",
        AutomaticHint = "Nothing is saved when nothing changed since the last backup.",
        /// {time}: how long until then, such as "2 h 5 min".
        NextBackupIn = "Next backup in {time}",
        NextBackupSoon = "Next backup soon",
        /// A backup is due, but the schedule only backs up after players were on.
        NextBackupWhenPlayerJoins = "Next backup once a player joins",
        Interval = "How often",
        EveryHour = "Every hour",
        EveryDay = "Every day",
        OnlyWhenActive = "Only when players were on",
        OnlyWhenActiveHint = "Skips the backup when nobody played since the last one.",
        /// How many automatic backups are kept.
        Keep = "Keep",
        KeepHint = "How many automatic backups are kept. Manual and pinned backups are always kept.",
        KeepFewer = "Fewer",
        KeepNormal = "Normal",
        KeepMore = "More",
        /// The retention matches none of the choices (set through the API).
        KeepCustom = "Custom",
        ScheduleSaveFailed = "Couldn’t save the schedule: {error}",
        /// The storage line: "12 backups · 48.2 GB of files · 3.1 GB stored".
        StorageFiles = "{size} of files",
        StorageStored = "{size} stored",
        StorageFree = "{size} free",
        Empty = "No backups yet. Back up now, or turn on automatic backups.",
        /// A backup without a name is called by when it was taken: {date} such as
        /// "Nov 3", {time} such as "14:30".
        DateAt = "{date}, {time}",
        /// A backup's size: "1.2 GB · 34 MB new".
        SizeNew = "{size} · {new} new",
        /// Tags on a backup.
        TagManual = "Manual",
        TagAutomatic = "Automatic",
        TagBeforeRestore = "Before restore",
        TagLive = "Live",
        TagLiveHint = "Taken while the server was running.",
        TagInconsistent = "May be inconsistent",
        TagInconsistentHint = "The server didn’t confirm that it saved the world before the copy, so the world may have been caught mid-save.",
        TagSkippedHint = "Files that couldn’t be read, such as files in use, were left out or kept from the previous backup.",
        TagPinned = "Pinned",
        TagPinnedHint = "Automatic cleanup never deletes it.",
        /// A backup's actions.
        RestoreAction = "Restore…",
        DownloadLabel = "Download as a zip",
        PinLabel = "Pin, so it’s never deleted automatically",
        UnpinLabel = "Unpin",
        RestoreAsNewLabel = "Restore as a new server",
        DeleteLabel = "Delete backup",
        /// The restore dialog; {name} is the backup's name or date.
        RestoreTitle = "Restore “{name}”",
        ScopeEverything = "Everything",
        ScopeWorld = "Only the world",
        ScopeEverythingHint = "Replaces the server’s files with the backup’s. Its name, port and Java settings stay.",
        ScopeWorldHint = "Replaces the world, its Nether and its End. Settings, mods and plugins stay as they are.",
        NoWorld = "This backup has no world folder, so only the whole server can be restored.",
        StopFirst = "Stop the server first",
        StartAfter = "Start it again after",
        MustStop = "The server must be stopped to restore a backup.",
        SafetyNote = "A backup of the server as it is now is taken first, so you can undo this.",
        Restore = "Restore",
        /// A toast; {name} is the backup's name or date.
        RestoreStarted = "Restoring “{name}”…",
        /// The delete confirmation.
        DeleteTitle = "Delete this backup?",
        DeleteBody = "“{name}” will be deleted. The server itself doesn’t change.",
        Deleted = "Backup deleted",
        /// The restore-as-new-server dialog.
        NewTitle = "Restore as a new server",
        NewBody = "Makes a new server from this backup, with its own port. This server doesn’t change.",
        NewName = "Name",
        /// The new server's name until changed; {name} is this server's.
        NewDefaultName = "{name} (restored)",
        NewCreate = "Create server",
        /// A toast; {name} is the new server's name.
        NewStarted = "Creating “{name}”…",
        /// Downloading a backup.
        Zipping = "Zipping the backup for download…",
        /// {name}: the zip's file name.
        Downloaded = "Saved {name}",
        DownloadFailed = "Couldn’t download the backup: {error}",
        /// Toasts when a job ends without a message from the core.
        BackupDone = "Backup done",
        BackupFailed = "Backup failed",
        RestoreDone = "Restore done",
        RestoreFailed = "Restore failed",
        /// Exporting a backup as a file for another computer.
        ExportLabel = "Export as a file for another Host or a Launcher",
        /// The export dialog; {name} is the backup's name or date.
        ExportTitle = "Export “{name}”",
        ExportBody = "Saves this backup as a .lodeimg file. Another Lodestone Host can make a server from it, and Lodestone Launcher can make a singleplayer instance of its world. Passwords in server.properties are blanked, and files that hold a password or token are left out.",
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
        ExportFailed = "Couldn’t export the backup: {error}",
        ExportCancelled = "The export was cancelled",
    }

    pub enum BackupsPlural {
        Count = ("{count} backup", "{count} backups"),
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
    }
}
