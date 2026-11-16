//! Moving instances and servers between computers as `.lodeimg` files: what a file holds
//! before a server is made from it (the GPUI Host's "From a file", the Launcher's "Host
//! this"), and what the server didn't get once made ([`crate::sharing`]).

crate::i18n::messages! {
    pub enum SharingMsg {
        ImageMadeByOther = "Made by another program",
        /// {names}: mod files whose bytes the file carries.
        ImageCarried = "In the file itself, not downloaded from Modrinth",
        /// {names}: mods dropped because only players run them.
        ImageClientOnly = "Mods only players run, left out",
        /// {names}: CurseForge files the file doesn't carry, which the core can't download.
        ImageNotDownloadable = "Lodestone servers can’t download from CurseForge yet, so these files won’t be installed.",
        /// {names}: what the sender's Lodestone left out (patterns).
        ImageTakenOut = "Taken out by the sender",
        /// {names}: secrets the sender's Lodestone made empty.
        ImageBlanked = "Made empty by the sender",
        /// {names}: files that would give the file's sender a way into the server, or hold
        /// a password or token (the sender's bot token, store key, proxy secret).
        ImageSecretsNotTaken = "Not taken, as they hold a key to the server, a password or a token",
        /// {names}: files the sender's copy couldn't hold.
        ImageWithheld = "Missing from the file",
        /// {what}: a list of the things below.
        ImageNotTaken = "Not taken from the file",
        NotTakenSoftware = "Server software",
        NotTakenEula = "EULA",
        NotTakenClientFiles = "Players’ own files",
        NotTakenOperators = "Operators",
        NotTakenScripts = "Scripts",
        ImageOfflineMode = "Players can join without a Minecraft account.",
        /// Choosing between a vanilla and a Paper server for a vanilla game's file.
        ImageServerSoftware = "Server software",
        ImageServerSoftwareHint = "Paper runs plugins. A vanilla server runs Minecraft as it is.",
        ImageWorld = "World",
        ImageWorldHint = "The world that becomes the server’s world.",
        ImageNoWorld = "No world (a new one)",
        /// {size}: what is downloaded.
        ImageDownloads = "From Modrinth",
        /// {needed} and {free}: sizes.
        ImageDisk = "Disk space",
        /// The free space needed is worked out for the world picked first.
        ImageNoRoom = "There isn’t enough free space to make this server with the file’s own world. A smaller world, or none, may fit.",
        ImageNeedsGlobal = "This file has jar files of its own, so making a server from it needs permission to change global files.",
        /// A file with no mods that still carries protected files of its own.
        ImageProtectedWarning = "It has protected files of its own, such as jar files or programs, from whoever made it. Only use files from people you trust.",
        /// {flavour}: the file's server software (Spigot, Purpur…).
        ImageMadeAsPaper = "Lodestone makes these servers as Paper servers, which run the same plugins.",
        /// {loader}: the file's mod loader (Quilt…).
        ImageUnsupported = "Lodestone can’t make servers with this loader.",
        /// A key: the app that made the file, then its version.
        ImageMadeWith = "Made with",
        /// A key: how many files the .lodeimg holds.
        ImageFiles = "Files in the file",
        /// A key: the .lodeimg's size.
        ImageSize = "File size",
        /// A key: the pack the file is built on.
        ImageBasedOn = "Based on",
        /// Over the CurseForge files that won't be installed.
        ImageNotInstalled = "Won’t be installed",
        /// A badge: a setting is off.
        ImageOff = "Off",
        /// Over the mods and plugins downloaded from Modrinth.
        ImageDownloadedMods = "Downloaded from Modrinth",
        /// Under the space needed.
        ImageDiskSpare = "Needed includes 1 GB to spare.",
        NotTakenSoftwareReason = "Lodestone installs its own",
        NotTakenOperatorsReason = "Can be kept",
    }

    pub enum SharingPlural {
        /// The trust warning of a file whose server takes mods or plugins.
        ImageModsWarning = (
            "It has {count} mod or plugin: a program that runs on the server. Only use files from people you trust.",
            "It has {count} mods or plugins: programs that run on the server. Only use files from people you trust."
        ),
        /// What a server made from a file didn't get (its creation's outcome). {names}: the
        /// first names, "…" after them when there are more.
        OutcomeNotDownloaded = ("{count} file couldn’t be downloaded and was left out", "{count} files couldn’t be downloaded and were left out"),
        OutcomeNoSource = ("{count} file can’t be downloaded by Lodestone servers and was left out", "{count} files can’t be downloaded by Lodestone servers and were left out"),
        OutcomeClientJars = ("{count} mod that runs only on players’ computers was removed", "{count} mods that run only on players’ computers were removed"),
        OutcomeSecrets = ("{count} file holding a key to the server, a password or a token was left out, so the server writes its defaults", "{count} files holding a key to the server, a password or a token were left out, so the server writes its defaults"),
        OutcomeSettingsRemoved = ("{count} settings file couldn’t be checked for keys to the server and was left out, so the server writes its defaults", "{count} settings files couldn’t be checked for keys to the server and were left out, so the server writes its defaults"),
        OutcomeScripts = (
            "{count} script was left out: only the owner can add scripts, while code execution is on.",
            "{count} scripts were left out: only the owner can add scripts, while code execution is on."
        ),
        /// After a list's names: how many more the core counted but didn't list.
        ImageMoreNames = ("{count} more not listed", "{count} more not listed"),
    }
}
