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
    }

    pub enum BackupsPlural {
        Count = ("{count} backup", "{count} backups"),
        Skipped = ("{count} file skipped", "{count} files skipped"),
        EveryMinutes = ("Every {count} minute", "Every {count} minutes"),
        EveryHours = ("Every {count} hour", "Every {count} hours"),
        EveryDays = ("Every {count} day", "Every {count} days"),
    }
}
