//! Converting values between the shared (canonical) format and the format of an
//! `options.txt` at a data version, plus how the launcher shows and accepts them.

use super::{
    keycodes,
    keys::{AO_BOOL_SINCE, BLUR_INT_SINCE, Codec, Kind, LOWERCASE_LANGUAGE_SINCE, Spec},
};

/// The canonical value of `raw`, a value from a file of data version `dv`. `None` when
/// it can't be read.
pub fn read(codec: Codec, dv: u32, raw: &str) -> Option<String> {
    let raw = raw.trim();
    match codec {
        Codec::Plain => (!raw.is_empty()).then(|| raw.to_string()),
        Codec::Bool => parse_bool(raw).map(bool_text),
        Codec::KeyBinding => keycodes::to_canonical(raw, dv),
        Codec::Language => (!raw.is_empty()).then(|| raw.to_lowercase()),
        Codec::AmbientOcclusion => match raw {
            // `OptionsAmbientOcclusionFix`: off, minimum, maximum.
            "0" => Some(bool_text(false)),
            "1" | "2" => Some(bool_text(true)),
            _ => parse_bool(raw).map(bool_text),
        },
        Codec::Blurriness => {
            if dv < BLUR_INT_SINCE && raw.contains('.') {
                // `OptionsMenuBlurrinessFix`: Math.round(f * 10).
                let value: f32 = raw.parse().ok()?;
                Some(((value * 10.0).round() as i64).to_string())
            } else {
                raw.parse::<i64>().ok().map(|n| n.to_string())
            }
        }
        Codec::FancyGraphics => {
            parse_bool(raw).map(|fancy| if fancy { "fancy" } else { "fast" }.to_string())
        }
        Codec::GraphicsMode => match raw {
            "0" => Some("fast".into()),
            "1" => Some("fancy".into()),
            "2" => Some("fabulous".into()),
            _ => None,
        },
        Codec::GraphicsPreset => {
            let preset = serde_json::from_str::<String>(raw)
                .ok()
                .unwrap_or_else(|| raw.to_string());
            is_preset(&preset).then_some(preset)
        }
    }
}

/// The value to write for the canonical `value` into a file of data version `dv`.
/// `None` when that version can't hold it, so the instance's line is left alone.
pub fn write(codec: Codec, dv: u32, value: &str) -> Option<String> {
    let value = value.trim();
    match codec {
        Codec::Plain => (!value.is_empty()).then(|| value.to_string()),
        Codec::Bool => parse_bool(value).map(bool_text),
        Codec::KeyBinding => keycodes::from_canonical(value, dv),
        Codec::Language => {
            if value.is_empty() {
                None
            } else if dv < LOWERCASE_LANGUAGE_SINCE {
                Some(legacy_language(value))
            } else {
                Some(value.to_lowercase())
            }
        }
        Codec::AmbientOcclusion => {
            let on = parse_bool(value)?;
            Some(if dv < AO_BOOL_SINCE {
                if on { "2" } else { "0" }.to_string()
            } else {
                bool_text(on)
            })
        }
        Codec::Blurriness => {
            let n: i64 = value.parse().ok()?;
            Some(if dv < BLUR_INT_SINCE {
                format!("{:?}", n as f64 / 10.0)
            } else {
                n.to_string()
            })
        }
        Codec::FancyGraphics => match value {
            "fast" => Some("false".into()),
            "fancy" | "fabulous" => Some("true".into()),
            _ => None,
        },
        Codec::GraphicsMode => match value {
            "fast" => Some("0".into()),
            "fancy" => Some("1".into()),
            "fabulous" => Some("2".into()),
            _ => None,
        },
        Codec::GraphicsPreset => {
            is_preset(value).then(|| serde_json::to_string(value).unwrap_or_default())
        }
    }
}

/// Whether two canonical values are the same setting: numbers by value (`1` and
/// `1.0`), anything else as text.
pub fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.trim(), b.trim());
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => (x - y).abs() < 1e-9,
        _ => a == b,
    }
}

fn parse_bool(text: &str) -> Option<bool> {
    match text {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn bool_text(value: bool) -> String {
    value.to_string()
}

fn is_preset(value: &str) -> bool {
    matches!(value, "fast" | "fancy" | "fabulous" | "custom")
}

/// `en_us` as releases before 1.11 name it: `en_US`.
fn legacy_language(code: &str) -> String {
    match code.rsplit_once('_') {
        Some((language, region)) => {
            format!("{}_{}", language.to_lowercase(), region.to_uppercase())
        }
        None => code.to_string(),
    }
}

/// The value as the launcher shows it.
pub fn display(spec: &Spec, value: &str) -> String {
    let number = value.trim().parse::<f64>().ok();
    let percent = |n: f64| format!("{}%", (n * 100.0).round() as i64);
    match spec.kind {
        Kind::Bool | Kind::AmbientOcclusion => match value {
            "true" => "On".into(),
            "false" => "Off".into(),
            other => other.into(),
        },
        Kind::KeyBinding => keycodes::label(value),
        Kind::Graphics => match value {
            "fast" => "Fast".into(),
            "fancy" => "Fancy".into(),
            "fabulous" => "Fabulous!".into(),
            "custom" => "Custom".into(),
            other => other.into(),
        },
        Kind::Language | Kind::Blurriness => value.into(),
        Kind::Number => match (spec.key, number) {
            // Stored as (degrees − 70) / 40.
            ("fov", Some(n)) => format!("{}°", (70.0 + n * 40.0).round() as i64),
            // The game shows the slider doubled: 0.5 is 100%.
            ("mouseSensitivity", Some(n)) => percent(n * 2.0),
            ("guiScale", Some(0.0)) => "Auto".into(),
            ("particles", Some(n)) => match n as i64 {
                0 => "All".into(),
                1 => "Decreased".into(),
                2 => "Minimal".into(),
                _ => value.into(),
            },
            ("maxFps", Some(n)) if n >= 260.0 => "Unlimited".into(),
            ("renderDistance" | "simulationDistance", Some(n)) => {
                format!("{} chunks", n as i64)
            }
            (key, Some(n))
                if key.starts_with("soundCategory_")
                    || matches!(
                        key,
                        "gamma" | "chatOpacity" | "textBackgroundOpacity" | "chatScale"
                    ) =>
            {
                percent(n)
            }
            _ => value.into(),
        },
    }
}

/// Reads a value typed into the launcher as the canonical value of `spec`.
pub fn parse_input(spec: &Spec, text: &str) -> Result<String, String> {
    let text = text.trim();
    let invalid = |hint: &str| Err(format!("{}: {hint}", spec.label));
    match spec.kind {
        Kind::Bool | Kind::AmbientOcclusion => match text.to_lowercase().as_str() {
            "true" | "on" | "yes" => Ok("true".into()),
            "false" | "off" | "no" => Ok("false".into()),
            _ => invalid("enter On or Off."),
        },
        Kind::Number => match text.parse::<f64>() {
            Ok(n) if n.is_finite() => Ok(text.to_string()),
            _ => invalid("enter a number."),
        },
        Kind::Blurriness => match text.parse::<i64>() {
            Ok(n) if (0..=10).contains(&n) => Ok(n.to_string()),
            _ => invalid("enter a whole number from 0 to 10."),
        },
        Kind::Language => {
            if !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                Ok(text.to_lowercase())
            } else {
                invalid("enter a language code such as en_us.")
            }
        }
        Kind::Graphics => {
            let value = text.to_lowercase().trim_end_matches('!').to_string();
            if is_preset(&value) {
                Ok(value)
            } else {
                invalid("enter Fast, Fancy, Fabulous or Custom.")
            }
        }
        Kind::KeyBinding => {
            let lower = text.to_lowercase();
            if lower.starts_with("key.keyboard.")
                || lower.starts_with("key.mouse.")
                || lower.starts_with("scancode.")
            {
                Ok(lower)
            } else if !lower.is_empty() && !lower.contains(char::is_whitespace) {
                // `w`, `left.shift`: a key name without the prefix.
                Ok(format!("key.keyboard.{lower}"))
            } else {
                invalid("enter a key name such as key.keyboard.w or a single letter.")
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_options::keys::find;

    #[test]
    fn graphics_across_versions() {
        let fancy = |codec, dv, raw| read(codec, dv, raw);
        assert_eq!(
            fancy(Codec::FancyGraphics, 1343, "true").as_deref(),
            Some("fancy")
        );
        assert_eq!(
            fancy(Codec::FancyGraphics, 99, "false").as_deref(),
            Some("fast")
        );
        assert_eq!(
            fancy(Codec::GraphicsMode, 3465, "2").as_deref(),
            Some("fabulous")
        );
        assert_eq!(
            fancy(Codec::GraphicsPreset, 4671, "\"fancy\"").as_deref(),
            Some("fancy")
        );
        assert_eq!(
            write(Codec::GraphicsPreset, 4671, "fast").as_deref(),
            Some("\"fast\"")
        );
        assert_eq!(
            write(Codec::GraphicsMode, 3465, "fabulous").as_deref(),
            Some("2")
        );
        assert_eq!(
            write(Codec::FancyGraphics, 1343, "fabulous").as_deref(),
            Some("true")
        );
        // A custom preset has no equivalent before 1.21.11.
        assert_eq!(write(Codec::GraphicsMode, 3465, "custom"), None);
        assert_eq!(write(Codec::FancyGraphics, 99, "custom"), None);
    }

    #[test]
    fn fixed_up_formats() {
        assert_eq!(
            write(Codec::Language, 99, "en_us").as_deref(),
            Some("en_US")
        );
        assert_eq!(
            write(Codec::Language, 512, "zh_cn").as_deref(),
            Some("zh_CN")
        );
        assert_eq!(
            write(Codec::Language, 1343, "en_us").as_deref(),
            Some("en_us")
        );
        assert_eq!(read(Codec::Language, 99, "en_US").as_deref(), Some("en_us"));
        assert_eq!(
            read(Codec::AmbientOcclusion, 1343, "2").as_deref(),
            Some("true")
        );
        assert_eq!(
            read(Codec::AmbientOcclusion, 1343, "0").as_deref(),
            Some("false")
        );
        assert_eq!(
            write(Codec::AmbientOcclusion, 1343, "true").as_deref(),
            Some("2")
        );
        assert_eq!(
            write(Codec::AmbientOcclusion, 3465, "true").as_deref(),
            Some("true")
        );
        assert_eq!(read(Codec::Blurriness, 3837, "0.5").as_deref(), Some("5"));
        assert_eq!(write(Codec::Blurriness, 3837, "5").as_deref(), Some("0.5"));
        assert_eq!(write(Codec::Blurriness, 4671, "5").as_deref(), Some("5"));
        assert_eq!(read(Codec::Blurriness, 4671, "7").as_deref(), Some("7"));
    }

    #[test]
    fn numbers_compare_by_value() {
        assert!(same("1", "1.0"));
        assert!(same("0.5", "0.50"));
        assert!(!same("0.5", "0.6"));
        assert!(same("key.keyboard.w", "key.keyboard.w"));
        assert!(!same("true", "false"));
    }

    #[test]
    fn display_and_input() {
        let fov = find("fov").unwrap();
        assert_eq!(display(fov, "0.0"), "70°");
        assert_eq!(display(fov, "1.0"), "110°");
        let sensitivity = find("mouseSensitivity").unwrap();
        assert_eq!(display(sensitivity, "0.5"), "100%");
        let forward = find("key_key.forward").unwrap();
        assert_eq!(display(forward, "key.keyboard.w"), "W");
        assert_eq!(parse_input(forward, "W").unwrap(), "key.keyboard.w");
        assert_eq!(parse_input(forward, "key.mouse.4").unwrap(), "key.mouse.4");
        assert!(parse_input(sensitivity, "fast").is_err());
        let graphics = find("graphicsPreset").unwrap();
        assert_eq!(parse_input(graphics, "Fabulous!").unwrap(), "fabulous");
        assert!(parse_input(find("menuBackgroundBlurriness").unwrap(), "11").is_err());
    }
}
