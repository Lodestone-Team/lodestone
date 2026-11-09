//! Minecraft's text formatting, as resource packs, shader packs, worlds and servers
//! use it in their names and descriptions: the legacy `§` codes (`§6Gold §lbold`)
//! and JSON text components (`{"text": "Hi", "color": "gold", "extra": [...]}`).
//!
//! [`parse`] and [`parse_component`] turn either into plain text plus styled spans;
//! [`strip`] gives only the text, for sorting, searching, titles and logs.

use std::ops::Range;

use serde_json::Value;

/// How a span of text is drawn. `None` and `false` keep the surrounding style.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Style {
    /// `0xRRGGBB`.
    pub color: Option<u32>,
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
    /// `§k`: the game scrambles these glyphs.
    pub obfuscated: bool,
}

impl Style {
    pub fn is_plain(&self) -> bool {
        *self == Style::default()
    }
}

/// Text without its formatting codes, and the styled ranges of it (byte ranges, in
/// order, not overlapping). Ranges with the plain style are left out.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Formatted {
    pub text: String,
    pub spans: Vec<(Range<usize>, Style)>,
}

impl Formatted {
    fn push(&mut self, text: &str, style: Style) {
        if text.is_empty() {
            return;
        }
        let start = self.text.len();
        self.text.push_str(text);
        if style.is_plain() {
            return;
        }
        // Join with the previous span when the style continues.
        if let Some((range, last)) = self.spans.last_mut()
            && range.end == start
            && *last == style
        {
            range.end = self.text.len();
            return;
        }
        self.spans.push((start..self.text.len(), style));
    }

    fn append(&mut self, other: Formatted) {
        let offset = self.text.len();
        self.text.push_str(&other.text);
        for (range, style) in other.spans {
            self.spans
                .push((range.start + offset..range.end + offset, style));
        }
    }

    /// Whether any of the text is styled.
    pub fn is_styled(&self) -> bool {
        !self.spans.is_empty()
    }
}

/// Minecraft's sixteen colours, by code (`0`–`f`) and name.
const COLORS: [(char, &str, u32); 16] = [
    ('0', "black", 0x000000),
    ('1', "dark_blue", 0x0000AA),
    ('2', "dark_green", 0x00AA00),
    ('3', "dark_aqua", 0x00AAAA),
    ('4', "dark_red", 0xAA0000),
    ('5', "dark_purple", 0xAA00AA),
    ('6', "gold", 0xFFAA00),
    ('7', "gray", 0xAAAAAA),
    ('8', "dark_gray", 0x555555),
    ('9', "blue", 0x5555FF),
    ('a', "green", 0x55FF55),
    ('b', "aqua", 0x55FFFF),
    ('c', "red", 0xFF5555),
    ('d', "light_purple", 0xFF55FF),
    ('e', "yellow", 0xFFFF55),
    ('f', "white", 0xFFFFFF),
];

fn code_color(code: char) -> Option<u32> {
    let code = code.to_ascii_lowercase();
    COLORS
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, _, rgb)| *rgb)
}

/// A JSON component's `color`: a name or `#RRGGBB`.
fn named_color(name: &str) -> Option<u32> {
    if let Some(hex) = name.strip_prefix('#') {
        return (hex.len() == 6)
            .then(|| u32::from_str_radix(hex, 16).ok())
            .flatten();
    }
    COLORS
        .iter()
        .find(|(_, n, _)| n.eq_ignore_ascii_case(name))
        .map(|(_, _, rgb)| *rgb)
}

/// Text with legacy `§` codes: colours reset the formats, `§r` resets everything,
/// unknown codes and a trailing `§` are dropped. Spigot's hex form (`§x§R§R§G§G§B§B`)
/// sets a colour too.
pub fn parse(text: &str) -> Formatted {
    parse_from(text, '§', Style::default())
}

/// Like [`parse`], also reading `&` codes, which some pack and server names use in
/// place of `§`. Only for names that clearly use them: see [`uses_ampersand_codes`].
pub fn parse_with_ampersand(text: &str) -> Formatted {
    if uses_ampersand_codes(text) {
        parse(&text.replace('&', "§"))
    } else {
        parse(text)
    }
}

/// Whether `text` uses `&` as a code marker: no `§`, and it starts with an `&` code
/// followed by at least one more, or has three or more. "Rock & Roll", "R&D" and
/// "R&D &a" stay as they are.
pub fn uses_ampersand_codes(text: &str) -> bool {
    if text.contains('§') {
        return false;
    }
    let mut count = 0;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '&'
            && let Some(next) = chars.peek()
            && is_code(*next)
        {
            count += 1;
        }
    }
    let starts = text
        .trim_start()
        .strip_prefix('&')
        .and_then(|rest| rest.chars().next())
        .is_some_and(is_code);
    (starts && count >= 2) || count >= 3
}

fn is_code(c: char) -> bool {
    matches!(c.to_ascii_lowercase(), '0'..='9' | 'a'..='f' | 'k'..='o' | 'r' | 'x')
}

fn parse_from(text: &str, marker: char, base: Style) -> Formatted {
    let mut out = Formatted::default();
    let mut style = base;
    let mut run = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c != marker {
            run.push(c);
            continue;
        }
        let Some(code) = chars.next() else {
            break;
        };
        out.push(&run, style);
        run.clear();
        let code = code.to_ascii_lowercase();
        if let Some(rgb) = code_color(code) {
            // A colour code also ends bold, italic and the rest, as in the game.
            style = Style {
                color: Some(rgb),
                ..Style::default()
            };
            continue;
        }
        match code {
            'l' => style.bold = true,
            'o' => style.italic = true,
            'n' => style.underlined = true,
            'm' => style.strikethrough = true,
            'k' => style.obfuscated = true,
            'r' => style = base,
            'x' => {
                // §x§R§R§G§G§B§B
                let mut hex = String::new();
                let mut probe = chars.clone();
                for _ in 0..6 {
                    match (probe.next(), probe.next()) {
                        (Some(m), Some(d)) if m == marker && d.is_ascii_hexdigit() => hex.push(d),
                        _ => break,
                    }
                }
                if hex.len() == 6 {
                    chars = probe;
                    style = Style {
                        color: u32::from_str_radix(&hex, 16).ok(),
                        ..Style::default()
                    };
                }
            }
            // An unknown code is dropped, as the game does.
            _ => {}
        }
    }
    out.push(&run, style);
    out
}

/// A JSON text component, as `pack.mcmeta`'s `description` can be: a string (which
/// may hold `§` codes), an object with `text`, styles and `extra` children, or an
/// array whose first element styles the rest. `translate` and `keybind` show their
/// `fallback` or their key. Nesting is followed up to a sane depth.
pub fn parse_component(value: &Value) -> Formatted {
    let mut out = Formatted::default();
    component_into(value, Style::default(), 0, &mut out);
    out
}

const MAX_DEPTH: usize = 32;

fn component_into(value: &Value, inherited: Style, depth: usize, out: &mut Formatted) {
    if depth > MAX_DEPTH {
        return;
    }
    match value {
        Value::String(text) => out.append(parse_from(text, '§', inherited)),
        Value::Number(n) => out.push(&n.to_string(), inherited),
        Value::Bool(b) => out.push(&b.to_string(), inherited),
        Value::Array(items) => {
            // The first element is the parent of the others.
            let mut items = items.iter();
            let Some(first) = items.next() else {
                return;
            };
            let parent = match first {
                Value::Object(map) => apply_styles(map, inherited),
                _ => inherited,
            };
            component_into(first, inherited, depth + 1, out);
            for item in items {
                component_into(item, parent, depth + 1, out);
            }
        }
        Value::Object(map) => {
            let style = apply_styles(map, inherited);
            let text = map
                .get("text")
                .and_then(Value::as_str)
                .map(str::to_string)
                .or_else(|| {
                    map.get("translate").and_then(Value::as_str).map(|key| {
                        map.get("fallback")
                            .and_then(Value::as_str)
                            .unwrap_or(key)
                            .to_string()
                    })
                })
                .or_else(|| {
                    map.get("keybind")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                })
                .unwrap_or_default();
            out.append(parse_from(&text, '§', style));
            if let Some(Value::Array(extra)) = map.get("extra") {
                for child in extra {
                    component_into(child, style, depth + 1, out);
                }
            }
        }
        Value::Null => {}
    }
}

fn apply_styles(map: &serde_json::Map<String, Value>, inherited: Style) -> Style {
    let mut style = inherited;
    if let Some(color) = map
        .get("color")
        .and_then(Value::as_str)
        .and_then(named_color)
    {
        style.color = Some(color);
    }
    let flag = |key: &str, current: bool| match map.get(key) {
        Some(Value::Bool(b)) => *b,
        _ => current,
    };
    style.bold = flag("bold", style.bold);
    style.italic = flag("italic", style.italic);
    style.underlined = flag("underlined", style.underlined);
    style.strikethrough = flag("strikethrough", style.strikethrough);
    style.obfuscated = flag("obfuscated", style.obfuscated);
    style
}

/// The text without `§` codes.
pub fn strip(text: &str) -> String {
    if text.contains('§') {
        parse(text).text
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn styles(f: &Formatted) -> Vec<(&str, Style)> {
        f.spans
            .iter()
            .map(|(r, s)| (&f.text[r.clone()], *s))
            .collect()
    }

    #[test]
    fn legacy_codes_from_a_shader_pack_name() {
        let f = parse("§r§lAstra§4§lLex§r§l_By_LexBoosT_§4§lV93.0§r§l.zip");
        assert_eq!(f.text, "AstraLex_By_LexBoosT_V93.0.zip");
        let bold = Style {
            bold: true,
            ..Style::default()
        };
        let red_bold = Style {
            color: Some(0xAA0000),
            bold: true,
            ..Style::default()
        };
        assert_eq!(
            styles(&f),
            vec![
                ("Astra", bold),
                ("Lex", red_bold),
                ("_By_LexBoosT_", bold),
                ("V93.0", red_bold),
                (".zip", bold),
            ]
        );
    }

    #[test]
    fn colour_resets_formats_and_reset_resets_all() {
        let f = parse("§l§obold§6gold§rplain");
        assert_eq!(f.text, "boldgoldplain");
        assert!(f.spans[0].1.bold && f.spans[0].1.italic);
        assert_eq!(
            f.spans[1].1,
            Style {
                color: Some(0xFFAA00),
                ..Style::default()
            }
        );
        assert_eq!(f.spans.len(), 2, "the plain tail has no span");
    }

    #[test]
    fn unknown_and_trailing_codes_are_dropped() {
        assert_eq!(parse("a§zb§").text, "ab");
        assert_eq!(parse("§").text, "");
        assert_eq!(parse("no codes").spans, vec![]);
    }

    #[test]
    fn spigot_hex_colours() {
        let f = parse("§x§1§2§3§4§5§6hex");
        assert_eq!(f.text, "hex");
        assert_eq!(f.spans[0].1.color, Some(0x123456));
    }

    #[test]
    fn ampersand_codes_only_when_clearly_used() {
        assert_eq!(parse_with_ampersand("&6Gold &lPack").text, "Gold Pack");
        assert_eq!(parse_with_ampersand("Rock & Roll").text, "Rock & Roll");
        assert_eq!(parse_with_ampersand("R&D &a").text, "R&D &a");
        assert!(!uses_ampersand_codes("§6&aMixed"));
    }

    #[test]
    fn json_components_with_extra_and_inheritance() {
        let value = json!({
            "text": "Fresh ",
            "color": "gold",
            "extra": [
                {"text": "Animations", "bold": true},
                {"text": "!", "color": "#55FFFF"}
            ]
        });
        let f = parse_component(&value);
        assert_eq!(f.text, "Fresh Animations!");
        let s = styles(&f);
        assert_eq!(
            s[0],
            (
                "Fresh ",
                Style {
                    color: Some(0xFFAA00),
                    ..Style::default()
                }
            )
        );
        assert_eq!(
            s[1],
            (
                "Animations",
                Style {
                    color: Some(0xFFAA00),
                    bold: true,
                    ..Style::default()
                }
            )
        );
        assert_eq!(s[2].1.color, Some(0x55FFFF));
    }

    #[test]
    fn arrays_strings_and_fallbacks() {
        let value = json!([
            {"text": "", "color": "aqua"},
            "Blue ",
            {"translate": "pack.name", "fallback": "Name"},
            {"keybind": "key.jump"}
        ]);
        let f = parse_component(&value);
        assert_eq!(f.text, "Blue Namekey.jump");
        assert!(f.spans.iter().all(|(_, s)| s.color == Some(0x55FFFF)));
        // A plain string description may carry § codes.
        assert_eq!(
            parse_component(&json!("§cRed")).spans[0].1.color,
            Some(0xFF5555)
        );
    }

    #[test]
    fn malformed_components_are_harmless() {
        assert_eq!(parse_component(&json!(null)).text, "");
        assert_eq!(parse_component(&json!({"color": "nope"})).text, "");
        assert_eq!(parse_component(&json!({"text": 5})).text, "");
        assert_eq!(parse_component(&json!([])).text, "");
        assert_eq!(parse_component(&json!(42)).text, "42");
        // Deep nesting stops instead of overflowing the stack.
        let mut deep = json!("x");
        for _ in 0..200 {
            deep = json!({"text": "", "extra": [deep]});
        }
        let _ = parse_component(&deep);
    }

    #[test]
    fn strip_removes_codes() {
        assert_eq!(strip("§6Gold §lBold"), "Gold Bold");
        assert_eq!(strip("plain"), "plain");
    }
}
