//! Making names, paths and versions safe and short enough to show.
//!
//! Text from a server, a peer or a file is untrusted: a mod's file name can carry a
//! right-to-left override that makes `gpj.exe` read as `exe.jpg`, a newline that
//! breaks a one-line layout (GPUI's line shaper asserts there is none), or a
//! megabyte of characters. [`sanitize`] neutralises all three before anything is
//! drawn. The rest shorten a value in the middle, so the part people recognise (a
//! file's extension, a path's last folders, a version's end) stays visible.
//!
//! Lengths are counted in graphemes (what a reader sees as one character), so an
//! accented letter or an emoji is never split.

use std::borrow::Cow;

use unicode_segmentation::UnicodeSegmentation as _;

/// What stands for the part that was left out.
pub const ELLIPSIS: &str = "…";

/// The most graphemes a shown value keeps; longer ones end in an ellipsis.
pub const MAX_SHOWN: usize = 4096;

/// `text` made safe to draw on one line, at most `max_graphemes` long:
///
/// - newlines, carriage returns and tabs become spaces;
/// - other control characters (C0, DEL, C1) become U+FFFD, so they are seen;
/// - the bidirectional controls (U+061C, U+200E, U+200F, U+202A to U+202E and U+2066
///   to U+2069) are dropped, so text shows in the order it is stored and can't be
///   made to read backwards;
/// - longer text keeps its first graphemes and ends in an ellipsis.
pub fn sanitize(text: &str, max_graphemes: usize) -> String {
    let cleaned: Cow<'_, str> = if text.chars().any(needs_cleaning) {
        Cow::Owned(
            text.chars()
                .filter_map(|c| match c {
                    '\n' | '\r' | '\t' => Some(' '),
                    c if is_bidi_control(c) => None,
                    c if c.is_control() => Some('\u{FFFD}'),
                    c => Some(c),
                })
                .collect(),
        )
    } else {
        Cow::Borrowed(text)
    };
    // Bytes bound graphemes from above: short text needs no counting.
    if cleaned.len() <= max_graphemes {
        return cleaned.into_owned();
    }
    end(&cleaned, max_graphemes)
}

fn needs_cleaning(c: char) -> bool {
    c.is_control() || is_bidi_control(c)
}

fn is_bidi_control(c: char) -> bool {
    matches!(
        c,
        '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
    )
}

/// `text` cut at `max` graphemes, the last of them an ellipsis when it was cut.
pub fn end(text: &str, max: usize) -> String {
    let mut graphemes = text.grapheme_indices(true);
    match graphemes.nth(max) {
        None => text.to_string(),
        Some(_) if max == 0 => String::new(),
        Some(_) => {
            let cut = text
                .grapheme_indices(true)
                .nth(max - 1)
                .map_or(text.len(), |(i, _)| i);
            format!("{}{ELLIPSIS}", &text[..cut])
        }
    }
}

/// `path` with the home folder shown as `~`: `/Users/alex/Documents` becomes
/// `~/Documents`. Only a whole folder matches (`/Users/alexander` is not under
/// `/Users/alex`), ignoring ASCII case where the platform's disks usually do
/// (Windows and macOS). The path keeps its own separators.
pub fn display_path(path: &str, home: Option<&str>) -> String {
    display_path_with(
        path,
        home,
        cfg!(any(windows, target_os = "macos")),
        cfg!(windows),
    )
}

/// [`display_path`] with the platform's rules given, for the tests.
fn display_path_with(path: &str, home: Option<&str>, fold_case: bool, backslash: bool) -> String {
    let is_sep = |c: char| c == '/' || (backslash && c == '\\');
    let Some(home) = home.map(|home| home.trim_end_matches(is_sep)) else {
        return path.to_string();
    };
    if home.is_empty() || path.len() < home.len() || !path.is_char_boundary(home.len()) {
        return path.to_string();
    }
    let (prefix, rest) = path.split_at(home.len());
    let same = if fold_case {
        prefix.eq_ignore_ascii_case(home)
    } else {
        prefix == home
    };
    // The home folder itself, or something inside it.
    if same && (rest.is_empty() || rest.starts_with(is_sep)) {
        format!("~{rest}")
    } else {
        path.to_string()
    }
}

/// The ways to show a path, from fullest to shortest: the path, then its root with
/// the last two parts (`~/…/instances/old-pack`), the last two parts
/// (`…/instances/old-pack`) and the last part (`…/old-pack`). The first that fits is
/// shown; the leaf folder is the part people look for.
pub fn path_candidates(display: &str) -> Vec<String> {
    let sep = display
        .chars()
        .find(|c| *c == '/' || *c == '\\')
        .unwrap_or('/');
    let trimmed = display.trim_end_matches(['/', '\\']);
    let parts: Vec<&str> = trimmed.split(['/', '\\']).collect();
    let mut candidates = vec![display.to_string()];
    let mut push = |candidate: String| {
        if !candidates.contains(&candidate) {
            candidates.push(candidate);
        }
    };
    let n = parts.len();
    if n >= 4 {
        push(format!(
            "{}{sep}{ELLIPSIS}{sep}{}{sep}{}",
            parts[0],
            parts[n - 2],
            parts[n - 1]
        ));
    }
    if n >= 3 {
        push(format!(
            "{ELLIPSIS}{sep}{}{sep}{}",
            parts[n - 2],
            parts[n - 1]
        ));
    }
    if n >= 2 {
        push(format!("{ELLIPSIS}{sep}{}", parts[n - 1]));
    }
    candidates
}

/// `text` shortened to `max` graphemes by leaving out its middle, as much kept at
/// each end: `mc1.21.1-0.6.0-fabric` at 12 is `mc1.21…fabric`.
pub fn middle(text: &str, max: usize) -> String {
    let graphemes: Vec<&str> = text.graphemes(true).collect();
    if graphemes.len() <= max {
        return text.to_string();
    }
    match max {
        0 => String::new(),
        1 => ELLIPSIS.to_string(),
        _ => {
            let keep = max - 1;
            let head = keep.div_ceil(2);
            let tail = keep - head;
            join(&graphemes[..head], &graphemes[graphemes.len() - tail..])
        }
    }
}

/// A file name shortened to `max` graphemes keeping its extension and a few
/// characters before it at the end: `sodium-fabric-0.6.0+mc1.21.1.jar` becomes
/// `sodium-fabr…21.1.jar`. An extension is 1 to 8 characters after the last dot, not
/// counting a leading dot (`.minecraft` has none). Without one, the name is cut in
/// the middle.
pub fn middle_keeping_extension(name: &str, max: usize) -> String {
    let graphemes: Vec<&str> = name.graphemes(true).collect();
    if graphemes.len() <= max {
        return name.to_string();
    }
    let Some(extension) = extension_len(&graphemes) else {
        return middle(name, max);
    };
    let stem = graphemes.len() - extension;
    // Keep up to 4 graphemes before the extension, fewer when the budget is small,
    // and at least one at the start.
    for before in (0..=4.min(stem.saturating_sub(1))).rev() {
        let tail = extension + before;
        if max >= tail + 2 {
            let head = max - 1 - tail;
            return join(&graphemes[..head], &graphemes[graphemes.len() - tail..]);
        }
    }
    middle(name, max)
}

/// The length in graphemes of a name's extension with its dot, if it has one.
fn extension_len(graphemes: &[&str]) -> Option<usize> {
    let dot = graphemes.iter().rposition(|g| *g == ".")?;
    let len = graphemes.len() - dot - 1;
    let ok = dot > 0
        && (1..=8).contains(&len)
        && graphemes[dot + 1..].iter().all(|g| {
            g.chars()
                .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        });
    ok.then_some(len + 1)
}

fn join(head: &[&str], tail: &[&str]) -> String {
    let mut out = String::new();
    out.extend(head.iter().copied());
    out.push_str(ELLIPSIS);
    out.extend(tail.iter().copied());
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn count(text: &str) -> usize {
        text.graphemes(true).count()
    }

    #[test]
    fn sanitize_neutralises_controls_and_caps_the_length() {
        // a right-to-left override can't flip "gpj.jar" into something else
        assert_eq!(sanitize("evil\u{202E}gpj.jar", 100), "evilgpj.jar");
        assert_eq!(sanitize("a\u{2066}b\u{2069}\u{200F}c", 100), "abc");
        assert_eq!(sanitize("two\nlines\r\tend", 100), "two lines  end");
        assert_eq!(sanitize("bell\u{7}", 100), "bell\u{FFFD}");
        assert_eq!(sanitize("c1\u{85}", 100), "c1\u{FFFD}");
        assert_eq!(sanitize("Fabric API", 100), "Fabric API");
        assert_eq!(sanitize("abcdef", 4), "abc…");
        assert_eq!(sanitize("abcdef", 0), "");
        // graphemes, not bytes: an accented e and a flag stay whole
        assert_eq!(sanitize("e\u{301}e\u{301}e\u{301}", 2), "e\u{301}…");
        assert_eq!(sanitize("🇫🇷🇫🇷", 2), "🇫🇷🇫🇷");
        let long = "x".repeat(10_000);
        assert_eq!(count(&sanitize(&long, MAX_SHOWN)), MAX_SHOWN);
    }

    #[test]
    fn home_is_shown_as_a_tilde_only_on_a_folder_boundary() {
        let home = Some("/Users/alex");
        assert_eq!(
            display_path_with("/Users/alex/Documents/x", home, false, false),
            "~/Documents/x"
        );
        assert_eq!(display_path_with("/Users/alex", home, false, false), "~");
        assert_eq!(
            display_path_with("/Users/alexander/x", home, false, false),
            "/Users/alexander/x"
        );
        assert_eq!(
            display_path_with("/Users/alex/x", Some("/Users/alex/"), false, false),
            "~/x"
        );
        assert_eq!(display_path_with("/opt/x", home, false, false), "/opt/x");
        assert_eq!(
            display_path_with("/Users/alex/x", None, false, false),
            "/Users/alex/x"
        );
        // case folded where the disks ignore case
        assert_eq!(display_path_with("/users/ALEX/x", home, true, false), "~/x");
        assert_eq!(
            display_path_with("/users/ALEX/x", home, false, false),
            "/users/ALEX/x"
        );
        // Windows separators are kept
        let home = Some("C:\\Users\\Alex");
        assert_eq!(
            display_path_with("c:\\users\\alex\\AppData\\x", home, true, true),
            "~\\AppData\\x"
        );
        assert_eq!(
            display_path_with("C:\\Users\\Alexa\\x", home, true, true),
            "C:\\Users\\Alexa\\x"
        );
        // multi-byte text near the cut doesn't panic
        assert_eq!(display_path_with("/Usé", Some("/Us"), false, false), "/Usé");
        assert_eq!(
            display_path_with("/ab", Some("/Users"), false, false),
            "/ab"
        );
    }

    #[test]
    fn path_candidates_keep_the_last_two_parts() {
        assert_eq!(
            path_candidates("~/Library/Lodestone/instances/old-pack"),
            [
                "~/Library/Lodestone/instances/old-pack",
                "~/…/instances/old-pack",
                "…/instances/old-pack",
                "…/old-pack",
            ]
        );
        assert_eq!(
            path_candidates("C:\\Games\\mods\\a.jar"),
            [
                "C:\\Games\\mods\\a.jar",
                "C:\\…\\mods\\a.jar",
                "…\\mods\\a.jar",
                "…\\a.jar"
            ]
        );
        assert_eq!(path_candidates("/a/b/"), ["/a/b/", "…/a/b", "…/b"]);
        assert_eq!(path_candidates("name"), ["name"]);
        assert_eq!(path_candidates(""), [""]);
    }

    #[test]
    fn middle_is_balanced_and_keeps_graphemes_whole() {
        assert_eq!(middle("mc1.21.1-0.6.0-fabric", 12), "mc1.21…abric");
        assert_eq!(count(&middle("mc1.21.1-0.6.0-fabric", 12)), 12);
        assert_eq!(middle("short", 10), "short");
        assert_eq!(middle("abcdef", 1), "…");
        assert_eq!(middle("abcdef", 0), "");
        assert_eq!(middle("", 0), "");
        assert_eq!(middle("ée\u{301}éée\u{301}", 3), "é…\u{65}\u{301}");
        assert_eq!(middle("世界世界世界", 4), "世界…界");
    }

    #[test]
    fn file_names_keep_their_extension() {
        let name = "sodium-fabric-0.6.0+mc1.21.1.jar";
        let short = middle_keeping_extension(name, 20);
        assert_eq!(short, "sodium-fabr…21.1.jar");
        assert_eq!(count(&short), 20);
        assert!(short.ends_with(".jar"));
        // a tight budget drops the characters before the extension first
        assert_eq!(middle_keeping_extension(name, 7), "s…1.jar");
        assert_eq!(middle_keeping_extension(name, 6), "s….jar");
        // too tight for the extension: cut in the middle
        assert_eq!(middle_keeping_extension(name, 4), "so…r");
        assert_eq!(middle_keeping_extension(name, 0), "");
        assert_eq!(middle_keeping_extension(name, 1), "…");
        // no extension, or a leading dot only
        assert_eq!(middle_keeping_extension(".minecraft-folder", 8), ".min…der");
        assert_eq!(
            middle_keeping_extension("noextension-at-all", 8),
            "noex…all"
        );
        // an "extension" too long to be one
        assert_eq!(
            middle_keeping_extension("archive.verylongext", 10),
            "archi…gext"
        );
        assert_eq!(middle_keeping_extension("a.jar", 3), "a…r");
        assert_eq!(middle_keeping_extension("", 3), "");
        assert_eq!(
            middle_keeping_extension("世界世界世界.zip", 8),
            "世…世界.zip"
        );
    }
}
