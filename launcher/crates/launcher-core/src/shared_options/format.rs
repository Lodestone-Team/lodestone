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
    if let Control::Choice(choices) = control(spec)
        && let Some((_, label)) = choices.iter().find(|(v, _)| *v == value.trim())
    {
        return (*label).to_string();
    }
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
        Kind::Number => {
            let n = text.parse::<f64>().ok().filter(|n| n.is_finite());
            let Some(range) = number_range(spec.key) else {
                return match n {
                    Some(_) => Ok(text.to_string()),
                    None => invalid("enter a number."),
                };
            };
            match n {
                Some(n) if range.whole && n.fract() == 0.0 && range.contains(n) => {
                    Ok((n as i64).to_string())
                }
                Some(n) if !range.whole && range.contains(n) => Ok(text.to_string()),
                _ => invalid(&range.hint()),
            }
        }
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
            let name = if lower.starts_with("key.keyboard.")
                || lower.starts_with("key.mouse.")
                || lower.starts_with("scancode.")
            {
                lower
            } else {
                // `w`, `left.shift`: a key name without the prefix.
                format!("key.keyboard.{lower}")
            };
            if keycodes::is_known(&name) {
                Ok(name)
            } else {
                invalid(
                    "enter a key Minecraft knows, such as key.keyboard.w, key.keyboard.left.shift, key.mouse.4 or a single letter.",
                )
            }
        }
    }
}

/// How the launcher lets the user change a setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Control {
    /// An on/off switch; the values are `true` and `false`.
    Toggle,
    /// One of a few named states: (canonical value, label).
    Choice(&'static [(&'static str, &'static str)]),
    /// A number in steps, as in the game's own slider.
    Slider { min: f64, max: f64, step: f64 },
    /// Typed in: key bindings, the language, and numbers without a fixed range.
    Text,
}

const GRAPHICS: &[(&str, &str)] = &[
    ("fast", "Fast"),
    ("fancy", "Fancy"),
    ("fabulous", "Fabulous!"),
];
const PARTICLES: &[(&str, &str)] = &[("0", "All"), ("1", "Decreased"), ("2", "Minimal")];
const NARRATOR: &[(&str, &str)] = &[
    ("0", "Off"),
    ("1", "All"),
    ("2", "Chat only"),
    ("3", "System only"),
];
const CHAT: &[(&str, &str)] = &[("0", "Shown"), ("1", "Commands only"), ("2", "Hidden")];
const FILTERING: &[(&str, &str)] = &[("0", "None"), ("1", "RGSS"), ("2", "Anisotropic")];
const ANISOTROPY: &[(&str, &str)] = &[("1", "2×"), ("2", "4×"), ("3", "8×")];

/// The control for `spec`: a switch for on/off settings, a choice for settings with a
/// few named states, a slider for numbers the game changes in steps.
pub fn control(spec: &Spec) -> Control {
    match spec.kind {
        Kind::Bool | Kind::AmbientOcclusion => Control::Toggle,
        Kind::Graphics => Control::Choice(GRAPHICS),
        Kind::Blurriness => Control::Slider {
            min: 0.0,
            max: 10.0,
            step: 1.0,
        },
        Kind::KeyBinding | Kind::Language => Control::Text,
        Kind::Number => {
            let choices = match spec.key {
                "particles" => Some(PARTICLES),
                "narrator" => Some(NARRATOR),
                "chatVisibility" => Some(CHAT),
                "textureFiltering" => Some(FILTERING),
                "maxAnisotropyBit" => Some(ANISOTROPY),
                _ => None,
            };
            if let Some(choices) = choices {
                return Control::Choice(choices);
            }
            match number_range(spec.key) {
                // GUI scale's top depends on the window; it stays typed.
                Some(range) if range.max != f64::MAX => Control::Slider {
                    min: range.min,
                    max: range.max,
                    step: slider_step(spec.key, range),
                },
                _ => Control::Text,
            }
        }
    }
}

/// The step of a number's slider: what one notch changes in the game.
fn slider_step(key: &str, range: Range) -> f64 {
    match key {
        // One degree: stored as (degrees − 70) / 40.
        "fov" => 0.025,
        // Shown doubled, so this is one percent.
        "mouseSensitivity" => 0.005,
        "maxFps" => 10.0,
        "entityDistanceScaling" => 0.25,
        "notificationDisplayTime" => 0.5,
        "chunkSectionFadeInTime" | "chatDelay" => 0.1,
        _ if range.whole => 1.0,
        _ => 0.01,
    }
}

/// The canonical text of a slider position: snapped to its step, inside its range.
/// Whole steps are written as whole numbers; others keep the step's decimals.
pub fn slider_text(min: f64, max: f64, step: f64, value: f64) -> String {
    let steps = ((value - min) / step).round();
    let n = (min + steps * step).clamp(min, max);
    if step.fract() == 0.0 && min.fract() == 0.0 {
        return format!("{}", n.round() as i64);
    }
    let decimals = format!("{step}")
        .split_once('.')
        .map_or(0, |(_, frac)| frac.len());
    let text = format!("{n:.decimals$}");
    let text = text.trim_end_matches('0');
    if text.ends_with('.') {
        format!("{text}0")
    } else {
        text.to_string()
    }
}

/// The values the game accepts for a number setting (its `OptionInstance` range in
/// the 26.3 client), as stored in `options.txt`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Range {
    min: f64,
    max: f64,
    /// A whole number (a slider of steps, or a list such as particles).
    whole: bool,
}

impl Range {
    const UNIT: Range = Range::float(0.0, 1.0);

    const fn float(min: f64, max: f64) -> Self {
        Self {
            min,
            max,
            whole: false,
        }
    }

    const fn whole(min: f64, max: f64) -> Self {
        Self {
            min,
            max,
            whole: true,
        }
    }

    fn contains(&self, n: f64) -> bool {
        (self.min..=self.max).contains(&n)
    }

    fn hint(&self) -> String {
        if !self.whole {
            format!("enter a number from {} to {}.", self.min, self.max)
        } else if self.max == f64::MAX {
            format!("enter a whole number, {} or more.", self.min)
        } else {
            format!("enter a whole number from {} to {}.", self.min, self.max)
        }
    }
}

fn number_range(key: &str) -> Option<Range> {
    if key.starts_with("soundCategory_") {
        return Some(Range::UNIT);
    }
    Some(match key {
        // Stored as (degrees − 70) / 40: 30° to 110°.
        "fov" => Range::float(-1.0, 1.0),
        "renderDistance" => Range::whole(2.0, 32.0),
        "simulationDistance" => Range::whole(5.0, 32.0),
        // 0 is Auto; the game caps the rest at what the window fits.
        "guiScale" => Range::whole(0.0, f64::MAX),
        // 260 is Unlimited.
        "maxFps" => Range::whole(10.0, 260.0),
        // All, Decreased, Minimal.
        "particles" => Range::whole(0.0, 2.0),
        "mipmapLevels" => Range::whole(0.0, 4.0),
        "biomeBlendRadius" => Range::whole(0.0, 7.0),
        "entityDistanceScaling" => Range::float(0.5, 5.0),
        "cloudRange" => Range::whole(2.0, 128.0),
        "weatherRadius" => Range::whole(3.0, 10.0),
        "chunkSectionFadeInTime" => Range::float(0.0, 2.0),
        // None, RGSS, Anisotropic.
        "textureFiltering" => Range::whole(0.0, 2.0),
        // 2×, 4×, 8×.
        "maxAnisotropyBit" => Range::whole(1.0, 3.0),
        "mouseWheelSensitivity" => Range::float(0.01, 10.0),
        "sprintWindow" => Range::whole(0.0, 10.0),
        // Off, All, Chat, System.
        "narrator" => Range::whole(0.0, 3.0),
        // Shown, Commands only, Hidden.
        "chatVisibility" => Range::whole(0.0, 2.0),
        "chatDelay" => Range::float(0.0, 6.0),
        "notificationDisplayTime" => Range::float(0.5, 10.0),
        "mouseSensitivity"
        | "gamma"
        | "textBackgroundOpacity"
        | "chatOpacity"
        | "chatScale"
        | "chatWidth"
        | "chatHeightFocused"
        | "chatHeightUnfocused"
        | "chatLineSpacing"
        | "screenEffectScale"
        | "fovEffectScale"
        | "darknessEffectScale"
        | "damageTiltStrength"
        | "glintSpeed"
        | "glintStrength"
        | "panoramaScrollSpeed" => Range::UNIT,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shared_options::keys::{SPECS, find};

    #[test]
    fn controls_follow_what_a_setting_holds() {
        let control_of = |key| control(find(key).unwrap());
        assert_eq!(control_of("enableVsync"), Control::Toggle);
        assert_eq!(control_of("ao"), Control::Toggle);
        assert_eq!(control_of("narrator"), Control::Choice(NARRATOR));
        assert_eq!(control_of("graphicsPreset"), Control::Choice(GRAPHICS));
        assert_eq!(
            control_of("renderDistance"),
            Control::Slider {
                min: 2.0,
                max: 32.0,
                step: 1.0
            }
        );
        assert_eq!(
            control_of("soundCategory_master"),
            Control::Slider {
                min: 0.0,
                max: 1.0,
                step: 0.01
            }
        );
        assert_eq!(control_of("key_key.forward"), Control::Text);
        assert_eq!(control_of("lang"), Control::Text);
        assert_eq!(control_of("guiScale"), Control::Text);
    }

    #[test]
    fn every_choice_and_slider_value_is_accepted() {
        for spec in SPECS {
            match control(spec) {
                Control::Choice(choices) => {
                    for (value, label) in choices {
                        assert_eq!(
                            parse_input(spec, value).as_deref(),
                            Ok(*value),
                            "{}",
                            spec.key
                        );
                        assert_eq!(display(spec, value), *label, "{}", spec.key);
                    }
                }
                Control::Slider { min, max, step } => {
                    for n in [min, max, (min + max) / 2.0, min + step] {
                        let text = slider_text(min, max, step, n);
                        assert!(parse_input(spec, &text).is_ok(), "{} {text}", spec.key);
                    }
                }
                Control::Toggle => {
                    assert_eq!(parse_input(spec, "true").as_deref(), Ok("true"));
                    assert_eq!(parse_input(spec, "false").as_deref(), Ok("false"));
                }
                Control::Text => {}
            }
        }
    }

    #[test]
    fn slider_text_snaps_to_the_step() {
        assert_eq!(slider_text(2.0, 32.0, 1.0, 12.4), "12");
        assert_eq!(slider_text(2.0, 32.0, 1.0, 99.0), "32");
        assert_eq!(slider_text(0.0, 1.0, 0.01, 0.4567), "0.46");
        assert_eq!(slider_text(0.0, 1.0, 0.01, 1.0), "1.0");
        assert_eq!(slider_text(-1.0, 1.0, 0.025, 0.5), "0.5");
        assert_eq!(slider_text(10.0, 260.0, 10.0, 133.0), "130");
        assert_eq!(slider_text(0.5, 10.0, 0.5, 0.2), "0.5");
    }

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

    #[test]
    fn every_number_has_a_range() {
        use crate::shared_options::keys::SPECS;
        for spec in SPECS.iter().filter(|spec| spec.kind == Kind::Number) {
            assert!(number_range(spec.key).is_some(), "{}", spec.key);
        }
    }

    #[test]
    fn numbers_out_of_range_are_rejected() {
        let render = find("renderDistance").unwrap();
        assert_eq!(parse_input(render, "12").unwrap(), "12");
        assert_eq!(parse_input(render, "32.0").unwrap(), "32");
        assert_eq!(
            parse_input(render, "1699999").unwrap_err(),
            "Render distance: enter a whole number from 2 to 32."
        );
        assert!(parse_input(render, "1").is_err());
        assert!(parse_input(render, "12.5").is_err());
        let simulation = find("simulationDistance").unwrap();
        assert!(parse_input(simulation, "4").is_err());
        assert_eq!(parse_input(simulation, "5").unwrap(), "5");
        let fov = find("fov").unwrap();
        assert_eq!(parse_input(fov, "-0.25").unwrap(), "-0.25");
        assert_eq!(
            parse_input(fov, "90").unwrap_err(),
            "FOV: enter a number from -1 to 1."
        );
        let gamma = find("gamma").unwrap();
        assert_eq!(parse_input(gamma, "0.5").unwrap(), "0.5");
        assert!(parse_input(gamma, "15").is_err());
        assert!(parse_input(gamma, "-0.1").is_err());
        assert!(parse_input(find("soundCategory_music").unwrap(), "1.5").is_err());
        let gui = find("guiScale").unwrap();
        assert_eq!(parse_input(gui, "0").unwrap(), "0");
        assert_eq!(parse_input(gui, "6").unwrap(), "6");
        assert_eq!(
            parse_input(gui, "-1").unwrap_err(),
            "GUI scale: enter a whole number, 0 or more."
        );
        assert!(parse_input(find("maxFps").unwrap(), "5").is_err());
        assert!(parse_input(find("particles").unwrap(), "3").is_err());
        assert!(parse_input(render, "NaN").is_err());
        assert!(parse_input(render, "inf").is_err());
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let forward = find("key_key.forward").unwrap();
        assert_eq!(
            parse_input(forward, "Left.Shift").unwrap(),
            "key.keyboard.left.shift"
        );
        assert_eq!(parse_input(forward, "scancode.42").unwrap(), "scancode.42");
        assert_eq!(
            parse_input(forward, "key.mouse.left").unwrap(),
            "key.mouse.left"
        );
        for text in [
            "key.keyboard.wboguskey.keyboard.w",
            "wbogus",
            "left shift",
            "key.mouse.3",
            "scancode.x",
            "",
        ] {
            let err = parse_input(forward, text).unwrap_err();
            assert!(
                err.starts_with("Walk forwards: enter a key"),
                "{text}: {err}"
            );
        }
    }
}
