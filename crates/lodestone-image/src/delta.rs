//! Deltas: the changes from one image to another, and applying them.
//!
//! Images are stored as full listings and sent as deltas. [`diff`] works between any two
//! images (no chain of parents needed): entries added, removed and replaced, by path, and
//! every other field that differs, whole. [`apply`] turns the `from` image into exactly the
//! `to` image, and checks the result against the `to` id when there is one, so a receiver
//! knows it has what the sender meant. Both are deterministic: the same two images always
//! give the same delta, byte for byte.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::manifest::{Base, Entry, Game, Image, Kind, Producer, Withheld, World};
use crate::paths::CanonicalPath;
use crate::settings::Settings;
use crate::Side;

/// One field outside the entries that differs, with its value in the `to` image. Written
/// as `{"<field>": <value>}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldChange {
    Kind(Kind),
    Parent(Option<String>),
    Created(i64),
    Producer(Producer),
    Label(Option<String>),
    Base(Option<Base>),
    Game(Game),
    Settings(Settings),
    Removed(Vec<CanonicalPath>),
    Worlds(Vec<World>),
    Content(Option<String>),
    Excludes(Vec<String>),
    Withheld(Vec<Withheld>),
    Rules(u32),
    Side(Option<Side>),
    ProjectedFrom(Option<String>),
    /// The fields this version doesn't know, all of them.
    Extra(BTreeMap<String, Value>),
}

/// The changes from the image `from` to the image `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Delta {
    /// The id of the image this applies to (empty when it wasn't sealed).
    pub from: String,
    /// The id of the image it gives (empty when it wasn't sealed).
    pub to: String,
    /// Entries only `to` has, sorted by path.
    #[serde(default)]
    pub added: Vec<Entry>,
    /// Paths only `from` has, sorted.
    #[serde(default)]
    pub removed: Vec<CanonicalPath>,
    /// Entries both have that differ in anything (contents, source, env, mode, timestamp),
    /// as `to` has them, sorted by path.
    #[serde(default)]
    pub replaced: Vec<Entry>,
    /// Fields outside the entries that differ, in the manifest's field order.
    #[serde(default)]
    pub changes: Vec<FieldChange>,
}

impl Delta {
    /// Whether the two images are the same in every field.
    pub fn is_empty(&self) -> bool {
        self.added.is_empty()
            && self.removed.is_empty()
            && self.replaced.is_empty()
            && self.changes.is_empty()
    }

    /// Entries whose contents the receiver needs: added ones, and replaced ones whose bytes
    /// changed (not only their env, mode or timestamp).
    pub fn new_contents<'a>(&'a self, from: &'a Image) -> impl Iterator<Item = &'a Entry> + 'a {
        self.added
            .iter()
            .chain(self.replaced.iter().filter(move |entry| {
                from.entry(&entry.path)
                    .is_none_or(|old| old.sha256 != entry.sha256 || old.size != entry.size)
            }))
    }
}

/// Why a delta couldn't be applied.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DeltaError {
    #[error("the delta is for image {expected}, not {actual}")]
    WrongBase { expected: String, actual: String },
    #[error("the delta adds {0}, which is already there")]
    AlreadyThere(CanonicalPath),
    #[error("the delta changes or removes {0}, which isn't there")]
    Missing(CanonicalPath),
    #[error("the delta names {0} more than once")]
    Repeated(CanonicalPath),
    #[error("the result has id {actual}, not {expected}")]
    WrongResult { expected: String, actual: String },
}

/// The changes from `from` to `to`.
pub fn diff(from: &Image, to: &Image) -> Delta {
    let mut from = from.clone();
    from.normalize();
    let mut to = to.clone();
    to.normalize();

    let old: BTreeMap<&CanonicalPath, &Entry> = from.entries.iter().map(|e| (&e.path, e)).collect();
    let new: BTreeMap<&CanonicalPath, &Entry> = to.entries.iter().map(|e| (&e.path, e)).collect();
    let mut added = Vec::new();
    let mut replaced = Vec::new();
    for (path, entry) in &new {
        match old.get(path) {
            None => added.push((*entry).clone()),
            Some(previous) if previous != entry => replaced.push((*entry).clone()),
            Some(_) => {}
        }
    }
    let removed = old
        .keys()
        .filter(|path| !new.contains_key(*path))
        .map(|path| (*path).clone())
        .collect();

    let mut changes = Vec::new();
    macro_rules! field {
        ($field:ident, $variant:ident) => {
            if from.$field != to.$field {
                changes.push(FieldChange::$variant(to.$field.clone()));
            }
        };
    }
    field!(kind, Kind);
    field!(parent, Parent);
    field!(created, Created);
    field!(producer, Producer);
    field!(label, Label);
    field!(base, Base);
    field!(game, Game);
    field!(settings, Settings);
    field!(removed, Removed);
    field!(worlds, Worlds);
    field!(content, Content);
    field!(excludes, Excludes);
    field!(withheld, Withheld);
    field!(rules, Rules);
    field!(side, Side);
    field!(projected_from, ProjectedFrom);
    field!(extra, Extra);

    Delta {
        from: from.id.clone(),
        to: to.id.clone(),
        added,
        removed,
        replaced,
        changes,
    }
}

/// `from` with `delta` applied: the image the delta was made to. Errors (and changes
/// nothing) when the delta is for another image, names paths that aren't where it says, or
/// gives a result whose id isn't the delta's `to`.
pub fn apply(from: &Image, delta: &Delta) -> Result<Image, DeltaError> {
    if !delta.from.is_empty() && delta.from != from.id {
        return Err(DeltaError::WrongBase {
            expected: delta.from.clone(),
            actual: from.id.clone(),
        });
    }
    let mut entries: BTreeMap<CanonicalPath, Entry> = from
        .entries
        .iter()
        .map(|e| (e.path.clone(), e.clone()))
        .collect();
    let mut named = BTreeSet::new();
    let mut name = |path: &CanonicalPath| {
        if named.insert(path.clone()) {
            Ok(())
        } else {
            Err(DeltaError::Repeated(path.clone()))
        }
    };
    for path in &delta.removed {
        name(path)?;
        if entries.remove(path).is_none() {
            return Err(DeltaError::Missing(path.clone()));
        }
    }
    for entry in &delta.replaced {
        name(&entry.path)?;
        match entries.get_mut(&entry.path) {
            Some(slot) => *slot = entry.clone(),
            None => return Err(DeltaError::Missing(entry.path.clone())),
        }
    }
    for entry in &delta.added {
        name(&entry.path)?;
        if entries.insert(entry.path.clone(), entry.clone()).is_some() {
            return Err(DeltaError::AlreadyThere(entry.path.clone()));
        }
    }

    let mut image = from.clone();
    image.entries = entries.into_values().collect();
    for change in &delta.changes {
        match change.clone() {
            FieldChange::Kind(v) => image.kind = v,
            FieldChange::Parent(v) => image.parent = v,
            FieldChange::Created(v) => image.created = v,
            FieldChange::Producer(v) => image.producer = v,
            FieldChange::Label(v) => image.label = v,
            FieldChange::Base(v) => image.base = v,
            FieldChange::Game(v) => image.game = v,
            FieldChange::Settings(v) => image.settings = v,
            FieldChange::Removed(v) => image.removed = v,
            FieldChange::Worlds(v) => image.worlds = v,
            FieldChange::Content(v) => image.content = v,
            FieldChange::Excludes(v) => image.excludes = v,
            FieldChange::Withheld(v) => image.withheld = v,
            FieldChange::Rules(v) => image.rules = v,
            FieldChange::Side(v) => image.side = v,
            FieldChange::ProjectedFrom(v) => image.projected_from = v,
            FieldChange::Extra(v) => image.extra = v,
        }
    }
    image.normalize();
    image.id = delta.to.clone();
    if !delta.to.is_empty() {
        let actual = image.compute_id();
        if actual != delta.to {
            return Err(DeltaError::WrongResult {
                expected: delta.to.clone(),
                actual,
            });
        }
    }
    Ok(image)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::env::{Env, EnvValue};
    use crate::hex::sha256_hex;
    use crate::manifest::tests::{fabric, object, path};
    use crate::manifest::{Kind, Source};
    use crate::view::{tests::fabric_pack, view};
    use crate::RULES;

    fn content(paths: &[&str]) -> Image {
        let mut image = Image::new(Kind::Content, fabric());
        image.entries = paths.iter().map(|p| object(p)).collect();
        image.seal();
        image
    }

    #[test]
    fn identical_images_give_an_empty_delta() {
        let image = fabric_pack();
        let delta = diff(&image, &image);
        assert!(delta.is_empty());
        assert_eq!(apply(&image, &delta).unwrap(), image);
    }

    #[test]
    fn added_removed_replaced() {
        let from = content(&["mods/a.jar", "mods/b.jar", "config/c.toml"]);
        let mut to = content(&["mods/a.jar", "mods/d.jar", "config/c.toml"]);
        to.entries
            .iter_mut()
            .find(|e| e.path.as_str() == "config/c.toml")
            .unwrap()
            .sha256 = sha256_hex(b"new");
        to.label = Some("Added d".into());
        to.seal();

        let delta = diff(&from, &to);
        assert_eq!(delta.from, from.id);
        assert_eq!(delta.to, to.id);
        assert_eq!(delta.added.len(), 1);
        assert_eq!(delta.added[0].path, path("mods/d.jar"));
        assert_eq!(delta.removed, [path("mods/b.jar")]);
        assert_eq!(delta.replaced.len(), 1);
        assert_eq!(delta.replaced[0].path, path("config/c.toml"));
        assert_eq!(delta.changes, [FieldChange::Label(Some("Added d".into()))]);
        let contents: Vec<&str> = delta.new_contents(&from).map(|e| e.path.as_str()).collect();
        assert_eq!(contents, ["mods/d.jar", "config/c.toml"]);
        assert_eq!(apply(&from, &delta).unwrap(), to);
    }

    #[test]
    fn env_only_changes_need_no_bytes() {
        let from = content(&["mods/a.jar"]);
        let mut to = from.clone();
        to.entries[0].env = Env::new(EnvValue::Required, EnvValue::Required);
        to.seal();
        let delta = diff(&from, &to);
        assert_eq!(delta.replaced.len(), 1);
        assert_eq!(delta.new_contents(&from).count(), 0);
        assert_eq!(apply(&from, &delta).unwrap(), to);
    }

    #[test]
    fn settings_and_game_changes() {
        let from = fabric_pack();
        let mut to = from.clone();
        to.game.loader_version = Some("0.17.2".into());
        to.settings.server.as_mut().unwrap().port = Some(25570);
        to.base = None;
        to.removed.clear();
        to.created += 60;
        to.seal();
        let delta = diff(&from, &to);
        let fields: Vec<&str> = delta
            .changes
            .iter()
            .map(|c| match c {
                FieldChange::Created(_) => "created",
                FieldChange::Base(_) => "base",
                FieldChange::Game(_) => "game",
                FieldChange::Settings(_) => "settings",
                FieldChange::Removed(_) => "removed",
                _ => "other",
            })
            .collect();
        assert_eq!(fields, ["created", "base", "game", "settings", "removed"]);
        assert_eq!(apply(&from, &delta).unwrap(), to);
    }

    #[test]
    fn works_between_any_two_images_both_ways() {
        let images = [
            fabric_pack(),
            content(&[]),
            content(&["mods/a.jar"]),
            content(&["mods/lithium.jar", "config/x.toml"]),
            view(&fabric_pack(), crate::Side::Client, RULES).unwrap(),
            view(&fabric_pack(), crate::Side::Server, RULES).unwrap(),
        ];
        for a in &images {
            for b in &images {
                let delta = diff(a, b);
                assert_eq!(&apply(a, &delta).unwrap(), b);
                // Deterministic, and it survives JSON.
                assert_eq!(
                    serde_json::to_vec(&delta).unwrap(),
                    serde_json::to_vec(&diff(a, b)).unwrap()
                );
                let json = serde_json::to_vec(&delta).unwrap();
                let read: Delta = serde_json::from_slice(&json).unwrap();
                assert_eq!(&apply(a, &read).unwrap(), b);
            }
        }
    }

    #[test]
    fn wrong_base_and_bad_deltas_are_refused() {
        let a = content(&["mods/a.jar"]);
        let b = content(&["mods/b.jar"]);
        let c = content(&["mods/c.jar"]);
        let delta = diff(&a, &b);
        assert!(matches!(
            apply(&c, &delta),
            Err(DeltaError::WrongBase { .. })
        ));

        let mut removes_missing = delta.clone();
        removes_missing.removed.push(path("mods/zzz.jar"));
        assert_eq!(
            apply(&a, &removes_missing),
            Err(DeltaError::Missing(path("mods/zzz.jar")))
        );

        let mut adds_existing = delta.clone();
        adds_existing.removed.clear();
        assert_eq!(
            apply(&a, &adds_existing).unwrap_err(),
            DeltaError::WrongResult {
                expected: b.id.clone(),
                actual: {
                    let mut both = content(&["mods/a.jar", "mods/b.jar"]);
                    both.seal();
                    both.id
                }
            }
        );
        let mut adds_twice = delta.clone();
        adds_twice.added.push(object("mods/a.jar"));
        assert_eq!(
            apply(&a, &adds_twice),
            Err(DeltaError::Repeated(path("mods/a.jar")))
        );
        let mut adds_present = diff(&content(&[]), &a);
        adds_present.from.clear();
        adds_present.to.clear();
        assert_eq!(
            apply(&a, &adds_present),
            Err(DeltaError::AlreadyThere(path("mods/a.jar")))
        );

        let mut replaces_missing = delta.clone();
        replaces_missing.replaced.push(object("mods/q.jar"));
        assert_eq!(
            apply(&a, &replaces_missing),
            Err(DeltaError::Missing(path("mods/q.jar")))
        );

        // A tampered entry gives another id.
        let mut tampered = delta.clone();
        tampered.added[0].source = Source::Base {};
        assert!(matches!(
            apply(&a, &tampered),
            Err(DeltaError::WrongResult { .. })
        ));
    }

    #[test]
    fn unsealed_images_still_round_trip() {
        let mut a = content(&["mods/a.jar"]);
        a.id.clear();
        let mut b = content(&["mods/b.jar"]);
        b.id.clear();
        let delta = diff(&a, &b);
        assert!(delta.from.is_empty() && delta.to.is_empty());
        assert_eq!(apply(&a, &delta).unwrap(), b);
    }

    /// SplitMix64, for random images without a dependency.
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    fn random_image(rng: &mut Rng) -> Image {
        let mut image = fabric_pack();
        image.entries.retain(|_| rng.below(3) != 0);
        for entry in &mut image.entries {
            if rng.below(4) == 0 {
                entry.sha256 = sha256_hex(&rng.next().to_le_bytes());
            }
            if rng.below(5) == 0 {
                entry.env.client = EnvValue::Unsupported;
            }
            if rng.below(5) == 0 {
                entry.modified = Some(rng.next() as i64);
            }
        }
        for i in 0..rng.below(4) {
            image
                .entries
                .push(object(&format!("mods/extra-{}.jar", rng.below(6) + i * 10)));
        }
        image.entries.sort_by(|a, b| a.path.cmp(&b.path));
        image.entries.dedup_by(|a, b| a.path == b.path);
        if rng.below(2) == 0 {
            image.settings.client = None;
        }
        if rng.below(3) == 0 {
            image.game.loader_version = Some(format!("0.16.{}", rng.below(20)));
        }
        if rng.below(3) == 0 {
            image.label = Some(format!("label {}", rng.below(100)));
        }
        image.seal();
        image
    }

    #[test]
    fn random_round_trips() {
        let mut rng = Rng(0xDE17A);
        for _ in 0..300 {
            let a = random_image(&mut rng);
            let b = random_image(&mut rng);
            let delta = diff(&a, &b);
            let applied = apply(&a, &delta).unwrap();
            assert_eq!(applied, b);
            applied.verify().unwrap();
            assert_eq!(delta.is_empty(), a == b);
        }
    }
}
