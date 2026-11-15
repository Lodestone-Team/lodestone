//! A server's backup as a `.lodeimg` file for another computer: its data image and the
//! content image it links to, in the shared crate's export container, with the server's
//! secrets taken out.
//!
//! The exported images are derived from the backup's, sealed anew, so they have ids of
//! their own:
//!
//! 1. **Sources come back.** A backup's data image lists every file as an `object` (it is
//!    a whole snapshot). Each data entry whose path and bytes its content image has takes
//!    that entry's source, hashes and sides, so Modrinth and CurseForge files travel as
//!    references (never jars) and the owner's "For players" marks and the jars' sides
//!    travel too. The core's `base` entries are the pack's overrides, which no importer
//!    fetches from the `.mrpack`, so they travel as objects; the images keep their `base`,
//!    so an importer still learns the pack.
//! 2. **Files that must not leave are taken out** of both images and named in their
//!    `excludes` ([`SERVER_PRIVATE`]; the plugins' folders unless the owner includes them):
//!    Lodestone's own files (its settings hold the Java command and arguments), the EULA
//!    (accepted per person), the commands the admin typed, the server software, every
//!    program or script at the top and the installers' logs (the importer's installers
//!    make them; an untrusted server jar never runs), the JVM's crash and dump files, keys
//!    to the server and services' logins ([`is_way_in`]: proxy forwarding secrets, Paper's
//!    backup of its old settings, Floodgate's folder and Geyser's copies of its key,
//!    database passwords, Discord bots' tokens, login mods' password hashes, vote
//!    listeners' keys, map plugins' databases and web pages for them), banned
//!    players' IP addresses, and what the Launcher's export takes out (the commands a player typed,
//!    account files) for a server made from a game folder. Logs, crash reports and caches
//!    are never in an image at all.
//! 3. **Secret values are blanked.** `server.properties` is written again from the
//!    properties it holds alone, every [`secret_property`] empty ([`blank_secrets`]: a line
//!    the reader can't parse, which Java may still read, is dropped), and the YAML files of
//!    [`YAML_SECRETS`] (Paper's Velocity forwarding `secret`, Pufferfish's Sentry key)
//!    with those values empty, the lines a value goes on over included; the cleaned copy
//!    is stored and the entry names the blanked keys in an extra `blanked` field, so an
//!    importer can say what was taken out. A copy that can't be cleaned (over 1 MiB,
//!    unreadable, a YAML file that isn't UTF-8 or has any line outside the strict subset
//!    of block style the editor reads the way a YAML reader does, [`read_simple_yaml`]) is
//!    taken out instead, its exact path added to the image's `excludes`: a file that
//!    leaves the computer never carries those secrets.
//!
//! 4. **Credentials found by their names are withheld** ([`super::secrets`]): every settings
//!    file the export would still carry is read, whatever mod or plugin wrote it, and one
//!    with a key whose name says password, secret, token or key (`botToken`, `secret-key`,
//!    `discord.bot.token`) holding a real value, or a value that is a login by its shape (a
//!    URL with a password, a Discord webhook or bot token), is taken out like the others and
//!    listed in [`Prepared::secrets_withheld`]; so are key and token files by their names
//!    (`.token`, `*.pem`, `id_rsa`), and a file too big to read. The named lists above stay:
//!    they also cover files with no credential key (a database, a key under another name).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use lodestone_image::container::{self, carried_objects};
use lodestone_image::manifest::App;
use lodestone_image::paths::{in_names, same_name};
use lodestone_image::share::{is_jvm_crash_name, is_player_private, strip, PLAYER_PRIVATE};
use lodestone_image::store::{Cancel, Store, NEVER};
use lodestone_image::{display_text, CanonicalPath, Entry, Image, Producer, Source};

use crate::backups::service::Backup;
use crate::error::{Error, ErrorKind};
use crate::images::scope::is_lodestone_name;
use crate::implementations::minecraft::properties::PropertiesDocument;

use super::secrets::{self, Format};

/// Files an export of a server never carries, as image exclude patterns (a name at any
/// depth, a path with `/` from the server folder, `*` within a name), besides a player's
/// (`PLAYER_PRIVATE`). [`is_private`] also matches them in any letter case, as the disks
/// of Windows and macOS do.
pub const SERVER_PRIVATE: &[&str] = &[
    // Lodestone's own files: its settings (the Java command and arguments, which never
    // come from another computer), the modpack records, the "For players" marks.
    ".lodestone*",
    // The EULA is accepted by whoever runs the server.
    "/eula.txt",
    // The commands the admin typed into Paper's console.
    "/.console_history",
    // Server software, which the importer's own installers make: never someone else's
    // server jar or libraries. The installers' logs too: Forge's and NeoForge's installer
    // writes them again on the other computer. And every program or script at the top
    // (start scripts, which hold environment tokens and which Spigot's restart runs):
    // the importer starts the server its own way.
    "/libraries/",
    "/versions/",
    "/*.jar",
    "/*.sh",
    "/*.bat",
    "/*.cmd",
    "/*.ps1",
    "/*.command",
    "/*.exe",
    "/*.msi",
    "/*.dll",
    "/*.so",
    "/*.dylib",
    "/user_jvm_args.txt",
    "/forge-installer.log",
    "/installer.log",
    "/*.jar.log",
    // The JVM's crash logs and dumps: a crash log holds the command line, a dump the
    // process's memory.
    "/hs_err_pid*",
    "/replay_pid*",
    "/java_pid*.hprof",
    "/*.mdmp",
    "/core",
    "/core.*",
    // Proxy forwarding secrets (Velocity's file, FabricProxy-Lite's, the old
    // FabricProxy's and Proxy Compatible Forge's settings: `proxy-compatible-forge.toml`
    // since 1.2.0, `.cfg` for Minecraft 1.12.2, `pcf-common.toml` before): with one, anyone could join
    // the original server as any player through its proxy. Floodgate's key (as a mod
    // and as a plugin) likewise lets whoever holds it join as any Bedrock player through
    // their own Geyser; its whole folder goes, as its settings name the key's file.
    "/forwarding.secret",
    "/config/FabricProxy-Lite.toml",
    "/config/FabricProxy.toml",
    "/config/pcf-common.toml",
    "/config/proxy-compatible-forge.toml",
    "/config/proxy-compatible-forge.cfg",
    // NeoForge's forwarding mods (NeoVelocity's `neovelocity-common.toml`, NeoForwarding's
    // `neoforwarding-server.toml`), and any other settings or folder at the top of
    // `config/` whose name says velocity, forwarding or proxy (any letter case), as new
    // forwarding mods keep appearing.
    "/config/neovelocity-common.toml",
    "/config/neoforwarding-server.toml",
    "/config/*velocity*",
    "/config/*forwarding*",
    "/config/*proxy*",
    // Paper's copy of the old `paper.yml` (`config/legacy-backup/paper.yml.old`, made when
    // it moved to 1.19's settings), which still holds the Velocity forwarding secret the
    // new settings took over. Paper never reads that folder again.
    "/config/legacy-backup/",
    "/config/floodgate/",
    "/plugins/floodgate/",
    // The settings and keys of plugins and mods that hold a password to a database, a
    // bot's token or a key that lets its holder in (see [`is_way_in`]): LuckPerms (as a
    // mod and as a plugin), DiscordSRV (its settings, the copies it keeps of them when it
    // updates them, `config.yml-build.<version>.old`, and its `.token`, which it reads
    // first), Discord Integration (as a mod and as a plugin) and EssentialsX Discord (a
    // Discord bot's token, which also lets Discord users run commands on the server),
    // CoreProtect, Plan, LiteBans, ServerTap (the key to its REST API, which runs console
    // commands), AuthMe (also its players' accounts), BungeeGuard's
    // tokens, Votifier's and NuVotifier's keys and tokens (votes give rewards), Geyser's
    // copies of Floodgate's key and its saved Microsoft logins, BlueMap's storages and
    // Dynmap's settings and web pages for a database (a password each), Simple Voice Chat
    // Discord Bridge's bots' tokens (as a mod and as a plugin), SDLink's encrypted bot
    // token and the key to it (`sdlinkstorage/`), EasyAuth's players' password hashes,
    // database passwords and proxy secret (its database folder and its settings), and
    // Ledger's settings (its database extension's password).
    "/config/luckperms/luckperms.conf",
    "/plugins/LuckPerms/config.yml",
    "/plugins/DiscordSRV/config.yml",
    "/plugins/DiscordSRV/config.yml*",
    "/plugins/DiscordSRV/.token",
    "/config/Discord-Integration.toml",
    "/plugins/DiscordIntegration/config.toml",
    "/plugins/EssentialsDiscord/config.yml",
    "/plugins/CoreProtect/config.yml",
    "/plugins/Plan/config.yml",
    "/config/plan/config.yml",
    "/plugins/LiteBans/config.yml",
    "/plugins/ServerTap/config.yml",
    "/plugins/AuthMe/",
    "/plugins/BungeeGuard/config.yml",
    "/plugins/Votifier/config.yml",
    "/plugins/Votifier/rsa/",
    "/plugins/NuVotifier/config.yml",
    "/plugins/NuVotifier/rsa/",
    "/config/nuvotifier/config.yml",
    "/config/nuvotifier/rsa/",
    "/config/Geyser-*/*.pem",
    "/plugins/Geyser-*/*.pem",
    "/config/Geyser-*/saved-refresh-tokens.json",
    "/plugins/Geyser-*/saved-refresh-tokens.json",
    "/config/bluemap/storages/",
    "/plugins/BlueMap/storages/",
    "sql.php",
    "/dynmap/configuration.txt",
    "/plugins/dynmap/configuration.txt",
    "/dynmap/web/standalone/",
    "/plugins/dynmap/web/standalone/",
    "MySQL_config.php",
    "PostgreSQL_config.php",
    "/config/voicechat-discord.yml",
    "/plugins/voicechat-discord/config.yml",
    "/config/simple-discord-link/",
    "/sdlinkstorage/",
    "/EasyAuth/",
    "/config/EasyAuth/",
    "/config/ledger.toml",
    // Discord-MC-Chat's bot token (2.x's settings and the backup it makes of them, 3.x's
    // folder), Mc2Discord's, and Tebex's store key (as a mod, `mods/tebex/`; as a plugin,
    // and the old BuycraftX's folder): the store's secret key lets its holder run the
    // store's commands on the server.
    "/config/discord-mc-chat.json",
    "/config/discord-mc-chat-backup.json",
    "/config/discord_mc_chat/",
    "/config/mc2discord.toml",
    "/mods/tebex/",
    "/plugins/Tebex/config.yml",
    "/plugins/BuycraftX/",
    // Banned players' IP addresses: personal data, and meaningless on another network.
    "/banned-ips.json",
    // And what the Launcher's export takes out (`lodestone_image::share::PLAYER_PRIVATE`,
    // added by [`prepare`]), for a server made from a game folder.
];

/// The plugins' folders, withheld unless the owner includes them: their settings,
/// databases and player data hold database passwords and bot tokens. Plugin jars travel.
pub const PLUGIN_DATA: &str = "plugins/*/";

/// The extensions of programs and scripts a computer runs: shell, batch and PowerShell
/// scripts, macOS's `.command`, Windows programs and installers, native libraries.
const RUNNABLE_EXTENSIONS: &[&str] = &[
    "sh", "bat", "cmd", "ps1", "command", "exe", "msi", "dll", "so", "dylib",
];

/// Whether the top-level name `name` is server software or what its installers leave (see
/// [`SERVER_PRIVATE`]), in any letter case: the jars, every program or script (the start
/// scripts, which may hold tokens and which Spigot's restart runs) and the installers'
/// logs. A new server's installer writes its own, and a server from a file never runs
/// its sender's programs.
pub fn is_server_software_name(name: &str) -> bool {
    let folded = name.to_uppercase().to_lowercase();
    let runnable = folded
        .rsplit_once('.')
        .is_some_and(|(_, extension)| RUNNABLE_EXTENSIONS.contains(&extension));
    in_names(
        name,
        &["user_jvm_args.txt", "forge-installer.log", "installer.log"],
    ) || runnable
        || folded.ends_with(".jar")
        || folded.ends_with(".jar.log")
}

/// Whether `path` is one of [`SERVER_PRIVATE`]'s files (or, without `include_plugin_data`,
/// in a plugin's folder), whatever its letter case.
pub fn is_private(path: &CanonicalPath, include_plugin_data: bool) -> bool {
    let parts: Vec<&str> = path.components().collect();
    if parts.iter().any(|part| is_lodestone_name(part)) {
        return true;
    }
    if is_player_private(path) || is_way_in(path) {
        return true;
    }
    match parts.as_slice() {
        [name] => {
            in_names(
                name,
                &[
                    "eula.txt",
                    ".console_history",
                    "forwarding.secret",
                    "banned-ips.json",
                ],
            ) || is_server_software_name(name)
                || is_jvm_crash_name(name)
        }
        [top, _, ..] if in_names(top, &["libraries", "versions"]) => true,
        [top, _, _, ..] if same_name(top, "plugins") => !include_plugin_data,
        _ => false,
    }
}

/// Plugins' and mods' folders (as `config/<folder>/` or `plugins/<folder>/`, any letter
/// case) whose `config.yml` holds a database's password, a Discord bot's token or tokens
/// that let their holder in: LuckPerms' (as a plugin; as a mod its settings are
/// `luckperms.conf`), DiscordSRV's and EssentialsX Discord's bots, CoreProtect's, Plan's
/// and LiteBans' databases, ServerTap's REST API key (which runs console commands),
/// BungeeGuard's tokens and Votifier's and NuVotifier's tokens.
const WAY_IN_SETTINGS: &[&str] = &[
    "luckperms",
    "discordsrv",
    "essentialsdiscord",
    "coreprotect",
    "plan",
    "litebans",
    "servertap",
    "bungeeguard",
    "votifier",
    "nuvotifier",
];

/// Whether `path` holds a key to the server or the login of a service it connects to,
/// whatever its letter case (see "M6a decisions", which lists them):
///
/// - proxy forwarding secrets: Velocity's `forwarding.secret`, the settings of
///   FabricProxy-Lite, FabricProxy and Proxy Compatible Forge (`proxy-compatible-forge.toml`
///   since 1.2.0, `proxy-compatible-forge.cfg` for Minecraft 1.12.2, `pcf-common.toml`
///   before), NeoVelocity (`neovelocity-common.toml`) and NeoForwarding
///   (`neoforwarding-server.toml`), every file or folder at the top of `config/` whose name
///   says velocity, forwarding or proxy, and Paper's `config/legacy-backup/` folder (the
///   old `paper.yml` it kept when moving to 1.19's settings, with the Velocity secret);
/// - Floodgate's whole folder (as a mod or a plugin: its key, and its settings, which say
///   which file the key is), and in Geyser's folders (`Geyser-Spigot`, `Geyser-Fabric`, …)
///   every `.pem` (a copy of Floodgate's key) and `saved-refresh-tokens.json` (the
///   Microsoft logins Geyser keeps for Bedrock players);
/// - the settings of [`WAY_IN_SETTINGS`]'s plugins and mods, LuckPerms' `luckperms.conf`
///   as a mod, DiscordSRV's `.token` (its bot token, read before its settings) and the
///   copies of its settings it keeps when it updates them (`config.yml-build.<version>.old`,
///   with the bot token: any name beginning `config.yml`), Discord
///   Integration's bot token (`config/Discord-Integration.toml` as a mod,
///   `plugins/DiscordIntegration/config.toml` as a plugin), AuthMe's whole folder (its database
///   settings, mail password and its players' password hashes), and Votifier's and
///   NuVotifier's `rsa/` keys;
/// - Discord bots and logins of mods: Simple Voice Chat Discord Bridge's bots' tokens
///   (`config/voicechat-discord.yml`, `plugins/voicechat-discord/config.yml`), SDLink's
///   settings (`config/simple-discord-link/`, its bot token encrypted) and the key to them
///   (`sdlinkstorage/`), EasyAuth's database folder (`EasyAuth/`, its players' password
///   hashes) and settings (`config/EasyAuth/`: database passwords, its proxy's shared
///   secret), and Ledger's `config/ledger.toml` (its database extension's password);
/// - Discord-MC-Chat's bot token (`config/discord-mc-chat.json`, the
///   `config/discord-mc-chat-backup.json` it makes when it upgrades them, and 3.x's
///   `config/discord_mc_chat/`), Mc2Discord's (`config/mc2discord.toml`) and Tebex's store
///   key (`mods/tebex/` as a mod, `plugins/Tebex/config.yml` as a plugin, the old
///   `plugins/BuycraftX/`);
/// - the database settings of the map plugins and mods: BlueMap's whole `storages/`
///   folder (any storage's file, `sql.conf` or another name) and its web page for SQL
///   (`sql.php`, anywhere: BlueMap's web root can be moved), Dynmap's
///   `configuration.txt` and its `web/standalone/` folder (rewritten at every start, with
///   the database's password in `MySQL_config.php` or `PostgreSQL_config.php`, which go
///   wherever they are).
///
/// An export never carries them (not even with the plugins' settings), and a server made
/// from a file never takes them: the sender would keep a way in, or the owner's passwords
/// would leave.
pub fn is_way_in(path: &CanonicalPath) -> bool {
    let parts: Vec<&str> = path.components().collect();
    let Some(last) = parts.last() else {
        return false;
    };
    if in_names(
        last,
        &["sql.php", "mysql_config.php", "postgresql_config.php"],
    ) {
        return true;
    }
    let geyser = |dir: &str| {
        let folded = dir.to_uppercase().to_lowercase();
        folded.starts_with("geyser-")
    };
    let forwarding = |name: &str| {
        let folded = name.to_uppercase().to_lowercase();
        ["velocity", "forwarding", "proxy"]
            .iter()
            .any(|word| folded.contains(word))
    };
    match parts.as_slice() {
        [name] => same_name(name, "forwarding.secret"),
        // any file or folder at the top of `config/` that names forwarding, as new proxy
        // forwarding mods keep appearing
        [top, name, ..] if same_name(top, "config") && forwarding(name) => true,
        [top, name] if same_name(top, "config") => in_names(
            name,
            &[
                "fabricproxy-lite.toml",
                "fabricproxy.toml",
                "pcf-common.toml",
                "proxy-compatible-forge.toml",
                "proxy-compatible-forge.cfg",
                "neovelocity-common.toml",
                "neoforwarding-server.toml",
                "discord-integration.toml",
                "voicechat-discord.yml",
                "ledger.toml",
                "discord-mc-chat.json",
                "discord-mc-chat-backup.json",
                "mc2discord.toml",
            ],
        ),
        // Paper's copy of the old `paper.yml`, with the Velocity forwarding secret
        [top, dir, _, ..] if same_name(top, "config") && same_name(dir, "legacy-backup") => true,
        [top, dir, _, ..]
            if same_name(top, "config")
                && in_names(dir, &["simple-discord-link", "easyauth", "discord_mc_chat"]) =>
        {
            true
        }
        [dir, _, ..] if in_names(dir, &["sdlinkstorage", "easyauth"]) => true,
        // Tebex's store key, as a mod and as the old BuycraftX plugin
        [top, dir, _, ..] if same_name(top, "mods") && same_name(dir, "tebex") => true,
        [top, dir, _, ..] if same_name(top, "plugins") && same_name(dir, "buycraftx") => true,
        [top, dir, name] if same_name(top, "plugins") && same_name(dir, "tebex") => {
            same_name(name, "config.yml")
        }
        [top, dir, name] if same_name(top, "plugins") && same_name(dir, "voicechat-discord") => {
            same_name(name, "config.yml")
        }
        // the whole folder: its settings name the key's file (`key-file-name`), so a key
        // under any other name would be read as well as `key.pem`
        [top, dir, _, ..]
            if in_names(top, &["config", "plugins"]) && same_name(dir, "floodgate") =>
        {
            true
        }
        [top, dir, _, ..] if same_name(top, "plugins") && same_name(dir, "authme") => true,
        [top, dir, name] if in_names(top, &["config", "plugins"]) && geyser(dir) => {
            let folded = name.to_uppercase().to_lowercase();
            folded.ends_with(".pem") || same_name(name, "saved-refresh-tokens.json")
        }
        // DiscordSRV reads its bot token from `.token` before its settings' `BotToken`, and
        // moves its settings to `config.yml-build.<version>.old` when it updates them
        [top, dir, name] if same_name(top, "plugins") && same_name(dir, "discordsrv") => {
            same_name(name, ".token")
                || name.to_uppercase().to_lowercase().starts_with("config.yml")
        }
        // Discord Integration as a plugin: its bot token and the Discord roles that run
        // commands on the server
        [top, dir, name] if same_name(top, "plugins") && same_name(dir, "discordintegration") => {
            same_name(name, "config.toml")
        }
        [top, dir, name] if same_name(top, "config") && same_name(dir, "luckperms") => {
            same_name(name, "luckperms.conf")
        }
        [top, dir, name]
            if in_names(top, &["config", "plugins"]) && in_names(dir, WAY_IN_SETTINGS) =>
        {
            same_name(name, "config.yml")
        }
        [top, dir, rsa, _, ..]
            if in_names(top, &["config", "plugins"])
                && in_names(dir, &["votifier", "nuvotifier"])
                && same_name(rsa, "rsa") =>
        {
            true
        }
        [top, dir, storages, _, ..]
            if in_names(top, &["config", "plugins"])
                && same_name(dir, "bluemap")
                && same_name(storages, "storages") =>
        {
            true
        }
        [dir, name] if same_name(dir, "dynmap") => same_name(name, "configuration.txt"),
        [top, dir, name] if same_name(top, "plugins") && same_name(dir, "dynmap") => {
            same_name(name, "configuration.txt")
        }
        [dir, web, standalone, _, ..]
            if same_name(dir, "dynmap")
                && same_name(web, "web")
                && same_name(standalone, "standalone") =>
        {
            true
        }
        [top, dir, web, standalone, _, ..]
            if same_name(top, "plugins")
                && same_name(dir, "dynmap")
                && same_name(web, "web")
                && same_name(standalone, "standalone") =>
        {
            true
        }
        _ => false,
    }
}

/// `bytes` with every line break Java's `Properties` reads (a lone carriage return too)
/// written as `\n`, so a setting can't hide inside another's line from a reader that
/// splits only at `\n`.
pub fn java_line_breaks(bytes: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    let lone = |i: usize| bytes[i] == b'\r' && bytes.get(i + 1) != Some(&b'\n');
    if !(0..bytes.len()).any(lone) {
        return std::borrow::Cow::Borrowed(bytes);
    }
    std::borrow::Cow::Owned(
        (0..bytes.len())
            .map(|i| if lone(i) { b'\n' } else { bytes[i] })
            .collect(),
    )
}

/// Whether the `server.properties` key `key` holds a secret: `rcon.password`,
/// `management-server-secret`, any key whose name says password, secret or token, and
/// `text-filtering-config` (its JSON holds the filtering service's API key).
pub fn secret_property(key: &str) -> bool {
    let key = key.to_lowercase();
    key == "text-filtering-config"
        || ["password", "secret", "token"]
            .iter()
            .any(|word| key.contains(word))
}

/// `server.properties` (`bytes`) written again with every [`secret_property`] empty, and
/// the keys that had a value. The copy is written from the properties alone
/// ([`rebuilt_properties`]), last value winning as Java reads them, so comments, repeated
/// keys (an old password a later line overrides, one commented out) and lines the reader
/// can't parse (`rcon.password=x\uD800`, which Java still reads) don't travel; Minecraft
/// rewrites the file at its next start anyway. `None` when the copy would be the same
/// bytes.
pub fn blank_secrets(bytes: &[u8]) -> Option<(Vec<u8>, Vec<String>)> {
    let (document, _) = PropertiesDocument::from_bytes(&java_line_breaks(bytes));
    let blanked = document
        .entries()
        .into_iter()
        .filter(|(key, value)| secret_property(key) && !value.is_empty())
        .map(|(key, _)| key)
        .collect();
    let out = rebuilt_properties(&document).to_bytes();
    (out != bytes).then_some((out, blanked))
}

/// A `.properties` document written again from `document`'s properties alone, in their
/// order, the last value of a repeated key winning as Java reads them, with every
/// [`secret_property`] empty: no comment, no repeated key and no line the reader couldn't
/// parse (kept by [`PropertiesDocument`] as it was, and read by Java, which takes unpaired
/// surrogates, `online-mode=false\uD800` among them). Every key and value is written in
/// the escaped ASCII form Java and the reader read alike, so what the server reads is what
/// the reader read.
pub fn rebuilt_properties(document: &PropertiesDocument) -> PropertiesDocument {
    let mut clean = PropertiesDocument::default();
    for (key, value) in document.entries() {
        let value = if secret_property(&key) {
            ""
        } else {
            value.as_str()
        };
        clean.set(&key, value);
    }
    clean
}

/// A line's text without its line ending, and the ending.
fn split_ending(line: &str) -> (&str, &str) {
    match line.strip_suffix("\r\n") {
        Some(body) => (body, "\r\n"),
        None => match line.strip_suffix('\n') {
            Some(body) => (body, "\n"),
            None => (line, ""),
        },
    }
}

/// A YAML file (`bytes`) with the values of the keys named `keys` (any letter case, at any
/// depth, quoted or not) made empty (`''`), and the keys blanked. A value that goes on
/// over the following lines (a block scalar `|` or `>`, a wrapped plain or single-quoted
/// scalar, the keys or items under an empty key) is dropped with them. For Paper's
/// `config/paper-global.yml`, whose `proxies.velocity.secret` is the forwarding secret,
/// and the like. `None` when nothing had a value, the file isn't UTF-8, or it isn't in the
/// subset of YAML the editor reads ([`read_simple_yaml`]; the caller then withholds it,
/// see [`clean_yaml`]).
pub fn blank_yaml_values(bytes: &[u8], keys: &[&str]) -> Option<(Vec<u8>, Vec<String>)> {
    match set_yaml_values(std::str::from_utf8(bytes).ok()?, keys, "''") {
        YamlEdit::Changed(out, keys) => Some((out, keys)),
        YamlEdit::Unchanged | YamlEdit::Unreadable => None,
    }
}

/// What [`set_yaml_values`] made of a file.
enum YamlEdit {
    /// Every key already had the value.
    Unchanged,
    /// A copy with these keys set.
    Changed(Vec<u8>, Vec<String>),
    /// It isn't in the subset of YAML the editor reads for certain ([`read_simple_yaml`]).
    Unreadable,
}

/// A YAML file read by [`read_simple_yaml`]: its lines and every key in it, where it is.
struct SimpleYaml<'a> {
    /// Whether the text began with a byte order mark (which `lines` leave out).
    bom: bool,
    /// The lines, with their endings.
    lines: Vec<&'a str>,
    /// Every key, in the order of the lines. With the subset's rules, these are all the
    /// keys a YAML reader finds in the file, at the lines where it finds them.
    keys: Vec<KeyLine<'a>>,
}

/// A key's line, as [`read_simple_yaml`] found it.
struct KeyLine<'a> {
    /// The line's index.
    line: usize,
    /// Its indentation, in spaces.
    indent: usize,
    /// The key as written, its quotes included, without the spaces before its colon.
    raw_key: &'a str,
    /// The key's text.
    key: &'a str,
    /// Its value, as written on its line.
    value: Found,
    /// The index of the first line after the key's own: after its value's lines (a wrapped
    /// scalar, a block scalar) or the keys and items under it.
    end: usize,
}

/// A key's value, as far as the editor needs it.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Found {
    /// Nothing on its line (a comment at most): null, or the keys or items under it.
    Empty,
    /// One line's plain or quoted scalar, without escapes: this text.
    Text(String),
    /// Anything else: a flow collection, a block scalar, a scalar over several lines or
    /// with escapes.
    Other,
}

/// Whether `c` may be anywhere in a YAML file the editor reads: printable ASCII, the
/// tab, line feed and carriage return (which [`read_simple_yaml`] checks where they are),
/// and any other letter SnakeYAML takes as text, but none it reads as a line break
/// (U+0085, U+2028, U+2029), the C1 controls and a byte order mark past the start.
fn simple_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | ' '..='~')
        || (c >= '\u{A0}'
            && !matches!(
                c,
                '\u{2028}' | '\u{2029}' | '\u{FEFF}' | '\u{FFFE}' | '\u{FFFF}'
            ))
}

/// Whether `key` is a key the editor compares for certain: ASCII letters, digits, `_`, `-`
/// and spaces, beginning with a letter, a digit or `_` and not ending in a space, or
/// several such parts joined by colons (a namespaced id: `minecraft:elytra`, in Paper
/// 1.21.4's item obfuscation settings). Never a `.`: Bukkit's settings (which Spigot,
/// Bukkit, the old `paper.yml` and Pufferfish read) split every key at its dots, so a
/// top-level `settings.restart-script:` is Spigot's restart script to them, out of the
/// sight of an editor that matches one key at a time. The files Bukkit saves nest their
/// keys instead, so a real one has none.
fn simple_key(key: &str) -> bool {
    key.split(':').all(|part| {
        let bytes = part.as_bytes();
        bytes
            .first()
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_')
            && bytes.last() != Some(&b' ')
            && bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b' '))
    })
}

/// The characters a plain scalar may not begin with (YAML's indicators), and that a line
/// going on with one may not begin with either.
const INDICATORS: &[char] = &[
    '-', '?', ':', ',', '[', ']', '{', '}', '#', '&', '*', '!', '|', '>', '\'', '"', '%', '@', '`',
];

/// Whether `rest`, what follows a scalar on its line, is nothing or a comment after a
/// space.
fn trailing_ok(rest: &str) -> bool {
    rest.is_empty()
        || (rest.starts_with(' ') && {
            let rest = rest.trim_start_matches(' ');
            rest.is_empty() || rest.starts_with('#')
        })
}

/// A plain scalar on one line of block style (`value`, from its first letter): its text
/// and whether a comment follows it. `None` when a YAML reader could read it as anything
/// but that text: an indicator first (`- `, `&`, `!`, a quote, `[`), a `: ` in it (a
/// mapping where none may be) or `:` at its end, or a tab.
fn plain_line(value: &str) -> Option<(&str, bool)> {
    let (text, commented) = match value.find(" #") {
        Some(at) => (&value[..at], true),
        None => (value, false),
    };
    let text = text.trim_end_matches(' ');
    let first = text.chars().next()?;
    let dash_only = first == '-' && (text.len() == 1 || text[1..].starts_with(' '));
    let indicator = INDICATORS.contains(&first) && !(first == '-' && !dash_only);
    if indicator || text.contains('\t') || text.contains(": ") || text.ends_with(':') {
        return None;
    }
    Some((text, commented))
}

/// Whether `rest`, a line more indented than the key above it (indentation taken off),
/// goes on with that key's plain scalar the same way for a YAML reader: text only, with
/// no indicator first, no `: `, `:` at the end, comment or tab.
fn continues_plain(rest: &str) -> bool {
    rest.chars()
        .next()
        .is_some_and(|c| !INDICATORS.contains(&c))
        && !rest.contains('\t')
        && !rest.contains(": ")
        && !rest.ends_with(':')
        && !rest.contains(" #")
}

/// Where a single-quoted scalar, `text` from its opening quote on, ends: the byte after
/// its closing quote (a `'` not doubled). `None` when it doesn't close in `text`.
fn single_quoted_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        if bytes[i] == b'\'' {
            if bytes.get(i + 1) == Some(&b'\'') {
                i += 2;
                continue;
            }
            return Some(i + 1);
        }
        i += 1;
    }
    None
}

/// Where a double-quoted scalar, `text` from its opening quote on, ends: the byte after
/// its closing quote (a `"` not escaped by a backslash). `None` when it doesn't close in
/// `text`.
fn double_quoted_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return Some(i + 1),
            _ => i += 1,
        }
    }
    None
}

/// Where a one-line flow collection, `text` from its `[` or `{` on, ends: the byte after
/// it. Only `{}` and lists of scalars (`[]`, `[a, 'b', "c"]`, `[minecraft:elytra]`) are
/// read: an item is a plain scalar with no flow indicator, quote, comment, `: `, `:` at its
/// end or tab, a single-quoted scalar or a double-quoted one without escapes. `None` for
/// anything else (a mapping, a nested collection, one that doesn't close on its line).
fn flow_end(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let skip = |mut i: usize| {
        while bytes.get(i) == Some(&b' ') {
            i += 1;
        }
        i
    };
    if bytes.first() == Some(&b'{') {
        let i = skip(1);
        return (bytes.get(i) == Some(&b'}')).then_some(i + 1);
    }
    let mut i = skip(1);
    if bytes.get(i) == Some(&b']') {
        return Some(i + 1);
    }
    loop {
        i = skip(i);
        match bytes.get(i)? {
            b'\'' => i += single_quoted_end(&text[i..])?,
            b'"' => {
                let len = double_quoted_end(&text[i..])?;
                if text[i..i + len].contains(['\\', '\t']) {
                    return None;
                }
                i += len;
            }
            _ => {
                let len = text[i..].find([',', ']']).unwrap_or(text.len() - i);
                let item = text[i..i + len].trim_end_matches(' ');
                let first = item.chars().next()?;
                let flow_safe = !item.contains(['[', ']', '{', '}', '#', '\'', '"', '\t'])
                    && !item.contains(": ")
                    && !item.ends_with(':');
                let dash_text = first == '-' && item.len() > 1 && !item[1..].starts_with(' ');
                if !flow_safe || (INDICATORS.contains(&first) && !dash_text) {
                    return None;
                }
                i += len;
            }
        }
        i = skip(i);
        match bytes.get(i)? {
            b',' => i += 1,
            b']' => return Some(i + 1),
            _ => return None,
        }
    }
}

/// Whether `value`, a list item's (after its `- `), is one scalar on its line: plain,
/// quoted or a one-line flow collection ([`flow_end`]), with at most a comment after it.
/// An item holding a mapping (`- key: value`), a nested list, a block scalar, an anchor,
/// alias or tag, or nothing (`-` alone, whose value would be the lines under it) isn't.
fn simple_item(value: &str) -> bool {
    let end = match value.chars().next() {
        None => return false,
        Some('\'') => single_quoted_end(value),
        Some('"') => double_quoted_end(value),
        Some('[' | '{') => flow_end(value),
        Some(_) => return plain_line(value).is_some(),
    };
    end.is_some_and(|end| !value[..end].contains('\t') && trailing_ok(&value[end..]))
}

/// A key at the start of `rest` (a line without its indentation): the key as written,
/// its text and what follows its colon. A plain key ends at its first `:` followed by a
/// space or the line's end ([`key_colon`]), a quoted one at its closing quote, which a
/// `:` must follow. `None` when the line has no key the editor compares for certain
/// ([`simple_key`]; a quoted key with an escape, `''` or `\`, isn't).
fn split_key(rest: &str) -> Option<(&str, &str, &str)> {
    let (raw, key, after) = match rest.chars().next()? {
        quote @ ('\'' | '"') => {
            let close = rest[1..].find(quote)? + 1;
            let after = rest[close + 1..].strip_prefix(':')?;
            (&rest[..=close], &rest[1..close], after)
        }
        _ => {
            let colon = key_colon(rest)?;
            let raw = rest[..colon].trim_end_matches(' ');
            (raw, raw, &rest[colon + 1..])
        }
    };
    (simple_key(key) && (after.is_empty() || after.starts_with(' '))).then_some((raw, key, after))
}

/// Reads a YAML file as the editor does, line by line, when every line is in a strict
/// subset of block style that a YAML reader such as SnakeYAML (which Paper, Spigot and
/// Bukkit use) reads the same way; `None` when any line isn't. The subset:
///
/// - one byte order mark at the start at most, no other U+FEFF, no control character but
///   the tab (in comments only), no line break but `\n` and `\r\n` ([`simple_char`]);
/// - indentation by spaces, a deeper line only under a key with an empty value (the keys
///   or items under it) or going on with a value (below), a list's items at the key's
///   indentation or deeper, and every less indented line back at an indentation a line
///   above opened;
/// - one `---` before everything else at most, no other document marker, no directive;
/// - every other line blank, a comment, a key or a list item. A key is plain or simply
///   quoted, from [`simple_key`]'s letters, followed by `: ` and its value or by `:` alone:
///   so no `?` key, merge key, tag, anchor or alias. A value is a plain scalar on its line
///   ([`plain_line`]), wrapped onto following lines more indented than its key that only
///   go on with it ([`continues_plain`]: Paper writes long messages so); a single-quoted
///   scalar, which may go on over following non-blank lines more indented than its key to
///   its closing quote; a double-quoted one on its line; a one-line flow collection
///   ([`flow_end`]); or a block scalar (`|`, `>`, with `-` or `+`), every non-blank line of
///   which is more indented than its key and at least as indented as its first. A list
///   item is one scalar on its line ([`simple_item`]).
///
/// So every key a YAML reader finds is a line's key here, at the line it begins on, and
/// each key's value is the lines the editor drops with it ([`KeyLine::end`]): the editor's
/// idea of which lines belong to which key comes from the same reading of indentation.
fn read_simple_yaml(text: &str) -> Option<SimpleYaml<'_>> {
    struct Frame {
        indent: usize,
        /// A list's items rather than keys.
        items: bool,
        /// A list at its key's own indentation (`key:` / `- item`).
        compact: bool,
        /// The key whose value it is, by its index in `keys`.
        owner: Option<usize>,
    }
    let (bom, text) = match text.strip_prefix('\u{FEFF}') {
        Some(rest) => (true, rest),
        None => (false, text),
    };
    if !text.chars().all(simple_char) {
        return None;
    }
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let bodies: Vec<&str> = lines.iter().map(|line| split_ending(line).0).collect();
    if bodies.iter().any(|body| body.contains('\r')) {
        return None;
    }
    let indentation = |body: &str| body.len() - body.trim_start_matches(' ').len();
    let mut frames = vec![Frame {
        indent: 0,
        items: false,
        compact: false,
        owner: None,
    }];
    let mut keys: Vec<KeyLine> = Vec::new();
    // the indentation and index of the key just read when its value is empty
    let mut open: Option<(usize, usize)> = None;
    let mut last_content = 0;
    let mut started = false;
    let mut i = 0;
    while i < bodies.len() {
        let body = bodies[i];
        let indent = indentation(body);
        let rest = &body[indent..];
        if rest.is_empty() || rest.starts_with('#') {
            i += 1;
            continue;
        }
        if indent == 0 && (rest.starts_with("---") || rest.starts_with("...")) {
            if started || !rest.starts_with("---") || !trailing_ok(&rest[3..]) {
                return None;
            }
            started = true;
            i += 1;
            continue;
        }
        started = true;
        let item = rest == "-" || rest.starts_with("- ");
        let opens = open
            .take()
            .filter(|(parent, _)| indent > *parent || (indent == *parent && item));
        if let Some((parent, owner)) = opens {
            frames.push(Frame {
                indent,
                items: item,
                compact: indent == parent,
                owner: Some(owner),
            });
        } else {
            while let Some(top) = frames.last() {
                let ends = top.indent > indent || (top.compact && top.indent == indent && !item);
                if !ends || top.owner.is_none() {
                    break;
                }
                if let Some(owner) = frames.pop().and_then(|frame| frame.owner) {
                    keys[owner].end = last_content + 1;
                }
            }
            let top = frames.last()?;
            if top.indent != indent || top.items != item {
                return None;
            }
        }
        if item {
            if !simple_item(rest[1..].trim_start_matches(' ')) {
                return None;
            }
            last_content = i;
            i += 1;
            continue;
        }
        let (raw_key, key, after) = split_key(rest)?;
        let value = after.trim_start_matches(' ');
        let (found, end) = match value.chars().next() {
            None | Some('#') => (Found::Empty, i + 1),
            Some('\'') => {
                // to its closing quote, over lines more indented than the key
                let mut joined = value.to_string();
                let mut end = i + 1;
                loop {
                    if let Some(close) = single_quoted_end(&joined) {
                        if joined[..close].contains('\t') || !trailing_ok(&joined[close..]) {
                            return None;
                        }
                        let found = if end == i + 1 {
                            Found::Text(joined[1..close - 1].replace("''", "'"))
                        } else {
                            Found::Other
                        };
                        break (found, end);
                    }
                    let below = bodies.get(end)?;
                    if below.trim_start_matches(' ').is_empty() || indentation(below) <= indent {
                        return None;
                    }
                    joined.push('\n');
                    joined.push_str(below);
                    end += 1;
                }
            }
            Some('"') => {
                let close = double_quoted_end(value)?;
                let inner = &value[1..close - 1];
                if inner.contains('\t') || !trailing_ok(&value[close..]) {
                    return None;
                }
                let found = if inner.contains('\\') {
                    Found::Other
                } else {
                    Found::Text(inner.to_string())
                };
                (found, i + 1)
            }
            Some('[' | '{') => {
                let close = flow_end(value)?;
                if !trailing_ok(&value[close..]) {
                    return None;
                }
                (Found::Other, i + 1)
            }
            Some('|' | '>') => {
                let header = value.split(' ').next().unwrap_or_default();
                if !matches!(header, "|" | "|-" | "|+" | ">" | ">-" | ">+")
                    || !trailing_ok(&value[header.len()..])
                {
                    return None;
                }
                // every following line more indented than the key, blank ones included,
                // at least as indented as the first
                let (mut end, mut look, mut content) = (i + 1, i + 1, None);
                while let Some(below) = bodies.get(look) {
                    if below.trim_start_matches(' ').is_empty() {
                        look += 1;
                        continue;
                    }
                    let deeper = indentation(below);
                    if deeper <= indent {
                        break;
                    }
                    if deeper < *content.get_or_insert(deeper) || below.contains('\t') {
                        return None;
                    }
                    look += 1;
                    end = look;
                }
                (Found::Other, end)
            }
            Some(_) => {
                let (text, commented) = plain_line(value)?;
                let mut end = i + 1;
                // wrapped onto lines more indented than the key that only go on with it
                while let Some(below) = bodies.get(end).filter(|_| !commented) {
                    let deeper = indentation(below);
                    if deeper <= indent || !continues_plain(&below[deeper..]) {
                        break;
                    }
                    end += 1;
                }
                let found = if end == i + 1 {
                    Found::Text(text.to_string())
                } else {
                    Found::Other
                };
                (found, end)
            }
        };
        if found == Found::Empty {
            open = Some((indent, keys.len()));
        }
        keys.push(KeyLine {
            line: i,
            indent,
            raw_key,
            key,
            value: found,
            end,
        });
        last_content = end - 1;
        i = end;
    }
    while let Some(frame) = frames.pop() {
        if let Some(owner) = frame.owner {
            keys[owner].end = last_content + 1;
        }
    }
    Some(SimpleYaml { bom, lines, keys })
}

/// `text` with the values of the keys named `keys` (any letter case, at any depth) set to
/// `value` (YAML as written: `''`, `./start.sh`), each key's own lines dropped with its
/// old value ([`KeyLine::end`]), and the keys changed. A key whose value is already
/// `value` (quoted or not; for `''`, nothing or a comment too) is left as it is.
/// [`YamlEdit::Unreadable`] when the file isn't in the subset [`read_simple_yaml`] reads,
/// whether it names one of the keys or not.
fn set_yaml_values(text: &str, keys: &[&str], value: &str) -> YamlEdit {
    let Some(yaml) = read_simple_yaml(text) else {
        return YamlEdit::Unreadable;
    };
    let wanted = match value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')) {
        Some(inner) => inner,
        None => value,
    };
    let mut changed = Vec::new();
    let mut out = String::with_capacity(text.len());
    if yaml.bom {
        out.push('\u{FEFF}');
    }
    let mut next = 0;
    for key in &yaml.keys {
        // a key in a value already dropped goes with it
        if key.line < next || !keys.iter().any(|k| key.key.eq_ignore_ascii_case(k)) {
            continue;
        }
        let already = match &key.value {
            Found::Empty => wanted.is_empty(),
            Found::Text(text) => text == wanted,
            Found::Other => false,
        };
        if already && key.end == key.line + 1 {
            continue;
        }
        out.extend(yaml.lines[next..key.line].iter().copied());
        let (_, ending) = split_ending(yaml.lines[key.line]);
        out.push_str(&format!(
            "{}{}: {value}{ending}",
            " ".repeat(key.indent),
            key.raw_key
        ));
        changed.push(key.key.to_string());
        next = key.end;
    }
    if changed.is_empty() {
        return YamlEdit::Unchanged;
    }
    out.extend(yaml.lines[next..].iter().copied());
    YamlEdit::Changed(out.into_bytes(), changed)
}

/// What [`clean_yaml`] made of a YAML file with secrets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum YamlCleaning {
    /// No secret had a value: it can go as it is.
    Unchanged,
    /// A copy with these keys made empty.
    Blanked(Vec<u8>, Vec<String>),
    /// It can't be cleaned, so it mustn't go at all.
    Withhold,
}

/// Where the key of a YAML line (`text`, from the key on) ends: at its first `:` followed
/// by a space, a tab or the line's end, as a YAML reader takes it. A `:` followed by
/// anything else belongs to the key (`minecraft:elytra:`) or to a plain value. `None`
/// when the line has no key.
fn key_colon(text: &str) -> Option<usize> {
    text.char_indices()
        .find(|(i, c)| {
            *c == ':'
                && text[i + 1..]
                    .chars()
                    .next()
                    .is_none_or(|next| next == ' ' || next == '\t')
        })
        .map(|(i, _)| i)
}

/// The cleaned form of a YAML file of `size` bytes (`bytes`, read from it) whose values
/// of `keys` are secrets: withheld when it is over 1 MiB, not UTF-8 or not in the strict
/// subset of YAML the editor reads the way a YAML reader does ([`read_simple_yaml`], an
/// allowlist: anything outside it is withheld, whether it names a secret or not), so the
/// cleaning never passes a secret it didn't see.
pub fn clean_yaml(size: u64, bytes: &[u8], keys: &[&str]) -> YamlCleaning {
    pin_yaml(size, bytes, keys, "''")
}

/// A YAML file of `size` bytes (`bytes`) with the values of `keys` set to `value` (YAML as
/// written), checked as [`clean_yaml`] checks it: withheld when that can't be done for
/// sure.
pub fn pin_yaml(size: u64, bytes: &[u8], keys: &[&str], value: &str) -> YamlCleaning {
    if size > MAX_CLEANED_FILE || bytes.len() as u64 > MAX_CLEANED_FILE {
        return YamlCleaning::Withhold;
    }
    let Ok(text) = std::str::from_utf8(bytes) else {
        return YamlCleaning::Withhold;
    };
    match set_yaml_values(text, keys, value) {
        YamlEdit::Unchanged => YamlCleaning::Unchanged,
        YamlEdit::Changed(out, keys) => YamlCleaning::Blanked(out, keys),
        YamlEdit::Unreadable => YamlCleaning::Withhold,
    }
}

/// [`pin_yaml`] for each of `pins` (a key and its value) in turn: withheld when any is.
pub fn pin_yaml_all(size: u64, bytes: &[u8], pins: &[(&str, &str)]) -> YamlCleaning {
    let mut current = bytes.to_vec();
    let mut changed = Vec::new();
    for (key, value) in pins {
        match pin_yaml(size, &current, &[key], value) {
            YamlCleaning::Unchanged => {}
            YamlCleaning::Blanked(out, keys) => {
                current = out;
                changed.extend(keys);
            }
            YamlCleaning::Withhold => return YamlCleaning::Withhold,
        }
    }
    if changed.is_empty() {
        YamlCleaning::Unchanged
    } else {
        YamlCleaning::Blanked(current, changed)
    }
}

/// The most blanked keys one entry names: within an entry's budget for unknown fields.
const MAX_BLANKED_PER_ENTRY: usize = 8;

/// The entry field that names the keys an export blanked in a file.
pub const BLANKED_FIELD: &str = "blanked";

/// Images ready to export, and what they leave out.
#[derive(Debug, Clone)]
pub struct Prepared {
    pub image: Image,
    /// Its content image, when the backup still has it.
    pub linked: Vec<Image>,
    /// Files taken out, by path.
    pub left_out: Vec<String>,
    /// Secret values made empty, as `<path>: <key>`.
    pub blanked: Vec<String>,
    /// Files taken out because they hold a password or token ([`super::secrets`]: a
    /// credential found by its key's name or its shape, a key file by its name, or a file
    /// too big to read), by path; also in `left_out`.
    pub secrets_withheld: Vec<String>,
}

impl Prepared {
    /// The plain bytes of the objects the export carries: what writing it reads, and about
    /// what it takes on disk.
    pub fn carried_bytes(&self) -> Result<u64, Error> {
        let images: Vec<&Image> = std::iter::once(&self.image).chain(&self.linked).collect();
        let carried = carried_objects(&images).map_err(internal)?;
        Ok(carried
            .values()
            .fold(0, |sum, size| sum.saturating_add(*size)))
    }
}

/// What an export wrote.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupExport {
    pub file: PathBuf,
    /// Its size.
    pub bytes: u64,
    /// Files it names as Modrinth or CurseForge downloads.
    pub references: usize,
    /// CurseForge files whose authors don't allow downloads elsewhere, by path.
    pub not_downloadable: Vec<String>,
    pub left_out: Vec<String>,
    pub blanked: Vec<String>,
    /// [`Prepared::secrets_withheld`].
    pub secrets_withheld: Vec<String>,
}

fn internal(message: impl std::fmt::Display) -> Error {
    Error {
        kind: ErrorKind::Internal,
        source: color_eyre::eyre::eyre!("{message}"),
    }
}

/// The images to export for `backup` of the server `server_name` (see the module). Puts
/// the cleaned copies of files into `store` (unnamed objects: call it while collections
/// are held off, and export before letting go). Blocking.
pub fn prepare(
    store: &Store,
    backup: &Backup,
    server_name: &str,
    include_plugin_data: bool,
) -> Result<Prepared, Error> {
    let mut image = backup.image.clone();
    let mut content = backup.content.clone();
    if let Some(content) = &content {
        resource_from_content(&mut image, content);
    }
    let mut patterns: Vec<&str> = SERVER_PRIVATE.to_vec();
    patterns.extend(PLAYER_PRIVATE);
    if !include_plugin_data {
        patterns.push(PLUGIN_DATA);
    }
    let private = |path: &CanonicalPath| is_private(path, include_plugin_data);
    let mut left_out = Vec::new();
    let mut blanked = Vec::new();
    let mut cleaned: HashMap<(String, String), Outcome> = HashMap::new();
    let mut scanned: HashMap<(String, Format), bool> = HashMap::new();
    let mut secrets_withheld = Vec::new();
    for target in std::iter::once(&mut image).chain(content.iter_mut()) {
        for entry in &mut target.entries {
            if matches!(entry.source, Source::Base {}) {
                entry.source = Source::Object {};
            }
        }
        left_out.extend(strip(target, &patterns, &private));
        let mut withhold = Vec::new();
        for entry in &mut target.entries {
            match clean_entry(store, entry, &mut cleaned)? {
                Cleaning::NotSecret => {
                    if holds_credential(store, entry, &mut scanned) {
                        secrets_withheld.push(entry.path.to_string());
                        withhold.push(format!("/{}", entry.path));
                    }
                }
                Cleaning::Blanked(keys) => {
                    blanked.extend(keys.iter().map(|key| format!("{}: {key}", entry.path)));
                }
                Cleaning::Withhold => withhold.push(format!("/{}", entry.path)),
            }
        }
        if !withhold.is_empty() {
            // the exact paths, so the image names what it doesn't carry
            let exact: Vec<&str> = withhold.iter().map(String::as_str).collect();
            left_out.extend(strip(target, &exact, &|_| false));
        }
    }
    if let Some(content) = &mut content {
        content.label = Some(label(server_name, backup));
        content.parent = None;
        content.seal();
        content
            .validate()
            .map_err(|e| internal(format!("The backup's content image can't be exported: {e}")))?;
    }
    image.content = content.as_ref().map(|c| c.id.clone());
    image.label = Some(label(server_name, backup));
    image.parent = None;
    image.producer = Producer {
        app: App::Core,
        version: env!("CARGO_PKG_VERSION").to_string(),
    };
    image.seal();
    image.validate().map_err(|e| Error {
        kind: ErrorKind::Conflict,
        source: color_eyre::eyre::eyre!(
            "This backup can't be exported: {e}. Its backup excludes may leave no room for what an export takes out"
        ),
    })?;
    left_out.sort();
    left_out.dedup();
    blanked.sort();
    blanked.dedup();
    secrets_withheld.sort();
    secrets_withheld.dedup();
    Ok(Prepared {
        image,
        linked: content.into_iter().collect(),
        left_out,
        blanked,
        secrets_withheld,
    })
}

/// The exported images' label: "<server> – <backup name or time>".
fn label(server_name: &str, backup: &Backup) -> String {
    let when = backup.record.name.clone().unwrap_or_else(|| {
        chrono::DateTime::from_timestamp(backup.record.created, 0)
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| backup.record.id.clone())
    });
    let name = server_name.trim();
    let text = if name.is_empty() {
        when
    } else {
        format!("{name} – {when}")
    };
    text.chars()
        .take(lodestone_image::manifest::MAX_TEXT_LEN)
        .collect()
}

/// Gives each `object` entry of `data` whose path and bytes `content` has the content
/// entry's source, hashes and sides.
fn resource_from_content(data: &mut Image, content: &Image) {
    let by_path: HashMap<&str, &Entry> = content
        .entries
        .iter()
        .map(|e| (e.path.as_str(), e))
        .collect();
    for entry in &mut data.entries {
        let Some(known) = by_path.get(entry.path.as_str()) else {
            continue;
        };
        if known.sha256 != entry.sha256 || known.size != entry.size {
            continue;
        }
        entry.source = known.source.clone();
        entry.sha1 = known.sha1.clone();
        entry.sha512 = known.sha512.clone();
        entry.env = known.env;
        entry.env_from = known.env_from.clone();
    }
}

/// A cleaned copy of a file, in the store.
#[derive(Debug, Clone)]
struct Cleaned {
    sha256: String,
    size: u64,
    keys: Vec<String>,
}

/// What cleaning one secret-holding file's bytes came to, remembered per object.
#[derive(Debug, Clone)]
enum Outcome {
    /// Nothing to take out: it travels as it is.
    Unchanged,
    Copy(Cleaned),
    /// It can't be cleaned (too big, unreadable, not text a YAML file can be), so it
    /// doesn't travel at all.
    Withhold,
}

/// What [`clean_entry`] did to an entry.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cleaning {
    /// Not a file that holds secrets the export knows, or one with none in it.
    NotSecret,
    /// Its bytes are now a cleaned copy; these keys were made empty (possibly none, for a
    /// copy that only dropped comments).
    Blanked(Vec<String>),
    /// It must be taken out.
    Withhold,
}

/// The largest file read to clean it: a real `server.properties` or Paper config is a
/// few KiB. One bigger is withheld rather than sent uncleaned.
const MAX_CLEANED_FILE: u64 = 1 << 20;

/// A file an export cleans: `server.properties`, or a YAML file with secret keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Secrets {
    Properties,
    Yaml(&'static [&'static str]),
}

/// The YAML files whose values an export blanks, by their path from the server folder (any
/// letter case), and the keys: Paper's Velocity forwarding secret (in
/// `config/paper-global.yml` since 1.19, in `paper.yml` before), and Pufferfish's Sentry
/// key.
const YAML_SECRETS: &[(&[&str], &[&str])] = &[
    (&["config", "paper-global.yml"], &["secret"]),
    (&["paper.yml"], &["secret"]),
    (&["pufferfish.yml"], &["sentry-dsn"]),
];

/// The settings a server made from a file sets in its YAML files when they are there,
/// whatever the file said, by their path from the server folder (any letter case): each
/// key and its value. Spigot's restart runs `sh <restart-script>`, so the script is
/// Spigot's default (`./start.sh`, which no file places: top-level scripts are server
/// software), not any file the sender placed; Bukkit's custom permissions are read from
/// `permissions.yml`, which is taken only with the operators, not from any other file the
/// sender placed; Bukkit's worlds stay in the server's own folder (`world-container`,
/// which could put them anywhere the core can write, out of its backups and its users'
/// permissions); and Bukkit's old command aliases (`aliases:`, a section in `bukkit.yml`
/// before `commands.yml`) are `now-in-commands.yml`, as Bukkit writes them once it has
/// moved them: with a section there and no `commands.yml`, Paper, Spigot, Purpur and Folia
/// move the section into a new `commands.yml` at the first start, and its aliases replace
/// any command, `stop` and `save-all` included, which the core types into the console, so
/// they would run as the console (`op <sender>`) without the operators being kept.
const PINNED_YAML: &[(&[&str], &[(&str, &str)])] = &[
    (&["spigot.yml"], &[("restart-script", "./start.sh")]),
    (
        &["bukkit.yml"],
        &[
            ("permissions-file", "permissions.yml"),
            ("world-container", "."),
            ("aliases", "now-in-commands.yml"),
        ],
    ),
];

/// The keys a server made from a file sets in the YAML file at `path`, and their values,
/// when it is one of [`PINNED_YAML`].
pub fn pinned_yaml_of(path: &CanonicalPath) -> Option<&'static [(&'static str, &'static str)]> {
    let parts: Vec<&str> = path.components().collect();
    PINNED_YAML.iter().find_map(|(file, pins)| {
        (parts.len() == file.len() && parts.iter().zip(*file).all(|(a, b)| same_name(a, b)))
            .then_some(*pins)
    })
}

/// The secret keys of the YAML file at `path` when it is one an export cleans
/// ([`YAML_SECRETS`]): a server made from a file empties them too.
pub fn yaml_secrets_of(path: &CanonicalPath) -> Option<&'static [&'static str]> {
    match secrets_of(path)? {
        Secrets::Yaml(keys) => Some(keys),
        Secrets::Properties => None,
    }
}

/// The secrets the file at `path` holds that an export cleans, when it is one of those
/// files.
fn secrets_of(path: &CanonicalPath) -> Option<Secrets> {
    let parts: Vec<&str> = path.components().collect();
    if matches!(parts.as_slice(), [name] if same_name(name, "server.properties")) {
        return Some(Secrets::Properties);
    }
    YAML_SECRETS.iter().find_map(|(file, keys)| {
        (parts.len() == file.len() && parts.iter().zip(*file).all(|(a, b)| same_name(a, b)))
            .then_some(Secrets::Yaml(keys))
    })
}

/// Whether the file at `path` is one an export cleans key by key (`server.properties` and
/// [`YAML_SECRETS`]' files) rather than one [`super::secrets`] reads for credentials.
pub fn is_cleaned(path: &CanonicalPath) -> bool {
    secrets_of(path).is_some()
}

/// The cleaned form of the object `entry` names, which holds `secrets`.
fn clean_object(store: &Store, entry: &Entry, secrets: Secrets) -> Result<Outcome, Error> {
    let shown = display_text(entry.path.as_str());
    if entry.size > MAX_CLEANED_FILE {
        tracing::warn!("{shown} is too big to clean, so the export leaves it out");
        return Ok(Outcome::Withhold);
    }
    let bytes = match store.read_object_bytes(&entry.sha256, entry.size) {
        Ok(bytes) => bytes,
        Err(e) => {
            tracing::warn!(
                "The backup's copy of {shown} can't be read, so the export leaves it out: {e}"
            );
            return Ok(Outcome::Withhold);
        }
    };
    let blanked = match secrets {
        Secrets::Properties => blank_secrets(&bytes),
        Secrets::Yaml(keys) => match clean_yaml(entry.size, &bytes, keys) {
            YamlCleaning::Withhold => {
                tracing::warn!("{shown} can't be cleaned, so the export leaves it out");
                return Ok(Outcome::Withhold);
            }
            YamlCleaning::Unchanged => None,
            YamlCleaning::Blanked(out, keys) => Some((out, keys)),
        },
    };
    let Some((out, keys)) = blanked else {
        return Ok(Outcome::Unchanged);
    };
    let stored = store
        .put_reader(&mut &out[..], out.len() as u64, None, NEVER)
        .map_err(|e| internal(format!("Failed to store a cleaned copy: {e}")))?;
    Ok(Outcome::Copy(Cleaned {
        sha256: stored.sha256,
        size: stored.size,
        keys,
    }))
}

/// Replaces `entry`'s bytes with a cleaned copy when it is a file with secrets the export
/// knows ([`secrets_of`]) and has one, or says to withhold it when it can't be cleaned.
/// `cleaned` remembers each object's outcome, so the same file in both images is cleaned
/// once.
fn clean_entry(
    store: &Store,
    entry: &mut Entry,
    cleaned: &mut HashMap<(String, String), Outcome>,
) -> Result<Cleaning, Error> {
    let Some(secrets) = secrets_of(&entry.path) else {
        return Ok(Cleaning::NotSecret);
    };
    // per object and kind of file: the same bytes may be two files cleaned differently
    let key = (entry.sha256.clone(), format!("{secrets:?}"));
    let outcome = match cleaned.get(&key) {
        Some(outcome) => outcome.clone(),
        None => {
            let outcome = clean_object(store, entry, secrets)?;
            cleaned.insert(key, outcome.clone());
            outcome
        }
    };
    let copy = match outcome {
        Outcome::Unchanged => return Ok(Cleaning::NotSecret),
        Outcome::Withhold => return Ok(Cleaning::Withhold),
        Outcome::Copy(copy) => copy,
    };
    entry.sha256 = copy.sha256;
    entry.size = copy.size;
    entry.sha1 = None;
    entry.sha512 = None;
    entry.modified = None;
    entry.source = Source::Object {};
    entry.extra.remove(BLANKED_FIELD);
    // Within an entry's budget for unknown fields (16 values, 1 KiB): at most 8 keys of
    // at most 64 bytes each; the export's report lists them all.
    let named: Vec<serde_json::Value> = copy
        .keys
        .iter()
        .filter(|key| key.len() <= 64)
        .take(MAX_BLANKED_PER_ENTRY)
        .map(|key| serde_json::Value::String(key.clone()))
        .collect();
    if !named.is_empty() {
        entry
            .extra
            .insert(BLANKED_FIELD.to_string(), serde_json::Value::Array(named));
    }
    Ok(Cleaning::Blanked(copy.keys))
}

/// Whether `entry`, a file the export would carry as it is, holds a password or token, so it
/// is withheld ([`super::secrets`]): a key or token file by its name, a file too big to
/// read, or a settings file with a credential in it. A file that can't be read from the
/// store is withheld too. Only files whose bytes travel are read: a Modrinth or CurseForge
/// file travels as a reference, its public bytes downloaded by the importer. `scanned`
/// remembers each object's verdict, so the same file in both images is read once.
fn holds_credential(
    store: &Store,
    entry: &Entry,
    scanned: &mut HashMap<(String, Format), bool>,
) -> bool {
    if !matches!(entry.source, Source::Object {}) {
        return false;
    }
    let shown = display_text(entry.path.as_str());
    if secrets::withheld_by_name(&entry.path) {
        tracing::warn!("{shown} is a key or token file, so the export leaves it out");
        return true;
    }
    if secrets::too_large(&entry.path, entry.size) {
        tracing::warn!("{shown} is too big to check for passwords, so the export leaves it out");
        return true;
    }
    let Some(format) = secrets::scanned_format(&entry.path) else {
        return false;
    };
    let key = (entry.sha256.clone(), format);
    if let Some(found) = scanned.get(&key) {
        return *found;
    }
    let found = match store.read_object_bytes(&entry.sha256, entry.size) {
        Ok(bytes) => match secrets::find_credential(&String::from_utf8_lossy(&bytes), format) {
            Some(credential) => {
                // the key's name only: never its value
                tracing::warn!(
                    "{shown} holds a password or token ({}), so the export leaves it out",
                    display_text(&credential)
                );
                true
            }
            None => false,
        },
        Err(e) => {
            tracing::warn!(
                "The backup's copy of {shown} can't be read, so the export leaves it out: {e}"
            );
            true
        }
    };
    scanned.insert(key, found);
    found
}

/// Writes `prepared` as an export at `to` (`<to>.part`, renamed when whole), telling
/// `progress` the bytes read so far and in all. Blocking; call it while collections are
/// still held off.
pub fn write(
    store: &Store,
    prepared: &Prepared,
    to: &Path,
    cancel: Cancel,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<BackupExport, Error> {
    container::export_with_progress(
        store,
        &prepared.image,
        &prepared.linked,
        to,
        cancel,
        progress,
    )
    .map_err(|e| match e {
        container::ContainerError::Cancelled => crate::backups::store::cancelled(),
        other => internal(format!("The export couldn't be written: {other}")),
    })?;
    let all = || {
        prepared
            .image
            .entries
            .iter()
            .chain(prepared.linked.iter().flat_map(|c| c.entries.iter()))
    };
    let references = lodestone_image::share::placed_entries(&prepared.image, &prepared.linked)
        .iter()
        .filter(|e| !matches!(e.source, Source::Object {}))
        .count();
    let mut not_downloadable: Vec<String> = all()
        .filter(|e| {
            matches!(
                e.source,
                Source::Curseforge {
                    downloadable: false,
                    ..
                }
            )
        })
        .map(|e| e.path.to_string())
        .collect();
    not_downloadable.sort();
    not_downloadable.dedup();
    Ok(BackupExport {
        file: to.to_path_buf(),
        bytes: std::fs::metadata(to).map_or(0, |m| m.len()),
        references,
        not_downloadable,
        left_out: prepared.left_out.clone(),
        blanked: prepared.blanked.clone(),
        secrets_withheld: prepared.secrets_withheld.clone(),
    })
}
