//! Discord Rich Presence: the "Playing" status on the player's Discord profile.
//!
//! The pieces, without GPUI and without threads of their own (the app runs them on
//! one worker thread, off the window's):
//! - [`ipc`]: the local connection to the Discord app and `SET_ACTIVITY`;
//! - [`activity`]: where the player is (menus, single player, a server), from the
//!   game's log lines;
//! - [`world`]: the dimension, coordinates and stats from the single-player world's
//!   save files, which the game writes when it saves (as it does when paused).
//!
//! Nothing here sends anything over the network: the Discord app on the same computer
//! does, and only while it runs.

pub mod activity;
pub mod ipc;
pub mod world;

/// Overrides the Discord application id compiled in.
pub const CLIENT_ID_ENV: &str = "LODESTONE_DISCORD_CLIENT_ID";

/// The Discord application whose name the status shows ("Playing Lodestone
/// Launcher"), and whose art assets it uses: `LODESTONE_DISCORD_CLIENT_ID` at run
/// time, else the one compiled in from the same variable. `None` without either, and
/// the launcher then shows no status.
pub fn client_id() -> Option<String> {
    client_id_from(
        std::env::var(CLIENT_ID_ENV).ok().as_deref(),
        option_env!("LODESTONE_DISCORD_CLIENT_ID"),
    )
}

fn client_id_from(runtime: Option<&str>, built: Option<&str>) -> Option<String> {
    [runtime, built]
        .into_iter()
        .flatten()
        .map(str::trim)
        .find(|id| !id.is_empty() && id.len() <= 32 && id.bytes().all(|b| b.is_ascii_digit()))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_runtime_id_wins_and_ids_are_numbers() {
        assert_eq!(client_id_from(Some("123"), Some("456")), Some("123".into()));
        assert_eq!(client_id_from(None, Some(" 456 ")), Some("456".into()));
        assert_eq!(client_id_from(Some(""), Some("456")), Some("456".into()));
        assert_eq!(client_id_from(Some("abc"), None), None);
        assert_eq!(client_id_from(None, None), None);
    }
}
