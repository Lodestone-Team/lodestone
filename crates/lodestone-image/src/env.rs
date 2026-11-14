//! Which sides a file is for, and why.
//!
//! Every entry has an [`Env`] (one [`EnvValue`] per side) and `env_from`, the inputs that
//! decided it. [`resolve`] is the one place the precedence lives, a pure function of the
//! inputs, so the core and the Launcher agree and views can be recomputed when the rules
//! change. In order of precedence:
//!
//! 1. the owner's "For players" mark ([`OwnerMark`]), client side only;
//! 2. the base pack's `env` for the file;
//! 3. Modrinth's version `environment` ([`modrinth_environment`]);
//! 4. Modrinth's project `client_side` and `server_side`;
//! 5. the jar's own `environment` (`fabric.mod.json` and Quilt's equivalent).
//!
//! Each side is decided on its own: the first input that says something other than
//! `unknown` for that side wins, so a Modrinth project that only knows its client side
//! still lets the jar decide the server side.

use serde::{Deserialize, Serialize};

use crate::strings::open_enum;
use crate::Side;

/// Whether a file is needed on one side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvValue {
    Required,
    Optional,
    Unsupported,
    /// Nothing says. Views treat it as wanted: a file goes to a side unless something
    /// says it is unsupported there.
    Unknown,
}

impl EnvValue {
    /// The value as Modrinth and `.mrpack` indexes write it (`required`, `optional`,
    /// `unsupported`, `unknown`), ignoring case and surrounding spaces. `None` for anything
    /// else, which counts as no input.
    pub fn parse_loose(text: &str) -> Option<EnvValue> {
        match text.trim().to_ascii_lowercase().as_str() {
            "required" => Some(EnvValue::Required),
            "optional" => Some(EnvValue::Optional),
            "unsupported" => Some(EnvValue::Unsupported),
            "unknown" => Some(EnvValue::Unknown),
            _ => None,
        }
    }
}

/// A file's need on each side. An unknown field in it makes the manifest invalid (see
/// [`crate::manifest`]). A side that is missing or `null` is `unknown`, as `null` is a
/// missing field everywhere in a manifest; both sides are always written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Env {
    #[serde(default = "unknown", deserialize_with = "unknown_if_null")]
    pub client: EnvValue,
    #[serde(default = "unknown", deserialize_with = "unknown_if_null")]
    pub server: EnvValue,
}

fn unknown() -> EnvValue {
    EnvValue::Unknown
}

fn unknown_if_null<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<EnvValue, D::Error> {
    Ok(Option::<EnvValue>::deserialize(deserializer)?.unwrap_or(EnvValue::Unknown))
}

impl Default for Env {
    /// Nothing known on either side.
    fn default() -> Self {
        Env::UNKNOWN
    }
}

impl Env {
    pub const UNKNOWN: Env = Env {
        client: EnvValue::Unknown,
        server: EnvValue::Unknown,
    };

    pub fn new(client: EnvValue, server: EnvValue) -> Env {
        Env { client, server }
    }

    /// The value for `side`.
    pub fn get(&self, side: Side) -> EnvValue {
        match side {
            Side::Client => self.client,
            Side::Server => self.server,
        }
    }

    /// Whether the file goes to `side`: anything but `unsupported`.
    pub fn allows(&self, side: Side) -> bool {
        self.get(side) != EnvValue::Unsupported
    }
}

open_enum! {
    /// An input that decided a side of a file's [`Env`]. Written into `env_from`, sorted, so
    /// the UI can say why a file did or didn't go to players.
    pub enum EnvFrom {
        /// The owner's "For players" mark.
        OwnerMark => "owner_mark",
        /// The base pack's `env` for the file.
        BasePack => "base_pack",
        /// Modrinth's version `environment`.
        ModrinthEnvironment => "modrinth_environment",
        /// Modrinth's project `client_side` / `server_side`.
        ModrinthProject => "modrinth_project",
        /// The jar's own `environment`.
        Jar => "jar_environment",
    }
}

/// The owner's mark on a file, for the client side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OwnerMark {
    /// Players need it: forced into the client view (and configs marked like this are
    /// shared with players).
    ForPlayers,
    /// Players must not get it: kept out of the client view.
    NotForPlayers,
}

/// Values for each side where an input has one; `None` is "this input says nothing".
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SideValues {
    pub client: Option<EnvValue>,
    pub server: Option<EnvValue>,
}

impl SideValues {
    pub fn new(client: Option<EnvValue>, server: Option<EnvValue>) -> SideValues {
        SideValues { client, server }
    }

    /// Both sides from strings as Modrinth and `.mrpack` indexes write them; unrecognised
    /// strings say nothing.
    pub fn parse(client: Option<&str>, server: Option<&str>) -> SideValues {
        SideValues {
            client: client.and_then(EnvValue::parse_loose),
            server: server.and_then(EnvValue::parse_loose),
        }
    }

    fn get(&self, side: Side) -> Option<EnvValue> {
        match side {
            Side::Client => self.client,
            Side::Server => self.server,
        }
    }
}

impl From<Env> for SideValues {
    fn from(env: Env) -> SideValues {
        SideValues::new(Some(env.client), Some(env.server))
    }
}

/// Where a jar says it runs (`fabric.mod.json` `environment`, or Quilt's). A jar for both
/// sides (`*`) or one that says nothing is no input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JarEnvironment {
    Client,
    Server,
}

/// Everything that can decide a file's sides. Leave out what isn't known.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct EnvInputs {
    pub owner_mark: Option<OwnerMark>,
    pub base_pack: Option<SideValues>,
    /// Modrinth's version `environment`, already mapped by [`modrinth_environment`].
    pub modrinth_environment: Option<SideValues>,
    pub modrinth_project: Option<SideValues>,
    pub jar: Option<JarEnvironment>,
}

impl EnvInputs {
    /// Each input as (its name, what it says), in order of precedence.
    fn ranked(&self) -> [(EnvFrom, SideValues); 5] {
        let mark = match self.owner_mark {
            Some(OwnerMark::ForPlayers) => SideValues::new(Some(EnvValue::Required), None),
            Some(OwnerMark::NotForPlayers) => SideValues::new(Some(EnvValue::Unsupported), None),
            None => SideValues::default(),
        };
        let jar = match self.jar {
            Some(JarEnvironment::Client) => {
                SideValues::new(Some(EnvValue::Required), Some(EnvValue::Unsupported))
            }
            Some(JarEnvironment::Server) => {
                SideValues::new(Some(EnvValue::Unsupported), Some(EnvValue::Required))
            }
            None => SideValues::default(),
        };
        [
            (EnvFrom::OwnerMark, mark),
            (EnvFrom::BasePack, self.base_pack.unwrap_or_default()),
            (
                EnvFrom::ModrinthEnvironment,
                self.modrinth_environment.unwrap_or_default(),
            ),
            (
                EnvFrom::ModrinthProject,
                self.modrinth_project.unwrap_or_default(),
            ),
            (EnvFrom::Jar, jar),
        ]
    }
}

/// A file's [`Env`] and `env_from` (sorted, without repeats) from its inputs. For each side,
/// the first input in order of precedence that says something other than `unknown` wins;
/// with none, the side is `unknown` and nothing is credited for it.
pub fn resolve(inputs: &EnvInputs) -> (Env, Vec<EnvFrom>) {
    let ranked = inputs.ranked();
    let mut env = Env::UNKNOWN;
    let mut from: Vec<EnvFrom> = Vec::new();
    for side in Side::BOTH {
        let decided = ranked.iter().find_map(|(name, values)| {
            values
                .get(side)
                .filter(|value| *value != EnvValue::Unknown)
                .map(|value| (name, value))
        });
        if let Some((name, value)) = decided {
            match side {
                Side::Client => env.client = value,
                Side::Server => env.server = value,
            }
            from.push(name.clone());
        }
    }
    from.sort();
    from.dedup();
    (env, from)
}

/// Modrinth's version `environment` as per-side values. `None` for values this version
/// doesn't know, which then count as no input.
///
/// `singleplayer_only` mods run in the client's integrated server, never on a dedicated
/// one; `dedicated_server_only` ones never on a client.
pub fn modrinth_environment(text: &str) -> Option<SideValues> {
    use EnvValue::{Optional, Required, Unknown, Unsupported};
    let (client, server) = match text {
        "client_and_server" => (Required, Required),
        "client_only" => (Required, Unsupported),
        "client_only_server_optional" => (Required, Optional),
        "singleplayer_only" => (Required, Unsupported),
        "server_only" => (Unsupported, Required),
        "server_only_client_optional" => (Optional, Required),
        "dedicated_server_only" => (Unsupported, Required),
        "client_or_server" => (Optional, Optional),
        "client_or_server_prefers_both" => (Optional, Optional),
        "unknown" => (Unknown, Unknown),
        _ => return None,
    };
    Some(SideValues::new(Some(client), Some(server)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use EnvValue::{Optional, Required, Unknown, Unsupported};

    fn sides(client: EnvValue, server: EnvValue) -> SideValues {
        SideValues::new(Some(client), Some(server))
    }

    #[test]
    fn a_missing_or_null_side_is_unknown() {
        for (json, expected) in [
            (r#"{}"#, Env::UNKNOWN),
            (r#"{"client":null,"server":null}"#, Env::UNKNOWN),
            (r#"{"server":"required"}"#, Env::new(Unknown, Required)),
            (
                r#"{"client":"optional","server":null}"#,
                Env::new(Optional, Unknown),
            ),
        ] {
            assert_eq!(
                serde_json::from_str::<Env>(json).unwrap(),
                expected,
                "{json}"
            );
        }
        assert!(serde_json::from_str::<Env>(r#"{"client":"x"}"#).is_err());
        assert!(serde_json::from_str::<Env>(r#"{"other":"required"}"#).is_err());
        let written = serde_json::to_string(&Env::new(Unknown, Required)).unwrap();
        assert_eq!(written, r#"{"client":"unknown","server":"required"}"#);
    }

    #[test]
    fn nothing_known_is_unknown() {
        let (env, from) = resolve(&EnvInputs::default());
        assert_eq!(env, Env::UNKNOWN);
        assert!(from.is_empty());
    }

    #[test]
    fn owner_mark_beats_everything_for_the_client_only() {
        let inputs = EnvInputs {
            owner_mark: Some(OwnerMark::ForPlayers),
            base_pack: Some(sides(Unsupported, Required)),
            modrinth_environment: modrinth_environment("server_only"),
            jar: Some(JarEnvironment::Server),
            ..Default::default()
        };
        let (env, from) = resolve(&inputs);
        assert_eq!(env, Env::new(Required, Required));
        assert_eq!(from, [EnvFrom::BasePack, EnvFrom::OwnerMark]);

        let kept_out = EnvInputs {
            owner_mark: Some(OwnerMark::NotForPlayers),
            modrinth_project: Some(sides(Optional, Optional)),
            ..Default::default()
        };
        let (env, from) = resolve(&kept_out);
        assert_eq!(env, Env::new(Unsupported, Optional));
        assert_eq!(from, [EnvFrom::ModrinthProject, EnvFrom::OwnerMark]);
    }

    #[test]
    fn precedence_goes_down_the_list() {
        // Each input on its own decides; with a higher one present, the higher one does.
        let pack = Some(sides(Optional, Required));
        let version = modrinth_environment("client_only");
        let project = Some(sides(Required, Unsupported));
        let jar = Some(JarEnvironment::Server);

        let all = EnvInputs {
            base_pack: pack,
            modrinth_environment: version,
            modrinth_project: project,
            jar,
            ..Default::default()
        };
        assert_eq!(
            resolve(&all),
            (Env::new(Optional, Required), vec![EnvFrom::BasePack])
        );

        let no_pack = EnvInputs {
            base_pack: None,
            ..all
        };
        assert_eq!(
            resolve(&no_pack),
            (
                Env::new(Required, Unsupported),
                vec![EnvFrom::ModrinthEnvironment]
            )
        );

        let only_project_and_jar = EnvInputs {
            modrinth_environment: None,
            ..no_pack
        };
        assert_eq!(
            resolve(&only_project_and_jar),
            (
                Env::new(Required, Unsupported),
                vec![EnvFrom::ModrinthProject]
            )
        );

        let only_jar = EnvInputs {
            modrinth_project: None,
            ..only_project_and_jar
        };
        assert_eq!(
            resolve(&only_jar),
            (Env::new(Unsupported, Required), vec![EnvFrom::Jar])
        );
    }

    #[test]
    fn sides_are_decided_separately_and_unknown_falls_through() {
        let inputs = EnvInputs {
            // The pack only knows the server side; Modrinth's project says unknown for the
            // client, so the jar decides the client.
            base_pack: Some(SideValues::new(None, Some(Optional))),
            modrinth_project: Some(sides(Unknown, Required)),
            jar: Some(JarEnvironment::Client),
            ..Default::default()
        };
        let (env, from) = resolve(&inputs);
        assert_eq!(env, Env::new(Required, Optional));
        assert_eq!(from, [EnvFrom::BasePack, EnvFrom::Jar]);
    }

    #[test]
    fn modrinth_unknown_environment_is_no_input() {
        let inputs = EnvInputs {
            modrinth_environment: modrinth_environment("unknown"),
            jar: Some(JarEnvironment::Client),
            ..Default::default()
        };
        assert_eq!(
            resolve(&inputs),
            (Env::new(Required, Unsupported), vec![EnvFrom::Jar])
        );
        assert_eq!(modrinth_environment("something_new"), None);
    }

    #[test]
    fn modrinth_environment_table() {
        let check = |text: &str, client: EnvValue, server: EnvValue| {
            assert_eq!(
                modrinth_environment(text),
                Some(sides(client, server)),
                "{text}"
            );
        };
        check("client_and_server", Required, Required);
        check("client_only", Required, Unsupported);
        check("client_only_server_optional", Required, Optional);
        check("singleplayer_only", Required, Unsupported);
        check("server_only", Unsupported, Required);
        check("server_only_client_optional", Optional, Required);
        check("dedicated_server_only", Unsupported, Required);
        check("client_or_server", Optional, Optional);
        check("client_or_server_prefers_both", Optional, Optional);
    }

    #[test]
    fn loose_parsing() {
        assert_eq!(EnvValue::parse_loose(" Required "), Some(Required));
        assert_eq!(EnvValue::parse_loose("UNSUPPORTED"), Some(Unsupported));
        assert_eq!(EnvValue::parse_loose("sometimes"), None);
        assert_eq!(
            SideValues::parse(Some("optional"), Some("bogus")),
            SideValues::new(Some(Optional), None)
        );
    }

    #[test]
    fn env_serde_shape() {
        let env = Env::new(Required, Optional);
        assert_eq!(
            serde_json::to_string(&env).unwrap(),
            r#"{"client":"required","server":"optional"}"#
        );
        assert!(serde_json::from_str::<Env>(r#"{"client":"maybe","server":"optional"}"#).is_err());
        let from: Vec<EnvFrom> =
            serde_json::from_str(r#"["modrinth_environment","owner_mark","future_input"]"#)
                .unwrap();
        assert_eq!(from[0], EnvFrom::ModrinthEnvironment);
        assert_eq!(from[2], EnvFrom::Other("future_input".into()));
    }

    #[test]
    fn allows() {
        let env = Env::new(Unsupported, Unknown);
        assert!(!env.allows(Side::Client));
        assert!(env.allows(Side::Server));
    }
}
