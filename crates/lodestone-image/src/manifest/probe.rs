//! Reading one whole-number field (a schema or document version) of a JSON object without
//! building the rest.
//!
//! A manifest's schema, or an old file's version, decides how the rest is read, so it is
//! read first. Deserializing even a one-field struct with a `serde_json::Value` would build
//! whatever that field holds as a tree of values (72 bytes each), so a field holding an
//! array of millions of zeros would cost tens of times the input's size before anything
//! refused it. Here every value but the probed field's number is skipped with
//! [`IgnoredAny`], which allocates nothing, and a probed field that isn't a number is
//! skipped the same way and reported.

use std::fmt;

use serde::de::{DeserializeSeed, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};

use super::read::prescan;

/// Why [`whole_number_field`] found no whole number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeError {
    /// [`prescan`] refused its shape (far too many values, or too deep); the text follows
    /// "it" or "the manifest".
    Shape(String),
    /// It isn't JSON.
    Json(String),
    /// It is JSON, but not an object.
    NotObject,
    /// The field is a number, but not a whole one that fits 64 bits unsigned.
    NotWhole,
    /// The field is something other than a number (`null` included).
    NotNumber,
}

/// Checks that `bytes` is a JSON object of a sane shape ([`prescan`]) and returns its
/// whole-number field `key`, `None` when it is missing. Nothing but that field is kept: the
/// rest is skipped without building JSON values, so a hostile document costs no memory
/// beyond its own bytes. Anything other than an object is refused (serde would read a
/// struct from an array too). The first `key` counts when it appears twice.
pub(crate) fn whole_number_field(bytes: &[u8], key: &str) -> Result<Option<u64>, ProbeError> {
    prescan(bytes).map_err(ProbeError::Shape)?;
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    let probed = Probe { key }
        .deserialize(&mut deserializer)
        .and_then(|probed| deserializer.end().map(|()| probed))
        .map_err(|e| ProbeError::Json(e.to_string()))?;
    match probed {
        None => Err(ProbeError::NotObject),
        Some(None) => Ok(None),
        Some(Some(Probed::Whole(v))) => Ok(Some(v)),
        Some(Some(Probed::NotWhole)) => Err(ProbeError::NotWhole),
        Some(Some(Probed::NotNumber)) => Err(ProbeError::NotNumber),
    }
}

/// What the probed field held.
enum Probed {
    Whole(u64),
    NotWhole,
    NotNumber,
}

struct ProbedVisitor;

impl<'de> Visitor<'de> for ProbedVisitor {
    type Value = Probed;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a version")
    }

    fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Probed, E> {
        Ok(Probed::Whole(v))
    }

    fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Probed, E> {
        Ok(u64::try_from(v).map_or(Probed::NotWhole, Probed::Whole))
    }

    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Probed, E> {
        Ok(Probed::NotWhole)
    }

    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Probed, E> {
        Ok(Probed::NotNumber)
    }

    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Probed, E> {
        Ok(Probed::NotNumber)
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Probed, E> {
        Ok(Probed::NotNumber)
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Probed, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(Probed::NotNumber)
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Probed, A::Error> {
        while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
        Ok(Probed::NotNumber)
    }
}

impl<'de> serde::Deserialize<'de> for Probed {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Probed, D::Error> {
        d.deserialize_any(ProbedVisitor)
    }
}

/// The whole document: `None` when it isn't an object, else the field (if any).
struct Probe<'k> {
    key: &'k str,
}

impl<'de> DeserializeSeed<'de> for Probe<'_> {
    type Value = Option<Option<Probed>>;

    fn deserialize<D: Deserializer<'de>>(self, d: D) -> Result<Self::Value, D::Error> {
        d.deserialize_any(self)
    }
}

impl<'de> Visitor<'de> for Probe<'_> {
    type Value = Option<Option<Probed>>;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("a JSON object")
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
        let mut found = None;
        while let Some(name) = map.next_key::<std::borrow::Cow<'de, str>>()? {
            if name == self.key && found.is_none() {
                found = Some(map.next_value::<Probed>()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(Some(found))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
        while seq.next_element::<IgnoredAny>()?.is_some() {}
        Ok(None)
    }

    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self::Value, E> {
        Ok(None)
    }

    fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_field_is_read() {
        let doc = br#"{"schema": 1, "bad": -1, "text": "1", "float": 1.5, "deep": {"schema": 9},
            "list": [1], "nothing": null, "again": 3, "again": 4}"#;
        assert_eq!(whole_number_field(doc, "schema"), Ok(Some(1)));
        assert_eq!(whole_number_field(doc, "missing"), Ok(None));
        assert_eq!(whole_number_field(doc, "again"), Ok(Some(3)));
        assert_eq!(whole_number_field(doc, "bad"), Err(ProbeError::NotWhole));
        assert_eq!(whole_number_field(doc, "float"), Err(ProbeError::NotWhole));
        for key in ["text", "deep", "list", "nothing"] {
            assert_eq!(whole_number_field(doc, key), Err(ProbeError::NotNumber));
        }
        assert_eq!(whole_number_field(b"[1]", "x"), Err(ProbeError::NotObject));
        assert_eq!(whole_number_field(b"7", "x"), Err(ProbeError::NotObject));
        assert!(matches!(
            whole_number_field(b"{} {}", "x"),
            Err(ProbeError::Json(_))
        ));
        let deep = format!("{{\"x\":{}0{}}}", "[".repeat(40), "]".repeat(40));
        assert!(matches!(
            whole_number_field(deep.as_bytes(), "x"),
            Err(ProbeError::Shape(_))
        ));
    }

    #[test]
    fn escaped_keys_are_compared_unescaped() {
        assert_eq!(
            whole_number_field(br#"{"schema": 2}"#, "schema"),
            Ok(Some(2))
        );
    }
}
