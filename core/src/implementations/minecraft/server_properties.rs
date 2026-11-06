//! The known `server.properties` settings and how they map to the configurable manifest.
//!
//! Each setting is one entry in [`PROPERTY_SPECS`]. Values are converted with one generic
//! path: a raw string from the file becomes a typed [`ConfigurableValue`] if it fits the
//! spec, and stays a raw string if it doesn't. Unknown keys are kept as strings too, so no
//! value from the file is ever dropped.

use color_eyre::eyre::eyre;
use indexmap::IndexMap;

use crate::error::{Error, ErrorKind};
use crate::traits::t_configurable::manifest::{
    ConfigurableValue, ConfigurableValueType, SettingManifest,
};

use super::properties::PropertiesDocument;

pub const SECTION_ID: &str = "server_properties_section";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropertyKind {
    Bool,
    /// Inclusive range. Ranges with `min >= 0` are shown as unsigned integers.
    Int {
        min: i64,
        max: i64,
    },
    Enum(&'static [&'static str]),
    String,
    /// A string that the dashboard hides
    Secret,
}

#[derive(Debug, Clone, Copy)]
pub struct PropertySpec {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub kind: PropertyKind,
}

const I32_MAX: i64 = i32::MAX as i64;
const I32_MIN: i64 = i32::MIN as i64;
const PORT: PropertyKind = PropertyKind::Int { min: 1, max: 65535 };

const fn spec(
    key: &'static str,
    name: &'static str,
    description: &'static str,
    kind: PropertyKind,
) -> PropertySpec {
    PropertySpec {
        key,
        name,
        description,
        kind,
    }
}

use PropertyKind::{Bool, Enum, Int, Secret, String as Str};

/// All `server.properties` keys Lodestone knows, in alphabetical order.
#[rustfmt::skip]
pub static PROPERTY_SPECS: &[PropertySpec] = &[
    spec("accepts-transfers", "Accepts Transfers", "Allows players to join through a transfer from another server. If disabled, transferred players are kicked.", Bool),
    spec("allow-flight", "Allow Flight", "Lets players fly in Survival mode if they have a mod that allows flight. If disabled, players who stay in the air for about 5 seconds are kicked.", Bool),
    spec("allow-nether", "Allow Nether", "Allows players to travel to the Nether.", Bool),
    spec("broadcast-console-to-ops", "Broadcast Console To Ops", "Sends the output of console commands to all online operators.", Bool),
    spec("broadcast-rcon-to-ops", "Broadcast Rcon To Ops", "Sends the output of RCON commands to all online operators.", Bool),
    spec("bug-report-link", "Bug Report Link", "A URL that players see on the report-bug screen. Leave blank to use the default.", Str),
    spec("difficulty", "Difficulty", "The difficulty of the world.", Enum(&["peaceful", "easy", "normal", "hard"])),
    spec("enable-command-block", "Enable Command Block", "Allows command blocks to run commands.", Bool),
    spec("enable-jmx-monitoring", "Enable JMX Monitoring", "Exposes server tick-time metrics through Java Management Extensions (JMX).", Bool),
    spec("enable-query", "Enable Query", "Enables the GameSpy4 query protocol, which lets tools get information about the server.", Bool),
    spec("enable-rcon", "Enable Rcon", "Enables remote access to the server console (RCON). Do not expose the RCON port to the internet.", Bool),
    spec("enable-status", "Enable Status", "Makes the server appear as online in the server list. If disabled, players can still join.", Bool),
    spec("enforce-secure-profile", "Enforce Secure Profile", "Requires players to have a Mojang-signed public key to join, so that their chat messages can be reported.", Bool),
    spec("enforce-whitelist", "Enforce Whitelist", "Kicks players who are not on the whitelist when the whitelist is reloaded.", Bool),
    spec("entity-broadcast-range-percentage", "Entity Broadcast Range Percentage", "How close entities must be before they are sent to clients, as a percentage of the default distance.", Int { min: 10, max: 1000 }),
    spec("force-gamemode", "Force Gamemode", "Puts players in the default game mode every time they join.", Bool),
    spec("function-permission-level", "Function Permission Level", "The permission level that functions run with.", Int { min: 1, max: 4 }),
    spec("gamemode", "Gamemode", "The default game mode for new players. Changing it does not change the game mode of players who have already joined.", Enum(&["survival", "creative", "adventure", "spectator"])),
    spec("generate-structures", "Generate Structures", "Generates structures such as villages in new chunks.", Bool),
    spec("generator-settings", "Generator Settings", "Settings for the world generator, as JSON. Used by flat and customized worlds.", Str),
    spec("hardcore", "Hardcore", "Enables hardcore mode: the difficulty is locked to hard and players become spectators when they die.", Bool),
    spec("hide-online-players", "Hide Online Players", "Hides the list of online players from the server status.", Bool),
    spec("initial-disabled-packs", "Initial Disabled Packs", "A comma-separated list of data packs that are not enabled automatically when the world is created.", Str),
    spec("initial-enabled-packs", "Initial Enabled Packs", "A comma-separated list of data packs to enable when the world is created.", Str),
    spec("level-name", "Level Name", "The name of the world folder.", Str),
    spec("level-seed", "Level Seed", "The seed for world generation. Leave blank for a random seed.", Str),
    spec("level-type", "Level Type", "The world preset for a new world, for example minecraft:normal, minecraft:flat or minecraft:large_biomes.", Str),
    spec("log-ips", "Log IPs", "Writes player IP addresses to the server log.", Bool),
    spec("max-build-height", "Max Build Height", "The highest block that players can build at. Used by versions before 1.17.", Int { min: 0, max: I32_MAX }),
    spec("max-chained-neighbor-updates", "Max Chained Neighbor Updates", "The number of consecutive neighbor updates before more updates are skipped. A negative value removes the limit.", Int { min: I32_MIN, max: I32_MAX }),
    spec("max-players", "Max Players", "The maximum number of players that can be on the server at the same time.", Int { min: 0, max: I32_MAX }),
    spec("max-tick-time", "Max Tick Time", "The longest time, in milliseconds, that a single tick may take before the watchdog stops the server. -1 disables the watchdog.", Int { min: -1, max: I32_MAX }),
    spec("max-world-size", "Max World Size", "The maximum radius of the world border, in blocks.", Int { min: 1, max: 29_999_984 }),
    spec("motd", "Motd", "The message shown under the server name in the server list.", Str),
    spec("network-compression-threshold", "Network Compression Threshold", "Packets of this size in bytes or larger are compressed. -1 disables compression and 0 compresses every packet.", Int { min: -1, max: I32_MAX }),
    spec("online-mode", "Online Mode", "Checks each player's account with Mojang. Disable it only behind a proxy that checks accounts, because anyone can then join with any name.", Bool),
    spec("op-permission-level", "Op Permission Level", "The default permission level for operators.", Int { min: 0, max: 4 }),
    spec("pause-when-empty-seconds", "Pause When Empty Seconds", "The number of seconds without players after which the server pauses. 0 or a negative value disables pausing.", Int { min: I32_MIN, max: I32_MAX }),
    spec("player-idle-timeout", "Player Idle Timeout", "Players who are idle for this many minutes are kicked. 0 disables the timeout.", Int { min: 0, max: I32_MAX }),
    spec("prevent-proxy-connections", "Prevent Proxy Connections", "Kicks players whose connection does not come from the network that Mojang's authentication servers saw. This blocks most VPNs and proxies.", Bool),
    spec("previews-chat", "Previews Chat", "Lets the server show players a preview of their chat message before it is sent. Used by versions 1.19 to 1.19.2.", Bool),
    spec("pvp", "Pvp", "Allows players to damage each other.", Bool),
    spec("query.port", "Query Port", "The port of the GameSpy4 query listener.", PORT),
    spec("rate-limit", "Rate Limit", "The number of packets a player may send per second before they are kicked. 0 disables the limit.", Int { min: 0, max: I32_MAX }),
    spec("rcon.password", "Rcon Password", "The password for RCON connections.", Secret),
    spec("rcon.port", "Rcon Port", "The port for RCON connections.", PORT),
    spec("region-file-compression", "Region File Compression", "The compression used for region files.", Enum(&["deflate", "lz4", "none"])),
    spec("require-resource-pack", "Require Resource Pack", "Kicks players who decline the server resource pack.", Bool),
    spec("resource-pack", "Resource Pack", "A URL to a resource pack that players are asked to download.", Str),
    spec("resource-pack-id", "Resource Pack Id", "The UUID of the server resource pack. Clients use it to cache the pack.", Str),
    spec("resource-pack-prompt", "Resource Pack Prompt", "A custom message, as a JSON text component, shown when players are asked to download the resource pack.", Str),
    spec("resource-pack-sha1", "Resource Pack Sha1", "The SHA-1 hash of the resource pack, used to check the download.", Str),
    spec("server-ip", "Server Ip", "The IP address the server listens on. Leave blank to listen on all addresses.", Str),
    spec("server-port", "Server Port", "The port the server listens on.", PORT),
    spec("simulation-distance", "Simulation Distance", "The distance, in chunks, around each player within which entities and blocks are updated.", Int { min: 3, max: 32 }),
    spec("snooper-enabled", "Snooper Enabled", "Sends usage data to Mojang. Used by versions before 1.18.", Bool),
    spec("spawn-animals", "Spawn Animals", "Allows animals to spawn.", Bool),
    spec("spawn-monsters", "Spawn Monsters", "Allows hostile mobs to spawn.", Bool),
    spec("spawn-npcs", "Spawn Npcs", "Allows villagers to spawn.", Bool),
    spec("spawn-protection", "Spawn Protection", "The radius, in blocks, of the area around the world spawn that only operators can change. 0 disables spawn protection.", Int { min: 0, max: I32_MAX }),
    spec("sync-chunk-writes", "Sync Chunk Writes", "Writes chunk files synchronously. This is safer but slower.", Bool),
    spec("text-filtering-config", "Text Filtering Config", "The configuration of the chat text filtering service.", Str),
    spec("use-native-transport", "Use Native Transport", "Uses optimized Linux networking (epoll) when it is available.", Bool),
    spec("view-distance", "View Distance", "The distance, in chunks, of world data that the server sends to clients.", Int { min: 2, max: 32 }),
    spec("white-list", "White List", "Allows only players on the whitelist to join.", Bool),
];

pub fn spec_for(key: &str) -> Option<&'static PropertySpec> {
    PROPERTY_SPECS.iter().find(|spec| spec.key == key)
}

fn bad_request(message: String) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message),
    }
}

fn parse_bool(raw: &str) -> Option<bool> {
    if raw.eq_ignore_ascii_case("true") {
        Some(true)
    } else if raw.eq_ignore_ascii_case("false") {
        Some(false)
    } else {
        None
    }
}

impl PropertyKind {
    fn describe(&self) -> String {
        match self {
            Bool => "true or false".to_string(),
            Int { min, max } => format!("a whole number from {min} to {max}"),
            Enum(options) => format!("one of {}", options.join(", ")),
            Str | Secret => "text".to_string(),
        }
    }

    fn value_type(&self) -> ConfigurableValueType {
        match *self {
            Bool => ConfigurableValueType::Boolean,
            Int { min, max } if min >= 0 => ConfigurableValueType::UnsignedInteger {
                min: u32::try_from(min).ok(),
                max: u32::try_from(max).ok(),
            },
            Int { min, max } => ConfigurableValueType::Integer {
                min: i32::try_from(min).ok(),
                max: i32::try_from(max).ok(),
            },
            Enum(options) => ConfigurableValueType::Enum {
                options: options.iter().map(|o| o.to_string()).collect(),
            },
            Str | Secret => ConfigurableValueType::String { regex: None },
        }
    }

    /// The typed value for a raw string from the file, or `None` if it does not fit.
    fn typed_value(&self, raw: &str) -> Option<ConfigurableValue> {
        match *self {
            Bool => parse_bool(raw).map(ConfigurableValue::Boolean),
            Int { min, max } => {
                let n: i64 = raw.parse().ok()?;
                if n < min || n > max {
                    return None;
                }
                if min >= 0 {
                    u32::try_from(n)
                        .ok()
                        .map(ConfigurableValue::UnsignedInteger)
                } else {
                    i32::try_from(n).ok().map(ConfigurableValue::Integer)
                }
            }
            Enum(options) => options
                .contains(&raw)
                .then(|| ConfigurableValue::Enum(raw.to_string())),
            Str | Secret => Some(ConfigurableValue::String(raw.to_string())),
        }
    }

    /// The raw string to write for a value sent by a client. Strings are accepted for every
    /// kind as long as they parse, so a setting shown as text (because its current value did
    /// not fit) can be changed back to a valid value.
    fn serialize(&self, key: &str, value: &ConfigurableValue) -> Result<String, Error> {
        let invalid = || {
            bad_request(format!(
                "Invalid value for \"{key}\": expected {}",
                self.describe()
            ))
        };
        let raw = match (*self, value) {
            (Bool, ConfigurableValue::Boolean(b)) => b.to_string(),
            (Bool, ConfigurableValue::String(s)) => {
                parse_bool(s.trim()).ok_or_else(invalid)?.to_string()
            }
            (Int { .. }, ConfigurableValue::Integer(n)) => n.to_string(),
            (Int { .. }, ConfigurableValue::UnsignedInteger(n)) => n.to_string(),
            (Int { .. }, ConfigurableValue::String(s)) => {
                s.trim().parse::<i64>().map_err(|_| invalid())?.to_string()
            }
            (Enum(_), ConfigurableValue::Enum(s) | ConfigurableValue::String(s)) => s.clone(),
            (Str | Secret, ConfigurableValue::String(s)) => s.clone(),
            _ => return Err(invalid()),
        };
        if self.typed_value(&raw).is_none() {
            return Err(invalid());
        }
        Ok(raw)
    }
}

/// The value to write to `server.properties` for `key`, checked against its spec.
pub fn serialize_value(key: &str, value: &ConfigurableValue) -> Result<String, Error> {
    match spec_for(key) {
        Some(spec) => spec.kind.serialize(key, value),
        None => match value {
            ConfigurableValue::String(s) | ConfigurableValue::Enum(s) => Ok(s.clone()),
            ConfigurableValue::Boolean(b) => Ok(b.to_string()),
            ConfigurableValue::Integer(n) => Ok(n.to_string()),
            ConfigurableValue::UnsignedInteger(n) => Ok(n.to_string()),
            ConfigurableValue::Float(_) => Err(bad_request(format!(
                "Invalid value for \"{key}\": floats are not supported"
            ))),
        },
    }
}

fn display_name_for_unknown(key: &str) -> String {
    let mut chars = key.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// The manifest entry for one property. The setting id is the property key.
pub fn setting_manifest(key: &str, raw: &str) -> SettingManifest {
    let Some(spec) = spec_for(key) else {
        let lower = key.to_ascii_lowercase();
        let is_secret = lower.contains("password") || lower.contains("secret");
        return SettingManifest::new_required_value(
            key.to_string(),
            display_name_for_unknown(key),
            "Lodestone has no description for this property. Its value is kept as it is."
                .to_string(),
            ConfigurableValue::String(raw.to_string()),
            None,
            is_secret,
            true,
        );
    };
    let is_secret = spec.kind == Secret;
    match spec.kind.typed_value(raw) {
        Some(value) => SettingManifest::new_value_with_type(
            key.to_string(),
            spec.name.to_string(),
            spec.description.to_string(),
            Some(value),
            spec.kind.value_type(),
            None,
            is_secret,
            true,
        ),
        // Keep values that don't fit (e.g. a legacy numeric gamemode) as text.
        None => SettingManifest::new_required_value(
            key.to_string(),
            spec.name.to_string(),
            format!(
                "{} The current value \"{raw}\" is not {}, so it is shown as text.",
                spec.description,
                spec.kind.describe()
            ),
            ConfigurableValue::String(raw.to_string()),
            None,
            is_secret,
            true,
        ),
    }
}

/// The manifest section settings for a parsed `server.properties`, in file order.
pub fn section_settings(doc: &PropertiesDocument) -> IndexMap<String, SettingManifest> {
    doc.entries()
        .into_iter()
        .map(|(key, raw)| {
            let setting = setting_manifest(&key, &raw);
            (key, setting)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const VANILLA: &str = include_str!("testdata/vanilla_server.properties");

    fn value_of(settings: &IndexMap<String, SettingManifest>, key: &str) -> ConfigurableValue {
        settings[key].get_value().unwrap().clone()
    }

    #[test]
    fn specs_are_unique_and_consistent() {
        for (i, spec) in PROPERTY_SPECS.iter().enumerate() {
            assert!(
                PROPERTY_SPECS[i + 1..].iter().all(|s| s.key != spec.key),
                "duplicate spec {}",
                spec.key
            );
            if let Int { min, max } = spec.kind {
                assert!(min <= max, "{}", spec.key);
                assert!(min >= I32_MIN && max <= I32_MAX, "{}", spec.key);
            }
        }
    }

    #[test]
    fn vanilla_file_round_trips_unchanged() {
        let (doc, warnings) = PropertiesDocument::parse(VANILLA);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(doc.to_text(), VANILLA);

        let settings = section_settings(&doc);
        assert_eq!(settings.len(), doc.entries().len());
        // every key in a current vanilla file is known and its value fits
        for (key, setting) in &settings {
            if key == "text-filtering-version" {
                continue;
            }
            let spec = spec_for(key).unwrap_or_else(|| panic!("no spec for {key}"));
            assert!(
                spec.kind.typed_value(&doc.entries()[key]).is_some(),
                "{key} does not fit its spec"
            );
            assert_eq!(setting.get_identifier(), key);
        }
        assert_eq!(
            value_of(&settings, "level-type"),
            ConfigurableValue::String("minecraft:normal".to_string())
        );
        assert_eq!(
            value_of(&settings, "server-port"),
            ConfigurableValue::UnsignedInteger(25565)
        );
        assert_eq!(
            value_of(&settings, "gamemode"),
            ConfigurableValue::Enum("survival".to_string())
        );
        assert_eq!(
            value_of(&settings, "max-tick-time"),
            ConfigurableValue::Integer(60000)
        );
        assert_eq!(
            value_of(&settings, "online-mode"),
            ConfigurableValue::Boolean(true)
        );
    }

    #[test]
    fn changing_one_setting_changes_only_its_line() {
        let (mut doc, _) = PropertiesDocument::parse(VANILLA);
        let raw = serialize_value("motd", &ConfigurableValue::String("Hi a=b".into())).unwrap();
        doc.set("motd", &raw);
        let text = doc.to_text();
        let changed: Vec<_> = VANILLA
            .lines()
            .zip(text.lines())
            .filter(|(a, b)| a != b)
            .collect();
        assert_eq!(changed, vec![("motd=A Minecraft Server", "motd=Hi a\\=b")]);
        assert_eq!(VANILLA.lines().count(), text.lines().count());
        let (reparsed, _) = PropertiesDocument::parse(&text);
        assert_eq!(reparsed.get("motd"), Some("Hi a=b"));
    }

    #[test]
    fn values_that_do_not_fit_are_kept_as_text() {
        let text = "max-tick-time=-1\nnetwork-compression-threshold=-1\ngamemode=0\ndifficulty=1\nserver-port=70000\nmax-chained-neighbor-updates=-5\nmy-plugin-key=a=b\npvp=yes\n";
        let (doc, _) = PropertiesDocument::parse(text);
        let settings = section_settings(&doc);
        assert_eq!(settings.len(), 8);
        assert_eq!(
            value_of(&settings, "max-tick-time"),
            ConfigurableValue::Integer(-1)
        );
        assert_eq!(
            value_of(&settings, "network-compression-threshold"),
            ConfigurableValue::Integer(-1)
        );
        assert_eq!(
            value_of(&settings, "max-chained-neighbor-updates"),
            ConfigurableValue::Integer(-5)
        );
        for (key, raw) in [
            ("gamemode", "0"),
            ("difficulty", "1"),
            ("server-port", "70000"),
            ("my-plugin-key", "a=b"),
            ("pvp", "yes"),
        ] {
            assert_eq!(
                value_of(&settings, key),
                ConfigurableValue::String(raw.to_string()),
                "{key}"
            );
        }
        // nothing changes on disk when the manifest is built
        assert_eq!(doc.to_text(), text);
    }

    #[test]
    fn serialize_checks_the_spec() {
        use ConfigurableValue as V;
        assert_eq!(serialize_value("pvp", &V::Boolean(false)).unwrap(), "false");
        assert_eq!(
            serialize_value("pvp", &V::String("TRUE".into())).unwrap(),
            "true"
        );
        assert!(serialize_value("pvp", &V::String("yes".into())).is_err());
        assert_eq!(
            serialize_value("server-port", &V::UnsignedInteger(25570)).unwrap(),
            "25570"
        );
        assert!(serialize_value("server-port", &V::UnsignedInteger(70000)).is_err());
        assert!(serialize_value("server-port", &V::UnsignedInteger(0)).is_err());
        assert_eq!(
            serialize_value("max-tick-time", &V::Integer(-1)).unwrap(),
            "-1"
        );
        assert_eq!(
            serialize_value("max-tick-time", &V::UnsignedInteger(1000)).unwrap(),
            "1000"
        );
        assert!(serialize_value("max-tick-time", &V::Integer(-2)).is_err());
        assert_eq!(
            serialize_value("gamemode", &V::Enum("creative".into())).unwrap(),
            "creative"
        );
        assert_eq!(
            serialize_value("gamemode", &V::String("creative".into())).unwrap(),
            "creative"
        );
        assert!(serialize_value("gamemode", &V::Enum("god".into())).is_err());
        assert!(serialize_value("motd", &V::Boolean(true)).is_err());
        assert_eq!(
            serialize_value("unknown-key", &V::String("x=y".into())).unwrap(),
            "x=y"
        );
    }

    #[test]
    fn unknown_and_secret_settings() {
        let (doc, _) = PropertiesDocument::parse(
            "rcon.password=hunter2\nmanagement-server-secret=abc\nfoo=bar\n",
        );
        let settings = section_settings(&doc);
        let json = serde_json::to_value(&settings).unwrap();
        assert_eq!(json["rcon.password"]["is_secret"], true);
        assert_eq!(json["management-server-secret"]["is_secret"], true);
        assert_eq!(json["foo"]["is_secret"], false);
        assert_eq!(json["foo"]["name"], "Foo");
        assert_eq!(json["foo"]["value_type"]["type"], "String");
    }

    #[test]
    fn manifest_json_shape_is_unchanged() {
        let (doc, _) = PropertiesDocument::parse("pvp=true\nmax-players=20\ngamemode=creative\n");
        let json = serde_json::to_value(section_settings(&doc)).unwrap();
        assert_eq!(
            json["pvp"],
            serde_json::json!({
                "setting_id": "pvp",
                "name": "Pvp",
                "description": "Allows players to damage each other.",
                "value": {"type": "Boolean", "value": true},
                "value_type": {"type": "Boolean"},
                "default_value": null,
                "is_secret": false,
                "is_required": true,
                "is_mutable": true
            })
        );
        assert_eq!(
            json["max-players"]["value"],
            serde_json::json!({"type": "UnsignedInteger", "value": 20})
        );
        assert_eq!(
            json["max-players"]["value_type"],
            serde_json::json!({"type": "UnsignedInteger", "min": 0, "max": i32::MAX})
        );
        assert_eq!(
            json["gamemode"]["value_type"]["options"],
            serde_json::json!(["survival", "creative", "adventure", "spectator"])
        );
    }
}
