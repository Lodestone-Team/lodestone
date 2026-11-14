//! The reference matcher for tests: a policy's patterns matched as written, on the
//! server's disk paths, which `crate::images::excludes` checks its canonical translation
//! against. Backups never use it: they match the translation in the image store's walk.

use super::pattern_error;

/// Calls `f` with each folder `path` is in (as a folder), then `path` itself, and returns
/// true as soon as `f` does.
fn for_each_prefix(path: &str, is_dir: bool, mut f: impl FnMut(&str, bool) -> bool) -> bool {
    for (i, c) in path.char_indices() {
        if c == '/' && f(&path[..i], true) {
            return true;
        }
    }
    f(path, is_dir)
}

/// One part of a pattern.
#[derive(Debug, Clone, PartialEq)]
enum Part {
    /// `**`: any number of path parts.
    AnyParts,
    /// A name, possibly with `*`, stored as characters (lowercased when matching ignores case).
    Glob(Vec<char>),
}

#[derive(Debug, Clone)]
struct Pattern {
    parts: Vec<Part>,
    dir_only: bool,
}

/// The policy's exclude patterns matched as written, on server disk paths: the reference
/// model `crate::images::excludes`' tests check the canonical translation against (see the
/// module). Linear in the pattern and path sizes (a table over parts, and wildcard matching
/// that backtracks only to the last `*`).
#[derive(Debug, Clone, Default)]
pub struct Matcher {
    patterns: Vec<Pattern>,
    ignore_case: bool,
}

impl Matcher {
    /// A matcher for `patterns`, ignoring letter case when asked. Patterns that
    /// [`pattern_error`] refuses are skipped.
    pub fn with_case(patterns: &[String], ignore_case: bool) -> Matcher {
        let patterns = patterns
            .iter()
            .filter(|p| pattern_error(p).is_none())
            .map(|p| compile(p, ignore_case))
            .collect();
        Matcher {
            patterns,
            ignore_case,
        }
    }

    /// Whether a pattern matches `path` itself (`is_dir`: it is a folder).
    pub fn is_excluded(&self, path: &str, is_dir: bool) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        let folded;
        let path = if self.ignore_case {
            folded = path.to_lowercase();
            folded.as_str()
        } else {
            path
        };
        let parts: Vec<Vec<char>> = path.split('/').map(|p| p.chars().collect()).collect();
        self.patterns
            .iter()
            .any(|pattern| (is_dir || !pattern.dir_only) && matches_parts(&pattern.parts, &parts))
    }

    /// Whether a pattern matches `path` or any folder it is in.
    pub fn is_excluded_path(&self, path: &str, is_dir: bool) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        for_each_prefix(path, is_dir, |prefix, prefix_is_dir| {
            self.is_excluded(prefix, prefix_is_dir)
        })
    }
}

fn compile(pattern: &str, ignore_case: bool) -> Pattern {
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
            // `**/**` is the same as `**`
            if parts.last() != Some(&Part::AnyParts) {
                parts.push(Part::AnyParts);
            }
        } else if ignore_case {
            parts.push(Part::Glob(part.to_lowercase().chars().collect()));
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

mod tests {
    use super::*;
    use crate::backups::pattern::{default_patterns, pattern_error};

    fn matcher(patterns: &[&str]) -> Matcher {
        let patterns: Vec<String> = patterns.iter().map(|p| p.to_string()).collect();
        Matcher::with_case(&patterns, false)
    }

    #[test]
    fn pattern_table() {
        // (pattern, path, is_dir, excluded)
        let table: &[(&str, &str, bool, bool)] = &[
            // a plain name matches at any depth, files and folders
            ("cache", "cache", true, true),
            ("cache", "plugins/x/cache", true, true),
            ("cache", "plugins/x/cache", false, true),
            ("cache", "plugins/cache2", false, false),
            ("cache", "cache/inner", false, false),
            // trailing slash: folders only
            ("tiles/", "tiles", true, true),
            ("tiles/", "tiles", false, false),
            ("tiles/", "a/b/tiles", true, true),
            // a slash anchors to the server folder
            (
                "plugins/dynmap/web/tiles/",
                "plugins/dynmap/web/tiles",
                true,
                true,
            ),
            (
                "plugins/dynmap/web/tiles/",
                "x/plugins/dynmap/web/tiles",
                true,
                false,
            ),
            (
                "plugins/dynmap/web/tiles/",
                "plugins/dynmap/web",
                true,
                false,
            ),
            ("/backups", "backups", true, true),
            ("/backups", "world/backups", true, false),
            ("world/region", "world/region", true, true),
            // * within one part
            ("*.zip", "old.zip", false, true),
            ("*.zip", "a/b/old.zip", false, true),
            ("*.zip", "old.zip.bak", false, false),
            ("*.zip", "dir.zip/x", false, false),
            ("world*/stats", "world_nether/stats", true, true),
            ("world*/stats", "world/stats", true, true),
            ("world*/stats", "a/world/stats", true, false),
            ("a*b*c", "abc", false, true),
            ("a*b*c", "aXbYc", false, true),
            ("a*b*c", "aXbY", false, false),
            ("*", "anything", false, true),
            // ** matches any number of parts
            ("plugins/**/cache", "plugins/cache", true, true),
            ("plugins/**/cache", "plugins/a/b/c/cache", true, true),
            ("plugins/**/cache", "mods/a/cache", true, false),
            ("**/region", "world/region", true, true),
            ("**/region", "region", true, true),
            ("world/**", "world/a/b", false, true),
            ("**/**/**/x", "a/b/c/d/x", false, true),
            ("**/**/**/x", "a/b/c/d/y", false, false),
        ];
        for (pattern, path, is_dir, excluded) in table {
            assert_eq!(
                matcher(&[pattern]).is_excluded(path, *is_dir),
                *excluded,
                "{pattern:?} on {path:?} (dir: {is_dir})"
            );
        }
    }

    #[test]
    fn excluded_path_checks_parents() {
        let m = matcher(&["plugins/dynmap/web/tiles/"]);
        assert!(m.is_excluded_path("plugins/dynmap/web/tiles/a/b.png", false));
        assert!(!m.is_excluded_path("plugins/dynmap/web/x.png", false));
        // a folder-only pattern matches parents, which are folders
        let m = matcher(&["tiles/"]);
        assert!(m.is_excluded_path("a/tiles/b", false));
        assert!(!m.is_excluded_path("a/b/tiles", false));
    }

    #[test]
    fn ignores_case_when_asked() {
        let patterns = vec!["plugins/bluemap/web/maps/".to_string()];
        let m = Matcher::with_case(&patterns, true);
        assert!(m.is_excluded("plugins/BlueMap/web/maps", true));
        let m = Matcher::with_case(&patterns, false);
        assert!(!m.is_excluded("plugins/BlueMap/web/maps", true));
    }

    #[test]
    fn default_patterns_are_valid() {
        let defaults = default_patterns();
        assert!(defaults.iter().all(|p| pattern_error(p).is_none()));
        let m = Matcher::with_case(&defaults, false);
        assert!(m.is_excluded("plugins/BlueMap/web/maps", true));
        assert!(m.is_excluded("dynmap/web/tiles", true));
        assert!(!m.is_excluded("world", true));
    }

    #[test]
    fn refuses_bad_patterns() {
        assert!(pattern_error("").is_some());
        assert!(pattern_error("   ").is_some());
        assert!(pattern_error("a\\b").is_some());
        assert!(pattern_error("!keep").is_some());
        assert!(pattern_error("a//b").is_some());
        assert!(pattern_error("/").is_some());
        let longest = lodestone_image::manifest::MAX_EXCLUDE_LEN;
        assert!(pattern_error(&"x".repeat(longest + 1)).is_some());
        assert!(pattern_error(&"x".repeat(longest)).is_none());
        assert!(pattern_error("a/**/b/").is_none());
    }

    #[test]
    fn many_stars_stay_fast() {
        // exponential backtracking would never finish these
        let pattern = format!("{}x", "**/".repeat(50));
        let path = vec!["a"; 200].join("/");
        let m = matcher(&[&pattern]);
        let started = std::time::Instant::now();
        assert!(!m.is_excluded(&path, false));
        let glob = format!("{}b", "*a".repeat(100));
        let name = "a".repeat(250);
        assert!(!matcher(&[&glob]).is_excluded(&name, false));
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }
}
