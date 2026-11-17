//! The words of the readable components: status words, kinds, untitled names,
//! reasons, outcomes, and the patterns of accessible names (`components::readable`).

crate::i18n::messages! {
    pub enum ValueMsg {
        /// A key and its value as one text, for accessible names and tooltips.
        KeyValue = "{key}: {value}",
        /// The accessible name of a from-to value.
        FromTo = "Changes from {from} to {to}",
        /// The accessible name of a meter of needed against free space.
        NeededOfFree = "{needed} needed, {free} free",
        Online = "Online",
        Offline = "Offline",
        Running = "Running",
        Stopped = "Stopped",
        Starting = "Starting",
        Stopping = "Stopping",
        Failed = "Failed",
        Installing = "Installing",
        Downloading = "Downloading",
        Copying = "Copying",
        Paused = "Paused",
        /// Kinds, the accessible names of kind icons.
        KindFolder = "Folder",
        KindWorld = "World",
        KindMod = "Mod",
        KindResourcePack = "Resource pack",
        KindShaderPack = "Shader pack",
        KindDataPack = "Data pack",
        KindPlugin = "Plugin",
        KindModpack = "Modpack",
        /// A picture: a screenshot, an icon.
        KindImage = "Image",
        KindServer = "Server",
        KindInstance = "Instance",
        KindArchive = "Archive",
        KindFile = "File",
        KindBackup = "Backup",
        KindSnapshot = "Snapshot",
        KindFriend = "Friend",
        KindComputer = "Computer",
        /// A snapshot nobody named.
        UntitledSnapshot = "Untitled snapshot",
        UntitledBackup = "Untitled backup",
        /// Why a snapshot was made, on a reason tag.
        BeforeUpdate = "Before update",
        BeforeRestore = "Before restore",
        /// Before the object a copy was made from.
        CopyOf = "Copy of",
        Automatic = "Automatic",
        /// Outcomes, the headings of a list of names.
        Added = "Added",
        Skipped = "Skipped",
        NotAdded = "Not added",
        /// Shows a hidden value, such as a key.
        Reveal = "Reveal",
        Hide = "Hide",
        /// What a hidden value reads as to a screen reader.
        Hidden = "Hidden",
        OpenLink = "Open link",
    }

    pub enum ValuePlural {
        /// Under a list that shows its first names.
        ShowMore = ("Show {count} more", "Show {count} more"),
    }
}
