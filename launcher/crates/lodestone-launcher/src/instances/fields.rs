//! The game settings both Settings pages edit: memory, arguments, the window and
//! environment variables. The launcher's Settings page edits the global values; an
//! instance's Settings tab edits its overrides of them.

use gpui::{App, AppContext as _, Context, Entity, IntoElement, SharedString, div, prelude::*};
use launcher_core::{
    instances::{Resolution, WindowSettings, format_env, parse_env},
    settings::max_ram_is_high,
};

use crate::{
    components::TextInput,
    instances::thousands,
    theme::{self, text, weights},
};

/// Parses memory in MiB, with an optional `G`/`M` suffix: `4G`, `4096`, `512M`.
pub fn parse_ram(text: &str) -> Option<u32> {
    let text = text.trim().to_ascii_uppercase();
    let (number, factor) = if let Some(gib) = text.strip_suffix("GB").or(text.strip_suffix('G')) {
        (gib.trim(), 1024.0)
    } else if let Some(mib) = text.strip_suffix("MB").or(text.strip_suffix('M')) {
        (mib.trim(), 1.0)
    } else {
        (text.as_str(), 1.0)
    };
    let value: f64 = number.parse().ok()?;
    let mib = (value * factor).round();
    (256.0..=1024.0 * 1024.0)
        .contains(&mib)
        .then_some(mib as u32)
}

/// What is wrong with the memory fields as typed: an error that stops saving, and a
/// warning that does not.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MemoryCheck {
    pub values: Option<(u32, u32)>,
    pub error: Option<String>,
    pub warning: Option<String>,
}

pub fn check_memory(min: &str, max: &str, total_bytes: Option<u64>) -> MemoryCheck {
    let (Some(min), Some(max)) = (parse_ram(min), parse_ram(max)) else {
        return MemoryCheck {
            error: Some(
                "Enter memory in MB (such as 4096) or GB (such as 4G), at least 256 MB.".into(),
            ),
            ..MemoryCheck::default()
        };
    };
    if min > max {
        return MemoryCheck {
            error: Some("The minimum is more than the maximum.".into()),
            ..MemoryCheck::default()
        };
    }
    let warning = max_ram_is_high(max, total_bytes).then(|| {
        "That is more than three quarters of this Mac's memory, which leaves too little for macOS and your other apps.".to_string()
    });
    MemoryCheck {
        values: Some((min, max)),
        error: None,
        warning,
    }
}

/// "This Mac has 16 GB of memory."
pub fn total_memory_label(total_bytes: Option<u64>) -> Option<SharedString> {
    total_bytes.map(|bytes| {
        format!(
            "This Mac has {} GB of memory.",
            thousands(bytes / (1024 * 1024 * 1024))
        )
        .into()
    })
}

/// "512 MB – 4,096 MB".
pub fn memory_label((min, max): (u32, u32)) -> String {
    format!(
        "{} MB – {} MB",
        thousands(min.into()),
        thousands(max.into())
    )
}

pub fn window_label(window: &WindowSettings) -> String {
    let size = match window.resolution {
        Some(r) => format!("{} × {}", r.width, r.height),
        None => "The game's default size".to_string(),
    };
    if window.fullscreen {
        format!("{size}, fullscreen")
    } else {
        size
    }
}

/// The text fields of the game settings.
pub struct Fields {
    pub min_ram: Entity<TextInput>,
    pub max_ram: Entity<TextInput>,
    pub jvm_args: Entity<TextInput>,
    pub game_args: Entity<TextInput>,
    pub width: Entity<TextInput>,
    pub height: Entity<TextInput>,
    pub env: Entity<TextInput>,
    pub fullscreen: bool,
}

pub fn input<V: 'static>(
    cx: &mut Context<V>,
    text: String,
    placeholder: &str,
) -> Entity<TextInput> {
    let placeholder = placeholder.to_string();
    cx.new(|cx| {
        TextInput::new(cx)
            .with_placeholder(placeholder)
            .with_text(text)
    })
}

/// The values the fields start with.
pub struct Values<'a> {
    pub memory: (u32, u32),
    pub jvm_args: &'a str,
    pub game_args: &'a str,
    pub window: WindowSettings,
    pub env: &'a std::collections::BTreeMap<String, String>,
}

impl Fields {
    pub fn new<V: 'static>(values: &Values<'_>, cx: &mut Context<V>) -> Self {
        let (width, height) = values
            .window
            .resolution
            .map(|r| (r.width.to_string(), r.height.to_string()))
            .unwrap_or_default();
        Self {
            min_ram: input(cx, values.memory.0.to_string(), "512"),
            max_ram: input(cx, values.memory.1.to_string(), "4096"),
            jvm_args: input(cx, values.jvm_args.to_string(), "-XX:+UseG1GC"),
            game_args: input(
                cx,
                values.game_args.to_string(),
                "--quickPlaySingleplayer World",
            ),
            width: input(cx, width, "854"),
            height: input(cx, height, "480"),
            env: input(cx, format_env(values.env), "NAME=value"),
            fullscreen: values.window.fullscreen,
        }
    }

    /// Puts `values` back in the fields, for "Discard".
    pub fn set<V: 'static>(&mut self, values: &Values<'_>, cx: &mut Context<V>) {
        let (width, height) = values
            .window
            .resolution
            .map(|r| (r.width.to_string(), r.height.to_string()))
            .unwrap_or_default();
        let texts = [
            (&self.min_ram, values.memory.0.to_string()),
            (&self.max_ram, values.memory.1.to_string()),
            (&self.jvm_args, values.jvm_args.to_string()),
            (&self.game_args, values.game_args.to_string()),
            (&self.width, width),
            (&self.height, height),
            (&self.env, format_env(values.env)),
        ];
        for (input, text_) in texts {
            input.update(cx, |input, cx| input.set_text(text_, cx));
        }
        self.fullscreen = values.window.fullscreen;
    }

    fn text(input: &Entity<TextInput>, cx: &App) -> String {
        input.read(cx).text().trim().to_string()
    }

    pub fn memory(&self, total_bytes: Option<u64>, cx: &App) -> MemoryCheck {
        check_memory(
            &Self::text(&self.min_ram, cx),
            &Self::text(&self.max_ram, cx),
            total_bytes,
        )
    }

    pub fn jvm_args(&self, cx: &App) -> String {
        Self::text(&self.jvm_args, cx)
    }

    pub fn game_args(&self, cx: &App) -> String {
        Self::text(&self.game_args, cx)
    }

    pub fn window(&self, cx: &App) -> Result<WindowSettings, String> {
        let (width, height) = (Self::text(&self.width, cx), Self::text(&self.height, cx));
        let resolution = match (width.is_empty(), height.is_empty()) {
            (true, true) => None,
            _ => {
                let parse = |s: &str| s.parse::<u32>().ok().filter(|v| (100..=16384).contains(v));
                match (parse(&width), parse(&height)) {
                    (Some(width), Some(height)) => Some(Resolution { width, height }),
                    _ => return Err("The window size needs a width and a height in pixels.".into()),
                }
            }
        };
        Ok(WindowSettings {
            resolution,
            fullscreen: self.fullscreen,
        })
    }

    pub fn env(&self, cx: &App) -> Result<std::collections::BTreeMap<String, String>, String> {
        parse_env(&Self::text(&self.env, cx)).map_err(|err| format!("Environment variables: {err}"))
    }
}

/// A line of feedback under a field.
pub fn status(text_: impl Into<SharedString>, color: theme::Color) -> impl IntoElement {
    div()
        .text_size(theme::text_size(text::SMALL))
        .font_weight(weights::BOLD.into())
        .text_color(color)
        .child(text_.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_memory() {
        assert_eq!(parse_ram("4096"), Some(4096));
        assert_eq!(parse_ram("4G"), Some(4096));
        assert_eq!(parse_ram(" 2.5 gb "), Some(2560));
        assert_eq!(parse_ram("512M"), Some(512));
        assert_eq!(parse_ram("100"), None);
        assert_eq!(parse_ram("lots"), None);
        assert_eq!(parse_ram(""), None);
    }

    #[test]
    fn checks_memory() {
        let total = Some(16u64 << 30);
        assert_eq!(check_memory("512", "4G", total).values, Some((512, 4096)));
        assert!(check_memory("512", "4G", total).warning.is_none());
        let backwards = check_memory("8G", "4G", total);
        assert_eq!(backwards.values, None);
        assert!(backwards.error.unwrap().contains("more than the maximum"));
        let high = check_memory("1G", "14G", total);
        assert_eq!(high.values, Some((1024, 14336)));
        assert!(high.warning.unwrap().contains("three quarters"));
        assert!(check_memory("x", "4G", total).error.is_some());
        assert_eq!(memory_label((512, 4096)), "512 MB – 4,096 MB");
        assert_eq!(
            total_memory_label(total).unwrap(),
            "This Mac has 16 GB of memory."
        );
    }
}
