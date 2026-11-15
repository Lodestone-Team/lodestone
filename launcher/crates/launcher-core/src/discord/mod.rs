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

/// Lodestone's Discord application: its name is what the status shows ("Playing
/// Lodestone"), and it holds the art assets. Application ids are public (Discord shows
/// them to everyone who sees the status), so it is written here rather than kept in a
/// secret.
pub const LODESTONE_CLIENT_ID: &str = "1571639485601685634";

/// The Discord application the status uses: `LODESTONE_DISCORD_CLIENT_ID` at run time,
/// else one compiled in from the same variable, else [`LODESTONE_CLIENT_ID`].
pub fn client_id() -> Option<String> {
    client_id_from(
        std::env::var(CLIENT_ID_ENV).ok().as_deref(),
        option_env!("LODESTONE_DISCORD_CLIENT_ID"),
    )
}

fn client_id_from(runtime: Option<&str>, built: Option<&str>) -> Option<String> {
    [runtime, built, Some(LODESTONE_CLIENT_ID)]
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
        let lodestone = Some(LODESTONE_CLIENT_ID.to_owned());
        assert_eq!(client_id_from(Some("abc"), None), lodestone);
        assert_eq!(client_id_from(None, None), lodestone);
    }
}
