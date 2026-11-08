//! A reader and writer for Java `.properties` files, such as `server.properties`.
//!
//! The reader follows `java.util.Properties::load`:
//! - leading whitespace is skipped, and lines that start with `#` or `!` are comments;
//! - the key ends at the first unescaped `=`, `:` or whitespace;
//! - a line that ends with an odd number of `\` continues on the next line;
//! - `\t`, `\n`, `\r`, `\f`, `\uXXXX` and `\<any char>` escapes are decoded.
//!
//! The document keeps every line as it was read. Writing it back only changes the lines of
//! the keys that were set, so comments, unknown keys and the line order are kept.

use indexmap::IndexMap;

/// A line that could not be used as a property. The line itself is kept in the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseWarning {
    /// 1-based number of the first physical line
    pub line_number: usize,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Utf8,
    /// Older Minecraft versions write ISO-8859-1. Non-ASCII text we write is always escaped
    /// as `\uXXXX`, so only lines we did not change contain non-ASCII characters.
    Latin1,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    /// A comment, a blank line or a line that could not be parsed. Written back unchanged.
    Raw(String),
    /// A property. `raw` is the original text (physical lines joined with '\n'), or the
    /// newly formatted line after `set`.
    Entry {
        key: String,
        value: String,
        raw: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropertiesDocument {
    lines: Vec<Line>,
    newline: &'static str,
    trailing_newline: bool,
    encoding: Encoding,
}

impl Default for PropertiesDocument {
    fn default() -> Self {
        Self {
            lines: Vec::new(),
            newline: "\n",
            trailing_newline: true,
            encoding: Encoding::Utf8,
        }
    }
}

fn is_whitespace(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\u{000C}')
}

/// True if `s` ends with an odd number of backslashes.
fn ends_with_continuation(s: &str) -> bool {
    s.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1
}

/// Decodes the escapes in a key or value (`Properties.loadConvert`).
fn unescape(s: &str) -> Result<String, String> {
    let mut units: Vec<u16> = Vec::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            let mut buf = [0u16; 2];
            units.extend_from_slice(c.encode_utf16(&mut buf));
            continue;
        }
        let Some(escaped) = chars.next() else {
            // a lone trailing backslash is dropped, like Java does
            break;
        };
        match escaped {
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                if hex.len() != 4 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(format!("Malformed \\uXXXX escape: \\u{hex}"));
                }
                let unit = u16::from_str_radix(&hex, 16)
                    .map_err(|_| format!("Malformed \\uXXXX escape: \\u{hex}"))?;
                units.push(unit);
            }
            't' => units.push('\t' as u16),
            'n' => units.push('\n' as u16),
            'r' => units.push('\r' as u16),
            'f' => units.push(0x0C),
            other => {
                let mut buf = [0u16; 2];
                units.extend_from_slice(other.encode_utf16(&mut buf));
            }
        }
    }
    // Java strings may hold unpaired surrogates; Rust strings cannot.
    String::from_utf16(&units).map_err(|_| "Invalid UTF-16 surrogate in \\u escape".to_string())
}

/// Escapes a key or value for writing (`Properties.saveConvert` with unicode escapes).
fn escape(s: &str, is_key: bool) -> String {
    let mut out = String::with_capacity(s.len());
    for (i, c) in s.chars().enumerate() {
        match c {
            ' ' if is_key || i == 0 => out.push_str("\\ "),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{000C}' => out.push_str("\\f"),
            '=' | ':' | '#' | '!' => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut buf = [0u16; 2];
                for unit in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{unit:04X}"));
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Formats one `key=value` line.
pub fn format_entry(key: &str, value: &str) -> String {
    format!("{}={}", escape(key, true), escape(value, false))
}

/// Splits a logical line (leading whitespace removed, continuations joined) into the raw
/// key and value text (`Properties.load0`).
fn split_key_value(line: &str) -> (&str, &str) {
    let mut key_end = line.len();
    let mut value_start = line.len();
    let mut has_separator = false;
    let mut preceding_backslash = false;
    for (i, c) in line.char_indices() {
        if !preceding_backslash && (c == '=' || c == ':') {
            key_end = i;
            value_start = i + 1;
            has_separator = true;
            break;
        }
        if !preceding_backslash && is_whitespace(c) {
            key_end = i;
            value_start = i + c.len_utf8();
            break;
        }
        preceding_backslash = c == '\\' && !preceding_backslash;
    }
    // skip the whitespace around the separator, and at most one '=' or ':'
    let rest = &line[value_start..];
    let mut offset = 0;
    for (i, c) in rest.char_indices() {
        if is_whitespace(c) {
            offset = i + c.len_utf8();
        } else if !has_separator && (c == '=' || c == ':') {
            has_separator = true;
            offset = i + 1;
        } else {
            offset = i;
            break;
        }
    }
    (&line[..key_end], &rest[offset..])
}

impl PropertiesDocument {
    /// Parses raw file bytes. UTF-8 is tried first; files that are not valid UTF-8 are read
    /// as ISO-8859-1, like Minecraft does.
    pub fn from_bytes(bytes: &[u8]) -> (Self, Vec<ParseWarning>) {
        match std::str::from_utf8(bytes) {
            Ok(text) => Self::parse(text),
            Err(_) => {
                let text: String = bytes.iter().map(|b| *b as char).collect();
                let (mut doc, warnings) = Self::parse(&text);
                doc.encoding = Encoding::Latin1;
                (doc, warnings)
            }
        }
    }

    /// Parses the text of a properties file. Never fails: lines that can't be used are kept
    /// unchanged and reported as warnings.
    pub fn parse(text: &str) -> (Self, Vec<ParseWarning>) {
        let text = text.strip_prefix('\u{FEFF}').unwrap_or(text);
        let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let trailing_newline = text.is_empty() || text.ends_with('\n');
        let body = text.strip_suffix('\n').unwrap_or(text);
        let physical: Vec<&str> = if text.is_empty() {
            Vec::new()
        } else {
            body.split('\n')
                .map(|l| l.strip_suffix('\r').unwrap_or(l))
                .collect()
        };

        let mut lines = Vec::new();
        let mut warnings = Vec::new();
        let mut i = 0;
        while i < physical.len() {
            let first_line_number = i + 1;
            let first = physical[i];
            let trimmed = first.trim_start_matches(is_whitespace);
            i += 1;
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('!') {
                lines.push(Line::Raw(first.to_string()));
                continue;
            }

            // join continuation lines
            let mut raw_parts = vec![first];
            let mut logical = String::new();
            let mut current = trimmed;
            loop {
                if ends_with_continuation(current) && i < physical.len() {
                    logical.push_str(&current[..current.len() - 1]);
                    let next = physical[i];
                    raw_parts.push(next);
                    i += 1;
                    current = next.trim_start_matches(is_whitespace);
                } else {
                    logical.push_str(current);
                    break;
                }
            }
            let raw = raw_parts.join("\n");

            let (raw_key, raw_value) = split_key_value(&logical);
            let parsed = unescape(raw_key).and_then(|key| Ok((key, unescape(raw_value)?)));
            match parsed {
                Ok((key, _)) if key.is_empty() => {
                    warnings.push(ParseWarning {
                        line_number: first_line_number,
                        message: format!("Ignoring a line with an empty key: {raw}"),
                    });
                    lines.push(Line::Raw(raw));
                }
                Ok((key, value)) => lines.push(Line::Entry { key, value, raw }),
                Err(message) => {
                    warnings.push(ParseWarning {
                        line_number: first_line_number,
                        message: format!("Ignoring a malformed line ({message}): {raw}"),
                    });
                    lines.push(Line::Raw(raw));
                }
            }
        }

        (
            Self {
                lines,
                newline,
                trailing_newline,
                encoding: Encoding::Utf8,
            },
            warnings,
        )
    }

    /// All properties in file order. If a key appears more than once, the last value wins
    /// (like Java), at the position of the first occurrence.
    pub fn entries(&self) -> IndexMap<String, String> {
        let mut map = IndexMap::new();
        for line in &self.lines {
            if let Line::Entry { key, value, .. } = line {
                map.insert(key.clone(), value.clone());
            }
        }
        map
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.lines.iter().rev().find_map(|line| match line {
            Line::Entry { key: k, value, .. } if k == key => Some(value.as_str()),
            _ => None,
        })
    }

    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Sets a property. Only the line of that key changes; a new key is appended.
    pub fn set(&mut self, key: &str, value: &str) {
        let existing = self.lines.iter_mut().rev().find_map(|line| match line {
            Line::Entry { key: k, value, raw } if k == key => Some((value, raw)),
            _ => None,
        });
        match existing {
            Some((old_value, _)) if old_value == value => {}
            Some((old_value, raw)) => {
                *old_value = value.to_string();
                *raw = format_entry(key, value);
            }
            None => self.lines.push(Line::Entry {
                key: key.to_string(),
                value: value.to_string(),
                raw: format_entry(key, value),
            }),
        }
    }

    /// The file content as text.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        for (i, line) in self.lines.iter().enumerate() {
            if i > 0 {
                out.push_str(self.newline);
            }
            let raw = match line {
                Line::Raw(raw) => raw,
                Line::Entry { raw, .. } => raw,
            };
            if self.newline == "\n" {
                out.push_str(raw);
            } else {
                out.push_str(&raw.replace('\n', self.newline));
            }
        }
        if self.trailing_newline && !self.lines.is_empty() {
            out.push_str(self.newline);
        }
        out
    }

    /// The file content as bytes, in the encoding the file was read in.
    pub fn to_bytes(&self) -> Vec<u8> {
        let text = self.to_text();
        match self.encoding {
            Encoding::Utf8 => text.into_bytes(),
            // Every char came from a single byte or from our ASCII-only writer.
            Encoding::Latin1 => text.chars().map(|c| c as u32 as u8).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(text: &str) -> PropertiesDocument {
        PropertiesDocument::parse(text).0
    }

    #[test]
    fn splits_on_first_separator() {
        let doc = parse("motd=Welcome a=b\nlevel-type=minecraft\\:normal\n");
        assert_eq!(doc.get("motd"), Some("Welcome a=b"));
        assert_eq!(doc.get("level-type"), Some("minecraft:normal"));
    }

    #[test]
    fn colon_and_whitespace_separators() {
        let doc = parse("a:1\nb 2\nc = 3\nd\t:\t4\ne\n   f=  6\ng   \n");
        assert_eq!(doc.get("a"), Some("1"));
        assert_eq!(doc.get("b"), Some("2"));
        assert_eq!(doc.get("c"), Some("3"));
        assert_eq!(doc.get("d"), Some("4"));
        assert_eq!(doc.get("e"), Some(""));
        assert_eq!(doc.get("f"), Some("6"));
        assert_eq!(doc.get("g"), Some(""));
    }

    #[test]
    fn value_keeps_trailing_whitespace_and_inner_separators() {
        let doc = parse("motd=a:b c=d  \n");
        assert_eq!(doc.get("motd"), Some("a:b c=d  "));
    }

    #[test]
    fn comments_are_skipped_and_kept() {
        let text = "#Minecraft server properties\n  # indented\n! bang\n\nkey=value\n";
        let (doc, warnings) = PropertiesDocument::parse(text);
        assert!(warnings.is_empty());
        assert_eq!(doc.entries().len(), 1);
        assert_eq!(doc.to_text(), text);
    }

    #[test]
    fn escapes() {
        let doc = parse(
            "motd=\\u00A7aGreen \\u00e9t\\u00E9\ntab=a\\tb\nnl=a\\nb\nslash=a\\\\b\nother=\\q\nkey\\ with\\ space=x\nemoji=\\uD83D\\uDE00\n",
        );
        assert_eq!(doc.get("motd"), Some("\u{A7}aGreen \u{e9}t\u{e9}"));
        assert_eq!(doc.get("tab"), Some("a\tb"));
        assert_eq!(doc.get("nl"), Some("a\nb"));
        assert_eq!(doc.get("slash"), Some("a\\b"));
        assert_eq!(doc.get("other"), Some("q"));
        assert_eq!(doc.get("key with space"), Some("x"));
        assert_eq!(doc.get("emoji"), Some("\u{1F600}"));
    }

    #[test]
    fn continuation_lines() {
        let text = "motd=hello \\\n    world\nnext=1\n";
        let doc = parse(text);
        assert_eq!(doc.get("motd"), Some("hello world"));
        assert_eq!(doc.get("next"), Some("1"));
        assert_eq!(doc.to_text(), text);
        // an even number of backslashes is an escaped backslash, not a continuation
        let doc = parse("path=C:\\\\\nnext=1\n");
        assert_eq!(doc.get("path"), Some("C:\\"));
        assert_eq!(doc.get("next"), Some("1"));
    }

    #[test]
    fn malformed_lines_are_kept_and_reported() {
        let text = "=foo\nbad=\\u12\ngood=1\n:also-empty\n";
        let (doc, warnings) = PropertiesDocument::parse(text);
        assert_eq!(warnings.len(), 3);
        assert_eq!(warnings[0].line_number, 1);
        assert_eq!(warnings[1].line_number, 2);
        assert_eq!(doc.entries().len(), 1);
        assert_eq!(doc.get("good"), Some("1"));
        assert_eq!(doc.to_text(), text);
    }

    #[test]
    fn set_changes_only_that_line() {
        let text = "#comment\nb=2\nunknown-key=keep me\na=1\n";
        let mut doc = parse(text);
        doc.set("a", "100");
        assert_eq!(doc.to_text(), "#comment\nb=2\nunknown-key=keep me\na=100\n");
        doc.set("new", "x");
        assert_eq!(
            doc.to_text(),
            "#comment\nb=2\nunknown-key=keep me\na=100\nnew=x\n"
        );
        // setting the same value keeps the original text
        let mut doc = parse("level-type=minecraft\\:normal\n");
        doc.set("level-type", "minecraft:normal");
        assert_eq!(doc.to_text(), "level-type=minecraft\\:normal\n");
    }

    #[test]
    fn set_escapes_values() {
        let mut doc = parse("motd=x\n");
        doc.set("motd", " A=B:C #! \\ \u{A7}l\nnew");
        let text = doc.to_text();
        assert_eq!(text, "motd=\\ A\\=B\\:C \\#\\! \\\\ \\u00A7l\\nnew\n");
        assert_eq!(parse(&text).get("motd"), Some(" A=B:C #! \\ \u{A7}l\nnew"));
    }

    #[test]
    fn duplicate_keys_last_wins() {
        let mut doc = parse("a=1\nb=2\na=3\n");
        assert_eq!(doc.get("a"), Some("3"));
        assert_eq!(
            doc.entries().into_iter().collect::<Vec<_>>(),
            vec![
                ("a".to_string(), "3".to_string()),
                ("b".to_string(), "2".to_string())
            ]
        );
        doc.set("a", "4");
        assert_eq!(doc.to_text(), "a=1\nb=2\na=4\n");
    }

    #[test]
    fn keeps_crlf_and_missing_trailing_newline() {
        let text = "#c\r\na=1\r\nb=2";
        let mut doc = parse(text);
        assert_eq!(doc.get("b"), Some("2"));
        assert_eq!(doc.to_text(), text);
        doc.set("a", "5");
        assert_eq!(doc.to_text(), "#c\r\na=5\r\nb=2");
    }

    #[test]
    fn latin1_files_are_written_back_as_latin1() {
        let bytes = b"motd=caf\xe9\nmax-players=20\n";
        let (mut doc, warnings) = PropertiesDocument::from_bytes(bytes);
        assert!(warnings.is_empty());
        assert_eq!(doc.get("motd"), Some("caf\u{e9}"));
        doc.set("max-players", "30");
        assert_eq!(doc.to_bytes(), b"motd=caf\xe9\nmax-players=30\n".to_vec());
    }

    #[test]
    fn empty_input() {
        let (mut doc, warnings) = PropertiesDocument::parse("");
        assert!(warnings.is_empty());
        assert!(doc.entries().is_empty());
        assert_eq!(doc.to_text(), "");
        doc.set("server-port", "25565");
        assert_eq!(doc.to_text(), "server-port=25565\n");
    }
}
