//! Console lines as styled text: ANSI SGR escape sequences (colours and text styles) and
//! Minecraft `§` formatting codes. Every other escape sequence and control character is
//! removed, so the text never shows them.
//!
//! This is the dashboard's `utils/consoleFormat.ts`, ported so that Lodestone Launcher
//! shows a server's console exactly as the dashboard does. Keep the two in step; the
//! tests below mirror `consoleFormat.test.ts`.
//!
//! The console text is untrusted: it only ever becomes text and colours, never markup.

/// A colour: 0-255 is an xterm palette index (0-15 are the 16 theme colours),
/// [`RGB_BASE`] + `0xRRGGBB` is a 24-bit colour.
pub type ConsoleColor = u32;

pub const RGB_BASE: u32 = 256;

pub const BOLD: u8 = 1;
pub const DIM: u8 = 2;
pub const ITALIC: u8 = 4;
pub const UNDERLINE: u8 = 8;
pub const STRIKE: u8 = 16;
pub const INVERSE: u8 = 32;

/// A run of text in one style.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConsoleSegment {
    pub text: String,
    pub fg: Option<ConsoleColor>,
    pub bg: Option<ConsoleColor>,
    /// [`BOLD`] | [`DIM`] | ...
    pub flags: u8,
}

impl ConsoleSegment {
    pub fn is_plain(&self) -> bool {
        self.fg.is_none() && self.bg.is_none() && self.flags == 0
    }
}

/// A log level worth colouring a line for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleLevel {
    Warn,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsoleTheme {
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParsedConsoleLine {
    /// The text without escape sequences or formatting codes.
    pub plain: String,
    /// The text in runs of the same style, in order; they add up to `plain`.
    pub segments: Vec<ConsoleSegment>,
    /// Whether any of the text has a style (a colour, bold, ...).
    pub styled: bool,
    /// The log level, for lines without styles; `None` for INFO and unknown.
    pub level: Option<ConsoleLevel>,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Style {
    fg: Option<ConsoleColor>,
    bg: Option<ConsoleColor>,
    flags: u8,
}

const ESC: u32 = 0x1b;
const BEL: u32 = 0x07;
const SECTION: u32 = 0xa7; // §

/// Minecraft colour codes 0-f in the ANSI palette, as Paper and Spigot map them for the
/// console: §4 dark red is ANSI red, §c red is bright red, and so on.
const MINECRAFT_COLORS: [ConsoleColor; 16] = [0, 4, 2, 6, 1, 5, 3, 7, 8, 12, 10, 14, 9, 13, 11, 15];

fn hex_digit(code: Option<u32>) -> Option<u32> {
    let c = char::from_u32(code?)?;
    c.to_digit(16).filter(|_| c.is_ascii_hexdigit())
}

/// `""` and a missing value are 0, as in `"\x1b[m"` and `"\x1b[;1m"`; anything else that
/// isn't a number (the private markers `< = > ?`) is `None`.
fn to_int(value: Option<&str>) -> Option<u64> {
    match value {
        None | Some("") => Some(0),
        Some(v) if v.bytes().all(|b| b.is_ascii_digit()) => Some(v.bytes().fold(0u64, |n, b| {
            n.saturating_mul(10).saturating_add(u64::from(b - b'0'))
        })),
        Some(_) => None,
    }
}

/// A colour component; a missing one (a sequence cut short) is `None`.
fn byte(value: Option<&str>) -> Option<u32> {
    let n = to_int(Some(value?))?;
    (n <= 255).then_some(n as u32)
}

fn rgb(r: Option<u32>, g: Option<u32>, b: Option<u32>) -> Option<ConsoleColor> {
    Some(RGB_BASE + ((r? << 16) | (g? << 8) | b?))
}

/// Applies an SGR parameter string (the part of `"\x1b[...m"` between `[` and `m`).
fn apply_sgr(style: &mut Style, params: &str) {
    let parts: Vec<&str> = params.split(';').collect();
    let mut k = 0;
    while k < parts.len() {
        let part = parts[k];
        let code;
        if part.contains(':') {
            // sub-parameters: 38:5:n, 38:2:r:g:b, 38:2:colourspace:r:g:b, 4:3, ...
            let sub: Vec<&str> = part.split(':').collect();
            code = to_int(sub.first().copied());
            if matches!(code, Some(38 | 48 | 58)) {
                let mode = to_int(sub.get(1).copied());
                let color = match mode {
                    Some(5) => byte(sub.get(2).copied()),
                    Some(2) => {
                        let c = if sub.len() >= 6 { &sub[3..] } else { &sub[2..] };
                        rgb(
                            byte(c.first().copied()),
                            byte(c.get(1).copied()),
                            byte(c.get(2).copied()),
                        )
                    }
                    _ => None,
                };
                if let Some(color) = color {
                    match code {
                        Some(38) => style.fg = Some(color),
                        Some(48) => style.bg = Some(color),
                        // 58 is the underline colour, which isn't shown
                        _ => {}
                    }
                }
                k += 1;
                continue;
            }
            if code == Some(4) {
                // 4:0 is no underline, 4:1-5 are underline styles
                if to_int(sub.get(1).copied()) == Some(0) {
                    style.flags &= !UNDERLINE;
                } else {
                    style.flags |= UNDERLINE;
                }
                k += 1;
                continue;
            }
        } else {
            code = to_int(Some(part));
        }
        if matches!(code, Some(38 | 48 | 58)) {
            // 38;5;n or 38;2;r;g;b
            let mode = to_int(parts.get(k + 1).copied());
            let color = match mode {
                Some(5) => {
                    let index = byte(parts.get(k + 2).copied());
                    k += 2;
                    index
                }
                Some(2) => {
                    let color = rgb(
                        byte(parts.get(k + 2).copied()),
                        byte(parts.get(k + 3).copied()),
                        byte(parts.get(k + 4).copied()),
                    );
                    k += 4;
                    color
                }
                // unknown colour mode: the rest can't be read reliably
                _ => return,
            };
            if let Some(color) = color {
                match code {
                    Some(38) => style.fg = Some(color),
                    Some(48) => style.bg = Some(color),
                    _ => {}
                }
            }
            k += 1;
            continue;
        }
        if let Some(code) = code {
            match code {
                0 => *style = Style::default(),
                1 => style.flags |= BOLD,
                2 => style.flags |= DIM,
                3 => style.flags |= ITALIC,
                4 | 21 => style.flags |= UNDERLINE,
                7 => style.flags |= INVERSE,
                9 => style.flags |= STRIKE,
                22 => style.flags &= !(BOLD | DIM),
                23 => style.flags &= !ITALIC,
                24 => style.flags &= !UNDERLINE,
                27 => style.flags &= !INVERSE,
                29 => style.flags &= !STRIKE,
                30..=37 => style.fg = Some(code as u32 - 30),
                39 => style.fg = None,
                40..=47 => style.bg = Some(code as u32 - 40),
                49 => style.bg = None,
                90..=97 => style.fg = Some(code as u32 - 90 + 8),
                100..=107 => style.bg = Some(code as u32 - 100 + 8),
                // anything else (blink, conceal, overline, fonts, ...) is ignored
                _ => {}
            }
        }
        k += 1;
    }
}

/// The line as code points, which is how the parser indexes it.
struct Line {
    chars: Vec<char>,
}

impl Line {
    fn at(&self, i: usize) -> Option<u32> {
        self.chars.get(i).map(|&c| c as u32)
    }

    fn len(&self) -> usize {
        self.chars.len()
    }

    fn slice(&self, from: usize, to: usize) -> String {
        self.chars[from.min(self.len())..to.min(self.len())]
            .iter()
            .collect()
    }
}

/// Skips a control string (OSC, DCS, SOS, PM, APC) that starts at `i`, the first
/// character after its introducer. It ends with BEL or ST (ESC \); one that is never
/// terminated ends at the end of the line, so the rest of the console stays readable.
fn skip_control_string(s: &Line, mut i: usize) -> usize {
    while i < s.len() {
        let c = s.at(i);
        if c == Some(BEL) || c == Some(0x9c) {
            return i + 1;
        }
        if c == Some(ESC) && s.at(i + 1) == Some(0x5c) {
            return i + 2;
        }
        if c == Some(0x0a) {
            return i;
        }
        i += 1;
    }
    i
}

/// Reads a CSI sequence whose parameters start at `i`. Returns where it ends and, for
/// SGR (`m` without a private marker or intermediates), its parameters. A sequence cut
/// short by a character that can't be part of it ends there, and that character is read
/// as text again.
fn read_csi(s: &Line, mut i: usize) -> (usize, Option<String>) {
    let start = i;
    while s.at(i).is_some_and(|c| (0x30..=0x3f).contains(&c)) {
        i += 1; // parameters
    }
    let params_end = i;
    while s.at(i).is_some_and(|c| (0x20..=0x2f).contains(&c)) {
        i += 1; // intermediates
    }
    let Some(last) = s.at(i).filter(|c| (0x40..=0x7e).contains(c)) else {
        return (i, None);
    };
    let params = s.slice(start, params_end);
    let is_sgr = last == 0x6d && params_end == i && !params.starts_with(['<', '=', '>', '?']);
    (i + 1, is_sgr.then_some(params))
}

/// Handles the escape (ESC, or a C1 CSI / OSC / DCS) at `i`; returns the next index.
fn apply_escape(s: &Line, i: usize, style: &mut Style) -> usize {
    let c = s.at(i);
    let (next, at) = if c == Some(ESC) {
        match s.at(i + 1) {
            None => return i + 1,
            Some(next) => (next, i + 2),
        }
    } else {
        // C1 forms: 0x9b is CSI, 0x9d OSC, 0x90 DCS
        let next = match c {
            Some(0x9b) => 0x5b,
            Some(0x9d) => 0x5d,
            _ => 0x50,
        };
        (next, i + 1)
    };
    if next == 0x5b {
        // CSI: ESC [
        let (end, sgr) = read_csi(s, at);
        if let Some(params) = sgr {
            apply_sgr(style, &params);
        }
        return end;
    }
    if matches!(
        next,
        0x5d // OSC (window titles, hyperlinks, ...)
        | 0x50 // DCS
        | 0x58 // SOS
        | 0x5e // PM
        | 0x5f // APC
    ) {
        return skip_control_string(s, at);
    }
    if (0x20..=0x2f).contains(&next) {
        // ESC with intermediates and a final character, e.g. ESC ( B
        let mut j = at;
        while s.at(j).is_some_and(|c| (0x20..=0x2f).contains(&c)) {
            j += 1;
        }
        return if s.at(j).is_some_and(|c| (0x30..=0x7e).contains(&c)) {
            j + 1
        } else {
            j
        };
    }
    if (0x30..=0x7e).contains(&next) {
        return at; // ESC 7, ESC M, ESC c, ...
    }
    // ESC followed by something that can't continue it: drop the ESC alone
    i + 1
}

/// Collects the text into segments.
struct Builder {
    segments: Vec<ConsoleSegment>,
    style: Style,
    text: String,
    /// The style of `text`.
    text_style: Style,
}

impl Builder {
    fn flush(&mut self) {
        if self.text.is_empty() {
            return;
        }
        let text = std::mem::take(&mut self.text);
        let style = self.text_style;
        match self.segments.last_mut() {
            Some(last)
                if last.fg == style.fg && last.bg == style.bg && last.flags == style.flags =>
            {
                last.text.push_str(&text)
            }
            _ => self.segments.push(ConsoleSegment {
                text,
                fg: style.fg,
                bg: style.bg,
                flags: style.flags,
            }),
        }
    }

    fn append(&mut self, chunk: &str) {
        if chunk.is_empty() {
            return;
        }
        if !self.text.is_empty() && self.text_style != self.style {
            self.flush();
        }
        if self.text.is_empty() {
            self.text_style = self.style;
        }
        self.text.push_str(chunk);
    }
}

/// Handles the § at `i`; returns the next index. § followed by anything that isn't a
/// formatting code is kept as text.
fn apply_section(s: &Line, i: usize, b: &mut Builder) -> usize {
    let code = s.at(i + 1);
    if let Some(digit) = hex_digit(code) {
        // a colour; in Minecraft it also ends bold, italic, ...
        b.style.fg = Some(MINECRAFT_COLORS[digit as usize]);
        b.style.flags = 0;
        return i + 2;
    }
    let lower = code
        .and_then(char::from_u32)
        .filter(char::is_ascii_alphabetic)
        .map(|c| c.to_ascii_lowercase());
    match lower {
        // obfuscated, shown as plain text
        Some('k') => return i + 2,
        Some('l') => {
            b.style.flags |= BOLD;
            return i + 2;
        }
        Some('m') => {
            b.style.flags |= STRIKE;
            return i + 2;
        }
        Some('n') => {
            b.style.flags |= UNDERLINE;
            return i + 2;
        }
        Some('o') => {
            b.style.flags |= ITALIC;
            return i + 2;
        }
        Some('r') => {
            b.style = Style::default();
            return i + 2;
        }
        Some('x') => {
            // a hex colour, §x§R§R§G§G§B§B (Spigot / BungeeCord)
            let mut value = 0;
            for k in 0..6 {
                let at = i + 2 + k * 2;
                let digit = hex_digit(s.at(at + 1));
                match digit {
                    Some(d) if s.at(at) == Some(SECTION) => value = value * 16 + d,
                    _ => {
                        b.append("§");
                        return i + 1;
                    }
                }
            }
            b.style.fg = Some(RGB_BASE + value);
            b.style.flags = 0;
            return i + 14;
        }
        _ => {}
    }
    b.append("§");
    i + 1
}

/// Parses one console line (one output event). The style starts plain at the start of
/// the line and again after each newline in it, so a colour that is never reset doesn't
/// run into the following lines. The trailing line break is dropped.
pub fn parse_console_line(message: &str) -> ParsedConsoleLine {
    let s = Line {
        chars: message.trim_end_matches(['\r', '\n']).chars().collect(),
    };
    let mut b = Builder {
        segments: Vec::new(),
        style: Style::default(),
        text: String::new(),
        text_style: Style::default(),
    };
    let mut run_start = 0;
    let mut i = 0;
    while i < s.len() {
        let c = s.at(i).unwrap_or_default();
        let special = (c < 0x20 && c != 0x09) || (0x7f..=0x9f).contains(&c) || c == SECTION;
        if !special {
            i += 1;
            continue;
        }
        b.append(&s.slice(run_start, i));
        if c == 0x0a {
            // a new line inside the event starts plain again
            b.style = Style::default();
            b.append("\n");
            i += 1;
        } else if c == SECTION {
            i = apply_section(&s, i, &mut b);
        } else if matches!(c, ESC | 0x9b | 0x9d | 0x90) {
            i = apply_escape(&s, i, &mut b.style);
        } else if matches!(c, 0x98 | 0x9e | 0x9f) {
            i = skip_control_string(&s, i + 1); // C1 SOS, PM, APC
        } else {
            i += 1; // other control characters (\r, BEL, backspace, ...) are dropped
        }
        run_start = i;
    }
    b.append(&s.slice(run_start, s.len()));
    b.flush();

    let plain: String = b.segments.iter().map(|s| s.text.as_str()).collect();
    let styled = b.segments.iter().any(|s| !s.is_plain());
    ParsedConsoleLine {
        level: if styled { None } else { detect_level(&plain) },
        plain,
        segments: b.segments,
        styled,
    }
}

// ---- Log levels ----

const LEVEL_TOKENS: [&str; 8] = [
    "TRACE", "DEBUG", "INFO", "WARN", "WARNING", "ERROR", "SEVERE", "FATAL",
];
const PREFIX_TOKENS: [&str; 5] = ["WARN", "WARNING", "ERROR", "SEVERE", "FATAL"];

fn to_level(token: &str) -> Option<ConsoleLevel> {
    if token.starts_with("WARN") {
        Some(ConsoleLevel::Warn)
    } else if matches!(token, "ERROR" | "SEVERE" | "FATAL") {
        Some(ConsoleLevel::Error)
    } else {
        None
    }
}

/// A `[...]` group at the start of `s` (after whitespace) without a newline or `]` in
/// it: its contents and the rest of `s`.
fn bracket_group(s: &str) -> Option<(&str, &str)> {
    let s = s.trim_start().strip_prefix('[')?;
    let end = s.find([']', '\n'])?;
    (s.as_bytes()[end] == b']').then(|| (&s[..end], &s[end + 1..]))
}

/// The level token at the end of a bracket group, after its start, whitespace or `/`.
fn level_token(group: &str) -> Option<&'static str> {
    let group = group.trim_end();
    LEVEL_TOKENS.into_iter().find(|token| {
        group.strip_suffix(token).is_some_and(|before| {
            before
                .chars()
                .next_back()
                .is_none_or(|c| c.is_whitespace() || c == '/')
        })
    })
}

/// The log level of a line, from the level in its first one or two bracketed groups,
/// which covers the common server formats: `[12:00:00] [Server thread/WARN]: ...`
/// (vanilla, Fabric, Forge), `[12:00:00 WARN]: ...` (Paper, Velocity) and
/// `[WARN] ...`; or from a `WARN: ` / `ERROR: ` prefix.
pub fn detect_level(plain: &str) -> Option<ConsoleLevel> {
    if let Some((first, rest)) = bracket_group(plain) {
        let second = bracket_group(rest).map(|(group, _)| group);
        for group in [Some(first), second].into_iter().flatten() {
            // the first group with a level decides: "[INFO]: [x] [ERROR]" is INFO
            if let Some(token) = level_token(group) {
                return to_level(token);
            }
        }
    }
    let trimmed = plain.trim_start();
    PREFIX_TOKENS.into_iter().find_map(|token| {
        let rest = trimmed.strip_prefix(token)?.strip_prefix(':')?;
        rest.chars()
            .next()
            .filter(|c| c.is_whitespace())
            .and_then(|_| to_level(token))
    })
}

// ---- Colours ----

/// The 16 colours used on badges (text with a background colour), in both themes. Such
/// text carries its own background, so these are the usual terminal colours rather than
/// the theme's.
pub const BADGE_PALETTE: [u32; 16] = [
    0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd, 0xe5e5e5, 0x666666,
    0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6, 0x29b8db, 0xffffff,
];

/// The `0xRRGGBB` value of an xterm palette index 16-255.
pub fn xterm256(index: u32) -> u32 {
    if index >= 232 {
        let v = 8 + (index - 232) * 10;
        return (v << 16) | (v << 8) | v;
    }
    let n = index.saturating_sub(16);
    let level = |x: u32| if x == 0 { 0 } else { 55 + x * 40 };
    (level(n / 36) << 16) | (level((n / 6) % 6) << 8) | level(n % 6)
}

pub fn to_hex(value: u32) -> String {
    format!("#{value:06x}")
}

fn channel(v: u32) -> f64 {
    let x = f64::from(v) / 255.0;
    if x <= 0.03928 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of `0xRRGGBB`.
pub fn luminance(value: u32) -> f64 {
    0.2126 * channel((value >> 16) & 255)
        + 0.7152 * channel((value >> 8) & 255)
        + 0.0722 * channel(value & 255)
}

/// WCAG contrast ratio between two `0xRRGGBB` colours.
pub fn contrast(a: u32, b: u32) -> f64 {
    let x = luminance(a);
    let y = luminance(b);
    (x.max(y) + 0.05) / (x.min(y) + 0.05)
}

/// The background console text has to be readable on, per theme: the harder of the
/// console (gray-900) and a hovered row (gray-800), as in the dashboard.
pub fn readable_on(theme: ConsoleTheme) -> u32 {
    match theme {
        ConsoleTheme::Dark => 0x2b2d32,
        ConsoleTheme::Light => 0xf3f5f8,
    }
}

pub const MIN_CONTRAST: f64 = 4.5;

fn to_hsl(value: u32) -> (f64, f64, f64) {
    let r = f64::from((value >> 16) & 255) / 255.0;
    let g = f64::from((value >> 8) & 255) / 255.0;
    let b = f64::from(value & 255) / 255.0;
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let sat = if l > 0.5 {
        d / (2.0 - max - min)
    } else {
        d / (max + min)
    };
    let hue = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (hue / 6.0, sat, l)
}

fn from_hsl(h: f64, sat: f64, l: f64) -> u32 {
    let q = if l < 0.5 {
        l * (1.0 + sat)
    } else {
        l + sat - l * sat
    };
    let p = 2.0 * l - q;
    let hue = |mut t: f64| {
        if t < 0.0 {
            t += 1.0;
        }
        if t > 1.0 {
            t -= 1.0;
        }
        if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 1.0 / 2.0 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        }
    };
    let to = |v: f64| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
    (to(hue(h + 1.0 / 3.0)) << 16) | (to(hue(h)) << 8) | to(hue(h - 1.0 / 3.0))
}

/// A 256 or 24-bit text colour made readable on the console: if it has less than 4.5:1
/// contrast, its lightness is moved towards white (dark theme) or black (light theme)
/// until it has. The hue stays the same.
pub fn readable_color(value: u32, theme: ConsoleTheme) -> u32 {
    let bg = readable_on(theme);
    if contrast(value, bg) >= MIN_CONTRAST {
        return value;
    }
    let (h, sat, l0) = to_hsl(value);
    let step = match theme {
        ConsoleTheme::Dark => 0.02,
        ConsoleTheme::Light => -0.02,
    };
    let mut result = value;
    let mut l = l0 + step;
    while (0.0..=1.0).contains(&l) {
        result = from_hsl(h, sat, l);
        if contrast(result, bg) >= MIN_CONTRAST {
            break;
        }
        l += step;
    }
    if contrast(result, bg) < MIN_CONTRAST {
        result = match theme {
            ConsoleTheme::Dark => 0xffffff,
            ConsoleTheme::Light => 0x000000,
        };
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    const E: &str = "\x1b";

    type Seg = (String, Option<u32>, Option<u32>, u8);

    /// `(text, fg, bg, flags)` per segment, for short assertions.
    fn segs(message: &str) -> Vec<Seg> {
        parse_console_line(message)
            .segments
            .into_iter()
            .map(|s| (s.text, s.fg, s.bg, s.flags))
            .collect()
    }

    fn seg(text: &str, fg: Option<u32>, bg: Option<u32>, flags: u8) -> Seg {
        (text.to_string(), fg, bg, flags)
    }

    fn plain(message: &str) -> String {
        parse_console_line(message).plain
    }

    // ---- SGR ----

    #[test]
    fn plain_text_is_one_unstyled_segment() {
        assert_eq!(segs("hello world"), vec![seg("hello world", None, None, 0)]);
        assert!(!parse_console_line("hello world").styled);
    }

    #[test]
    fn sixteen_colours_bright_colours_and_backgrounds() {
        assert_eq!(
            segs(&format!("{E}[31mred{E}[0m {E}[92mgreen{E}[44mon blue")),
            vec![
                seg("red", Some(1), None, 0),
                seg(" ", None, None, 0),
                seg("green", Some(10), None, 0),
                seg("on blue", Some(10), Some(4), 0),
            ]
        );
        assert_eq!(
            segs(&format!("{E}[103mx")),
            vec![seg("x", None, Some(11), 0)]
        );
    }

    #[test]
    fn combined_parameters_in_one_sequence() {
        assert_eq!(
            segs(&format!("{E}[1;3;4;9;2;7;33mx")),
            vec![seg(
                "x",
                Some(3),
                None,
                BOLD | ITALIC | UNDERLINE | STRIKE | DIM | INVERSE
            )]
        );
    }

    #[test]
    fn turning_single_attributes_off() {
        assert_eq!(
            segs(&format!(
                "{E}[1;3;4mA{E}[22mB{E}[23mC{E}[24mD{E}[1;31mE{E}[39mF"
            )),
            vec![
                seg("A", None, None, BOLD | ITALIC | UNDERLINE),
                seg("B", None, None, ITALIC | UNDERLINE),
                seg("C", None, None, UNDERLINE),
                seg("D", None, None, 0),
                seg("E", Some(1), None, BOLD),
                seg("F", None, None, BOLD),
            ]
        );
    }

    #[test]
    fn reset_zero_empty_parameters_and_a_bare_m() {
        assert_eq!(
            segs(&format!("{E}[1;31ma{E}[mb{E}[1;31mc{E}[0md")),
            vec![
                seg("a", Some(1), None, BOLD),
                seg("b", None, None, 0),
                seg("c", Some(1), None, BOLD),
                seg("d", None, None, 0),
            ]
        );
        // a leading empty parameter is a 0
        assert_eq!(
            segs(&format!("{E}[31m{E}[;1mx")),
            vec![seg("x", None, None, BOLD)]
        );
    }

    #[test]
    fn colours_256() {
        assert_eq!(
            segs(&format!("{E}[38;5;208mx{E}[48;5;21my")),
            vec![
                seg("x", Some(208), None, 0),
                seg("y", Some(208), Some(21), 0)
            ]
        );
        assert_eq!(
            segs(&format!("{E}[38:5:9mx")),
            vec![seg("x", Some(9), None, 0)]
        );
        assert_eq!(xterm256(16), 0x000000);
        assert_eq!(xterm256(196), 0xff0000);
        assert_eq!(xterm256(231), 0xffffff);
        assert_eq!(xterm256(232), 0x080808);
        assert_eq!(xterm256(255), 0xeeeeee);
    }

    #[test]
    fn truecolor_with_semicolons_and_colons() {
        let orange = Some(RGB_BASE + 0xff8000);
        for sequence in ["38;2;255;128;0", "38:2:255:128:0", "38:2::255:128:0"] {
            assert_eq!(
                segs(&format!("{E}[{sequence}mx")),
                vec![seg("x", orange, None, 0)],
                "{sequence}"
            );
        }
        assert_eq!(
            segs(&format!("{E}[1;48;2;0;0;255;4mx")),
            vec![seg("x", None, Some(RGB_BASE + 0xff), BOLD | UNDERLINE)]
        );
    }

    #[test]
    fn parameters_after_an_extended_colour_still_apply() {
        assert_eq!(
            segs(&format!("{E}[38;5;100;1mx")),
            vec![seg("x", Some(100), None, BOLD)]
        );
    }

    #[test]
    fn a_style_lasts_across_chunks_until_reset() {
        assert_eq!(
            segs(&format!("{E}[32m[12:00] {E}[1mDone{E}[22m (1s)")),
            vec![
                seg("[12:00] ", Some(2), None, 0),
                seg("Done", Some(2), None, BOLD),
                seg(" (1s)", Some(2), None, 0),
            ]
        );
    }

    #[test]
    fn the_style_starts_plain_again_after_a_newline() {
        assert_eq!(
            segs(&format!("{E}[31mone\ntwo")),
            vec![seg("one", Some(1), None, 0), seg("\ntwo", None, None, 0)]
        );
    }

    #[test]
    fn redundant_sequences_merge_into_one_segment() {
        assert_eq!(
            segs(&format!("{E}[31ma{E}[31mb{E}[0m{E}[0mc")),
            vec![seg("ab", Some(1), None, 0), seg("c", None, None, 0)]
        );
    }

    // ---- other escapes are removed ----

    #[test]
    fn cursor_movement_erase_and_private_modes() {
        let line = parse_console_line(&format!(
            "{E}[2K{E}[1G{E}[?25l>{E}[3A ok{E}[?2004h{E}[>4;1m"
        ));
        assert_eq!(line.plain, "> ok");
        assert!(!line.styled);
    }

    #[test]
    fn osc_titles_and_hyperlinks_ended_by_bel_or_st() {
        assert_eq!(
            plain(&format!(
                "{E}]0;My title\x07before {E}]8;;https://x.y{E}\\link{E}]8;;{E}\\ after"
            )),
            "before link after"
        );
    }

    #[test]
    fn short_escapes_and_charset_selection() {
        assert_eq!(plain(&format!("{E}7a{E}8b{E}(Bc{E}=d{E}Me")), "abcde");
    }

    #[test]
    fn dcs_apc_and_c1_controls() {
        assert_eq!(plain(&format!("a{E}Pq#0;1{E}\\b{E}_x{E}\\c")), "abc");
        assert_eq!(plain("a\u{9b}31mb\u{9d}0;t\x07c"), "abc");
        assert_eq!(segs("\u{9b}31mx"), vec![seg("x", Some(1), None, 0)]);
    }

    #[test]
    fn control_characters_are_dropped_tabs_are_kept() {
        assert_eq!(plain("a\x07b\x08c\x00d\re\tf\x7f"), "abcde\tf");
    }

    #[test]
    fn the_trailing_line_break_is_dropped() {
        assert_eq!(plain("done\r\n"), "done");
        assert_eq!(plain("a\nb\n"), "a\nb");
    }

    // ---- malformed sequences ----

    #[test]
    fn a_lone_esc_at_the_end_or_before_a_newline() {
        assert_eq!(plain(&format!("abc{E}")), "abc");
        assert_eq!(plain(&format!("a{E}\nb")), "a\nb");
    }

    #[test]
    fn a_csi_cut_off_at_the_end_of_the_line() {
        assert_eq!(plain(&format!("abc{E}[31;1")), "abc");
        assert_eq!(plain(&format!("abc{E}[")), "abc");
    }

    #[test]
    fn a_csi_interrupted_by_a_character_that_cannot_be_part_of_it() {
        // the § is read again as text (it isn't followed by a code)
        assert_eq!(plain(&format!("a{E}[31§ b")), "a§ b");
        assert_eq!(plain(&format!("a{E}[31\nb")), "a\nb");
    }

    #[test]
    fn an_osc_that_is_never_terminated_ends_at_the_newline() {
        assert_eq!(plain(&format!("a{E}]0;title\nb")), "a\nb");
    }

    #[test]
    fn invalid_colours_are_ignored() {
        for sequence in ["38;5;300", "38;2;255;0", "38;2;256;0;0", "38;7;1"] {
            assert_eq!(
                segs(&format!("{E}[{sequence}mx")),
                vec![seg("x", None, None, 0)],
                "{sequence}"
            );
        }
    }

    #[test]
    fn unknown_sgr_codes_are_ignored() {
        assert_eq!(
            segs(&format!("{E}[5;53;31;73mx")),
            vec![seg("x", Some(1), None, 0)]
        );
    }

    // ---- Minecraft § codes ----

    #[test]
    fn section_colours() {
        assert_eq!(
            segs("§aGreen §cRed §6Gold §7Gray"),
            vec![
                seg("Green ", Some(10), None, 0),
                seg("Red ", Some(9), None, 0),
                seg("Gold ", Some(3), None, 0),
                seg("Gray", Some(7), None, 0),
            ]
        );
    }

    #[test]
    fn formats_reset_and_a_colour_ending_the_formats() {
        assert_eq!(
            segs("§l§nBold§r plain §o§mx§eyellow"),
            vec![
                seg("Bold", None, None, BOLD | UNDERLINE),
                seg(" plain ", None, None, 0),
                seg("x", None, None, ITALIC | STRIKE),
                seg("yellow", Some(11), None, 0),
            ]
        );
        assert_eq!(
            segs("§c§lBOLD RED§r."),
            vec![
                seg("BOLD RED", Some(9), None, BOLD),
                seg(".", None, None, 0)
            ]
        );
    }

    #[test]
    fn obfuscated_is_shown_as_plain_text() {
        assert_eq!(segs("§kmagic"), vec![seg("magic", None, None, 0)]);
    }

    #[test]
    fn uppercase_codes() {
        assert_eq!(
            segs("§Ax§Ly"),
            vec![seg("x", Some(10), None, 0), seg("y", Some(10), None, BOLD)]
        );
    }

    #[test]
    fn hex_colours() {
        assert_eq!(
            segs("§x§f§f§8§0§0§0orange"),
            vec![seg("orange", Some(RGB_BASE + 0xff8000), None, 0)]
        );
        // incomplete: kept as text
        assert_eq!(plain("§x§f§fab"), "§xab");
    }

    #[test]
    fn a_section_that_is_not_a_code_stays() {
        assert_eq!(plain("§ 5 §z §"), "§ 5 §z §");
        assert_eq!(plain("Section §12"), "Section 2");
    }

    #[test]
    fn ampersand_is_never_read_as_a_code() {
        assert_eq!(
            segs("Tom &amp; Jerry &a &l"),
            vec![seg("Tom &amp; Jerry &a &l", None, None, 0)]
        );
    }

    #[test]
    fn mixed_with_ansi() {
        assert_eq!(
            segs(&format!("§a{E}[1mx{E}[0my")),
            vec![seg("x", Some(10), None, BOLD), seg("y", None, None, 0)]
        );
    }

    // ---- log levels ----

    #[test]
    fn common_server_formats() {
        use ConsoleLevel::{Error, Warn};
        for (line, level) in [
            ("[12:00:00] [Server thread/WARN]: Can't keep up!", Warn),
            ("[12:00:00 WARN]: Plugin x is outdated", Warn),
            ("[12:00:00 ERROR]: Could not load", Error),
            (
                "[05Nov2024 12:00:00.123] [main/FATAL] [net.minecraftforge/]: boom",
                Error,
            ),
            ("[12:00:00] [Server thread/ERROR]: x", Error),
            ("[12:00:00 SEVERE]: x", Error),
            ("[WARN] something", Warn),
            ("[2024-01-01 12:00:00:000 WARN] Bedrock", Warn),
            ("ERROR: container failed", Error),
            ("WARNING: low memory", Warn),
        ] {
            assert_eq!(detect_level(line), Some(level), "{line}");
        }
    }

    #[test]
    fn info_and_unknown_lines_have_no_level() {
        for line in [
            "[12:00:00] [Server thread/INFO]: Done (3.2s)!",
            "[12:00:00 INFO]: [MyPlugin] [ERROR] in the message",
            "<Steve> the server has an ERROR: lol",
            "Loading libraries, please wait...",
            "[12:00:00] [Server thread/INFO]: [WARN] x",
        ] {
            assert_eq!(detect_level(line), None, "{line}");
        }
    }

    #[test]
    fn only_lines_without_styles_get_a_level() {
        assert_eq!(
            parse_console_line("[12:00:00 WARN]: x\n").level,
            Some(ConsoleLevel::Warn)
        );
        assert_eq!(
            parse_console_line(&format!("{E}[33m[12:00:00 WARN]: x{E}[m")).level,
            None
        );
        // resets alone don't count as styles
        assert_eq!(
            parse_console_line(&format!("[12:00:00 ERROR]: x{E}[m")).level,
            Some(ConsoleLevel::Error)
        );
    }

    // ---- plain text ----

    #[test]
    fn plain_text_has_no_codes_and_matches_the_segments() {
        let message = format!("{E}[1;38;2;1;2;3m§aHi{E}[0m {E}]0;t\x07there§r!\r\n");
        let line = parse_console_line(&message);
        assert_eq!(line.plain, "Hi there!");
        assert_eq!(
            line.segments
                .iter()
                .map(|s| s.text.as_str())
                .collect::<String>(),
            line.plain
        );
    }

    // ---- readable colours ----

    #[test]
    fn dark_colours_are_lightened_on_the_dark_console() {
        let navy = 0x000080;
        let adjusted = readable_color(navy, ConsoleTheme::Dark);
        assert_ne!(adjusted, navy);
        assert!(contrast(adjusted, 0x2b2d32) >= MIN_CONTRAST);
    }

    #[test]
    fn light_colours_are_darkened_on_the_light_console() {
        for color in [0xffff00, 0xffffff, 0x55ffff, 0xffaa00] {
            assert!(contrast(readable_color(color, ConsoleTheme::Light), 0xf3f5f8) >= MIN_CONTRAST);
        }
    }

    #[test]
    fn readable_colours_are_kept() {
        assert_eq!(readable_color(0xff8000, ConsoleTheme::Dark), 0xff8000);
        assert_eq!(readable_color(0x1d4ed8, ConsoleTheme::Light), 0x1d4ed8);
    }

    #[test]
    fn every_256_colour_ends_up_readable_in_both_themes() {
        for i in 16..256 {
            assert!(
                contrast(readable_color(xterm256(i), ConsoleTheme::Dark), 0x2b2d32) >= MIN_CONTRAST
            );
            assert!(
                contrast(readable_color(xterm256(i), ConsoleTheme::Light), 0xf3f5f8)
                    >= MIN_CONTRAST
            );
        }
    }

    #[test]
    fn hex_and_non_ascii_text() {
        assert_eq!(to_hex(0x0a0b0c), "#0a0b0c");
        // multi-byte text around codes stays whole
        assert_eq!(plain("§aé✓ 世界§r"), "é✓ 世界");
    }
}
