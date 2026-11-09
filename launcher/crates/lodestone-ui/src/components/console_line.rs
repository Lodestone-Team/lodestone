//! A server console's line as the dashboard draws it (`components/ConsoleLine.tsx`):
//! ANSI and Minecraft colours and styles, made readable on the console's dark
//! background, text on a background colour as a badge, and plain lines coloured by
//! their log level, in the dark or the light theme. Parsing is
//! `lodestone_minecraft::console_format`.

use std::ops::Range;

use gpui::{
    FontStyle, FontWeight, HighlightStyle, SharedString, StrikethroughStyle, UnderlineStyle,
};
use lodestone_minecraft::console_format::{
    BADGE_PALETTE, BOLD, ConsoleColor, ConsoleLevel, ConsoleSegment, ConsoleTheme, DIM, INVERSE,
    ITALIC, RGB_BASE, STRIKE, UNDERLINE, luminance, readable_color, xterm256,
};
pub use lodestone_minecraft::console_format::{ParsedConsoleLine, parse_console_line};

use crate::theme::{Color, colors, px};

/// The dashboard's 16 console colours in its dark theme (`--console-0` … in
/// `globals.css`).
pub const CONSOLE_PALETTE: [u32; 16] = [
    0x979CA5, 0xF46F6B, 0x4FC46C, 0xD9B93C, 0x6AA5F2, 0xC985DD, 0x3FC2C9, 0xC9CBD0, 0xA2A7AF,
    0xFF8A85, 0x7AE58F, 0xF5E168, 0x93BEFF, 0xE8A6F6, 0x78E7EC, 0xF5F6F7,
];

/// The same in the light theme (`globals.css`'s light `--console-*`).
pub const CONSOLE_PALETTE_LIGHT: [u32; 16] = [
    0x1A1D23, 0xB4282C, 0x1D7A3A, 0x8A5A00, 0x1F5FBF, 0x8E3AA8, 0x0E6F7C, 0x555B66, 0x666C77,
    0xC62F2F, 0x0F7D3B, 0x8F6400, 0x2563D0, 0xA23AB6, 0x0A7684, 0x2B3038,
];

fn rgb(value: u32) -> gpui::Hsla {
    Color::rgb(value).into()
}

/// A text colour on the console background, readable in the current theme.
fn text_color(color: ConsoleColor) -> u32 {
    let light = crate::theme::palette::is_light();
    if color < 16 {
        let palette = if light {
            CONSOLE_PALETTE_LIGHT
        } else {
            CONSOLE_PALETTE
        };
        palette[color as usize]
    } else {
        let value = if color < RGB_BASE {
            xterm256(color)
        } else {
            color - RGB_BASE
        };
        let theme = if light {
            ConsoleTheme::Light
        } else {
            ConsoleTheme::Dark
        };
        readable_color(value, theme)
    }
}

/// The `0xRRGGBB` value of a colour on a badge, the same in both themes.
fn badge_value(color: ConsoleColor) -> u32 {
    if color < 16 {
        BADGE_PALETTE[color as usize]
    } else if color < RGB_BASE {
        xterm256(color)
    } else {
        color - RGB_BASE
    }
}

/// A segment's style, as the dashboard's `segmentStyle`.
pub fn segment_style(segment: &ConsoleSegment) -> HighlightStyle {
    // the console's own text and background colour, after an inverse swap
    enum Side {
        Color(ConsoleColor),
        DefaultFg,
        DefaultBg,
        None,
    }
    let as_side = |c: Option<ConsoleColor>| c.map_or(Side::None, Side::Color);
    let (fg, bg) = if segment.flags & INVERSE != 0 {
        (
            segment.bg.map_or(Side::DefaultBg, Side::Color),
            segment.fg.map_or(Side::DefaultFg, Side::Color),
        )
    } else {
        (as_side(segment.fg), as_side(segment.bg))
    };
    let mut style = HighlightStyle::default();
    match bg {
        Side::DefaultFg => {
            // inverse without colours: the console's text and background swapped
            style.background_color = Some(colors::GRAY_300.into());
            style.color = Some(match fg {
                Side::Color(c) => rgb(badge_value(c)),
                _ => colors::GRAY_900.into(),
            });
        }
        Side::Color(bg) => {
            // text on a background colour: a badge, readable on its own
            let background = badge_value(bg);
            style.background_color = Some(rgb(background));
            style.color = Some(match fg {
                Side::Color(c) => rgb(badge_value(c)),
                _ if luminance(background) > 0.179 => rgb(0x000000),
                _ => rgb(0xffffff),
            });
        }
        Side::DefaultBg | Side::None => {
            if let Side::Color(fg) = fg {
                style.color = Some(rgb(text_color(fg)));
            }
        }
    }
    if segment.flags & DIM != 0 {
        style.fade_out = Some(0.35);
    }
    if segment.flags & BOLD != 0 {
        style.font_weight = Some(FontWeight::BOLD);
    }
    if segment.flags & ITALIC != 0 {
        style.font_style = Some(FontStyle::Italic);
    }
    if segment.flags & UNDERLINE != 0 {
        style.underline = Some(UnderlineStyle {
            thickness: px(1.),
            ..Default::default()
        });
    }
    if segment.flags & STRIKE != 0 {
        style.strikethrough = Some(StrikethroughStyle {
            thickness: px(1.),
            ..Default::default()
        });
    }
    style
}

/// A console line, ready for a `StyledText`.
#[derive(Clone, Debug, PartialEq)]
pub struct ConsoleText {
    pub text: SharedString,
    pub highlights: Vec<(Range<usize>, HighlightStyle)>,
    /// The line's own colour: its log level's, or the console's text colour.
    pub color: Color,
}

/// How `line` is drawn: with its colours, or (`colored` off) as plain text in the
/// console's text colour.
pub fn console_text(line: &ParsedConsoleLine, colored: bool) -> ConsoleText {
    let color = match line.level.filter(|_| colored) {
        Some(ConsoleLevel::Warn) => colors::YELLOW,
        Some(ConsoleLevel::Error) => colors::RED_200,
        None => colors::GRAY_300,
    };
    if !colored || !line.styled {
        return ConsoleText {
            text: line.plain.clone().into(),
            highlights: Vec::new(),
            color,
        };
    }
    let mut text = String::with_capacity(line.plain.len());
    let mut highlights = Vec::new();
    for segment in &line.segments {
        let start = text.len();
        text.push_str(&segment.text);
        if !segment.is_plain() && text.len() > start {
            highlights.push((start..text.len(), segment_style(segment)));
        }
    }
    ConsoleText {
        text: text.into(),
        highlights,
        color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_and_levels() {
        let line = parse_console_line("\u{1b}[31mred\u{1b}[0m and §aGreen");
        let text = console_text(&line, true);
        assert_eq!(text.text.as_ref(), "red and Green");
        assert_eq!(text.highlights.len(), 2);
        assert_eq!(text.highlights[0].0, 0..3);
        let plain = console_text(&line, false);
        assert!(plain.highlights.is_empty());
        let warn = parse_console_line("[12:00:00] [Server thread/WARN]: Can't keep up!");
        assert_eq!(console_text(&warn, true).color, colors::YELLOW);
        assert_eq!(console_text(&warn, false).color, colors::GRAY_300);
    }
}
