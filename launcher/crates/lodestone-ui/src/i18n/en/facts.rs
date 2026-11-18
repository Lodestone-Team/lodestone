//! The keys of facts: the short labels before or above a value ("Size", "Last played").
//!
//! Keys are separate messages with no trailing colon, and a key and its value are two
//! elements, never one translated string: the components lay them out
//! (`components::readable`). An app's own keys stay in the app's areas.

crate::i18n::messages! {
    pub enum FactMsg {
        /// The game, keyed in a strip or a fact table: its value is "Minecraft 26.3".
        Game = "Game",
        /// A loader or server software, with the phrase "Fabric 0.20.1" as its value.
        Loader = "Loader",
        Version = "Version",
        Size = "Size",
        /// When something was added.
        Added = "Added",
        Modified = "Modified",
        Created = "Created",
        LastPlayed = "Last played",
        PlayTime = "Play time",
        Players = "Players",
        Port = "Port",
        Address = "Address",
        Folder = "Folder",
        Status = "Status",
        World = "World",
        Worlds = "Worlds",
        Mods = "Mods",
        /// A count of files.
        Files = "Files",
        /// Free disk space.
        Free = "Free",
        /// Disk space something needs.
        Needed = "Needed",
        File = "File",
        Path = "Path",
        Url = "URL",
        Server = "Server",
        /// The computer a server runs on.
        Host = "Host",
        Instance = "Instance",
        Owner = "Owner",
        Author = "Author",
        /// How many times a project was downloaded.
        Downloads = "Downloads",
        Followers = "Followers",
        Published = "Published",
        Updated = "Updated",
        Released = "Released",
        Installed = "Installed",
        /// How much there is to download.
        Download = "Download",
        /// When a snapshot or a backup was made.
        Made = "Made",
        /// When a screenshot was taken.
        Taken = "Taken",
        /// When a player or a friend joined.
        Joined = "Joined",
        Expires = "Expires",
        Reason = "Reason",
        Progress = "Progress",
        Items = "Items",
        Modpack = "Modpack",
        Java = "Java",
        Memory = "Memory",
        Uptime = "Uptime",
        Ping = "Ping",
        Connection = "Connection",
        /// Space that removing something gives back.
        FreesUp = "Frees up",
        ExitCode = "Exit code",
        /// An API key's or a token's visible part.
        Key = "Key",
        /// What a restore puts in place of.
        Replaces = "Replaces",
        KeptAsideIn = "Kept aside in",
        SavedTo = "Saved to",
        LeftOut = "Left out",
        From = "From",
        To = "To",
        /// A comparison's column for the instance on this computer.
        ThisInstance = "This instance",
        Type = "Type",
        /// A release build's number.
        Build = "Build",
        Commit = "Commit",
        Removed = "Removed",
        Backups = "Backups",
        Snapshots = "Snapshots",
        Instances = "Instances",
        /// A server's software: Paper, Fabric, Vanilla.
        Software = "Software",
        Available = "Available",
        /// The Loader fact of an instance without one, in a fact table.
        Vanilla = "Vanilla",
    }

    pub enum FactPlural {
    }
}
