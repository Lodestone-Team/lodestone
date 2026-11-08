//! The Java command line that starts the game.
//!
//! The JVM and game arguments come from the version's `arguments.jvm` and
//! `arguments.game` (1.13 and later), evaluated with rules and features, or from the
//! legacy `minecraftArguments` string with the JVM arguments the official launcher
//! uses for such versions. Every `${placeholder}` is substituted.
//!
//! The command holds the access token. Its `Debug` output and
//! [`LaunchCommand::redacted`] replace it with `<redacted>`; only
//! [`LaunchCommand::to_command`] has the real value, to start the process.

use std::{
    fmt,
    path::{Path, PathBuf},
    process::Command,
};

use uuid::Uuid;

use super::{
    platform::{Arch, Os, Platform},
    rules::{self, Features},
    version::{Argument, VersionJson},
};
use crate::{Secret, accounts::Account};

/// `${launcher_name}`.
pub const LAUNCHER_NAME: &str = "lodestone-launcher";
/// `${launcher_version}`.
pub const LAUNCHER_VERSION: &str = env!("CARGO_PKG_VERSION");
/// What offline accounts pass as the access token. The game only checks it against
/// Mojang when joining an online-mode server, which offline accounts cannot.
pub const OFFLINE_TOKEN: &str = "offline";

/// `${user_type}`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UserType {
    /// A Microsoft account.
    Msa,
    /// An offline account.
    Legacy,
}

impl UserType {
    pub fn as_str(self) -> &'static str {
        match self {
            UserType::Msa => "msa",
            UserType::Legacy => "legacy",
        }
    }
}

/// Who the game is started as.
#[derive(Clone, Debug)]
pub struct LaunchAccount {
    pub username: String,
    pub uuid: Uuid,
    pub access_token: Secret,
    /// The Xbox user id, for Microsoft accounts.
    pub xuid: Option<String>,
    pub user_type: UserType,
}

impl LaunchAccount {
    /// An offline account: its offline UUID and a placeholder token.
    pub fn offline(account: &Account) -> Self {
        Self {
            username: account.username.clone(),
            uuid: account.uuid,
            access_token: Secret::new(OFFLINE_TOKEN),
            xuid: None,
            user_type: UserType::Legacy,
        }
    }

    /// A Microsoft account with its Minecraft access token.
    pub fn microsoft(
        username: &str,
        uuid: Uuid,
        access_token: Secret,
        xuid: Option<String>,
    ) -> Self {
        Self {
            username: username.to_string(),
            uuid,
            access_token,
            xuid,
            user_type: UserType::Msa,
        }
    }
}

/// The instance's settings that change the command line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSettings {
    pub min_ram_mb: u32,
    pub max_ram_mb: u32,
    pub jvm_args: Vec<String>,
    pub game_args: Vec<String>,
    /// The window size, if not the game's default.
    pub resolution: Option<(u32, u32)>,
    pub fullscreen: bool,
    /// Environment variables for the game.
    pub env: Vec<(String, String)>,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            min_ram_mb: 512,
            max_ram_mb: 2048,
            jvm_args: Vec::new(),
            game_args: Vec::new(),
            resolution: None,
            fullscreen: false,
            env: Vec::new(),
        }
    }
}

/// Where everything is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LaunchPaths {
    pub java: PathBuf,
    /// The instance's `minecraft/` folder, the working directory.
    pub game_dir: PathBuf,
    pub natives_dir: PathBuf,
    pub libraries_dir: PathBuf,
    pub assets_dir: PathBuf,
    /// `${game_assets}`: where old versions read assets by name.
    pub game_assets: PathBuf,
    /// The classpath, in order, with the client jar last.
    pub classpath: Vec<PathBuf>,
    /// Mojang's Log4j config, if the version has one.
    pub logging_config: Option<PathBuf>,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LaunchError {
    #[error(
        "Minecraft {0} uses Log4j but has no logging config, so it can't be started safely (CVE-2021-44228)"
    )]
    NoLoggingConfig(String),
    #[error("the path {0} is not valid UTF-8")]
    NonUtf8Path(PathBuf),
    #[error("Minecraft {0} has no arguments in its version file")]
    NoArguments(String),
}

/// A command line ready to run.
#[derive(Clone)]
pub struct LaunchCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    /// Set on top of the launcher's own environment.
    pub env: Vec<(String, String)>,
    secret: Secret,
}

impl LaunchCommand {
    /// The arguments with the access token replaced by `<redacted>`, for logs and
    /// the console.
    pub fn redacted(&self) -> Vec<String> {
        let secret = self.secret.expose();
        self.args
            .iter()
            .map(|arg| {
                if secret.is_empty() || !arg.contains(secret) {
                    arg.clone()
                } else {
                    arg.replace(secret, "<redacted>")
                }
            })
            .collect()
    }

    /// The command line as one string, redacted, for the console.
    pub fn display(&self) -> String {
        std::iter::once(self.program.display().to_string())
            .chain(self.redacted().into_iter().map(|arg| shell_quote(&arg)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The secrets the game's output should be scrubbed of.
    pub fn secrets(&self) -> Vec<String> {
        vec![self.secret.expose().to_string()]
    }

    /// The process to spawn.
    pub fn to_command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.cwd)
            .envs(self.env.iter().map(|(key, value)| (key, value)));
        command
    }
}

impl fmt::Debug for LaunchCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LaunchCommand")
            .field("program", &self.program)
            .field("args", &self.redacted())
            .field("cwd", &self.cwd)
            // Only the names: the values are the user's and may be private.
            .field(
                "env",
                &self.env.iter().map(|(key, _)| key).collect::<Vec<_>>(),
            )
            .finish()
    }
}

fn shell_quote(arg: &str) -> String {
    if !arg.is_empty()
        && arg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./=:,+@%${}<>".contains(c))
    {
        arg.to_string()
    } else {
        format!("'{}'", arg.replace('\'', "'\\''"))
    }
}

fn utf8(path: &Path) -> Result<String, LaunchError> {
    path.to_str()
        .map(str::to_string)
        .ok_or_else(|| LaunchError::NonUtf8Path(path.to_owned()))
}

/// The classpath separator of `os`.
fn classpath_separator(os: Os) -> &'static str {
    match os {
        Os::Windows => ";",
        Os::MacOs | Os::Linux => ":",
    }
}

/// The values of `${...}` placeholders.
struct Values(Vec<(&'static str, String)>);

impl Values {
    fn substitute(&self, arg: &str) -> String {
        if !arg.contains("${") {
            return arg.to_string();
        }
        let mut out = String::with_capacity(arg.len());
        let mut rest = arg;
        while let Some(start) = rest.find("${") {
            out.push_str(&rest[..start]);
            let after = &rest[start + 2..];
            match after.find('}') {
                Some(end) => {
                    let key = &after[..end];
                    match self.0.iter().find(|(name, _)| *name == key) {
                        Some((_, value)) => out.push_str(value),
                        // Unknown placeholders are left as they are, like the
                        // official launcher does.
                        None => out.push_str(&rest[start..start + 2 + end + 1]),
                    }
                    rest = &after[end + 1..];
                }
                None => {
                    out.push_str(&rest[start..]);
                    rest = "";
                }
            }
        }
        out.push_str(rest);
        out
    }
}

/// The JVM arguments of versions before 1.13, which have none in their file. These
/// are the arguments 1.13 and later list, without `-XstartOnFirstThread`: LWJGL 2
/// uses AWT on macOS, which must not start on the first thread.
fn legacy_jvm_arguments(platform: &Platform) -> Vec<String> {
    let mut args = Vec::new();
    if platform.os == Os::MacOs {
        args.push("-Xdock:name=Minecraft".to_string());
    }
    if platform.os == Os::Windows {
        args.push(
            "-XX:HeapDumpPath=MojangTricksIntelDriversForPerformance_javaw.exe_minecraft.exe.heapdump"
                .to_string(),
        );
    }
    if platform.arch == Arch::X86 {
        args.push("-Xss1M".to_string());
    }
    args.extend(
        [
            "-Djava.library.path=${natives_directory}",
            "-Dminecraft.launcher.brand=${launcher_name}",
            "-Dminecraft.launcher.version=${launcher_version}",
            "-cp",
            "${classpath}",
        ]
        .map(str::to_string),
    );
    args
}

fn evaluate(arguments: &[Argument], platform: &Platform, features: &Features) -> Vec<String> {
    let mut out = Vec::new();
    for argument in arguments {
        match argument {
            Argument::Plain(value) => out.push(value.clone()),
            Argument::Conditional { rules, value } => {
                if rules::allows(rules, platform, features) {
                    out.extend(value.values().iter().cloned());
                }
            }
        }
    }
    out
}

/// A multiplayer server to join as soon as the game has started.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JoinServer {
    /// A host name or IP address (IPv6 without brackets).
    pub host: String,
    pub port: u16,
}

impl JoinServer {
    /// `host:port`, with an IPv6 address in brackets, as the game's server list takes it.
    pub fn address(&self) -> String {
        if self.host.contains(':') {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

/// How a version joins a server it is started with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JoinArguments {
    /// `--quickPlayMultiplayer host:port`, from the version file's
    /// `is_quick_play_multiplayer` argument (1.20 and later, since snapshot 23w14a).
    QuickPlay,
    /// `--server host --port port`, which the game reads from 1.6 to 1.19.4 (and
    /// which 1.20 replaced with Quick Play).
    ServerAndPort,
    /// Versions before 1.6 take their arguments by position and cannot join a server
    /// at startup.
    Unsupported,
}

/// Which arguments make `version` join a server at startup. Decided from the version
/// file itself, as the official launcher does, rather than from version numbers.
pub fn join_arguments(version: &VersionJson) -> JoinArguments {
    match (&version.arguments, &version.minecraft_arguments) {
        (Some(arguments), _) => {
            let quick_play = arguments.game.iter().any(|argument| match argument {
                Argument::Plain(_) => false,
                Argument::Conditional { rules, .. } => rules.iter().any(|rule| {
                    rule.features
                        .as_ref()
                        .is_some_and(|f| f.contains_key("is_quick_play_multiplayer"))
                }),
            });
            if quick_play {
                JoinArguments::QuickPlay
            } else {
                JoinArguments::ServerAndPort
            }
        }
        // 1.6 to 1.12.2 name their arguments; older versions list them by position
        (None, Some(legacy)) if legacy.contains("--username") => JoinArguments::ServerAndPort,
        _ => JoinArguments::Unsupported,
    }
}

/// Builds the command that starts `version` on `platform` (the platform of the
/// Java that runs it).
pub fn build(
    version: &VersionJson,
    platform: &Platform,
    paths: &LaunchPaths,
    account: &LaunchAccount,
    settings: &GameSettings,
) -> Result<LaunchCommand, LaunchError> {
    build_joining(version, platform, paths, account, settings, None)
}

/// Like [`build`], joining `join` once the game has started, with the arguments
/// [`join_arguments`] picks. Versions that cannot join at startup start normally.
pub fn build_joining(
    version: &VersionJson,
    platform: &Platform,
    paths: &LaunchPaths,
    account: &LaunchAccount,
    settings: &GameSettings,
    join: Option<&JoinServer>,
) -> Result<LaunchCommand, LaunchError> {
    let logging = version.client_logging();
    if version.uses_log4j() && (logging.is_none() || paths.logging_config.is_none()) {
        return Err(LaunchError::NoLoggingConfig(version.id.clone()));
    }

    let join_with = join.map(|server| (server, join_arguments(version)));
    let features = Features {
        has_custom_resolution: settings.resolution.is_some(),
        is_quick_play_multiplayer: matches!(join_with, Some((_, JoinArguments::QuickPlay))),
        ..Features::default()
    };
    let classpath = paths
        .classpath
        .iter()
        .map(|path| utf8(path))
        .collect::<Result<Vec<_>, _>>()?
        .join(classpath_separator(platform.os));
    let (width, height) = settings.resolution.unwrap_or((854, 480));
    let uuid = account.uuid.simple().to_string();
    let token = account.access_token.expose().to_string();
    let values = Values(vec![
        ("natives_directory", utf8(&paths.natives_dir)?),
        ("launcher_name", LAUNCHER_NAME.to_string()),
        ("launcher_version", LAUNCHER_VERSION.to_string()),
        ("classpath", classpath),
        (
            "classpath_separator",
            classpath_separator(platform.os).to_string(),
        ),
        ("library_directory", utf8(&paths.libraries_dir)?),
        ("auth_player_name", account.username.clone()),
        ("version_name", version.id.clone()),
        ("game_directory", utf8(&paths.game_dir)?),
        ("assets_root", utf8(&paths.assets_dir)?),
        ("game_assets", utf8(&paths.game_assets)?),
        ("assets_index_name", version.assets_id().to_string()),
        ("auth_uuid", uuid.clone()),
        ("auth_access_token", token.clone()),
        ("auth_session", format!("token:{token}:{uuid}")),
        ("clientid", crate::auth::CLIENT_ID.to_string()),
        (
            "auth_xuid",
            account.xuid.clone().unwrap_or_else(|| "0".into()),
        ),
        ("user_type", account.user_type.as_str().to_string()),
        ("user_properties", "{}".to_string()),
        ("version_type", version.kind.as_str().to_string()),
        ("resolution_width", width.to_string()),
        ("resolution_height", height.to_string()),
        (
            "quickPlayPath",
            utf8(&paths.game_dir.join("quickPlay").join("log.json"))?,
        ),
        ("quickPlaySingleplayer", String::new()),
        (
            "quickPlayMultiplayer",
            join.map(JoinServer::address).unwrap_or_default(),
        ),
        ("quickPlayRealms", String::new()),
    ]);

    // JVM arguments: the version's (or the legacy ones), memory, the user's.
    let (mut jvm, mut game) = match (&version.arguments, &version.minecraft_arguments) {
        (Some(arguments), _) => (
            evaluate(&arguments.jvm, platform, &features),
            evaluate(&arguments.game, platform, &features),
        ),
        (None, Some(legacy)) => {
            let mut game: Vec<String> = legacy.split_whitespace().map(str::to_string).collect();
            if settings.resolution.is_some() {
                game.extend(
                    [
                        "--width",
                        "${resolution_width}",
                        "--height",
                        "${resolution_height}",
                    ]
                    .map(str::to_string),
                );
            }
            (legacy_jvm_arguments(platform), game)
        }
        (None, None) => return Err(LaunchError::NoArguments(version.id.clone())),
    };
    // LWJGL 3 needs the first thread on macOS. Versions since 1.13 ask for it in
    // their file; add it if a file does not.
    if platform.os == Os::MacOs
        && version.uses_lwjgl3()
        && !jvm.iter().any(|arg| arg == "-XstartOnFirstThread")
    {
        jvm.insert(0, "-XstartOnFirstThread".to_string());
    }

    let mut args: Vec<String> = jvm.iter().map(|arg| values.substitute(arg)).collect();
    args.push(format!(
        "-Xms{}M",
        settings.min_ram_mb.min(settings.max_ram_mb)
    ));
    args.push(format!("-Xmx{}M", settings.max_ram_mb));
    if settings.jvm_args.is_empty()
        && let Some(arguments) = &version.arguments
    {
        // Mojang's suggested settings (garbage collector and so on), without its
        // memory sizes, which the instance sets.
        args.extend(
            evaluate(&arguments.default_user_jvm, platform, &features)
                .into_iter()
                .filter(|arg| !arg.starts_with("-Xms") && !arg.starts_with("-Xmx"))
                .map(|arg| values.substitute(&arg)),
        );
    }
    args.extend(settings.jvm_args.iter().cloned());
    if let (Some(logging), Some(config)) = (logging, &paths.logging_config) {
        args.push(logging.argument.replace("${path}", &utf8(config)?));
    }
    args.push(version.main_class.clone());
    if settings.fullscreen {
        game.push("--fullscreen".to_string());
    }
    args.extend(game.iter().map(|arg| values.substitute(arg)));
    if let Some((server, JoinArguments::ServerAndPort)) = join_with {
        args.extend([
            "--server".to_string(),
            server.host.clone(),
            "--port".to_string(),
            server.port.to_string(),
        ]);
    }
    args.extend(settings.game_args.iter().cloned());

    Ok(LaunchCommand {
        program: paths.java.clone(),
        args,
        cwd: paths.game_dir.clone(),
        env: settings.env.clone(),
        secret: account.access_token.clone(),
    })
}

/// Splits user-typed arguments on whitespace, keeping quoted parts together:
/// `-Dname="a b" -Xss2m` is `-Dname=a b` and `-Xss2m`.
pub fn split_args(text: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut has_arg = false;
    for c in text.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                has_arg = true;
            }
            (None, c) if c.is_whitespace() => {
                if has_arg {
                    args.push(std::mem::take(&mut current));
                    has_arg = false;
                }
            }
            (None, c) => {
                current.push(c);
                has_arg = true;
            }
        }
    }
    if has_arg {
        args.push(current);
    }
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn substitutes_placeholders() {
        let values = Values(vec![("a", "1".into()), ("b", "two".into())]);
        assert_eq!(values.substitute("-Dx=${a}/${b}"), "-Dx=1/two");
        assert_eq!(values.substitute("${unknown} ${a}"), "${unknown} 1");
        assert_eq!(values.substitute("${unterminated"), "${unterminated");
        assert_eq!(values.substitute("plain"), "plain");
    }

    #[test]
    fn splits_user_arguments() {
        assert_eq!(
            split_args(r#"  -Xss2m -Dname="a b" '-Dq=it''s'  "" "#),
            ["-Xss2m", "-Dname=a b", "-Dq=its", ""]
        );
        assert!(split_args("   ").is_empty());
    }

    #[test]
    fn quotes_for_display() {
        assert_eq!(shell_quote("-Xmx2048M"), "-Xmx2048M");
        assert_eq!(shell_quote("Render thread"), "'Render thread'");
        assert_eq!(shell_quote("it's"), "'it'\\''s'");
        assert_eq!(shell_quote(""), "''");
    }
}
