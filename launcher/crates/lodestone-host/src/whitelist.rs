//! Putting a friend's Minecraft accounts on a server's whitelist when it's shared with
//! them ([`crate::friends`]): their Launcher lists the accounts in its signed state.
//!
//! Only when the server's `server.properties` has `white-list=true`. The players go into
//! `whitelist.json` (with the UUID the server matches: the account's own in online
//! mode, the offline one otherwise, where offline accounts count too), and a running
//! server is told `whitelist reload`. Sharing never takes anyone off the whitelist.

use launcher_core::{
    cores::{CoreError, admin::AdminClient},
    friend_list::StatePlayer,
    offline::offline_uuid,
};
use serde_json::{Value, json};
use uuid::Uuid;

/// The whitelist file, in the server's folder.
const FILE: &str = "whitelist.json";

/// A server's settings that decide the whitelist, from its `server.properties`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub enabled: bool,
    pub online_mode: bool,
}

impl Settings {
    /// From `server.properties`' text: the whitelist is off and online mode on unless
    /// it says otherwise, as Minecraft's defaults.
    pub fn parse(properties: &str) -> Self {
        let value = |key: &str| {
            properties.lines().find_map(|line| {
                let line = line.trim();
                if line.starts_with('#') {
                    return None;
                }
                let (k, v) = line.split_once('=')?;
                (k.trim() == key).then(|| v.trim().to_ascii_lowercase())
            })
        };
        Self {
            enabled: value("white-list").as_deref() == Some("true"),
            online_mode: value("online-mode").as_deref() != Some("false"),
        }
    }
}

/// The entries `players` add to `current` (a `whitelist.json`'s text, or empty), and the
/// whole new text; `None` when there's nothing to add. In online mode offline accounts
/// can't join and are left out; in offline mode every player goes in with their offline
/// UUID. A player already there, by UUID or name, isn't added again.
pub fn merged(
    current: &str,
    players: &[StatePlayer],
    online_mode: bool,
) -> Option<(Vec<String>, String)> {
    let mut entries: Vec<Value> = if current.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(current).ok()?
    };
    let mut added = Vec::new();
    for player in players {
        if online_mode && player.offline {
            continue;
        }
        let uuid = if online_mode {
            Uuid::from_bytes(player.uuid)
        } else {
            offline_uuid(&player.name)
        };
        let uuid = uuid.hyphenated().to_string();
        let present = entries.iter().any(|entry| {
            entry["uuid"]
                .as_str()
                .is_some_and(|u| u.eq_ignore_ascii_case(&uuid))
                || entry["name"]
                    .as_str()
                    .is_some_and(|n| n.eq_ignore_ascii_case(&player.name))
        });
        if present {
            continue;
        }
        entries.push(json!({ "uuid": uuid, "name": player.name }));
        added.push(player.name.clone());
    }
    if added.is_empty() {
        return None;
    }
    let text = serde_json::to_string_pretty(&entries).ok()?;
    Some((added, text))
}

/// Puts `players` on the whitelist of the server `instance`, if it has one on. Returns
/// the names added (none when the whitelist is off or they're all on it already).
pub async fn add(
    admin: &AdminClient,
    instance: &str,
    players: &[StatePlayer],
) -> Result<Vec<String>, CoreError> {
    if players.is_empty() {
        return Ok(Vec::new());
    }
    let settings = Settings::parse(&admin.read_file(instance, "server.properties").await?);
    if !settings.enabled {
        return Ok(Vec::new());
    }
    // no file yet: an empty whitelist
    let current = admin.read_file(instance, FILE).await.unwrap_or_default();
    let Some((added, text)) = merged(&current, players, settings.online_mode) else {
        return Ok(Vec::new());
    };
    admin.write_file(instance, FILE, &text).await?;
    // a running server keeps the list in memory: read the file again (fails, harmlessly,
    // while it's stopped, which reads it when it starts)
    let _ = admin.send_console(instance, "whitelist reload").await;
    Ok(added)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn player(name: &str, byte: u8, offline: bool) -> StatePlayer {
        StatePlayer {
            name: name.into(),
            uuid: [byte; 16],
            offline,
        }
    }

    #[test]
    fn settings_follow_server_properties() {
        let on = Settings::parse("#comment\nwhite-list=true\nonline-mode=false\n");
        assert_eq!(
            on,
            Settings {
                enabled: true,
                online_mode: false
            }
        );
        let defaults = Settings::parse("motd=A server\n# white-list=true\n");
        assert_eq!(
            defaults,
            Settings {
                enabled: false,
                online_mode: true
            }
        );
    }

    #[test]
    fn players_join_the_whitelist_once() {
        let players = [player("Peter_mc", 0x11, false), player("peter", 0x22, true)];
        // online mode: the Microsoft account with its own UUID, not the offline one
        let (added, text) = merged("", &players, true).unwrap();
        assert_eq!(added, ["Peter_mc"]);
        let list: Vec<Value> = serde_json::from_str(&text).unwrap();
        assert_eq!(list[0]["uuid"], "11111111-1111-1111-1111-111111111111");
        // already there: nothing to add
        assert!(merged(&text, &players, true).is_none());
        // someone else already on it stays, by name too
        let current = r#"[{"uuid":"aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa","name":"PETER_MC"}]"#;
        assert!(merged(current, &players, true).is_none());

        // offline mode: both, with offline UUIDs
        let (added, text) = merged("[]", &players, false).unwrap();
        assert_eq!(added, ["Peter_mc", "peter"]);
        let list: Vec<Value> = serde_json::from_str(&text).unwrap();
        assert_eq!(
            list[1]["uuid"],
            offline_uuid("peter").hyphenated().to_string()
        );
        // a file that isn't a list is left alone
        assert!(merged("{", &players, true).is_none());
    }
}
