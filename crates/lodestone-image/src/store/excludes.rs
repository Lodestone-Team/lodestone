//! An image's exclude patterns (`excludes`): what a snapshot leaves out on purpose, such as
//! web map tiles that are large, change all the time and can be rendered again.
//!
//! Patterns match canonical paths (so `saves/world/…` for a world's files, on a server as
//! on a client), case-sensitively, so an image's excludes mean the same on every computer.
//! The syntax is the core's backup patterns (a small subset of gitignore):
//!
//! - `name` matches a file or folder with that name at any depth.
//! - A pattern with a `/` in it (other than a trailing one) matches from the game folder:
//!   `dynmap/web/tiles`. A leading `/` is allowed and means the same.
//! - `*` matches any run of characters within one part of a path.
//! - `**` as a whole part matches any number of parts, none included.
//! - A trailing `/` matches folders only.
//!
//! A pattern that matches a folder leaves out everything in it. Matching is linear in the
//! pattern and path sizes (a table over parts, and wildcard matching that backtracks only
//! to the last `*`), so patterns like `**/**/**/x` from another computer can't stall a
//! snapshot.

/// Why `pattern` isn't accepted, as a sentence; `None` when it is fine.
pub fn pattern_error(pattern: &str) -> Option<String> {
    if pattern.trim().is_empty() {
        return Some("Exclude patterns can't be empty".to_string());
    }
    if pattern.chars().count() > crate::manifest::MAX_EXCLUDE_LEN {
        return Some(format!(
            "Exclude patterns can be at most {} characters long",
            crate::manifest::MAX_EXCLUDE_LEN
        ));
    }
    if pattern.chars().any(|c| c.is_control() || c == '\\') {
        return Some(format!(
            "The exclude pattern {pattern:?} has a backslash or a control character; use / between folders"
        ));
    }
    if pattern.starts_with('!') {
        return Some(format!(
            "The exclude pattern {pattern:?} starts with !, which isn't supported"
        ));
    }
    let body = pattern.trim_start_matches('/').trim_end_matches('/');
    if body.is_empty() || body.split('/').any(|part| part.is_empty()) {
        return Some(format!("The exclude pattern {pattern:?} has an empty part"));
    }
    None
}

#[derive(Debug, Clone, PartialEq)]
enum Part {
    /// `**`: any number of path parts.
    AnyParts,
    /// A name, possibly with `*`, as characters.
    Glob(Vec<char>),
}

#[derive(Debug, Clone)]
struct Pattern {
    parts: Vec<Part>,
    dir_only: bool,
}

/// Compiled exclude patterns.
#[derive(Debug, Clone, Default)]
pub struct Excludes {
    patterns: Vec<Pattern>,
}

impl Excludes {
    /// Compiles `patterns`; those [`pattern_error`] refuses match nothing.
    pub fn new(patterns: &[String]) -> Excludes {
        Excludes {
            patterns: patterns
                .iter()
                .filter(|p| pattern_error(p).is_none())
                .map(|p| compile(p))
                .collect(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Whether a pattern matches `path` itself (`is_dir`: it is a folder).
    pub fn matches(&self, path: &str, is_dir: bool) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        let parts: Vec<Vec<char>> = path.split('/').map(|p| p.chars().collect()).collect();
        self.patterns
            .iter()
            .any(|pattern| (is_dir || !pattern.dir_only) && matches_parts(&pattern.parts, &parts))
    }

    /// Whether a pattern matches `path` or any folder it is in.
    pub fn excludes(&self, path: &str, is_dir: bool) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        for (i, c) in path.char_indices() {
            if c == '/' && self.matches(&path[..i], true) {
                return true;
            }
        }
        self.matches(path, is_dir)
    }
}

fn compile(pattern: &str) -> Pattern {
    let dir_only = pattern.ends_with('/');
    let body = pattern.trim_end_matches('/');
    let anchored = body.contains('/');
    let body = body.trim_start_matches('/');
    let mut parts = Vec::new();
    if !anchored {
        parts.push(Part::AnyParts);
    }
    for part in body.split('/') {
        if part == "**" {
            // `**/**` is the same as `**`.
            if parts.last() != Some(&Part::AnyParts) {
                parts.push(Part::AnyParts);
            }
        } else {
            parts.push(Part::Glob(part.chars().collect()));
        }
    }
    Pattern { parts, dir_only }
}

/// Whether `pattern` matches all of `path`'s parts. `reach[j]` is true when the pattern
/// parts seen so far can match exactly the first `j` path parts.
fn matches_parts(pattern: &[Part], path: &[Vec<char>]) -> bool {
    let n = path.len();
    let mut reach = vec![false; n + 1];
    reach[0] = true;
    for part in pattern {
        let mut next = vec![false; n + 1];
        match part {
            Part::AnyParts => {
                let mut seen = false;
                for j in 0..=n {
                    seen |= reach[j];
                    next[j] = seen;
                }
            }
            Part::Glob(glob) => {
                for j in 0..n {
                    if reach[j] && glob_matches(glob, &path[j]) {
                        next[j + 1] = true;
                    }
                }
            }
        }
        reach = next;
        if !reach.iter().any(|r| *r) {
            return false;
        }
    }
    reach[n]
}

/// `*` wildcard matching within one name, backtracking only to the last `*`.
fn glob_matches(glob: &[char], name: &[char]) -> bool {
    let (mut g, mut s) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while s < name.len() {
        if g < glob.len() && glob[g] == '*' {
            star = Some((g, s));
            g += 1;
        } else if g < glob.len() && glob[g] == name[s] {
            g += 1;
            s += 1;
        } else if let Some((star_g, star_s)) = star {
            g = star_g + 1;
            s = star_s + 1;
            star = Some((star_g, star_s + 1));
        } else {
            return false;
        }
    }
    glob[g..].iter().all(|c| *c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn excludes(patterns: &[&str]) -> Excludes {
        Excludes::new(&patterns.iter().map(|p| p.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn pattern_table() {
        // (pattern, path, is_dir, excluded)
        let table: &[(&str, &str, bool, bool)] = &[
            ("tiles", "tiles", true, true),
            ("tiles", "dynmap/web/tiles", true, true),
            ("tiles", "dynmap/web/tiles2", true, false),
            ("tiles/", "dynmap/tiles", false, false),
            ("tiles/", "dynmap/tiles", true, true),
            ("dynmap/web/tiles/", "dynmap/web/tiles", true, true),
            ("dynmap/web/tiles/", "plugins/dynmap/web/tiles", true, false),
            ("/dynmap/web/tiles", "dynmap/web/tiles", true, true),
            ("*.log", "config/x.log", false, true),
            ("*.log", "config/x.log.gz", false, false),
            ("saves/*/DIM1/", "saves/world/DIM1", true, true),
            ("saves/*/DIM1/", "saves/world/region/DIM1", true, false),
            ("saves/**/poi", "saves/world/DIM-1/poi", false, true),
            ("**/x", "x", false, true),
            ("a/**/b", "a/b", false, true),
            ("Tiles", "tiles", true, false),
        ];
        for &(pattern, path, is_dir, expected) in table {
            assert_eq!(
                excludes(&[pattern]).matches(path, is_dir),
                expected,
                "{pattern} against {path} (folder: {is_dir})"
            );
        }
    }

    #[test]
    fn a_matched_folder_excludes_its_contents() {
        let e = excludes(&["dynmap/web/tiles/"]);
        assert!(e.excludes("dynmap/web/tiles/a/b.png", false));
        assert!(!e.excludes("dynmap/web/other.png", false));
    }

    #[test]
    fn bad_patterns_match_nothing() {
        for bad in ["", " ", "a//b", "!x", "a\\b", "/", "x\u{7}"] {
            assert!(pattern_error(bad).is_some(), "{bad:?}");
            assert!(excludes(&[bad]).is_empty());
        }
        assert!(pattern_error(&"a".repeat(257)).is_some());
    }

    #[test]
    fn hostile_patterns_stay_fast() {
        let pattern = "**/".repeat(70) + "x*x*x*x*x*x*x*x*x*y";
        let e = excludes(&[&pattern]);
        let path = vec!["x".repeat(100); 100].join("/");
        let started = std::time::Instant::now();
        assert!(!e.matches(&path, false));
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
