//! The command line: every command, its flags and its examples.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};

const ABOUT: &str = "Controls a running Lodestone Launcher: instances, games, mods, worlds, accounts, settings, and Lodestone cores and their servers. Everything the launcher's window does, from scripts and AI agents.";

const AFTER_HELP: &str = "\
Output:
  JSON when stdout isn't a terminal (or with --json): one JSON document on stdout.
  Progress of long commands goes to stderr, one JSON object per line. Errors go to
  stderr as {\"error\":{\"code\":\"…\",\"message\":\"…\"}}. --human prints text instead.

Exit codes:
  0 success                  5 confirmation required (pass --yes)
  1 failed                   6 needs the user's decision (trust, EULA, mods changed)
  2 wrong command line       7 busy (the game or a job runs)
  3 launcher not running     8 timed out waiting (the job goes on)
  4 not found / ambiguous    9 not available, not signed in, refused
                            10 cancelled

Examples:
  lodestone-launcher-cli status
  lodestone-launcher-cli instances create --name Survival --version 1.21.1 --loader fabric
  lodestone-launcher-cli content install Survival sodium
  lodestone-launcher-cli play launch Survival
  lodestone-launcher-cli schema          # every command and flag, as JSON";

#[derive(Parser, Debug)]
#[command(
    name = "lodestone-launcher-cli",
    version,
    about = ABOUT,
    after_help = AFTER_HELP
)]
pub struct Cli {
    #[command(flatten)]
    pub global: Global,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Args, Debug, Clone, Default)]
pub struct Global {
    /// Print JSON (the default when stdout isn't a terminal).
    #[arg(long, global = true, conflicts_with = "human")]
    pub json: bool,
    /// Print text for people (the default on a terminal).
    #[arg(long, global = true)]
    pub human: bool,
    /// The launcher's control socket. Default: LODESTONE_LAUNCHER_SOCKET, or
    /// control.sock in the data directory (LODESTONE_LAUNCHER_DATA_DIR, as for the
    /// launcher).
    #[arg(long, global = true, value_name = "PATH")]
    pub socket: Option<PathBuf>,
    /// Confirm destructive commands (deleting, removing, replacing). Ask the user first.
    #[arg(short = 'y', long, global = true)]
    pub yes: bool,
    /// Wait for long commands to finish, with progress (the default).
    #[arg(long, global = true, conflicts_with = "no_wait")]
    pub wait: bool,
    /// Start long commands and return their job at once; follow with `jobs wait`.
    #[arg(long, global = true)]
    pub no_wait: bool,
    /// Stop waiting after this many seconds (exit code 8; the job goes on).
    #[arg(long, global = true, value_name = "SECS")]
    pub timeout: Option<u64>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// The whole launcher at a glance: instances, games, jobs, accounts, cores.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli status\n  lodestone-launcher-cli --json status"
    )]
    Status,
    /// Checks that the launcher answers; prints its version and protocol.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli ping")]
    Ping,
    /// Every command with its arguments, flags and method, as JSON (no launcher needed).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli schema\n  lodestone-launcher-cli schema | jq '.commands[].name'"
    )]
    Schema,
    /// The launcher's API methods and error codes.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli methods")]
    Methods,
    /// Instances: list, make, rename, group, copy, delete, export, verify.
    #[command(subcommand)]
    Instances(Instances),
    /// The game: launch, stop, status, logs.
    #[command(subcommand)]
    Play(Play),
    /// Game settings of an instance, the launcher defaults, General and Developer.
    #[command(subcommand)]
    Settings(Settings),
    /// Shared game settings (options.txt values shared by instances).
    #[command(subcommand)]
    Shared(Shared),
    /// Mods, resource packs and shader packs: search Modrinth, install, update, remove.
    #[command(subcommand)]
    Content(Content),
    /// Modpacks: search, install as an instance, check for and apply updates.
    #[command(subcommand)]
    Modpacks(Modpacks),
    /// Worlds and their backups.
    #[command(subcommand)]
    Worlds(Worlds),
    /// Minecraft accounts. Never prints tokens.
    #[command(subcommand)]
    Accounts(Accounts),
    /// Import instances from other launchers.
    #[command(subcommand)]
    Import(Import),
    /// Lodestone cores: add, remove, trust, sign out.
    #[command(subcommand)]
    Cores(Cores),
    /// Servers on Lodestone cores: list, start, stop, join, make matching instances, host.
    #[command(subcommand)]
    Servers(Servers),
    /// Questions the launcher's window is asking about servers, and answers to them.
    #[command(subcommand)]
    Prompts(Prompts),
    /// Friends' servers (Lodestone Connect): join one from an invite link, list, remove.
    #[command(subcommand)]
    Friends(Friends),
    /// Long operations: list, wait for, cancel.
    #[command(subcommand)]
    Jobs(Jobs),
    /// Disk space: what the data folder takes, and removing what can go.
    #[command(subcommand)]
    Storage(Storage),
    /// Snapshots of instances: take, list, go back to, share as .lodesnap files, and
    /// make new instances from them.
    #[command(subcommand)]
    Snapshots(Snapshots),
    /// The launcher's look: list skins, use one, import or export a pack.
    #[command(subcommand)]
    Skins(Skins),
    /// Streams events as JSON lines: log lines, game states, progress, jobs, notices.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli events --topics state,notices\n  lodestone-launcher-cli events --instance Survival --topics log --count 50"
    )]
    Events(EventsArgs),
    /// Debug traces: what the user does in the window and what the launcher does, as
    /// events, without screenshots. Local only.
    #[command(subcommand)]
    Trace(Trace),
    /// The launcher's memory, CPU, threads and open files, how its window draws, a
    /// running game's memory and CPU, and what it holds. `perf` alone is `perf show`.
    Perf(PerfArgs),
}

#[derive(Args, Debug)]
#[command(
    after_help = "Examples:\n  lodestone-launcher-cli perf\n  lodestone-launcher-cli perf watch --count 10"
)]
pub struct PerfArgs {
    #[command(subcommand)]
    pub command: Option<Perf>,
}

#[derive(Subcommand, Debug)]
pub enum Perf {
    /// One reading, taken over a second (CPU is time used over that second).
    #[command(after_help = "Examples:\n  lodestone-launcher-cli perf show")]
    Show,
    /// A reading every second, as JSON lines.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli perf watch\n  lodestone-launcher-cli perf watch --count 30"
    )]
    Watch {
        /// Stop after this many readings.
        #[arg(long)]
        count: Option<usize>,
    },
}

/// An instance: its id, or its name (any case).
#[derive(Args, Debug, Clone)]
pub struct InstanceArg {
    /// The instance's id or name.
    #[arg(value_name = "INSTANCE")]
    pub instance: String,
}

#[derive(Subcommand, Debug)]
pub enum Instances {
    /// Lists the instances, with their effective sidebar group and state.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli instances list\n  lodestone-launcher-cli --json instances list | jq '.instances[] | {name, effective_group}'"
    )]
    List,
    /// Shows an instance: its settings, modpack, linked server and run.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli instances show Survival")]
    Show(InstanceArg),
    /// Selects the instance in the launcher's window.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli instances select Survival")]
    Select(InstanceArg),
    /// Minecraft versions to make an instance with.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli instances versions --limit 10\n  lodestone-launcher-cli instances versions --snapshots"
    )]
    Versions {
        /// Include snapshots.
        #[arg(long)]
        snapshots: bool,
        /// Include old alphas and betas.
        #[arg(long)]
        old: bool,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// A mod loader's versions for a Minecraft version.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli instances loader-versions fabric 1.21.1"
    )]
    LoaderVersions {
        /// fabric, quilt, forge or neoforge.
        loader: String,
        game_version: String,
    },
    /// Makes an instance, as the New instance dialog does.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli instances create --version 1.21.1 --loader fabric --name Survival\n  lodestone-launcher-cli instances create            # the newest release, vanilla\n  lodestone-launcher-cli instances create --loader forge --loader-version 47.4.0 --version 1.20.1 --group Modded"
    )]
    Create {
        /// The name (default: \"<version> <loader>\").
        #[arg(long)]
        name: Option<String>,
        /// A Minecraft version, or \"latest\" (the newest release, the default).
        #[arg(long)]
        version: Option<String>,
        /// vanilla, fabric, quilt, forge or neoforge.
        #[arg(long)]
        loader: Option<String>,
        /// The loader version (default: the recommended one).
        #[arg(long)]
        loader_version: Option<String>,
        /// A sidebar group.
        #[arg(long)]
        group: Option<String>,
    },
    /// Renames an instance.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli instances rename Survival \"Survival 2\""
    )]
    Rename { instance: String, name: String },
    /// Puts an instance in a sidebar group, or takes it out (--none).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli instances group Survival Friends\n  lodestone-launcher-cli instances group Survival --none"
    )]
    Group {
        instance: String,
        group: Option<String>,
        /// No group of its own (an automatic group, when those are on).
        #[arg(long, conflicts_with = "group")]
        none: bool,
    },
    /// Whether an instance's mods, packs and shaders may update to beta and alpha
    /// versions. Off, each only updates to a version at least as stable as the one
    /// installed.
    #[command(
        name = "prerelease-updates",
        after_help = "Examples:\n  lodestone-launcher-cli instances prerelease-updates Survival on\n  lodestone-launcher-cli instances prerelease-updates Survival off"
    )]
    PrereleaseUpdates {
        instance: String,
        #[arg(value_parser = ["on", "off"])]
        state: String,
    },
    /// Sets an instance's icon: an image, a library icon, an emoji, or the default.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli instances icon Survival --image ./icon.png\n  lodestone-launcher-cli instances icon Survival --library grass\n  lodestone-launcher-cli instances icon Survival --emoji 🗡 --tile 3A6EA5\n  lodestone-launcher-cli instances icon Survival --clear"
    )]
    Icon {
        instance: String,
        #[arg(long, value_name = "FILE")]
        image: Option<PathBuf>,
        #[arg(long, value_name = "NAME")]
        library: Option<String>,
        #[arg(long)]
        emoji: Option<String>,
        /// The emoji tile's colour, RRGGBB.
        #[arg(long, requires = "emoji")]
        tile: Option<String>,
        #[arg(long)]
        clear: bool,
    },
    /// Sets an instance's Play background: an image, the default, or a slideshow of
    /// screenshots or of its own images (each shown ~15 s, crossfading).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli instances background Survival --image ./castle.jpg\n  lodestone-launcher-cli instances background Survival --clear\n  lodestone-launcher-cli instances background Survival --slideshow all\n  lodestone-launcher-cli instances background Survival --slideshow custom --add ./a.png --add ./b.jpg"
    )]
    Background {
        instance: String,
        #[arg(long, value_name = "FILE", conflicts_with_all = ["clear", "slideshow", "add"])]
        image: Option<PathBuf>,
        #[arg(long, conflicts_with_all = ["slideshow", "add"])]
        clear: bool,
        /// instance (its screenshots), all (every instance's), custom (its own images)
        /// or off (back to the image).
        #[arg(long, value_parser = ["instance", "all", "custom", "off"])]
        slideshow: Option<String>,
        /// An image to add to the slideshow's own set (repeat for several).
        #[arg(long, value_name = "FILE")]
        add: Vec<PathBuf>,
    },
    /// Copies an instance (with its worlds and mods).
    #[command(after_help = "Examples:\n  lodestone-launcher-cli instances duplicate Survival")]
    Duplicate(InstanceArg),
    /// Moves an instance to the Trash. Destructive: needs --yes.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli instances delete Survival --yes")]
    Delete(InstanceArg),
    /// Writes an instance as a Modrinth modpack (.mrpack).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli instances export Survival ./survival.mrpack"
    )]
    Export {
        instance: String,
        /// Where to write it; ends in .mrpack.
        path: PathBuf,
    },
    /// Checks every game file and downloads missing or broken ones.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli instances verify Survival")]
    Verify(InstanceArg),
    /// Prints a share code: the instance's Modrinth mods, resource packs and shaders,
    /// for another Lodestone Launcher to paste. For an instance from a Modrinth modpack,
    /// the code names the pack and only your changes to it. Lists what won't arrive as
    /// it is (CurseForge files, files added by hand, changed pack files).
    #[command(
        name = "share-code",
        after_help = "Examples:\n  lodestone-launcher-cli instances share-code Survival\n  lodestone-launcher-cli instances share-code Survival --json | jq -r .code"
    )]
    ShareCode(InstanceArg),
    /// Makes an instance from a share code and installs its content from Modrinth (for
    /// a code based on a modpack: the pack, then the changes). Waits for the downloads.
    #[command(
        name = "import-code",
        after_help = "Examples:\n  lodestone-launcher-cli instances import-code 'LDS1:…'\n  lodestone-launcher-cli instances import-code \"$CODE\" --name \"Friend's pack\""
    )]
    ImportCode {
        /// The code, starting with LDS1: or LDS2:.
        code: String,
        /// The new instance's name; the code's own name without it.
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum Play {
    /// Starts the game (signing in, installing what is missing). Waits until it runs.
    ///
    /// An instance linked to a server joins it, as the Play button does; --no-join
    /// plays without joining. Starting Minecraft accepts Mojang's EULA: ask the user.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli play launch Survival\n  lodestone-launcher-cli play launch Survival --join mc.example.com:25565\n  lodestone-launcher-cli play launch Smp --update-mods\n  lodestone-launcher-cli play launch Smp --no-join"
    )]
    Launch {
        instance: String,
        /// Join this server (host or host:port) once the game starts.
        #[arg(long, value_name = "ADDRESS", conflicts_with = "no_join")]
        join: Option<String>,
        /// Don't join the server the instance is linked to.
        #[arg(long)]
        no_join: bool,
        /// The server's mods changed: bring the instance in step first.
        #[arg(long, conflicts_with = "skip_update")]
        update_mods: bool,
        /// The server's mods changed: play as it is.
        #[arg(long)]
        skip_update: bool,
        /// Install mods from a core that isn't trusted for mods, this once. Ask the user.
        #[arg(long)]
        accept_advisory: bool,
    },
    /// Stops the game, or cancels its preparation.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli play stop Survival")]
    Stop(InstanceArg),
    /// What each instance's game is doing.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli play status\n  lodestone-launcher-cli play status Survival"
    )]
    Status { instance: Option<String> },
    /// An instance's console: the last lines, and with --follow the new ones.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli play logs Survival --lines 50\n  lodestone-launcher-cli play logs Survival --follow --until \"Sound engine started\" --timeout 300\n  lodestone-launcher-cli play logs Survival --follow --until-exit"
    )]
    Logs {
        instance: String,
        #[arg(long, default_value_t = 100)]
        lines: usize,
        /// Keep printing new lines.
        #[arg(short, long)]
        follow: bool,
        /// With --follow: stop at the first new line containing TEXT.
        #[arg(long, value_name = "TEXT", requires = "follow")]
        until: Option<String>,
        /// With --follow: stop when the game exits.
        #[arg(long, requires = "follow")]
        until_exit: bool,
    },
    /// Clears an instance's console. Needs --yes.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli play clear-logs Survival --yes")]
    ClearLogs(InstanceArg),
}

/// Game settings, as the Settings tab takes them.
#[derive(Args, Debug, Clone, Default)]
pub struct GameSettingsArgs {
    /// Minimum memory: 2048 or 2G.
    #[arg(long, value_name = "MB")]
    pub min_memory: Option<String>,
    /// Maximum memory: 4096 or 4G.
    #[arg(long, value_name = "MB")]
    pub max_memory: Option<String>,
    /// JVM arguments, as one string.
    #[arg(long, allow_hyphen_values = true)]
    pub jvm_args: Option<String>,
    /// Game arguments, as one string.
    #[arg(long, allow_hyphen_values = true)]
    pub game_args: Option<String>,
    /// Window size, WIDTHxHEIGHT, or \"default\".
    #[arg(long)]
    pub resolution: Option<String>,
    /// Start fullscreen: true or false.
    #[arg(long)]
    pub fullscreen: Option<bool>,
    /// Environment variables: \"NAME=value NAME2='a b'\".
    #[arg(long)]
    pub env: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Settings {
    /// An instance's game settings.
    #[command(subcommand)]
    Instance(InstanceSettings),
    /// The launcher's defaults for instances.
    #[command(subcommand)]
    Defaults(DefaultSettings),
    /// General settings: automatic sidebar groups.
    #[command(subcommand)]
    General(GeneralSettings),
    /// Developer settings.
    #[command(subcommand)]
    Developer(DeveloperSettings),
}

#[derive(Subcommand, Debug)]
pub enum InstanceSettings {
    /// The instance's own settings, the effective ones, and which follow the defaults.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli settings instance get Survival")]
    Get(InstanceArg),
    /// Overrides settings for one instance.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli settings instance set Survival --max-memory 6G\n  lodestone-launcher-cli settings instance set Survival --resolution 1920x1080 --fullscreen false\n  lodestone-launcher-cli settings instance set Survival --java /Library/Java/JavaVirtualMachines/zulu-21.jdk/Contents/Home/bin/java"
    )]
    Set {
        instance: String,
        #[command(flatten)]
        settings: GameSettingsArgs,
        /// The java executable to use instead of Mojang's ("" for Mojang's).
        #[arg(long)]
        java: Option<String>,
    },
    /// Makes settings follow the launcher defaults again (all without --key).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli settings instance reset Survival --key memory\n  lodestone-launcher-cli settings instance reset Survival"
    )]
    Reset {
        instance: String,
        /// memory, java, jvm_args, game_args, window or env (repeat for several).
        #[arg(long = "key", value_name = "KEY")]
        keys: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DefaultSettings {
    #[command(after_help = "Examples:\n  lodestone-launcher-cli settings defaults get")]
    /// The launcher's defaults.
    Get,
    /// Changes the defaults that instances without overrides follow.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli settings defaults set --max-memory 4G --jvm-args \"-XX:+UseG1GC\""
    )]
    Set {
        #[command(flatten)]
        settings: GameSettingsArgs,
    },
    /// Resets defaults to the built-in values (all without --key).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli settings defaults reset --key memory"
    )]
    Reset {
        /// memory, jvm_args, game_args, window or env.
        #[arg(long = "key", value_name = "KEY")]
        keys: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum GeneralSettings {
    /// General settings.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli settings general get")]
    Get,
    /// Changes general settings.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli settings general set --auto-groups false\n  lodestone-launcher-cli settings general set --auto-configure false"
    )]
    Set {
        /// Group instances without a group of their own by loader (and Imported).
        #[arg(long)]
        auto_groups: Option<bool>,
        /// Set up new instances automatically: memory, Java arguments and game options
        /// for this computer and the instance's mods, before its first game.
        #[arg(long)]
        auto_configure: Option<bool>,
    },
}

#[derive(Subcommand, Debug)]
pub enum DeveloperSettings {
    /// Developer settings, and the accounts signed in with the launcher identity.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli settings developer get")]
    Get,
    /// "Sign in as the Minecraft Launcher" and "Connect service (relay)". Turning the
    /// first on needs --yes after the user read the disclosure (printed without --yes).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli settings developer set --sign-in-as-minecraft-launcher true --yes\n  lodestone-launcher-cli settings developer set --sign-in-as-minecraft-launcher false --sign-out-accounts --yes\n  lodestone-launcher-cli settings developer set --connect-service https://connect-dev.lodestone.cc\n  lodestone-launcher-cli settings developer set --connect-service default"
    )]
    #[command(group(
        clap::ArgGroup::new("change")
            .required(true)
            .multiple(true)
            .args(["sign_in_as_minecraft_launcher", "connect_service"])
    ))]
    Set {
        #[arg(long, action = clap::ArgAction::Set, value_name = "BOOL")]
        sign_in_as_minecraft_launcher: Option<bool>,
        /// Turning it off: also sign out the accounts signed in with it.
        #[arg(long)]
        sign_out_accounts: bool,
        /// The Lodestone Connect service the friend list uses, for testing:
        /// https://name (or http://127.0.0.1:PORT for one on this computer), or
        /// "default" for connect.lodestone.cc.
        #[arg(long, value_name = "URL")]
        connect_service: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum Shared {
    /// Shared values, categories, and the instance they sync from.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli shared get")]
    Get,
    /// Sets one shared value (as typed in the window).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli shared set guiScale 3\n  lodestone-launcher-cli shared set lang en_gb"
    )]
    Set { key: String, value: String },
    /// Shares a category, or stops sharing it.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli shared category video --off\n  lodestone-launcher-cli shared category controls --on"
    )]
    Category {
        /// controls, video, sound, accessibility or language.
        category: String,
        #[arg(long, conflicts_with = "off", required_unless_present = "off")]
        on: bool,
        #[arg(long)]
        off: bool,
    },
    /// The instance whose in-game changes update the shared values (--none for none).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli shared sync-from Survival\n  lodestone-launcher-cli shared sync-from --none"
    )]
    SyncFrom {
        instance: Option<String>,
        #[arg(
            long,
            conflicts_with = "instance",
            required_unless_present = "instance"
        )]
        none: bool,
    },
    /// Copies an instance's current game settings into the shared values, once.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli shared copy-from Survival")]
    CopyFrom(InstanceArg),
    /// Forgets every shared value (instances keep their own). Needs --yes.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli shared clear --yes")]
    Clear,
    /// One instance's sharing.
    #[command(subcommand)]
    Instance(SharedInstance),
}

#[derive(Subcommand, Debug)]
pub enum SharedInstance {
    /// Whether the instance shares, its overrides and values.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli shared instance get Survival")]
    Get(InstanceArg),
    /// Turns sharing on or off for the instance, or overrides keys.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli shared instance set Survival --enabled false\n  lodestone-launcher-cli shared instance set Survival --override guiScale --value guiScale=2\n  lodestone-launcher-cli shared instance set Survival --share guiScale"
    )]
    Set {
        instance: String,
        #[arg(long)]
        enabled: Option<bool>,
        /// Keep this instance's own value of KEY (repeat for several).
        #[arg(long = "override", value_name = "KEY")]
        override_keys: Vec<String>,
        /// Follow the shared value of KEY again.
        #[arg(long, value_name = "KEY")]
        share: Vec<String>,
        /// Keep the settings changed in-game as overrides.
        #[arg(long)]
        keep_changes: bool,
        /// Forget the settings changed in-game.
        #[arg(long)]
        dismiss_changes: bool,
        /// KEY=VALUE for an overridden key, written to the instance's options.txt.
        #[arg(long = "value", value_name = "KEY=VALUE")]
        values: Vec<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum Content {
    /// Installed mods, resource packs and shader packs.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli content list Survival\n  lodestone-launcher-cli content list Survival --kind mod"
    )]
    List {
        instance: String,
        /// mod, resourcepack or shader.
        #[arg(long)]
        kind: Option<String>,
    },
    /// Searches Modrinth. With --instance, only what fits it (unless --all).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli content search sodium --instance Survival\n  lodestone-launcher-cli content search \"faithful\" --kind resourcepack --sort downloads\n  lodestone-launcher-cli content search --kind shader --instance Survival"
    )]
    Search {
        query: Option<String>,
        #[arg(long)]
        instance: Option<String>,
        /// mod (default), resourcepack or shader.
        #[arg(long)]
        kind: Option<String>,
        /// Also what doesn't fit the instance.
        #[arg(long)]
        all: bool,
        /// relevance, downloads, follows, newest or updated.
        #[arg(long)]
        sort: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        offset: Option<u32>,
    },
    /// A Modrinth project's versions, and whether each fits the instance.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli content versions sodium --instance Survival --limit 5"
    )]
    Versions {
        /// A slug, project id or modrinth.com link.
        project: String,
        #[arg(long)]
        instance: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
    },
    /// Installs from Modrinth, with required mods; a shader pack also gets its shader mod.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli content install Survival fabric-api\n  lodestone-launcher-cli content install Survival https://modrinth.com/mod/lithium\n  lodestone-launcher-cli content install Survival sodium --version mc1.21.1-0.6.0-fabric\n  lodestone-launcher-cli content install Survival complementary-reimagined --kind shader"
    )]
    Install {
        instance: String,
        /// A slug, project id or modrinth.com link.
        project: String,
        /// A version id or number (default: the newest that fits).
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        kind: Option<String>,
        /// Don't install required mods.
        #[arg(long)]
        no_deps: bool,
        /// Install a version that doesn't fit the instance.
        #[arg(long)]
        force: bool,
    },
    /// Adds a local file (a .jar mod; resource and shader packs need --kind).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli content add Survival ./my-mod.jar\n  lodestone-launcher-cli content add Survival ./pack.zip --kind resourcepack"
    )]
    Add {
        instance: String,
        path: PathBuf,
        #[arg(long)]
        kind: Option<String>,
    },
    /// Moves installed content to the Trash. Needs --yes.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli content remove Survival sodium --yes"
    )]
    Remove(ItemsArgs),
    /// Turns installed content on (a shader pack: makes it the one in use).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli content enable Survival sodium lithium"
    )]
    Enable(ItemsArgs),
    /// Turns installed content off.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli content disable Survival sodium")]
    Disable(ItemsArgs),
    /// Newer versions of installed content.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli content updates Survival")]
    Updates {
        instance: String,
        #[arg(long)]
        kind: Option<String>,
    },
    /// Updates installed content: the named items, or all.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli content update Survival --all\n  lodestone-launcher-cli content update Survival sodium-fabric-0.6.0.jar"
    )]
    Update {
        instance: String,
        /// File names or project ids.
        items: Vec<String>,
        #[arg(long, conflicts_with = "items", required_unless_present = "items")]
        all: bool,
        #[arg(long)]
        kind: Option<String>,
    },
    /// Changes the instance's mod loader (installs next launch). Without LOADER: the
    /// Content tab's offer for a vanilla instance (Fabric, or Forge before 1.14).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli content switch-loader Survival\n  lodestone-launcher-cli content switch-loader Survival quilt\n  lodestone-launcher-cli content switch-loader Survival forge --disable-mismatched"
    )]
    SwitchLoader {
        instance: String,
        /// fabric, quilt, forge, neoforge or vanilla.
        loader: Option<String>,
        #[arg(long)]
        loader_version: Option<String>,
        /// Turn off mods made for another loader.
        #[arg(long)]
        disable_mismatched: bool,
    },
}

#[derive(Args, Debug, Clone)]
pub struct ItemsArgs {
    pub instance: String,
    /// File names, slugs, titles or mod ids.
    #[arg(required = true)]
    pub items: Vec<String>,
    /// mod, resourcepack or shader.
    #[arg(long)]
    pub kind: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Modpacks {
    /// Searches Modrinth for modpacks.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli modpacks search \"fabulously optimized\""
    )]
    Search {
        query: Option<String>,
        #[arg(long)]
        sort: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
        #[arg(long)]
        offset: Option<u32>,
    },
    /// Makes an instance from a modpack on Modrinth or a .mrpack file.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli modpacks install fabulously-optimized\n  lodestone-launcher-cli modpacks install https://modrinth.com/modpack/fabulously-optimized --name FO\n  lodestone-launcher-cli modpacks install --file ./pack.mrpack"
    )]
    Install {
        /// A slug, project id or modrinth.com link.
        #[arg(required_unless_present = "file")]
        project: Option<String>,
        #[arg(long, conflicts_with = "project")]
        file: Option<PathBuf>,
        #[arg(long)]
        version: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },
    /// Whether the instance's modpack has a newer version.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli modpacks check FO")]
    Check(InstanceArg),
    /// Updates the instance's modpack. Without --yes it prints the plan (exit code 5).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli modpacks update FO          # the plan\n  lodestone-launcher-cli modpacks update FO --yes    # apply it\n  lodestone-launcher-cli modpacks update FO --version 6.0.1 --no-backup-worlds --yes"
    )]
    Update {
        instance: String,
        #[arg(long)]
        version: Option<String>,
        /// Don't back up the worlds first.
        #[arg(long)]
        no_backup_worlds: bool,
    },
}

#[derive(Args, Debug, Clone)]
pub struct WorldArgs {
    pub instance: String,
    /// The world's folder or name.
    pub world: String,
}

#[derive(Subcommand, Debug)]
pub enum Worlds {
    /// An instance's worlds.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli worlds list Survival")]
    List(InstanceArg),
    /// A world's backups, newest first.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli worlds backups Survival \"New World\""
    )]
    Backups(WorldArgs),
    /// Backs up a world (a zip in the instance's backups folder).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli worlds backup Survival \"New World\""
    )]
    Backup(WorldArgs),
    /// Restores a backup: as a copy beside the world, or replacing it (--replace, --yes).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli worlds restore Survival \"New World\"\n  lodestone-launcher-cli worlds restore Survival \"New World\" --backup 2026-11-08_14-30-00.zip --replace --yes"
    )]
    Restore {
        #[command(flatten)]
        world: WorldArgs,
        /// The backup's file name (default: the newest).
        #[arg(long, default_value = "latest")]
        backup: String,
        /// Replace the world (it is backed up, then moved to the Trash).
        #[arg(long)]
        replace: bool,
    },
    /// Moves a world to the Trash. Needs --yes.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli worlds delete Survival \"Old World\" --yes"
    )]
    Delete(WorldArgs),
    /// Moves a backup to the Trash. Needs --yes.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli worlds delete-backup Survival \"New World\" 2026-11-01_10-00-00.zip --yes"
    )]
    DeleteBackup {
        #[command(flatten)]
        world: WorldArgs,
        backup: String,
    },
}

/// A password: never on the command line.
#[derive(Args, Debug, Clone, Default)]
pub struct PasswordArgs {
    /// Read the password from the first line of stdin (or set LODESTONE_CORE_PASSWORD).
    #[arg(long)]
    pub password_stdin: bool,
}

#[derive(Subcommand, Debug)]
pub enum Accounts {
    /// Accounts and the active one.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli accounts list")]
    List,
    /// Makes an account the active one.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli accounts switch Steve")]
    Switch {
        /// The account's id or username.
        account: String,
    },
    /// Adds an offline account (once Settings › Developer › Offline accounts is on;
    /// always in debug builds and with LODESTONE_LAUNCHER_OFFLINE=1).
    #[command(after_help = "Examples:\n  lodestone-launcher-cli accounts add-offline Tester")]
    AddOffline { username: String },
    /// Removes an account and its saved sign-in. Needs --yes.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli accounts remove Steve --yes")]
    Remove { account: String },
    /// Signs in with Microsoft: prints a code and a URL for the user, then waits.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli accounts sign-in\n  lodestone-launcher-cli accounts sign-in --no-wait   # the code now; `jobs wait <job>` later"
    )]
    SignIn,
}

#[derive(Subcommand, Debug)]
pub enum Import {
    /// Other launchers' instances on this computer, or in a folder.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli import detect\n  lodestone-launcher-cli import detect --folder ~/Library/Application\\ Support/PrismLauncher"
    )]
    Detect {
        #[arg(long)]
        folder: Option<PathBuf>,
    },
    /// Imports instances by their key from `import detect`.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli import run prism:Fabulous --without-worlds"
    )]
    Run {
        #[arg(required = true)]
        keys: Vec<String>,
        #[arg(long)]
        folder: Option<PathBuf>,
        /// Leave the worlds behind.
        #[arg(long)]
        without_worlds: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum Cores {
    /// The cores, with their servers.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli cores list\n  lodestone-launcher-cli cores list --cached"
    )]
    List {
        /// Don't ask the cores again.
        #[arg(long)]
        cached: bool,
    },
    /// Adds a core; with --username, signs in (the password from stdin) and saves a key
    /// for this device in the Keychain. A certificate this computer doesn't trust fails
    /// with its fingerprint (exit 6): the user compares it, then add again with
    /// --fingerprint.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli cores add 192.168.1.20\n  printf '%s\\n' \"$PASSWORD\" | lodestone-launcher-cli cores add 192.168.1.20 --username alice --password-stdin\n  lodestone-launcher-cli cores add core.example.com --fingerprint AB:CD:…"
    )]
    Add {
        address: String,
        #[arg(long)]
        username: Option<String>,
        #[command(flatten)]
        password: PasswordArgs,
        /// How long the device key lasts: 90d, 1y (default) or never.
        #[arg(long)]
        expiry: Option<String>,
        /// Ask for console read access.
        #[arg(long)]
        console_read: bool,
        /// Ask for console write access (sending commands).
        #[arg(long)]
        console_write: bool,
        /// The certificate fingerprint the user compared.
        #[arg(long)]
        fingerprint: Option<String>,
    },
    /// Removes a core and forgets this device's key. Needs --yes.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli cores remove \"Home core\" --yes")]
    Remove { core: String },
    /// Trusts a core for mods: its servers' mods install without the advisory. Needs --yes.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli cores trust \"Home core\" --yes")]
    Trust { core: String },
    /// Stops trusting a core for mods.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli cores untrust \"Home core\"")]
    Untrust { core: String },
    /// Pins a core's certificate after the user compared its fingerprint.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli cores trust-certificate \"Home core\" --fingerprint AB:CD:…"
    )]
    TrustCertificate {
        core: String,
        #[arg(long)]
        fingerprint: String,
    },
    /// Forgets this device's key (it stays valid on the core until revoked).
    #[command(after_help = "Examples:\n  lodestone-launcher-cli cores sign-out \"Home core\"")]
    SignOut { core: String },
    /// Connects as owner to the Lodestone Host or Lodestone Core on this computer: proves
    /// the launcher can write the core's data folder, and gets an owner-level key. A core
    /// without an owner gets one the launcher makes up (`owner_xxxxx`); the key and that
    /// account are kept in plain text in the launcher's `cores/local_credentials.json`.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli cores claim-local\n  lodestone-launcher-cli cores claim-local --folder ~/lodestone-data"
    )]
    ClaimLocal {
        /// The core's data folder (its LODESTONE_PATH), when it isn't $LODESTONE_PATH or
        /// ~/.lodestone: a core in Docker keeps it where its volume is mounted.
        #[arg(long)]
        folder: Option<PathBuf>,
        /// The core's id, when more than one core runs on this computer.
        #[arg(long)]
        core: Option<String>,
    },
}

#[derive(Args, Debug, Clone)]
pub struct ServerArg {
    /// The server's id or name.
    pub server: String,
    /// The core's id, name or address, when two cores have a server of that name.
    #[arg(long)]
    pub core: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum Servers {
    /// The servers on the cores.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli servers list\n  lodestone-launcher-cli servers list --core \"Home core\""
    )]
    List {
        #[arg(long)]
        core: Option<String>,
        #[arg(long)]
        cached: bool,
    },
    /// Starts a server (needs this device signed in to its core).
    #[command(after_help = "Examples:\n  lodestone-launcher-cli servers start Smp")]
    Start(ServerArg),
    /// Stops a server.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli servers stop Smp")]
    Stop(ServerArg),
    /// Plays the server's linked instance and joins it, starting the server if it's
    /// stopped and this device may.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli servers join Smp\n  lodestone-launcher-cli servers join Smp --update-mods --accept-advisory"
    )]
    Join {
        #[command(flatten)]
        server: ServerArg,
        #[arg(long, conflicts_with = "skip_update")]
        update_mods: bool,
        #[arg(long)]
        skip_update: bool,
        /// Install mods from a core that isn't trusted for mods, this once. Ask the user.
        #[arg(long)]
        accept_advisory: bool,
    },
    /// Makes an instance matching the server (version, loader, client mods) and links it.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli servers create-instance Smp\n  lodestone-launcher-cli servers create-instance Smp --accept-advisory"
    )]
    CreateInstance {
        #[command(flatten)]
        server: ServerArg,
        #[arg(long)]
        accept_advisory: bool,
    },
    /// Links an instance to a server: Play then joins it and keeps its mods in step.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli servers link Survival Smp")]
    Link {
        instance: String,
        #[command(flatten)]
        server: ServerArg,
    },
    /// Unlinks an instance from its server (its mods stay).
    #[command(after_help = "Examples:\n  lodestone-launcher-cli servers unlink Survival")]
    Unlink(InstanceArg),
    /// Hosts an instance's modpack as a new (stopped) server on a core. Hosting accepts
    /// Minecraft's EULA: ask the user, then pass --accept-eula. On a core this computer
    /// is the owner of (`cores claim-local`) the owner key creates it; on any other core,
    /// sign in with --username (the password from stdin).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli servers host Survival --core \"Lodestone Host\" --accept-eula\n  printf '%s\\n' \"$PASSWORD\" | lodestone-launcher-cli servers host Survival --core \"Home core\" --username alice --password-stdin --max-ram 4096 --accept-eula"
    )]
    Host {
        instance: String,
        #[arg(long)]
        core: String,
        #[arg(long)]
        username: Option<String>,
        #[command(flatten)]
        password: PasswordArgs,
        /// The server's name (default: the instance's).
        #[arg(long)]
        name: Option<String>,
        /// MB: 2048, 4096, 6144 or 8192.
        #[arg(long)]
        max_ram: Option<u32>,
        #[arg(long)]
        accept_eula: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum Friends {
    /// Friends' servers joined before: name, version, who shares it, last status.
    /// Never prints invite links (they are secrets).
    #[command(after_help = "Examples:\n  lodestone-launcher-cli friends list")]
    List,
    /// Joins a friend's server: connects to their computer through Lodestone Connect,
    /// gets a matching instance (with the server's mods), and plays. Give the invite
    /// link, or the name of a server joined before. An invite link lets anyone join,
    /// so prefer "-" (the link on stdin's first line) to keep it out of shell history.
    #[command(
        after_help = "Examples:\n  printf '%s\\n' \"$LINK\" | lodestone-launcher-cli friends join -\n  lodestone-launcher-cli friends join \"Alex's SMP\"\n  lodestone-launcher-cli friends join - --no-play < link.txt\n  lodestone-launcher-cli friends join \"Alex's SMP\" --accept-advisory"
    )]
    Join {
        /// The invite link (https://connect.lodestone.cc/j/…, lodestone://join?…), "-"
        /// to read it from stdin, or a friend's server joined before (its name or id).
        link_or_name: String,
        /// Only connect and show the server; don't start the game.
        #[arg(long)]
        no_play: bool,
        #[arg(long, conflicts_with = "skip_update")]
        update_mods: bool,
        #[arg(long)]
        skip_update: bool,
        /// Install the server's mods this once, though it isn't trusted for mods. Ask
        /// the user.
        #[arg(long)]
        accept_advisory: bool,
    },
    /// Forgets a friend's server and its invite link. Instances made for it stay.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli friends remove \"Alex's SMP\" --yes"
    )]
    Remove {
        /// Its name or id.
        friend: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum Prompts {
    /// What the window is asking: update, advisory, certificate or trust.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli prompts list")]
    List,
    /// Answers a question in the window (trusting needs --yes; a certificate --fingerprint).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli prompts answer advisory install_once\n  lodestone-launcher-cli prompts answer update play\n  lodestone-launcher-cli prompts answer certificate trust --fingerprint AB:CD:…"
    )]
    Answer {
        /// update, advisory, certificate or trust.
        prompt: String,
        /// update: update|play|cancel; advisory: install_once|trust|cancel;
        /// certificate: trust|reject; trust: trust|no.
        answer: String,
        #[arg(long)]
        fingerprint: Option<String>,
    },
}

/// The parts of an instance a snapshot takes or puts back.
#[derive(clap::Args, Debug, Default)]
pub struct PartsArgs {
    /// A part to include (repeatable): mods, resource_packs, shader_packs, configs,
    /// worlds, screenshots, settings, other. Default: take all but screenshots and
    /// other; put back or import all the snapshot has.
    #[arg(long = "include", value_name = "PART", value_parser = ["mods", "resource_packs", "shader_packs", "configs", "worlds", "screenshots", "settings", "other"])]
    pub include: Vec<String>,
    /// Only this world (its folder name; repeatable). Default: every world.
    #[arg(long = "world", value_name = "FOLDER")]
    pub worlds: Vec<String>,
}

#[derive(Subcommand, Debug)]
pub enum Snapshots {
    /// An instance's snapshots, newest first: id, name, date, sizes (total, and new
    /// bytes it added to the store), and the parts it has.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli snapshots list Survival")]
    List(InstanceArg),
    /// Takes a snapshot. Files shared with other snapshots are stored once.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli snapshots create Survival --name \"Before 1.21\"\n  lodestone-launcher-cli snapshots create Survival --include worlds --world \"New World\""
    )]
    Create {
        instance: String,
        #[arg(long)]
        name: Option<String>,
        #[command(flatten)]
        parts: PartsArgs,
    },
    /// Puts back the chosen parts of a snapshot, replacing them whole, after an
    /// automatic snapshot of them. Needs --yes.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli snapshots revert Survival \"Before 1.21\" --yes\n  lodestone-launcher-cli snapshots revert Survival latest --include mods --include configs --yes"
    )]
    Revert {
        instance: String,
        /// The snapshot's id, its name, or latest.
        snapshot: String,
        #[command(flatten)]
        parts: PartsArgs,
    },
    /// Writes a snapshot to a .lodesnap file to share (a folder: named after it).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli snapshots export Survival latest ~/Desktop"
    )]
    Export {
        instance: String,
        /// The snapshot's id, its name, or latest.
        snapshot: String,
        path: PathBuf,
    },
    /// Makes a new instance from a .lodesnap file. --preview only reads it (its mods,
    /// parts and sizes). One with mods needs --yes: only use files from people you trust.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli snapshots import ~/Downloads/Survival.lodesnap --preview\n  lodestone-launcher-cli snapshots import ~/Downloads/Survival.lodesnap --yes"
    )]
    Import {
        path: PathBuf,
        #[arg(long)]
        preview: bool,
        #[command(flatten)]
        parts: PartsArgs,
    },
    /// Deletes a snapshot, and the files no other snapshot uses. Needs --yes.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli snapshots delete Survival \"Before 1.21\" --yes"
    )]
    Delete {
        instance: String,
        /// The snapshot's id or name.
        snapshot: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum Storage {
    /// What the launcher's data folder takes: each instance (worlds, mods and packs,
    /// other), game files and how much no instance uses, Java runtimes and which
    /// versions use them, the cache, and logs. Sizes are in bytes.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli storage show\n  lodestone-launcher-cli --json storage show | jq .totals"
    )]
    Show,
    /// Removes unused game files (versions, libraries, assets) or unused Java runtimes,
    /// clears the cache, removes leftovers of unfinished downloads and imports, or
    /// deletes logs and crash reports, or removes automatic snapshots, those of
    /// deleted instances and shared snapshot files. Game files and runtimes
    /// download again when needed; logs are gone for good. Needs --yes.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli storage clean game-files --yes\n  lodestone-launcher-cli storage clean old-logs --yes"
    )]
    Clean {
        #[arg(value_parser = ["game-files", "runtimes", "cache", "leftovers", "old-logs", "logs", "snapshots"])]
        what: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum Skins {
    /// The skins: built-in ones first, then imported packs, and the one in use.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli skins list")]
    List,
    /// Uses a skin by id ("default" for the default look).
    #[command(
        name = "use",
        after_help = "Examples:\n  lodestone-launcher-cli skins use grassblock\n  lodestone-launcher-cli skins use default"
    )]
    Use { id: String },
    /// Imports a skin pack: a .zip file or a folder with skin.json and its images.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli skins import ~/Downloads/Cozy.zip --use"
    )]
    Import {
        path: PathBuf,
        /// Use it once imported.
        #[arg(long = "use")]
        use_it: bool,
    },
    /// Checks a skin's readability: each surface's text against what it shows, with
    /// a suggested fix for what falls short. The one in use, an id, or a pack at
    /// --path (checked as importing would, without keeping it).
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli skins check\n  lodestone-launcher-cli skins check cozy\n  lodestone-launcher-cli skins check --path ./cozy"
    )]
    Check {
        id: Option<String>,
        #[arg(long)]
        path: Option<PathBuf>,
    },
    /// Applies the suggested readability fixes to an imported skin (the one in use,
    /// or an id). Built-in skins can't be changed.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli skins fix cozy")]
    Fix { id: Option<String> },
    /// Writes a skin to a .zip file: the one in use, or --id.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli skins export ~/Desktop/mine.zip\n  lodestone-launcher-cli skins export neon.zip --id neon"
    )]
    Export {
        path: PathBuf,
        #[arg(long)]
        id: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum Jobs {
    /// Long operations, running and finished.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli jobs list\n  lodestone-launcher-cli jobs list --running"
    )]
    List {
        #[arg(long)]
        running: bool,
    },
    /// One job.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli jobs get 3")]
    Get { job: u64 },
    /// Waits for a job, with progress.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli jobs wait 3 --timeout 600")]
    Wait { job: u64 },
    /// Cancels a job.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli jobs cancel 3")]
    Cancel { job: u64 },
}

#[derive(Subcommand, Debug)]
pub enum Trace {
    /// The trace topics, what each says, and how many events each had so far.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli trace topics\n  lodestone-launcher-cli --json trace topics | jq '.topics[] | select(.events > 0)'"
    )]
    Topics,
    /// Streams trace events: the last earlier ones first (--backlog), then each new one.
    /// Patterns: `ui` or `ui.*` for everything under ui, `*` for one segment
    /// (`*.error`); none for everything.
    #[command(
        after_help = "Examples:\n  lodestone-launcher-cli trace watch\n  lodestone-launcher-cli trace watch ui nav settings.change --backlog 20\n  lodestone-launcher-cli trace watch game.log '*.error' --backlog 0 --count 100\n  lodestone-launcher-cli --json trace watch http --timeout 30"
    )]
    Watch(TraceWatchArgs),
    /// What the window shows now: page, tab, instance, dialog, Settings page, size,
    /// zoom, account; each instance's game state; running jobs.
    #[command(after_help = "Examples:\n  lodestone-launcher-cli trace snapshot")]
    Snapshot,
}

#[derive(Args, Debug)]
pub struct TraceWatchArgs {
    /// Topic patterns (all topics without any).
    pub topics: Vec<String>,
    /// Earlier events to send first (the launcher keeps the last 2000).
    #[arg(long, default_value_t = 50)]
    pub backlog: usize,
    /// Only earlier events after this sequence number.
    #[arg(long)]
    pub since: Option<u64>,
    /// Stop after this many events.
    #[arg(long)]
    pub count: Option<usize>,
}

#[derive(Args, Debug)]
pub struct EventsArgs {
    /// log, state, progress, jobs, notices (comma-separated; all by default).
    #[arg(long, value_delimiter = ',')]
    pub topics: Vec<String>,
    /// One instance's log, state and progress.
    #[arg(long)]
    pub instance: Option<String>,
    /// Console rows from before to send first.
    #[arg(long, default_value_t = 0)]
    pub backlog: usize,
    /// Stop after this many events.
    #[arg(long)]
    pub count: Option<usize>,
}
