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

//!
//! The canonical form is written straight into the hasher, one item of each large list
//! (entries, removed paths, withheld files) at a time, and the image isn't copied when it
//! is already normalized (as a read or sealed image is): hashing a manifest of a million
//! entries or withheld files costs memory for one item and a pointer per item, not for the
//! manifest. Only the settings are turned into one JSON value whole; their lists are capped
//! far lower (100,000 items).

use std::cmp::Ordering;
use std::io::Write;

use serde::Serialize;
use serde_json::Value;
use crate::sha256::Sha256;

use crate::hex::lower_hex;
use crate::manifest::{Entry, Image, Withheld, ENTRY_FIELDS, IMAGE_FIELDS};

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
    let mut out = Vec::new();
    write_image(image, &mut out);
    out
}

/// The id `image` should have: the SHA-256 of [`canonical_bytes`], hashed as it is written.
pub fn compute_id(image: &Image) -> String {
    let mut hasher = Hashing(Sha256::new());
    write_image(image, &mut hasher);
    lower_hex(&hasher.0.finalize())
}

/// A hasher that takes writes.
struct Hashing(Sha256);

impl Write for Hashing {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// One top-level field of the canonical form.
enum Field<'a> {
    /// A small field, as a JSON value.
    Value(Value),
    /// A borrowed unknown field.
    Extra(&'a Value),
    /// A list written one item at a time, in order.
    Entries(Vec<&'a Entry>),
    Strings(Vec<&'a str>),
    Withheld(Vec<&'a Withheld>),
}

/// `value` as JSON. An image is maps with string keys and plain values: it always becomes
/// one.
fn json<T: Serialize + ?Sized>(value: &T) -> Value {
    serde_json::to_value(value).unwrap_or(Value::Null)
}

/// `items` sorted (stably) by `order` if they aren't already, and with repeats dropped when
/// `dedup`: as [`Image::normalize`] leaves them, without copying the items.
fn normalized<T: PartialEq>(
    items: &[T],
    order: impl Fn(&T, &T) -> Ordering,
    dedup: bool,
) -> Vec<&T> {
    let mut out: Vec<&T> = items.iter().collect();
    if !items.is_sorted_by(|a, b| order(a, b).is_le()) {
        out.sort_by(|a, b| order(a, b));
    }
    if dedup {
        out.dedup_by(|a, b| a == b);
    }
    out
}

/// Writes the canonical form of `image`, normalized as [`Image::normalize`] would, without
/// copying it.
fn write_image<W: Write>(image: &Image, out: &mut W) {
    let mut settings = image.settings.clone();
    settings.normalize();
    let mut worlds: Vec<_> = image.worlds.iter().collect();
    worlds.sort_by(|a, b| a.name.cmp(&b.name));
    let mut excludes: Vec<&String> = image.excludes.iter().collect();
    excludes.sort();
    excludes.dedup();

    let mut fields: Vec<(&str, Field)> = vec![
        ("schema", Field::Value(json(&image.schema))),
        ("kind", Field::Value(json(&image.kind))),
        ("base", Field::Value(json(&image.base))),
        ("game", Field::Value(json(&image.game))),
        ("settings", Field::Value(json(&settings))),
        (
            "entries",
            Field::Entries(normalized(
                &image.entries,
                |a, b| a.path.cmp(&b.path),
                false,
            )),
        ),
        (
            "removed",
            Field::Strings(
                normalized(&image.removed, Ord::cmp, true)
                    .into_iter()
                    .map(|p| p.as_str())
                    .collect(),
            ),
        ),
        ("worlds", Field::Value(json(&worlds))),
        ("content", Field::Value(json(&image.content))),
        ("excludes", Field::Value(json(&excludes))),
        (
            "withheld",
            Field::Withheld(normalized(&image.withheld, Ord::cmp, true)),
        ),
        ("rules", Field::Value(json(&image.rules))),
        ("side", Field::Value(json(&image.side))),
    ];
    debug_assert!(fields.iter().all(|(name, _)| IMAGE_FIELDS.contains(name)));
    debug_assert_eq!(fields.len() + INFORMATIONAL.len(), IMAGE_FIELDS.len());
    // An unknown field named after a known one is invalid ([`Image::validate`]); the known
    // field wins here, so the id always hashes the image's own fields.
    for (key, value) in &image.extra {
        if !IMAGE_FIELDS.contains(&key.as_str()) {
            fields.push((key, Field::Extra(value)));
        }
    }
    fields.retain(|(_, field)| {
        !matches!(field, Field::Value(Value::Null) | Field::Extra(Value::Null))
    });
    fields.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));

    let _ = out.write_all(b"{");
    for (i, (key, field)) in fields.iter().enumerate() {
        if i > 0 {
            let _ = out.write_all(b",");
        }
        write_string(key, out);
        let _ = out.write_all(b":");
        match field {
            Field::Value(value) => write_canonical(value, out),
            Field::Extra(value) => write_canonical(value, out),
            Field::Entries(entries) => write_list(entries, out, |entry, out| {
                write_entry(entry, out);
            }),
            Field::Strings(items) => write_list(items, out, |item, out| write_string(item, out)),
            Field::Withheld(items) => write_list(items, out, |item, out| write_withheld(item, out)),
        }
    }
    let _ = out.write_all(b"}");
}

fn write_list<T, W: Write>(items: &[T], out: &mut W, mut each: impl FnMut(&T, &mut W)) {
    let _ = out.write_all(b"[");
    for (i, item) in items.iter().enumerate() {
        if i > 0 {
            let _ = out.write_all(b",");
        }
        each(item, out);
    }
    let _ = out.write_all(b"]");
}

/// One withheld file, `{"path":…,"reason":…}` (its keys in sorted order), written without
/// building a JSON value.
fn write_withheld<W: Write>(withheld: &Withheld, out: &mut W) {
    let _ = out.write_all(b"{\"path\":");
    write_string(&withheld.path, out);
    let _ = out.write_all(b",\"reason\":");
    write_string(withheld.reason.as_str(), out);
    let _ = out.write_all(b"}");
}

/// One entry, normalized and without its informational fields, built as a JSON value only
/// while it is written.
///
/// An unknown field named after a known one is invalid ([`Image::validate`]), but an image
/// built in code can still have one; flattened, it would replace the known field, so it is
/// dropped here and the id always hashes the entry's own fields, as for the image's.
fn write_entry<W: Write>(entry: &Entry, out: &mut W) {
    let sorted = entry.env_from.windows(2).all(|w| w[0] < w[1]);
    let shadowing = entry
        .extra
        .keys()
        .any(|key| ENTRY_FIELDS.contains(&key.as_str()));
    let mut value = if sorted && !shadowing {
        json(entry)
    } else {
        let mut entry = entry.clone();
        entry.env_from.sort();
        entry.env_from.dedup();
        entry
            .extra
            .retain(|key, _| !ENTRY_FIELDS.contains(&key.as_str()));
        json(&entry)
    };
    if let Value::Object(map) = &mut value {
        for key in ENTRY_INFORMATIONAL {
            map.remove(key);
        }
    }
    write_canonical(&value, out);
}

/// Writes `value` as compact JSON with sorted keys and without `null` object values. Sorts
/// explicitly rather than trusting the map's order, which a serde_json feature turned on
/// elsewhere in the build would change.
fn write_canonical<W: Write>(value: &Value, out: &mut W) {
    match value {
        Value::Null => {
            let _ = out.write_all(b"null");
        }
        Value::Bool(true) => {
            let _ = out.write_all(b"true");
        }
        Value::Bool(false) => {
            let _ = out.write_all(b"false");
        }
        Value::Number(n) => {
            let _ = write!(out, "{n}");
        }
        Value::String(s) => write_string(s, out),
        Value::Array(items) => write_list(items, out, |item, out| write_canonical(item, out)),
        Value::Object(map) => {
            let mut fields: Vec<(&String, &Value)> =
                map.iter().filter(|(_, v)| !v.is_null()).collect();
            fields.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
            let _ = out.write_all(b"{");
            for (i, (key, item)) in fields.into_iter().enumerate() {
                if i > 0 {
                    let _ = out.write_all(b",");
                }
                write_string(key, out);
                let _ = out.write_all(b":");
                write_canonical(item, out);
            }
            let _ = out.write_all(b"}");
        }
    }
}

fn write_string<W: Write>(text: &str, out: &mut W) {
    // Writing a string into a Vec or a hasher can't fail.
    let _ = serde_json::to_writer(&mut *out, text);
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
    use crate::hex::sha256_hex;
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

    /// The canonical form the plain way: the whole image normalized, as one JSON value.
    fn reference(image: &Image) -> Vec<u8> {
        let mut normalized = image.clone();
        normalized.normalize();
        let mut value = serde_json::to_value(&normalized).unwrap();
        let map = value.as_object_mut().unwrap();
        for key in INFORMATIONAL {
            map.remove(key);
        }
        for entry in map["entries"].as_array_mut().unwrap() {
            for key in ENTRY_INFORMATIONAL {
                entry.as_object_mut().unwrap().remove(key);
            }
        }
        let mut out = Vec::new();
        write_canonical(&value, &mut out);
        out
    }

    #[test]
    fn streamed_form_is_the_plain_one() {
        let mut full = sample();
        full.entries.reverse();
        full.entries[0].env_from = vec![EnvFrom::Jar, EnvFrom::BasePack, EnvFrom::Jar];
        full.entries[1].modified = Some(5);
        full.entries[1]
            .extra
            .insert("future".into(), serde_json::json!({"b": 1, "a": [null, 2]}));
        full.removed = vec![path("mods/z.jar"), path("mods/a.jar"), path("mods/z.jar")];
        full.worlds.reverse();
        full.content = Some("c".repeat(64));
        full.base = Some(crate::manifest::Base::Loader {});
        full.side = Some(crate::Side::Server);
        full.projected_from = Some("d".repeat(64));
        full.parent = Some("e".repeat(64));
        full.label = Some("label".into());
        full.created = 3;
        full.extra.insert("zzz".into(), serde_json::json!("last"));
        full.extra.insert("aaa".into(), serde_json::json!(null));
        full.extra.insert("Rules".into(), serde_json::json!(1));
        let mut sealed = full.clone();
        sealed.seal();
        for image in [sample(), full, sealed, Image::new(Kind::Content, fabric())] {
            assert_eq!(
                String::from_utf8(canonical_bytes(&image)).unwrap(),
                String::from_utf8(reference(&image)).unwrap()
            );
            assert_eq!(compute_id(&image), sha256_hex(&reference(&image)));
        }
    }

    #[test]
    fn an_unknown_field_never_replaces_a_known_one_in_the_id() {
        let mut image = sample();
        image.seal();
        let mut forged = image.clone();
        forged.extra.insert("entries".into(), serde_json::json!([]));
        // Invalid, and its canonical form still lists its own entries.
        assert!(forged.validate().is_err());
        assert_eq!(forged.canonical_bytes(), image.canonical_bytes());

        // The same for an entry's: the known field is hashed, never the unknown one.
        for key in ["sha256", "size", "path", "source", "env"] {
            let mut forged = image.clone();
            forged.entries[0]
                .extra
                .insert(key.into(), serde_json::json!("ff"));
            assert!(forged.validate().is_err(), "{key}");
            assert_eq!(forged.canonical_bytes(), image.canonical_bytes(), "{key}");
            assert_eq!(forged.compute_id(), image.id, "{key}");
        }
    }

    #[test]
    fn withheld_files_are_written_as_their_json() {
        let mut image = sample();
        image.withheld = vec![
            Withheld {
                path: "mods/\"quoted\"\\é.jar".into(),
                reason: WithheldReason::Symlink,
            },
            Withheld {
                path: "a".into(),
                reason: serde_json::from_value(serde_json::json!("future_reason")).unwrap(),
            },
        ];
        let mut expected = Vec::new();
        let mut withheld = image.withheld.clone();
        withheld.sort();
        write_canonical(&json(&withheld), &mut expected);
        let canonical = String::from_utf8(image.canonical_bytes()).unwrap();
        let expected = String::from_utf8(expected).unwrap();
        assert!(
            canonical.contains(&format!("\"withheld\":{expected}")),
            "{canonical}"
        );
    }
}
