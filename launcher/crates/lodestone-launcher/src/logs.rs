//! The launcher's log file ([`launcher_core::logging`]): where it goes, the level from
//! Settings › Developer, and opening the folder.
//!
//! The logs live in the default data folder's `logs/` (`launcher.log`, rotated to
//! `.1` … `.5`, and `last-crash.txt`), which stays put when the data folder moves.

use std::path::PathBuf;

use gpui::App;
use launcher_core::logging::{self, AppKind, Config, LogLevel, Started};

/// Starts logging, before anything else that could fail. The offscreen scenes
/// (`scene`) log to their own data folder's `logs/` when `LODESTONE_LAUNCHER_DATA_DIR`
/// names one, and otherwise only to the terminal.
pub fn start(scene: bool) -> Started {
    let location = launcher_control::paths::locate();
    let dir = match (&location, scene) {
        (_, true) => std::env::var_os(launcher_control::paths::DATA_DIR_ENV)
            .filter(|dir| !dir.is_empty())
            .map(|dir| launcher_control::paths::logs_dir(&PathBuf::from(dir))),
        (Some(location), false) => Some(launcher_control::paths::logs_dir(&location.default)),
        (None, false) => None,
    };
    let level = match (&location, scene) {
        (Some(location), false) => {
            launcher_core::settings::load_or_default(&location.dir.join("settings.json"))
                .developer
                .log_level
        }
        _ => LogLevel::Info,
    };
    logging::install(Config {
        app: AppKind::Launcher,
        dir,
        level,
        env_filter: std::env::var(logging::ENV).ok(),
        version: version(),
    })
}

/// "1.2.0 (build 812, 1a2b3c4)", or "(development build)" for one the publish script
/// didn't make.
fn version() -> String {
    let version = env!("CARGO_PKG_VERSION");
    match launcher_core::update::Build::current() {
        Some(build) => match build.commit {
            Some(commit) => format!(
                "{version} (build {}, {})",
                build.number,
                launcher_core::update::short_commit(commit)
            ),
            None => format!("{version} (build {})", build.number),
        },
        None => format!("{version} (development build)"),
    }
}

/// The logs folder: the one logging writes to, or where it would be.
pub fn dir() -> Option<PathBuf> {
    logging::logs_dir().or_else(|| {
        launcher_control::paths::locate()
            .map(|location| launcher_control::paths::logs_dir(&location.default))
    })
}

/// Shows the logs folder in Finder or Explorer, making it first if needed.
pub fn open_folder(cx: &mut App) {
    let Some(dir) = dir() else {
        return;
    };
    if let Err(err) = std::fs::create_dir_all(&dir) {
        log::warn!("couldn't make the logs folder: {err}");
    }
    logging::flush();
    cx.open_with_system(&dir);
}
