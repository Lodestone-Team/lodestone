//! Resource packs' `pack.mcmeta`: the description and the pack formats a pack is made
//! for, and which resource pack format each Minecraft release reads.

use std::cmp::Ordering;

use lodestone_minecraft::versions::compare_versions;
use serde_json::Value;

/// A pack format, `major.minor`. Before 1.21.9 formats were whole numbers (minor 0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PackFormat {
    pub major: u32,
    pub minor: u32,
}

impl PackFormat {
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}

impl std::fmt::Display for PackFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.minor == 0 {
            write!(f, "{}", self.major)
        } else {
            write!(f, "{}.{}", self.major, self.minor)
        }
    }
}

/// The formats a pack says it supports, inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FormatRange {
    pub min: PackFormat,
    pub max: PackFormat,
}

/// What `pack.mcmeta` says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PackMeta {
    /// The description as plain text.
    pub description: String,
    pub formats: Option<FormatRange>,
}

/// The resource pack format of each Minecraft release: the first and last release that
/// read it. From the Minecraft Wiki's "Pack format" page
/// (<https://minecraft.wiki/w/Pack_format>), read 2026-11-08.
pub const RESOURCE_FORMATS: &[(&str, &str, PackFormat)] = &[
    ("1.6.1", "1.8.9", PackFormat::new(1, 0)),
    ("1.9", "1.10.2", PackFormat::new(2, 0)),
    ("1.11", "1.12.2", PackFormat::new(3, 0)),
    ("1.13", "1.14.4", PackFormat::new(4, 0)),
    ("1.15", "1.16.1", PackFormat::new(5, 0)),
    ("1.16.2", "1.16.5", PackFormat::new(6, 0)),
    ("1.17", "1.17.1", PackFormat::new(7, 0)),
    ("1.18", "1.18.2", PackFormat::new(8, 0)),
    ("1.19", "1.19.2", PackFormat::new(9, 0)),
    ("1.19.3", "1.19.3", PackFormat::new(12, 0)),
    ("1.19.4", "1.19.4", PackFormat::new(13, 0)),
    ("1.20", "1.20.1", PackFormat::new(15, 0)),
    ("1.20.2", "1.20.2", PackFormat::new(18, 0)),
    ("1.20.3", "1.20.4", PackFormat::new(22, 0)),
    ("1.20.5", "1.20.6", PackFormat::new(32, 0)),
    ("1.21", "1.21.1", PackFormat::new(34, 0)),
    ("1.21.2", "1.21.3", PackFormat::new(42, 0)),
    ("1.21.4", "1.21.4", PackFormat::new(46, 0)),
    ("1.21.5", "1.21.5", PackFormat::new(55, 0)),
    ("1.21.6", "1.21.6", PackFormat::new(63, 0)),
    ("1.21.7", "1.21.8", PackFormat::new(64, 0)),
    ("1.21.9", "1.21.10", PackFormat::new(69, 0)),
    ("1.21.11", "1.21.11", PackFormat::new(75, 0)),
    ("26.1", "26.1.2", PackFormat::new(84, 0)),
    ("26.2", "26.2", PackFormat::new(88, 0)),
    ("26.3", "26.3", PackFormat::new(97, 1)),
];

/// Whether `version` looks like a release (`1.20.1`, `26.3`), not a snapshot.
fn is_release(version: &str) -> bool {
    let parts: Vec<&str> = version.split('.').collect();
    (2..=3).contains(&parts.len())
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()))
}

/// The resource pack format `game_version` reads, if it is a release in the table.
pub fn resource_format(game_version: &str) -> Option<PackFormat> {
    if !is_release(game_version) {
        return None;
    }
    RESOURCE_FORMATS
        .iter()
        .find(|(first, last, _)| {
            compare_versions(game_version, first) != Ordering::Less
                && compare_versions(game_version, last) != Ordering::Greater
        })
        .map(|(_, _, format)| *format)
}

/// Whether a pack fits the game, as the game's pack screen says.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compatibility {
    Compatible,
    /// "Made for an older version of Minecraft".
    TooOld,
    /// "Made for a newer version of Minecraft".
    TooNew,
    /// The game version or the pack's formats are unknown.
    Unknown,
}

impl Compatibility {
    /// The game's own warning, if any.
    pub fn warning(self) -> Option<&'static str> {
        match self {
            Compatibility::TooOld => Some("Made for an older version of Minecraft"),
            Compatibility::TooNew => Some("Made for a newer version of Minecraft"),
            Compatibility::Compatible | Compatibility::Unknown => None,
        }
    }
}

/// Whether a pack with `formats` fits `game_version`.
pub fn compatibility(formats: Option<&FormatRange>, game_version: &str) -> Compatibility {
    let (Some(formats), Some(game)) = (formats, resource_format(game_version)) else {
        return Compatibility::Unknown;
    };
    if formats.max < game {
        Compatibility::TooOld
    } else if formats.min > game {
        Compatibility::TooNew
    } else {
        Compatibility::Compatible
    }
}

/// A format value: a whole number, or `[major, minor]` (`min_format`/`max_format`).
/// `is_max` says how a bare major number is read: as `major.0` for a minimum, and as any
/// minor of that major for a maximum.
fn format_value(value: &Value, is_max: bool) -> Option<PackFormat> {
    let whole = |v: &Value| v.as_u64().and_then(|n| u32::try_from(n).ok());
    match value {
        Value::Number(_) => {
            let major = whole(value)?;
            Some(PackFormat::new(major, if is_max { u32::MAX } else { 0 }))
        }
        Value::Array(parts) => match parts.as_slice() {
            [major] => Some(PackFormat::new(
                whole(major)?,
                if is_max { u32::MAX } else { 0 },
            )),
            [major, minor] => Some(PackFormat::new(whole(major)?, whole(minor)?)),
            _ => None,
        },
        _ => None,
    }
}

/// The formats a `pack` object of `pack.mcmeta` declares: `min_format`/`max_format`
/// (25w31a and later), else `supported_formats` (a number, `[min, max]` or
/// `{min_inclusive, max_inclusive}`, 23w31a and later), else `pack_format`.
fn formats(pack: &serde_json::Map<String, Value>) -> Option<FormatRange> {
    if let (Some(min), Some(max)) = (pack.get("min_format"), pack.get("max_format"))
        && let (Some(min), Some(max)) = (format_value(min, false), format_value(max, true))
    {
        return Some(FormatRange { min, max });
    }
    if let Some(supported) = pack.get("supported_formats") {
        let range = match supported {
            Value::Number(_) => Some((supported, supported)),
            Value::Array(bounds) if bounds.len() == 2 => Some((&bounds[0], &bounds[1])),
            Value::Object(bounds) => bounds.get("min_inclusive").zip(bounds.get("max_inclusive")),
            _ => None,
        };
        if let Some((min, max)) = range
            && let (Some(min), Some(max)) = (format_value(min, false), format_value(max, true))
        {
            return Some(FormatRange { min, max });
        }
    }
    let format = pack.get("pack_format")?;
    Some(FormatRange {
        min: format_value(format, false)?,
        max: format_value(format, true)?,
    })
}

/// Reads `pack.mcmeta`. `None` if it isn't JSON with a `pack` object.
pub fn parse_pack_mcmeta(text: &str) -> Option<PackMeta> {
    let text = text.trim_start_matches('\u{feff}');
    let value: Value = serde_json::from_str(text).ok()?;
    let pack = value.get("pack")?.as_object()?;
    Some(PackMeta {
        description: pack
            .get("description")
            .map(plain_text)
            .unwrap_or_default()
            .trim()
            .to_string(),
        formats: formats(pack),
    })
}

/// A text component (a string, a list, or an object with `text`, `translate`
/// (with its `fallback`) and `extra`) as plain text, with `§` formatting codes removed.
pub fn plain_text(component: &Value) -> String {
    fn walk(value: &Value, out: &mut String, depth: usize) {
        if depth > 16 || out.len() > 4096 {
            return;
        }
        match value {
            Value::String(text) => out.push_str(text),
            Value::Number(n) => out.push_str(&n.to_string()),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Array(parts) => {
                for part in parts {
                    walk(part, out, depth + 1);
                }
            }
            Value::Object(object) => {
                if let Some(text) = object.get("text") {
                    walk(text, out, depth + 1);
                } else if let Some(fallback) = object.get("fallback") {
                    walk(fallback, out, depth + 1);
                } else if let Some(key) = object.get("translate").and_then(Value::as_str) {
                    out.push_str(key);
                }
                if let Some(extra) = object.get("extra") {
                    walk(extra, out, depth + 1);
                }
            }
            Value::Null => {}
        }
    }
    let mut out = String::new();
    walk(component, &mut out, 0);
    strip_formatting(&out)
}

/// Removes `§x` formatting codes.
pub fn strip_formatting(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '§' {
            chars.next();
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_of_releases() {
        assert_eq!(resource_format("1.20.1"), Some(PackFormat::new(15, 0)));
        assert_eq!(resource_format("1.20"), Some(PackFormat::new(15, 0)));
        assert_eq!(resource_format("1.8.9"), Some(PackFormat::new(1, 0)));
        assert_eq!(resource_format("1.21.1"), Some(PackFormat::new(34, 0)));
        assert_eq!(resource_format("1.21.10"), Some(PackFormat::new(69, 0)));
        assert_eq!(resource_format("1.21.11"), Some(PackFormat::new(75, 0)));
        assert_eq!(resource_format("26.1.1"), Some(PackFormat::new(84, 0)));
        assert_eq!(resource_format("26.3"), Some(PackFormat::new(97, 1)));
        assert_eq!(resource_format("1.5.2"), None);
        assert_eq!(resource_format("24w14a"), None);
        assert_eq!(resource_format("1.21-pre1"), None);
        assert_eq!(resource_format("99.1"), None);
        // the table has no gaps or overlaps between releases
        for pair in RESOURCE_FORMATS.windows(2) {
            assert!(pair[0].2 < pair[1].2, "{pair:?}");
            assert_eq!(compare_versions(pair[0].1, pair[1].0), Ordering::Less);
        }
    }

    #[test]
    fn reads_pack_mcmeta() {
        let meta =
            parse_pack_mcmeta(r#"{"pack": {"pack_format": 15, "description": "§6Gold §lpack"}}"#)
                .unwrap();
        assert_eq!(meta.description, "Gold pack");
        assert_eq!(
            meta.formats,
            Some(FormatRange {
                min: PackFormat::new(15, 0),
                max: PackFormat::new(15, u32::MAX)
            })
        );
        let meta = parse_pack_mcmeta(
            r#"{"pack": {"pack_format": 15, "supported_formats": [15, 34],
                "description": [{"text": "A "}, {"text": "pack", "color": "red", "extra": ["!"]}]}}"#,
        )
        .unwrap();
        assert_eq!(meta.description, "A pack!");
        assert_eq!(meta.formats.unwrap().max.major, 34);
        let meta = parse_pack_mcmeta(
            r#"{"pack": {"pack_format": 18, "supported_formats": {"min_inclusive": 16, "max_inclusive": 22},
                "description": {"translate": "pack.x", "fallback": "Fallback"}}}"#,
        )
        .unwrap();
        assert_eq!(meta.description, "Fallback");
        assert_eq!(meta.formats.unwrap().min.major, 16);
        let meta = parse_pack_mcmeta(
            "\u{feff}{\"pack\": {\"min_format\": [69, 0], \"max_format\": 97, \"description\": \"New\"}}",
        )
        .unwrap();
        assert_eq!(
            meta.formats,
            Some(FormatRange {
                min: PackFormat::new(69, 0),
                max: PackFormat::new(97, u32::MAX)
            })
        );
        assert!(parse_pack_mcmeta("{}").is_none());
        assert!(parse_pack_mcmeta("not json").is_none());
        assert_eq!(
            parse_pack_mcmeta(r#"{"pack": {"description": "x"}}"#)
                .unwrap()
                .formats,
            None
        );
    }

    #[test]
    fn compatibility_badges() {
        let range = |min: (u32, u32), max: (u32, u32)| FormatRange {
            min: PackFormat::new(min.0, min.1),
            max: PackFormat::new(max.0, max.1),
        };
        let pack_15 = range((15, 0), (15, u32::MAX));
        assert_eq!(
            compatibility(Some(&pack_15), "1.20.1"),
            Compatibility::Compatible
        );
        assert_eq!(
            compatibility(Some(&pack_15), "1.21.1"),
            Compatibility::TooOld
        );
        assert_eq!(
            compatibility(Some(&pack_15), "1.19.4"),
            Compatibility::TooNew
        );
        assert_eq!(
            compatibility(Some(&pack_15), "23w31a"),
            Compatibility::Unknown
        );
        assert_eq!(compatibility(None, "1.20.1"), Compatibility::Unknown);
        // supported_formats widens the range
        let wide = range((15, 0), (34, u32::MAX));
        assert_eq!(
            compatibility(Some(&wide), "1.21.1"),
            Compatibility::Compatible
        );
        // minor versions: 26.3 reads 97.1
        assert_eq!(
            compatibility(Some(&range((97, 0), (97, u32::MAX))), "26.3"),
            Compatibility::Compatible
        );
        assert_eq!(
            compatibility(Some(&range((97, 2), (98, 0))), "26.3"),
            Compatibility::TooNew
        );
        assert_eq!(
            compatibility(Some(&range((88, 0), (97, 0))), "26.3"),
            Compatibility::TooOld
        );
        assert_eq!(
            Compatibility::TooOld.warning(),
            Some("Made for an older version of Minecraft")
        );
        assert_eq!(PackFormat::new(97, 1).to_string(), "97.1");
        assert_eq!(PackFormat::new(15, 0).to_string(), "15");
    }
}
