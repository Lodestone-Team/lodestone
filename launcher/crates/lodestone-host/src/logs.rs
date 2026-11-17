//! The Host's log file ([`launcher_core::logging`]): `<Host folder>/logs/host.log`,
//! beside the core's `core.log`, with the level from Settings › Updates › Log detail.

use std::path::PathBuf;

use gpui::App;
use launcher_core::logging::{self, AppKind, Config, LogLevel, Started};

use crate::core_process::host_dir;

/// The logs folder: the Host's own and the core's logs.
pub fn dir() -> PathBuf {
    host_dir().join(logging::LOGS_DIR)
}

/// Starts logging, before anything else that could fail. The offscreen screenshots
/// (`screenshot`) log to their own Host folder when `LODESTONE_HOST_DATA_DIR` names
/// one, and otherwise only to the terminal.
pub fn start(screenshot: bool) -> Started {
    let own_folder =
        std::env::var_os(crate::core_process::env::HOST_DATA).is_some_and(|dir| !dir.is_empty());
    let logs = (!screenshot || own_folder).then(dir);
    let level = if logs.is_some() {
        crate::prefs::load_saved().log_level
    } else {
        LogLevel::Info
    };
    logging::install(Config {
        app: AppKind::Host,
        dir: logs,
        level,
        env_filter: std::env::var(logging::ENV).ok(),
        version: version(),
    })
}

fn version() -> String {
    logging::version_label(env!("CARGO_PKG_VERSION"), crate::updater::this_build())
}

/// Shows the logs folder in Finder or Explorer, making it first if needed.
pub fn open_folder(cx: &mut App) {
    let dir = logging::logs_dir().unwrap_or_else(dir);
    if let Err(err) = logging::make_dir(&dir) {
        log::warn!("couldn't make the logs folder: {err}");
    }
    logging::flush();
    cx.open_with_system(&dir);
}
