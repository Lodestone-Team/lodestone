//! Where the player is, from the game's log: starting, in the menus, in a
//! single-player world (and whether it is open to LAN), or on a server.
//!
//! [`GameActivity::observe`] runs on every line the game prints, on the window's
//! thread, so it only looks at the start of a message, and most lines are passed over
//! on their first byte.

/// Where the player is.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Place {
    /// The game is loading.
    #[default]
    Starting,
    /// The title screen and the other menus outside a world.
    Menu,
    /// A single-player world: the integrated server runs. `lan` once it is open to
    /// LAN.
    Singleplayer { lan: bool },
    /// A server, as the player typed its address.
    Multiplayer { host: String, port: u16 },
}

/// What the log says about the game.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GameActivity {
    pub place: Place,
    /// Goes up each time the single-player world saves (as it does when the game
    /// pauses), so the world's files are read again.
    pub saves: u64,
}

impl GameActivity {
    /// The game finished loading: from starting to the menus. Whether anything changed.
    pub fn reached_menu(&mut self) -> bool {
        if self.place == Place::Starting {
            self.place = Place::Menu;
            true
        } else {
            false
        }
    }

    /// Takes in one log message. Whether anything changed.
    pub fn observe(&mut self, message: &str) -> bool {
        match message.as_bytes().first() {
            Some(b'S' | b'C' | b'D') => {}
            _ => return false,
        }
        if message.starts_with("Starting integrated minecraft server") {
            return self.set(Place::Singleplayer { lan: false });
        }
        if message.starts_with("Saving and pausing game") {
            if matches!(self.place, Place::Singleplayer { .. }) {
                self.saves += 1;
                return true;
            }
            return false;
        }
        if message.starts_with("Started serving on") {
            if let Place::Singleplayer { lan: false } = self.place {
                return self.set(Place::Singleplayer { lan: true });
            }
            return false;
        }
        if message == "Stopping server" || message.starts_with("Stopping singleplayer server") {
            if matches!(self.place, Place::Singleplayer { .. }) {
                return self.set(Place::Menu);
            }
            return false;
        }
        if let Some(target) = message.strip_prefix("Connecting to ") {
            // "Connecting to mc.example.com, 25565": other mods connect to things too,
            // so the port must be there.
            if let Some((host, port)) = target.rsplit_once(", ")
                && let Ok(port) = port.trim().parse::<u16>()
                && !host.is_empty()
                && !host.contains(char::is_whitespace)
            {
                return self.set(Place::Multiplayer {
                    host: host.to_owned(),
                    port,
                });
            }
            return false;
        }
        if message.starts_with("Client disconnected") || message.starts_with("Disconnected from") {
            if matches!(self.place, Place::Multiplayer { .. }) {
                return self.set(Place::Menu);
            }
            return false;
        }
        false
    }

    fn set(&mut self, place: Place) -> bool {
        if self.place == place {
            return false;
        }
        self.place = place;
        true
    }
}

/// The part of a server address that can be shown: a host name, without the default
/// port. `None` for an IP address or a name that only means this computer or network
/// (a friend's address, a Lodestone Connect tunnel), which say where someone is.
pub fn public_server_name(host: &str, port: u16) -> Option<String> {
    let host = host.trim().trim_end_matches('.');
    let bare = host.trim_start_matches('[').trim_end_matches(']');
    if bare.parse::<std::net::IpAddr>().is_ok() {
        return None;
    }
    let lower = bare.to_ascii_lowercase();
    if !lower.contains('.')
        || lower == "localhost"
        || [".local", ".lan", ".home", ".internal", ".localhost"]
            .iter()
            .any(|suffix| lower.ends_with(suffix))
    {
        return None;
    }
    if !lower
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-' || b == b'_')
    {
        return None;
    }
    Some(if port == 25565 {
        lower
    } else {
        format!("{lower}:{port}")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(lines: &[&str]) -> GameActivity {
        let mut activity = GameActivity::default();
        for line in lines {
            activity.observe(line);
        }
        activity
    }

    #[test]
    fn follows_single_player() {
        let mut activity = GameActivity::default();
        assert!(activity.reached_menu());
        assert!(!activity.reached_menu());
        assert!(activity.observe("Starting integrated minecraft server version 1.21.1"));
        assert_eq!(activity.place, Place::Singleplayer { lan: false });
        assert!(activity.observe("Saving and pausing game..."));
        assert_eq!(activity.saves, 1);
        assert!(activity.observe("Started serving on 51234"));
        assert_eq!(activity.place, Place::Singleplayer { lan: true });
        assert!(activity.observe("Stopping server"));
        assert_eq!(activity.place, Place::Menu);
        assert!(
            !activity.observe("Saving and pausing game..."),
            "not in a world"
        );
    }

    #[test]
    fn follows_servers() {
        let activity = feed(&["Connecting to mc.hypixel.net, 25565"]);
        assert_eq!(
            activity.place,
            Place::Multiplayer {
                host: "mc.hypixel.net".into(),
                port: 25565
            }
        );
        let activity = feed(&[
            "Connecting to mc.hypixel.net, 25565",
            "Client disconnected with reason: Disconnected",
        ]);
        assert_eq!(activity.place, Place::Menu);
    }

    #[test]
    fn passes_over_other_lines() {
        let activity = feed(&[
            "Connecting to the Modrinth API",
            "Setting user: Steve",
            "Stopping server",
            "[CHAT] Hello",
        ]);
        assert_eq!(activity, GameActivity::default());
    }

    #[test]
    fn shows_only_public_server_names() {
        assert_eq!(
            public_server_name("Play.Example.com", 25565),
            Some("play.example.com".into())
        );
        assert_eq!(
            public_server_name("play.example.com", 25570),
            Some("play.example.com:25570".into())
        );
        assert_eq!(public_server_name("192.168.1.20", 25565), None);
        assert_eq!(public_server_name("[::1]", 25565), None);
        assert_eq!(public_server_name("localhost", 25565), None);
        assert_eq!(public_server_name("minecraft", 25565), None);
        assert_eq!(public_server_name("server.local", 25565), None);
    }
}
