//! Moving instances and servers between computers as `.lodeimg` files: what a file holds
//! before a server is made from it (the GPUI Host's "From a file", the Launcher's "Host
//! this"), and what the server didn't get once made ([`crate::sharing`]).

crate::i18n::messages! {
    pub enum SharingMsg {
        /// {version}: the Lodestone version that made the file.
        ImageMadeByHost = "Made with Lodestone Host {version}",
        ImageMadeByLauncher = "Made with Lodestone Launcher {version}",
        ImageMadeByOther = "Made by another program",
        /// {names}: mod files whose bytes the file carries.
        ImageCarried = "These are in the file itself, not downloaded from Modrinth: {names}",
        /// {names}: mods dropped because only players run them.
        ImageClientOnly = "Mods only players run, left out: {names}",
        /// {names}: CurseForge files the file doesn't carry, which the core can't download.
        ImageNotDownloadable = "Lodestone servers can’t download from CurseForge yet, so these files won’t be installed: {names}",
        /// {names}: what the sender's Lodestone left out (patterns).
        ImageTakenOut = "Taken out by the sender: {names}",
        /// {names}: secrets the sender's Lodestone made empty.
        ImageBlanked = "Made empty by the sender: {names}",
        /// {names}: files that would give the file's sender a way into the server, or hold
        /// a password or token (the sender's bot token, store key, proxy secret).
        ImageSecretsNotTaken = "Not taken, as they hold a key to the server, a password or a token: {names}",
        /// {names}: files the sender's copy couldn't hold.
        ImageWithheld = "Missing from the file: {names}",
        /// {what}: a list of the things below.
        ImageNotTaken = "Not taken from the file: {what}",
        NotTakenSoftware = "the server software (Lodestone installs its own)",
        NotTakenEula = "the EULA",
        NotTakenClientFiles = "a player’s own files",
        NotTakenOperators = "the operators",
        NotTakenScripts = "scripts",
        ImageOfflineMode = "Its settings let players join without a Minecraft account (online-mode is off).",
        /// Choosing between a vanilla and a Paper server for a vanilla game's file.
        ImageServerSoftware = "Server software",
        ImageServerSoftwareHint = "Paper runs plugins. A vanilla server runs Minecraft as it is.",
        ImageWorld = "World",
        ImageWorldHint = "The world that becomes the server’s world.",
        ImageNoWorld = "No world (a new one)",
        /// {size}: what is downloaded.
        ImageDownloads = "Downloads {size} from Modrinth",
        /// {size}: what is downloaded; {names}: the mods and plugins among it.
        ImageDownloadsMods = "Downloads {size} from Modrinth: {names}",
        /// {needed} and {free}: sizes.
        ImageDisk = "Needs {needed} of disk space, 1 GB of it to spare; {free} is free.",
        /// The free space needed is worked out for the world picked first.
        ImageNoRoom = "There isn’t enough free space to make this server with the file’s own world. A smaller world, or none, may fit.",
        ImageNeedsGlobal = "This file has jar files of its own, so making a server from it needs permission to change global files.",
        /// A file with no mods that still carries protected files of its own.
        ImageProtectedWarning = "It has protected files of its own, such as jar files or programs, from whoever made it. Only use files from people you trust.",
        /// {flavour}: the file's server software (Spigot, Purpur…).
        ImageMadeAsPaper = "Lodestone makes {flavour} servers as Paper servers, which run the same plugins.",
        /// {loader}: the file's mod loader (Quilt…).
        ImageUnsupported = "Lodestone can’t make {loader} servers.",
    }

    pub enum SharingPlural {
        /// The trust warning of a file whose server takes mods or plugins.
        ImageModsWarning = (
            "It has {count} mod or plugin: a program that runs on the server. Only use files from people you trust.",
            "It has {count} mods or plugins: programs that run on the server. Only use files from people you trust."
        ),
        /// What a server made from a file didn't get (its creation's outcome). {names}: the
        /// first names, "…" after them when there are more.
        OutcomeNotDownloaded = (
            "{count} file couldn’t be downloaded and was left out: {names}",
            "{count} files couldn’t be downloaded and were left out: {names}"
        ),
        OutcomeNoSource = (
            "{count} file can’t be downloaded by Lodestone servers and was left out: {names}",
            "{count} files can’t be downloaded by Lodestone servers and were left out: {names}"
        ),
        OutcomeClientJars = (
            "{count} mod that runs only on players’ computers was removed: {names}",
            "{count} mods that run only on players’ computers were removed: {names}"
        ),
        OutcomeSecrets = (
            "{count} file holding a key to the server, a password or a token was left out, so the server writes its defaults: {names}",
            "{count} files holding a key to the server, a password or a token were left out, so the server writes its defaults: {names}"
        ),
        OutcomeSettingsRemoved = (
            "{count} settings file couldn’t be checked for keys to the server and was left out, so the server writes its defaults: {names}",
            "{count} settings files couldn’t be checked for keys to the server and were left out, so the server writes its defaults: {names}"
        ),
        OutcomeScripts = (
            "{count} script was left out: only the owner can add scripts, while code execution is on.",
            "{count} scripts were left out: only the owner can add scripts, while code execution is on."
        ),
    }
}
