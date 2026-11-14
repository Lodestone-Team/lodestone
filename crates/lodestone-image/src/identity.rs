//! An image's identity: the SHA-256 of its canonical manifest.
//!
//! The canonical manifest is the image normalized ([`Image::normalize`]: entries sorted by
//! path, sets sorted) and serialized as compact JSON with every object's keys sorted by
//! their UTF-8 bytes and `null` values left out (so a missing optional field and an
//! explicit `null` are the same image). The informational fields are left out: `id`
//! itself, `created`, `label`, `producer`, `parent`, `projected_from`, and each entry's
//! `modified`. So two images with the same files, sources, base, game and settings have the
//! same id, wherever and whenever they were recorded and whatever they were recorded
//! after.
//!
//! Strings are written the way serde_json writes them (only `"`, `\` and control characters
//! escaped; everything else as UTF-8) and integers in decimal, which is the same on every
//! platform.

use serde_json::Value;

use crate::hex::sha256_hex;
use crate::manifest::Image;

/// Top-level fields that are not part of the id.
const INFORMATIONAL: [&str; 6] = [
    "id",
    "created",
    "label",
    "producer",
    "parent",
    "projected_from",
];
/// Entry fields that are not part of the id.
const ENTRY_INFORMATIONAL: [&str; 1] = ["modified"];

/// Why [`Image::verify`] failed.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum IdentityError {
    #[error("the image has no id")]
    Unsealed,
    #[error("the image's id is {claimed}, but its contents hash to {actual}")]
    Mismatch { claimed: String, actual: String },
}

/// The canonical manifest of `image`, the bytes its id is the hash of.
pub fn canonical_bytes(image: &Image) -> Vec<u8> {
    let mut normalized = image.clone();
    normalized.normalize();
    // An image is maps with string keys and plain values: it always becomes a JSON value.
    let mut value = serde_json::to_value(&normalized).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut value {
        for key in INFORMATIONAL {
            map.remove(key);
        }
        if let Some(Value::Array(entries)) = map.get_mut("entries") {
            for entry in entries {
                if let Value::Object(entry) = entry {
                    for key in ENTRY_INFORMATIONAL {
                        entry.remove(key);
                    }
                }
            }
        }
    }
    let mut out = Vec::new();
    write_canonical(&value, &mut out);
    out
}

/// Writes `value` as compact JSON with sorted keys and without `null` object values. Sorts
/// explicitly rather than trusting the map's order, which a serde_json feature turned on
/// elsewhere in the build would change.
fn write_canonical(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(true) => out.extend_from_slice(b"true"),
        Value::Bool(false) => out.extend_from_slice(b"false"),
        Value::Number(n) => out.extend_from_slice(n.to_string().as_bytes()),
        Value::String(s) => write_string(s, out),
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_canonical(item, out);
            }
            out.push(b']');
        }
        Value::Object(map) => {
            let mut fields: Vec<(&String, &Value)> =
                map.iter().filter(|(_, v)| !v.is_null()).collect();
            fields.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            out.push(b'{');
            for (i, (key, item)) in fields.into_iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_string(key, out);
                out.push(b':');
                write_canonical(item, out);
            }
            out.push(b'}');
        }
    }
}

fn write_string(text: &str, out: &mut Vec<u8>) {
    // Writing a string into a Vec can't fail.
    let _ = serde_json::to_writer(&mut *out, text);
}

/// The id `image` should have: the SHA-256 of [`canonical_bytes`].
pub fn compute_id(image: &Image) -> String {
    sha256_hex(&canonical_bytes(image))
}

impl Image {
    /// The canonical manifest; see the module.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        canonical_bytes(self)
    }

    /// The id this image's contents give.
    pub fn compute_id(&self) -> String {
        compute_id(self)
    }

    /// Normalizes the image and sets its id. Call after the last change; any change after
    /// it makes [`Image::verify`] fail until sealed again.
    pub fn seal(&mut self) {
        self.normalize();
        self.id = compute_id(self);
    }

    /// Checks that the id matches the contents: an image from another computer is only
    /// what its id says after this.
    pub fn verify(&self) -> Result<(), IdentityError> {
        if self.id.is_empty() {
            return Err(IdentityError::Unsealed);
        }
        let actual = compute_id(self);
        if actual != self.id {
            return Err(IdentityError::Mismatch {
                claimed: self.id.clone(),
                actual,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::{Env, EnvFrom, EnvValue};
    use crate::manifest::tests::{fabric, object, path};
    use crate::manifest::{App, Kind, Producer, Source, Withheld, WithheldReason, World};
    use crate::settings::{Op, ServerSettings, Settings};

    fn sample() -> Image {
        let mut image = Image::new(Kind::Data, fabric());
        let mut a = object("mods/a.jar");
        a.env = Env::new(EnvValue::Required, EnvValue::Required);
        a.env_from = vec![EnvFrom::ModrinthProject, EnvFrom::BasePack];
        image.entries = vec![object("saves/w/level.dat"), a, object("config/c.toml")];
        image.worlds = vec![
            World {
                name: "w".into(),
                level_name: None,
                data_version: Some(1),
                game_version: None,
                size: None,
                last_played: None,
                quiesced: None,
            },
            World {
                name: "a".into(),
                level_name: None,
                data_version: None,
                game_version: None,
                size: None,
                last_played: None,
                quiesced: None,
            },
        ];
        image.excludes = vec!["z/".into(), "a/".into()];
        image.withheld = vec![
            Withheld {
                path: "b".into(),
                reason: WithheldReason::Symlink,
            },
            Withheld {
                path: "a".into(),
                reason: WithheldReason::Unreadable,
            },
        ];
        image.settings = Settings {
            server: Some(ServerSettings {
                ops: vec![
                    Op {
                        uuid: "2".into(),
                        name: "b".into(),
                        level: 4,
                        bypasses_player_limit: false,
                    },
                    Op {
                        uuid: "1".into(),
                        name: "a".into(),
                        level: 4,
                        bypasses_player_limit: true,
                    },
                ],
                ..Default::default()
            }),
            client: None,
        };
        image
    }

    #[test]
    fn same_content_in_another_order_or_time_has_the_same_id() {
        let mut first = sample();
        first.seal();

        let mut second = sample();
        second.entries.reverse();
        second.worlds.reverse();
        second.excludes.reverse();
        second.withheld.reverse();
        second.settings.server.as_mut().unwrap().ops.reverse();
        for entry in &mut second.entries {
            entry.env_from.reverse();
            entry.modified = Some(123_456_789);
        }
        second.created = 1_900_000_000;
        second.label = Some("Changed by hand".into());
        second.producer = Producer {
            app: App::Launcher,
            version: "9.9.9".into(),
        };
        second.parent = Some("f".repeat(64));
        second.seal();

        assert_eq!(first.id, second.id);
        assert_eq!(first.canonical_bytes(), second.canonical_bytes());
        first.verify().unwrap();
        second.verify().unwrap();
    }

    #[test]
    fn any_content_change_changes_the_id() {
        let mut base = sample();
        base.seal();
        type Change = Box<dyn Fn(&mut Image)>;
        let changes: Vec<Change> = vec![
            Box::new(|i| i.entries[0].size += 1),
            Box::new(|i| i.entries[0].sha256 = "1".repeat(64)),
            Box::new(|i| i.entries[0].mode = 0o755),
            Box::new(|i| i.entries[0].env.client = EnvValue::Unsupported),
            Box::new(|i| i.entries[0].env_from.push(EnvFrom::Jar)),
            Box::new(|i| i.entries[0].source = Source::Base {}),
            Box::new(|i| i.entries[0].sha1 = Some("0".repeat(40))),
            Box::new(|i| i.entries.pop().map(drop).unwrap_or(())),
            Box::new(|i| i.kind = Kind::Full),
            Box::new(|i| i.game.loader_version = Some("0.17.0".into())),
            Box::new(|i| i.removed.push(path("mods/x.jar"))),
            Box::new(|i| i.excludes.push("q/".into())),
            Box::new(|i| i.content = Some("e".repeat(64))),
            Box::new(|i| i.rules = 2),
            Box::new(|i| i.worlds[0].data_version = Some(2)),
            Box::new(|i| i.settings.server.as_mut().unwrap().port = Some(1)),
            Box::new(|i| i.side = Some(crate::Side::Client)),
            Box::new(|i| {
                i.extra.insert("future".into(), serde_json::json!(1));
            }),
        ];
        for (n, change) in changes.iter().enumerate() {
            let mut changed = base.clone();
            change(&mut changed);
            assert_ne!(changed.compute_id(), base.id, "change {n} kept the id");
            assert!(changed.verify().is_err(), "change {n} still verifies");
        }
    }

    #[test]
    fn informational_fields_are_left_out() {
        let mut image = sample();
        image.created = 5;
        image.label = Some("x".into());
        image.parent = Some("a".repeat(64));
        image.projected_from = Some("b".repeat(64));
        image.entries[0].modified = Some(9);
        image.seal();
        let text = String::from_utf8(image.canonical_bytes()).unwrap();
        for key in [
            "\"id\"",
            "\"created\"",
            "\"label\"",
            "\"producer\"",
            "\"parent\"",
            "\"projected_from\"",
            "\"modified\"",
        ] {
            assert!(!text.contains(key), "{key} in {text}");
        }
    }

    #[test]
    fn canonical_form_is_sorted_and_compact() {
        let mut image = Image::new(Kind::Content, fabric());
        image.entries = vec![object("mods/b.jar")];
        let text = String::from_utf8(image.canonical_bytes()).unwrap();
        assert!(!text.contains(' ') && !text.contains('\n'), "{text}");
        // Keys sorted at every level, nulls left out.
        assert!(text.starts_with(r#"{"entries":[{"env":{"client":"unknown","server":"unknown"},"env_from":[],"mode":420,"path":"mods/b.jar","#), "{text}");
        assert!(!text.contains("null"));
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(value["schema"], 2);
    }

    #[test]
    fn known_id_is_stable_across_platforms_and_versions() {
        // A fixed image, its canonical bytes and its id. If this changes, every stored
        // image's id changes: that needs a new schema, not a new expected value.
        let mut image = Image::new(Kind::Content, fabric());
        let mut jar = object("mods/jei.jar");
        jar.env = Env::new(EnvValue::Required, EnvValue::Optional);
        jar.env_from = vec![EnvFrom::ModrinthEnvironment];
        image.entries = vec![jar, object("config/Café 世界.toml")];
        image.seal();
        let config_hash = crate::hex::sha256_hex("config/Café 世界.toml".as_bytes());
        let jar_hash = crate::hex::sha256_hex(b"mods/jei.jar");
        let expected = format!(
            concat!(
                r#"{{"entries":["#,
                r#"{{"env":{{"client":"unknown","server":"unknown"}},"env_from":[],"mode":420,"#,
                r#""path":"config/Café 世界.toml","sha256":"{config}","size":24,"source":{{"object":{{}}}}}},"#,
                r#"{{"env":{{"client":"required","server":"optional"}},"env_from":["modrinth_environment"],"#,
                r#""mode":420,"path":"mods/jei.jar","sha256":"{jar}","size":12,"source":{{"object":{{}}}}}}],"#,
                r#""excludes":[],"game":{{"loader":"fabric","loader_version":"0.16.10","minecraft":"1.21.1"}},"#,
                r#""kind":"content","removed":[],"rules":1,"schema":2,"settings":{{}},"withheld":[],"worlds":[]}}"#
            ),
            config = config_hash,
            jar = jar_hash,
        );
        assert_eq!(
            String::from_utf8(image.canonical_bytes()).unwrap(),
            expected
        );
        assert_eq!(image.id, KNOWN_ID);
    }

    const KNOWN_ID: &str = "a4ee3396b737346cc1009faccbc6ca6f40c08e192fab7512d62bbdadd98f59aa";

    #[test]
    fn verify_errors() {
        let mut image = sample();
        assert_eq!(image.verify(), Err(IdentityError::Unsealed));
        image.seal();
        image.verify().unwrap();
        image.entries[0].size = 99;
        assert!(matches!(
            image.verify(),
            Err(IdentityError::Mismatch { .. })
        ));
    }

    #[test]
    fn null_and_missing_are_the_same() {
        let mut image = sample();
        image.seal();
        let mut value = serde_json::to_value(&image).unwrap();
        value["content"] = serde_json::Value::Null;
        value["base"] = serde_json::Value::Null;
        value.as_object_mut().unwrap().remove("removed");
        let read = Image::from_json(&serde_json::to_vec(&value).unwrap()).unwrap();
        read.verify().unwrap();
    }
}
