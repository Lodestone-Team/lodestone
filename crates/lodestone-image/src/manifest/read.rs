//! Reading a manifest without trusting its size.
//!
//! A manifest can come from another computer, inside an export whose zip compresses JSON a
//! thousand to one, so reading one must never cost much more memory than a real image of
//! its size would. Three things keep it bounded:
//!
//! - [`prescan`] counts the JSON values and the nesting depth before anything is parsed,
//!   and refuses a manifest with far more values than the largest real image has.
//! - Every list is read with a bound ([`Bounded`]): no more entries, removed paths or
//!   withheld files than [`MAX_ENTRIES`], no more worlds than [`MAX_WORLDS`], and so on, so
//!   the error comes when the bound is passed, not after the whole list is in memory.
//! - Fields this version doesn't know are kept (so the image's id still verifies), but
//!   only within a small budget ([`UnknownBudget`]) per image and per entry, and only so
//!   deep. They are read straight into JSON values, never buffered first (which is what
//!   `#[serde(flatten)]` would do, at many times the input's size).
//!
//! Fields with a default read `null` as that default, so a missing field and `null` are
//! the same image, as the identity rules say. Required fields (`schema`, `kind`, `game`; an
//! entry's `path`, `size`, `sha256` and `source`) can't be `null`.

use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;

use serde::de::{self, DeserializeSeed, Deserializer, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use serde_json::{Map, Number, Value};

use super::{
    default_mode, Entry, Image, FIRST_RULES, MAX_ENTRIES, MAX_ENV_FROM, MAX_EXCLUDES, MAX_WORLDS,
};

/// A manifest has at most this many JSON values (roughly: objects, arrays and the commas
/// between items). A real entry has about 20, so a million entries fit with room; a
/// manifest past this is built to exhaust memory, not to describe an instance.
pub const MAX_JSON_VALUES: u64 = 32 * MAX_ENTRIES as u64;
/// Objects and arrays nest at most this deep. A real manifest nests 5 deep.
pub const MAX_JSON_DEPTH: usize = 32;
/// Unknown fields nest at most this deep (below the field itself).
pub const MAX_UNKNOWN_DEPTH: usize = 16;

/// How much of fields this version doesn't know is kept: JSON values, and bytes of their
/// keys and strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownBudget {
    pub values: usize,
    pub bytes: usize,
}

/// For the image's own unknown fields.
pub const IMAGE_UNKNOWN: UnknownBudget = UnknownBudget {
    values: 4096,
    bytes: 64 << 10,
};
/// For each entry's unknown fields: room for a few new fields, not for a payload.
pub const ENTRY_UNKNOWN: UnknownBudget = UnknownBudget {
    values: 16,
    bytes: 1024,
};
/// For the unknown fields of all of an image's entries together. Each entry's own budget
/// alone would let a million entries keep 16 values each, many times the memory of the
/// manifest's text; this keeps unknown fields under about 100 MiB however many entries
/// carry them.
pub const ENTRIES_UNKNOWN: UnknownBudget = UnknownBudget {
    values: 1 << 20,
    bytes: 16 << 20,
};
/// What an entry's first unknown field costs from [`ENTRIES_UNKNOWN`] on top of its values:
/// the map that holds an entry's unknown fields is allocated for the first one.
pub const ENTRY_MAP_COST: usize = 16;

impl UnknownBudget {
    fn spend<E: de::Error>(&mut self, values: usize, bytes: usize) -> Result<(), E> {
        if values > self.values || bytes > self.bytes {
            return Err(E::custom(
                "fields this version doesn't know are larger than it keeps",
            ));
        }
        self.values -= values;
        self.bytes -= bytes;
        Ok(())
    }
}

/// Checks the shape of `bytes` before parsing: at most [`MAX_JSON_VALUES`] values and
/// [`MAX_JSON_DEPTH`] levels of nesting. One pass, no allocation. Not a JSON validator:
/// what it lets through is parsed (and refused, if it isn't JSON) afterwards.
pub fn prescan(bytes: &[u8]) -> Result<(), String> {
    let mut values: u64 = 0;
    let mut depth: usize = 0;
    let mut in_string = false;
    let mut escaped = false;
    for &byte in bytes {
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
            continue;
        }
        match byte {
            b'"' => in_string = true,
            b'{' | b'[' => {
                depth += 1;
                if depth > MAX_JSON_DEPTH {
                    return Err(format!("nests deeper than {MAX_JSON_DEPTH} levels"));
                }
                values += 1;
            }
            b'}' | b']' => depth = depth.saturating_sub(1),
            b',' => values += 1,
            _ => continue,
        }
        if values > MAX_JSON_VALUES {
            return Err(format!("has more than {MAX_JSON_VALUES} values"));
        }
    }
    Ok(())
}

/// Reads one unknown field's value into a JSON value, spending `budget` on every value and
/// every byte of text, and refusing to nest past [`MAX_UNKNOWN_DEPTH`].
struct Capped<'b> {
    budget: &'b mut UnknownBudget,
    depth: usize,
}

impl Capped<'_> {
    fn deeper(&mut self) -> Capped<'_> {
        Capped {
            budget: self.budget,
            depth: self.depth + 1,
        }
    }
}

impl<'de> DeserializeSeed<'de> for Capped<'_> {
    type Value = Value;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Capped<'_> {
    type Value = Value;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("any JSON value")
    }

    fn visit_bool<E: de::Error>(self, v: bool) -> Result<Value, E> {
        self.budget.spend(1, 0)?;
        Ok(Value::Bool(v))
    }

    fn visit_i64<E: de::Error>(self, v: i64) -> Result<Value, E> {
        self.budget.spend(1, 0)?;
        Ok(Value::Number(v.into()))
    }

    fn visit_u64<E: de::Error>(self, v: u64) -> Result<Value, E> {
        self.budget.spend(1, 0)?;
        Ok(Value::Number(v.into()))
    }

    fn visit_f64<E: de::Error>(self, v: f64) -> Result<Value, E> {
        // Kept so validation can name the field: it refuses numbers that aren't whole.
        self.budget.spend(1, 0)?;
        Number::from_f64(v)
            .map(Value::Number)
            .ok_or_else(|| E::custom("an unknown field holds a number JSON can't have"))
    }

    fn visit_str<E: de::Error>(self, v: &str) -> Result<Value, E> {
        self.budget.spend(1, v.len())?;
        Ok(Value::String(v.to_string()))
    }

    fn visit_unit<E: de::Error>(self) -> Result<Value, E> {
        self.budget.spend(1, 0)?;
        Ok(Value::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<Value, E> {
        self.visit_unit()
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Value, D::Error> {
        deserializer.deserialize_any(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(mut self, mut seq: A) -> Result<Value, A::Error> {
        if self.depth >= MAX_UNKNOWN_DEPTH {
            return Err(de::Error::custom("an unknown field nests too deep"));
        }
        self.budget.spend(1, 0)?;
        let mut items = Vec::new();
        while let Some(item) = seq.next_element_seed(self.deeper())? {
            items.push(item);
        }
        Ok(Value::Array(items))
    }

    fn visit_map<A: MapAccess<'de>>(mut self, mut map: A) -> Result<Value, A::Error> {
        if self.depth >= MAX_UNKNOWN_DEPTH {
            return Err(de::Error::custom("an unknown field nests too deep"));
        }
        self.budget.spend(1, 0)?;
        let mut object = Map::new();
        while let Some(key) = map.next_key::<String>()? {
            self.budget.spend(0, key.len())?;
            let value = map.next_value_seed(self.deeper())?;
            object.insert(key, value);
        }
        Ok(Value::Object(object))
    }
}

/// Reads any JSON value within `budget` and [`MAX_UNKNOWN_DEPTH`], as unknown fields are
/// read: for JSON that is kept as it was (an old instance's settings, a delta's unknown
/// fields) and comes from elsewhere.
pub(crate) fn capped_value<'de, D: Deserializer<'de>>(
    deserializer: D,
    budget: &mut UnknownBudget,
) -> Result<Value, D::Error> {
    Capped { budget, depth: 0 }.deserialize(deserializer)
}

/// Reads a list of at most `max` items (`null` is the empty list), failing as soon as the
/// list passes the bound.
pub(super) struct Bounded<T> {
    max: usize,
    what: &'static str,
    item: PhantomData<T>,
}

impl<T> Bounded<T> {
    pub(super) fn new(max: usize, what: &'static str) -> Bounded<T> {
        Bounded {
            max,
            what,
            item: PhantomData,
        }
    }
}

impl<'de, T: Deserialize<'de>> DeserializeSeed<'de> for Bounded<T> {
    type Value = Vec<T>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Vec<T>, D::Error> {
        deserializer.deserialize_option(self)
    }
}

impl<'de, T: Deserialize<'de>> Visitor<'de> for Bounded<T> {
    type Value = Vec<T>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a list of at most {} {}", self.max, self.what)
    }

    fn visit_none<E: de::Error>(self) -> Result<Vec<T>, E> {
        Ok(Vec::new())
    }

    fn visit_unit<E: de::Error>(self) -> Result<Vec<T>, E> {
        Ok(Vec::new())
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Vec<T>, D::Error> {
        deserializer.deserialize_seq(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<T>, A::Error> {
        // The hint is the input's claim; don't reserve more than a little on its word.
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(1024));
        while let Some(item) = seq.next_element()? {
            if items.len() == self.max {
                return Err(de::Error::custom(format!(
                    "more than {} {}",
                    self.max, self.what
                )));
            }
            items.push(item);
        }
        Ok(items)
    }
}

/// Reads a list of at most `max` entries (`null` is none), charging all their unknown
/// fields to one budget ([`ENTRIES_UNKNOWN`]) as well as each entry's own.
pub(crate) struct EntriesSeed<'b> {
    pub(crate) max: usize,
    pub(crate) total: &'b mut UnknownBudget,
}

impl<'de> DeserializeSeed<'de> for EntriesSeed<'_> {
    type Value = Vec<Entry>;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Vec<Entry>, D::Error> {
        deserializer.deserialize_option(self)
    }
}

impl<'de> Visitor<'de> for EntriesSeed<'_> {
    type Value = Vec<Entry>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a list of at most {} entries", self.max)
    }

    fn visit_none<E: de::Error>(self) -> Result<Vec<Entry>, E> {
        Ok(Vec::new())
    }

    fn visit_unit<E: de::Error>(self) -> Result<Vec<Entry>, E> {
        Ok(Vec::new())
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Vec<Entry>, D::Error> {
        deserializer.deserialize_seq(self)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<Entry>, A::Error> {
        let mut items = Vec::with_capacity(seq.size_hint().unwrap_or(0).min(1024));
        while let Some(item) = seq.next_element_seed(EntrySeed {
            total: &mut *self.total,
        })? {
            if items.len() == self.max {
                return Err(de::Error::custom(format!("more than {} entries", self.max)));
            }
            items.push(item);
        }
        Ok(items)
    }
}

/// One entry, charging its unknown fields to `total` too.
struct EntrySeed<'b> {
    total: &'b mut UnknownBudget,
}

impl<'de> DeserializeSeed<'de> for EntrySeed<'_> {
    type Value = Entry;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<Entry, D::Error> {
        deserializer.deserialize_map(EntryVisitor {
            total: Some(self.total),
        })
    }
}

/// `deserialize_with` for a list of entries outside an image (a delta's): at most
/// [`MAX_ENTRIES`], `null` for none, unknown fields within [`ENTRIES_UNKNOWN`] together.
pub(crate) fn entry_list<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<Vec<Entry>, D::Error> {
    let mut total = ENTRIES_UNKNOWN;
    EntriesSeed {
        max: MAX_ENTRIES,
        total: &mut total,
    }
    .deserialize(deserializer)
}

/// `deserialize_with` for a list of at most `MAX` items, `null` for none.
pub(crate) fn bounded_list<'de, D, T, const MAX: usize>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Bounded::new(MAX, "items").deserialize(deserializer)
}

/// `deserialize_with` for fields kept as they were (a delta's unknown fields): within
/// [`IMAGE_UNKNOWN`] together, a key given twice or named after a known field refused.
pub(crate) fn capped_fields<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<BTreeMap<String, Value>, D::Error> {
    struct FieldsVisitor;

    impl<'de> Visitor<'de> for FieldsVisitor {
        type Value = BTreeMap<String, Value>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            f.write_str("a map of fields")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut budget = IMAGE_UNKNOWN;
            let mut out = BTreeMap::new();
            while let Some(key) = map.next_key::<String>()? {
                if IMAGE_FIELDS.contains(&key.as_str()) {
                    // Serialized after the known fields, it would take that field's place
                    // in the manifest and its id.
                    return Err(de::Error::custom(format!(
                        "`{key}` is a known field, not an unknown one"
                    )));
                }
                keep_unknown(&mut map, &mut out, &mut budget, key)?;
            }
            Ok(out)
        }
    }

    deserializer.deserialize_map(FieldsVisitor)
}

/// A setting's list or map (ops, whitelist, resource packs, properties) has at most this
/// many items.
pub const MAX_SETTING_ITEMS: usize = 100_000;

/// `deserialize_with` for a setting's list: at most [`MAX_SETTING_ITEMS`] items, `null`
/// for none.
pub(crate) fn setting_list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Bounded::new(MAX_SETTING_ITEMS, "items in a setting").deserialize(deserializer)
}

/// `deserialize_with` for a setting's map of text: at most [`MAX_SETTING_ITEMS`] keys,
/// `null` for none, a key given twice refused.
pub(crate) fn setting_map<'de, D>(deserializer: D) -> Result<BTreeMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    struct MapVisitor;

    impl<'de> Visitor<'de> for MapVisitor {
        type Value = BTreeMap<String, String>;

        fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "a map of at most {MAX_SETTING_ITEMS} texts")
        }

        fn visit_none<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(BTreeMap::new())
        }

        fn visit_unit<E: de::Error>(self) -> Result<Self::Value, E> {
            Ok(BTreeMap::new())
        }

        fn visit_some<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
            d.deserialize_map(self)
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut out = BTreeMap::new();
            while let Some((key, value)) = map.next_entry::<String, String>()? {
                if out.len() == MAX_SETTING_ITEMS {
                    return Err(de::Error::custom(format!(
                        "more than {MAX_SETTING_ITEMS} items in a setting"
                    )));
                }
                if out.contains_key(&key) {
                    return Err(de::Error::custom(format!("duplicate key `{key}`")));
                }
                out.insert(key, value);
            }
            Ok(out)
        }
    }

    deserializer.deserialize_option(MapVisitor)
}

/// `deserialize_with` for derived fields with a default: `null` reads as the default.
pub(crate) fn null_as_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Option::<T>::deserialize(deserializer)?.unwrap_or_default())
}

/// Defines a struct's field names: each known name, and `Other` for the rest, so the
/// visitor can keep what it doesn't know. `index` numbers the known fields for the
/// duplicate check.
macro_rules! field_names {
    ($name:ident { $($variant:ident => $text:literal,)* }) => {
        enum $name {
            $($variant,)*
            Other(String),
        }

        impl $name {
            const NAMES: &'static [&'static str] = &[$($text),*];

            fn index(&self) -> Option<usize> {
                let text = match self {
                    $($name::$variant => $text,)*
                    $name::Other(_) => return None,
                };
                Self::NAMES.iter().position(|name| *name == text)
            }

            fn name(&self) -> &str {
                match self {
                    $($name::$variant => $text,)*
                    $name::Other(text) => text,
                }
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct NameVisitor;

                impl<'de> Visitor<'de> for NameVisitor {
                    type Value = $name;

                    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                        f.write_str("a field name")
                    }

                    fn visit_str<E: de::Error>(self, v: &str) -> Result<$name, E> {
                        Ok(match v {
                            $($text => $name::$variant,)*
                            other => $name::Other(other.to_string()),
                        })
                    }
                }

                deserializer.deserialize_identifier(NameVisitor)
            }
        }
    };
}

field_names!(ImageField {
    Schema => "schema",
    Id => "id",
    Kind => "kind",
    Parent => "parent",
    Created => "created",
    Producer => "producer",
    Label => "label",
    Base => "base",
    Game => "game",
    Settings => "settings",
    Entries => "entries",
    Removed => "removed",
    Worlds => "worlds",
    Content => "content",
    Excludes => "excludes",
    Withheld => "withheld",
    Rules => "rules",
    Side => "side",
    ProjectedFrom => "projected_from",
});

field_names!(EntryField {
    Path => "path",
    Size => "size",
    Sha256 => "sha256",
    Sha1 => "sha1",
    Sha512 => "sha512",
    Mode => "mode",
    Modified => "modified",
    Source => "source",
    Env => "env",
    EnvFrom => "env_from",
    World => "world",
});

/// An image's field names. An unknown field can't take one: it is serialized after the
/// known fields, so it would replace that field in the manifest and in its id.
pub(crate) const IMAGE_FIELDS: &[&str] = ImageField::NAMES;
/// An entry's field names, which an entry's unknown field can't take either.
pub(crate) const ENTRY_FIELDS: &[&str] = EntryField::NAMES;

/// Which known fields were seen, to refuse one given twice.
#[derive(Default)]
struct Seen(u64);

impl Seen {
    fn first<E: de::Error>(&mut self, index: Option<usize>, name: &str) -> Result<(), E> {
        let Some(index) = index else {
            return Ok(());
        };
        let bit = 1u64 << index;
        if self.0 & bit != 0 {
            return Err(E::custom(format!("duplicate field `{name}`")));
        }
        self.0 |= bit;
        Ok(())
    }
}

/// Keeps one unknown field, within `budget`.
fn keep_unknown<'de, A: MapAccess<'de>>(
    map: &mut A,
    extra: &mut BTreeMap<String, Value>,
    budget: &mut UnknownBudget,
    name: String,
) -> Result<(), A::Error> {
    budget.spend::<A::Error>(0, name.len())?;
    let value = map.next_value_seed(Capped { budget, depth: 0 })?;
    if extra.contains_key(&name) {
        return Err(de::Error::custom(format!("duplicate field `{name}`")));
    }
    extra.insert(name, value);
    Ok(())
}

fn required<T, E: de::Error>(value: Option<T>, name: &'static str) -> Result<T, E> {
    value.ok_or_else(|| E::missing_field(name))
}

impl<'de> Deserialize<'de> for Image {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Image, D::Error> {
        deserializer.deserialize_map(ImageVisitor)
    }
}

struct ImageVisitor;

impl<'de> Visitor<'de> for ImageVisitor {
    type Value = Image;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an image manifest")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Image, A::Error> {
        let mut seen = Seen::default();
        let mut budget = IMAGE_UNKNOWN;
        let mut entries_budget = ENTRIES_UNKNOWN;
        let mut extra = BTreeMap::new();
        let (mut schema, mut id, mut kind, mut parent, mut created) =
            (None, None, None, None, None);
        let (mut producer, mut label, mut base, mut game, mut settings) =
            (None, None, None, None, None);
        let (mut entries, mut removed, mut worlds, mut content, mut excludes) =
            (None, None, None, None, None);
        let (mut withheld, mut rules, mut side, mut projected_from) = (None, None, None, None);
        while let Some(field) = map.next_key::<ImageField>()? {
            seen.first::<A::Error>(field.index(), field.name())?;
            match field {
                ImageField::Schema => schema = map.next_value()?,
                ImageField::Id => id = map.next_value()?,
                ImageField::Kind => kind = map.next_value()?,
                ImageField::Parent => parent = map.next_value()?,
                ImageField::Created => created = map.next_value()?,
                ImageField::Producer => producer = map.next_value()?,
                ImageField::Label => label = map.next_value()?,
                ImageField::Base => base = map.next_value()?,
                ImageField::Game => game = map.next_value()?,
                ImageField::Settings => settings = map.next_value()?,
                ImageField::Entries => {
                    entries = Some(map.next_value_seed(EntriesSeed {
                        max: MAX_ENTRIES,
                        total: &mut entries_budget,
                    })?)
                }
                ImageField::Removed => {
                    removed = Some(map.next_value_seed(Bounded::new(MAX_ENTRIES, "removed paths"))?)
                }
                ImageField::Worlds => {
                    worlds = Some(map.next_value_seed(Bounded::new(MAX_WORLDS, "worlds"))?)
                }
                ImageField::Content => content = map.next_value()?,
                ImageField::Excludes => {
                    excludes = Some(map.next_value_seed(Bounded::new(MAX_EXCLUDES, "excludes"))?)
                }
                ImageField::Withheld => {
                    withheld =
                        Some(map.next_value_seed(Bounded::new(MAX_ENTRIES, "withheld files"))?)
                }
                ImageField::Rules => rules = map.next_value()?,
                ImageField::Side => side = map.next_value()?,
                ImageField::ProjectedFrom => projected_from = map.next_value()?,
                ImageField::Other(name) => keep_unknown(&mut map, &mut extra, &mut budget, name)?,
            }
        }
        Ok(Image {
            schema: required(schema, "schema")?,
            id: id.unwrap_or_default(),
            kind: required(kind, "kind")?,
            parent,
            created: created.unwrap_or_default(),
            producer: producer.unwrap_or_default(),
            label,
            base,
            game: required(game, "game")?,
            settings: settings.unwrap_or_default(),
            entries: entries.unwrap_or_default(),
            removed: removed.unwrap_or_default(),
            worlds: worlds.unwrap_or_default(),
            content,
            excludes: excludes.unwrap_or_default(),
            withheld: withheld.unwrap_or_default(),
            // A manifest without rules was written under the first rules, whatever this
            // version implements now; the canonical form always writes them.
            rules: rules.unwrap_or(FIRST_RULES),
            side,
            projected_from,
            extra,
        })
    }
}

/// One entry on its own, within [`ENTRY_UNKNOWN`]. Lists of entries go through
/// `EntriesSeed`, which also bounds them together.
impl<'de> Deserialize<'de> for Entry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Entry, D::Error> {
        deserializer.deserialize_map(EntryVisitor { total: None })
    }
}

struct EntryVisitor<'b> {
    /// The budget all the list's entries share, if any.
    total: Option<&'b mut UnknownBudget>,
}

impl<'de> Visitor<'de> for EntryVisitor<'_> {
    type Value = Entry;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("an image entry")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Entry, A::Error> {
        let mut total = self.total;
        let mut seen = Seen::default();
        let mut budget = ENTRY_UNKNOWN;
        let mut extra = BTreeMap::new();
        let (mut path, mut size, mut sha256, mut sha1, mut sha512) = (None, None, None, None, None);
        let (mut mode, mut modified, mut source, mut env, mut env_from, mut world) =
            (None, None, None, None, None, None);
        while let Some(field) = map.next_key::<EntryField>()? {
            seen.first::<A::Error>(field.index(), field.name())?;
            match field {
                EntryField::Path => path = map.next_value()?,
                EntryField::Size => size = map.next_value()?,
                EntryField::Sha256 => sha256 = map.next_value()?,
                EntryField::Sha1 => sha1 = map.next_value()?,
                EntryField::Sha512 => sha512 = map.next_value()?,
                EntryField::Mode => mode = map.next_value()?,
                EntryField::Modified => modified = map.next_value()?,
                EntryField::Source => source = map.next_value()?,
                EntryField::Env => env = map.next_value()?,
                EntryField::EnvFrom => {
                    env_from =
                        Some(map.next_value_seed(Bounded::new(MAX_ENV_FROM, "env_from inputs"))?)
                }
                EntryField::World => world = map.next_value()?,
                EntryField::Other(name) => {
                    let before = budget;
                    let first = extra.is_empty();
                    keep_unknown(&mut map, &mut extra, &mut budget, name)?;
                    if let Some(total) = total.as_deref_mut() {
                        let map_cost = if first { ENTRY_MAP_COST } else { 0 };
                        total.spend::<A::Error>(
                            before.values - budget.values + map_cost,
                            before.bytes - budget.bytes,
                        )?;
                    }
                }
            }
        }
        Ok(Entry {
            path: required(path, "path")?,
            size: required(size, "size")?,
            sha256: required(sha256, "sha256")?,
            sha1,
            sha512,
            mode: mode.unwrap_or_else(default_mode),
            modified,
            source: required(source, "source")?,
            env: env.unwrap_or_default(),
            env_from: env_from.unwrap_or_default(),
            world,
            extra,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Image, ManifestError};

    const HEAD: &str =
        r#"{"schema":2,"kind":"content","game":{"minecraft":"1.21.1","loader":"fabric"}"#;

    fn read(rest: &str) -> Result<Image, ManifestError> {
        Image::from_json(format!("{HEAD}{rest}}}").as_bytes())
    }

    #[test]
    fn prescan_counts_values_and_depth() {
        assert_eq!(prescan(br#"{"a":[1,2,3],"b":"[[[,,,"}"#), Ok(()));
        assert!(prescan(&b"[".repeat(MAX_JSON_DEPTH + 1)).is_err());
        assert_eq!(prescan(&b"[".repeat(MAX_JSON_DEPTH)), Ok(()));
        // Brackets and commas inside strings (escaped quotes too) don't count.
        let quoted = format!("\"\\\"{}\"", "[".repeat(100));
        assert_eq!(prescan(quoted.as_bytes()), Ok(()));
    }

    #[test]
    fn prescan_refuses_too_many_values() {
        // Just past the bound, as an array of zeros.
        let n = usize::try_from(MAX_JSON_VALUES).unwrap();
        let mut bytes = Vec::with_capacity(n * 2 + 32);
        bytes.extend_from_slice(br#"{"schema":2,"x":["#);
        for _ in 0..n {
            bytes.extend_from_slice(b"0,");
        }
        bytes.extend_from_slice(b"0]}");
        assert!(prescan(&bytes).unwrap_err().contains("values"));
        assert!(matches!(
            Image::from_json(&bytes),
            Err(ManifestError::Invalid(_))
        ));
    }

    #[test]
    fn unknown_fields_have_a_budget() {
        // Within the image's budget: kept.
        let small = format!(r#","future":[{}0]"#, "0,".repeat(1000));
        assert_eq!(
            read(&small).unwrap().extra["future"]
                .as_array()
                .unwrap()
                .len(),
            1001
        );
        // Past it: refused, before the rest is read.
        let big = format!(r#","future":[{}0]"#, "0,".repeat(IMAGE_UNKNOWN.values));
        let error = read(&big).unwrap_err().to_string();
        assert!(error.contains("larger than it keeps"), "{error}");
        let long = format!(r#","future":"{}""#, "x".repeat(IMAGE_UNKNOWN.bytes + 1));
        assert!(read(&long).is_err());
        let deep = format!(
            r#","future":{}0{}"#,
            "[".repeat(MAX_UNKNOWN_DEPTH + 1),
            "]".repeat(MAX_UNKNOWN_DEPTH + 1)
        );
        assert!(read(&deep).unwrap_err().to_string().contains("too deep"));
        let ok_deep = format!(
            r#","future":{}0{}"#,
            "[".repeat(MAX_UNKNOWN_DEPTH),
            "]".repeat(MAX_UNKNOWN_DEPTH)
        );
        read(&ok_deep).unwrap();
    }

    #[test]
    fn each_entry_has_its_own_small_budget() {
        let entry = |extra: &str| {
            format!(
                r#","entries":[{{"path":"mods/a.jar","size":1,"sha256":"{}","source":{{"object":{{}}}}{extra}}}]"#,
                "0".repeat(64)
            )
        };
        let image = read(&entry(r#","origin":{"kind":"x","url":"y"}"#)).unwrap();
        assert_eq!(image.entries[0].extra["origin"]["kind"], "x");
        let payload = format!(r#","blob":[{}0]"#, "0,".repeat(ENTRY_UNKNOWN.values));
        assert!(read(&entry(&payload)).is_err());
    }

    #[test]
    fn entries_share_one_budget_for_unknown_fields() {
        // Each entry within its own budget, but together far past what a manifest of this
        // size should cost: refused, before the rest is read.
        let entry = |i: usize, extra: &str| {
            format!(
                r#"{{"path":"m/{i}","size":0,"sha256":"{}","source":{{"object":{{}}}}{extra}}}"#,
                "a".repeat(64)
            )
        };
        let payload = format!(r#","z":[{}0]"#, "0,".repeat(ENTRY_UNKNOWN.values - 2));
        let many: Vec<String> = (0..70_000).map(|i| entry(i, &payload)).collect();
        let error = read(&format!(r#","entries":[{}]"#, many.join(",")))
            .unwrap_err()
            .to_string();
        assert!(error.contains("larger than it keeps"), "{error}");
        // What is kept stays a small multiple of the manifest's size: the values kept per
        // byte of input are bounded by the shared budget, whatever the entry count.
        let per_byte = (ENTRIES_UNKNOWN.values as f64) / (70_000.0 * payload.len() as f64);
        assert!(per_byte < 1.0, "{per_byte}");
        // A new field on many entries, within the budget, is kept.
        let some: Vec<String> = (0..20_000).map(|i| entry(i, r#","origin":"x""#)).collect();
        let image = read(&format!(r#","entries":[{}]"#, some.join(","))).unwrap();
        assert_eq!(image.entries.len(), 20_000);
        assert!(image.entries.iter().all(|e| e.extra["origin"] == "x"));
    }

    #[test]
    fn lists_are_bounded_while_read() {
        let env_from = format!(
            r#","entries":[{{"path":"mods/a.jar","size":1,"sha256":"{}","source":{{"object":{{}}}},"env_from":[{}"x"]}}]"#,
            "0".repeat(64),
            r#""x","#.repeat(MAX_ENV_FROM)
        );
        assert!(read(&env_from)
            .unwrap_err()
            .to_string()
            .contains("env_from"));
        let worlds: Vec<String> = (0..=MAX_WORLDS)
            .map(|i| format!(r#"{{"name":"w{i}"}}"#))
            .collect();
        let error = read(&format!(r#","worlds":[{}]"#, worlds.join(","))).unwrap_err();
        assert!(error.to_string().contains("worlds"), "{error}");
    }

    #[test]
    fn setting_lists_are_bounded() {
        let packs: Vec<String> = (0..=MAX_SETTING_ITEMS)
            .map(|_| "\"\"".to_string())
            .collect();
        let error = read(&format!(
            r#","settings":{{"client":{{"resource_packs":[{}]}}}}"#,
            packs.join(",")
        ))
        .unwrap_err();
        assert!(error.to_string().contains("items in a setting"), "{error}");
        let image =
            read(r#","settings":{"server":{"properties":{"motd":"hi"},"ops":null}}"#).unwrap();
        assert_eq!(image.settings.server.unwrap().properties["motd"], "hi");
        assert!(read(r#","settings":{"server":{"properties":{"a":"1","a":"2"}}}"#).is_err());
    }

    #[test]
    fn null_is_the_default() {
        let image = read(
            r#","id":null,"settings":null,"entries":null,"removed":null,"worlds":null,
               "excludes":null,"withheld":null,"producer":null,"created":null,"rules":null"#,
        )
        .unwrap();
        assert!(image.entries.is_empty());
        // The first rules, not this version's: a writer that left them out wrote under
        // those, and the id was computed with them.
        assert_eq!(image.rules, FIRST_RULES);
        let entry = format!(
            r#","entries":[{{"path":"mods/a.jar","size":1,"sha256":"{}","source":{{"object":{{}}}},
                "mode":null,"env":null,"env_from":null,"world":null,"sha1":null}}]"#,
            "0".repeat(64)
        );
        let image = read(&entry).unwrap();
        assert_eq!(image.entries[0].mode, 0o644);
        // The same image as without the nulls.
        let plain = read(&format!(
            r#","entries":[{{"path":"mods/a.jar","size":1,"sha256":"{}","source":{{"object":{{}}}}}}]"#,
            "0".repeat(64)
        ))
        .unwrap();
        assert_eq!(image.compute_id(), plain.compute_id());
        // Required fields can't be null.
        assert!(Image::from_json(
            br#"{"schema":2,"kind":null,"game":{"minecraft":"1","loader":"vanilla"}}"#
        )
        .is_err());
    }

    #[test]
    fn duplicate_fields_are_refused() {
        assert!(read(r#","kind":"data""#)
            .unwrap_err()
            .to_string()
            .contains("duplicate"));
        assert!(read(r#","future":1,"future":2"#)
            .unwrap_err()
            .to_string()
            .contains("duplicate"));
        let entry = format!(
            r#","entries":[{{"path":"mods/a.jar","path":"mods/b.jar","size":1,"sha256":"{}","source":{{"object":{{}}}}}}]"#,
            "0".repeat(64)
        );
        assert!(read(&entry).unwrap_err().to_string().contains("duplicate"));
    }

    #[test]
    fn huge_unknown_array_is_refused_without_buffering() {
        // The shape of an export's amplification attack: one unknown field holding a long
        // array. It fails on the budget after IMAGE_UNKNOWN.values values, whatever follows.
        let mut json = String::from(r#"{"schema":2,"zz":["#);
        json.push_str(&"0,".repeat(4_000_000));
        json.push_str(r#"0],"kind":"content","game":{"minecraft":"1","loader":"vanilla"}}"#);
        let error = Image::from_json(json.as_bytes()).unwrap_err().to_string();
        assert!(error.contains("larger than it keeps"), "{error}");
    }
}
