//! Which files a backup leaves out: the rules every backup follows, and the policy's own
//! `exclude` patterns (a small subset of gitignore).
//!
//! Paths here are relative to the server folder, `/`-separated, with no leading `/`.
//!
//! Pattern syntax:
//! - `name` matches a file or folder with that name at any depth.
//! - A pattern with a `/` in it (other than a trailing one) matches from the server folder:
//!   `plugins/dynmap/web/tiles`. A leading `/` is allowed and means the same.
//! - `*` matches any run of characters within one part of a path.
//! - `**` as a whole part matches any number of parts, none included.
//! - A trailing `/` matches folders only.
//!
//! A pattern that matches a folder leaves out everything in it.
//!
//! Matching is linear in the pattern and path sizes (a table over parts, and wildcard
//! matching that backtracks only to the last `*`), so patterns like `**/**/**/x` can't
//! stall the backup.

/// At most this many patterns in a policy.
pub const MAX_PATTERNS: usize = 100;
/// At most this many characters per pattern.
pub const MAX_PATTERN_LEN: usize = 256;

/// The patterns a new policy starts with: web map tiles, which are large, change all the
/// time and can be rendered again.
pub fn default_patterns() -> Vec<String> {
    [
        "dynmap/web/tiles/",
        "plugins/dynmap/web/tiles/",
        "bluemap/web/maps/",
        "plugins/BlueMap/web/maps/",
        "squaremap/web/tiles/",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

/// Whether `path` is never backed up (and never replaced by a restore), whatever the policy.
///
/// - `.lodestone_config` holds the server's identity; a restore keeps the current one and a
///   restore as a new server writes a new one.
/// - `logs/`, `crash-reports/` and `cache/` (top level) are only useful for the running server.
/// - The modpack's own store (`.lodestone_modpack/{objects,staging,snapshot}/`) is large and
///   can be downloaded again; its `record.json`, `history.json` and `notified.json` are kept
///   so they match the restored config.
/// - `.lodestone-restore*` are a restore's own folders, at any depth.
/// - `session.lock` is the running server's lock on a world.
///
/// `is_dir` says whether `path` is a folder. Checks only `path` itself, not its parents; see
/// [`always_excluded_path`].
pub fn always_excluded(path: &str, is_dir: bool) -> bool {
    let mut parts = path.split('/');
    let first = parts.next().unwrap_or_default();
    let name = path.rsplit('/').next().unwrap_or_default();
    if name.starts_with(".lodestone-restore") {
        return true;
    }
    if !is_dir && name == "session.lock" {
        return true;
    }
    let depth = path.split('/').count();
    match depth {
        1 => {
            (!is_dir && first == ".lodestone_config")
                || (is_dir && matches!(first, "logs" | "crash-reports" | "cache"))
        }
        2 => {
            is_dir
                && first == ".lodestone_modpack"
                && matches!(name, "objects" | "staging" | "snapshot")
        }
        _ => false,
    }
}

/// Whether `path` or any folder it is in is [`always_excluded`].
pub fn always_excluded_path(path: &str, is_dir: bool) -> bool {
    for_each_prefix(path, is_dir, |prefix, prefix_is_dir| {
        always_excluded(prefix, prefix_is_dir)
    })
}

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

/// Why a pattern isn't accepted, as a sentence for the user. `None` when it is fine.
pub fn pattern_error(pattern: &str) -> Option<String> {
    if pattern.trim().is_empty() {
        return Some("Exclude patterns can't be empty".to_string());
    }
    if pattern.chars().count() > MAX_PATTERN_LEN {
        return Some(format!(
            "Exclude patterns can be at most {MAX_PATTERN_LEN} characters long"
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

/// The policy's exclude patterns, ready to match.
#[derive(Debug, Clone, Default)]
pub struct Matcher {
    patterns: Vec<Pattern>,
    ignore_case: bool,
}

impl Matcher {
    /// A matcher for `patterns`, ignoring case on Windows (where file names do).
    /// Patterns that [`pattern_error`] refuses are skipped.
    pub fn new(patterns: &[String]) -> Matcher {
        Matcher::with_case(patterns, cfg!(windows))
    }

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

    /// Whether a pattern matches `path` itself (`is_dir`: it is a folder). The walk calls
    /// this for each entry and doesn't go into excluded folders.
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

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(pattern_error(&"x".repeat(MAX_PATTERN_LEN + 1)).is_some());
        assert!(pattern_error(&"x".repeat(MAX_PATTERN_LEN)).is_none());
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

    #[test]
    fn always_excluded_table() {
        let table: &[(&str, bool, bool)] = &[
            (".lodestone_config", false, true),
            ("world/.lodestone_config", false, false),
            ("logs", true, true),
            ("logs", false, false),
            ("plugins/x/logs", true, false),
            ("crash-reports", true, true),
            ("cache", true, true),
            (".lodestone_modpack/objects", true, true),
            (".lodestone_modpack/staging", true, true),
            (".lodestone_modpack/snapshot", true, true),
            (".lodestone_modpack/record.json", false, false),
            (".lodestone_modpack", true, false),
            (".lodestone-restore-staging-0123456789abcdef", true, true),
            ("world/.lodestone-restore-old-x", true, true),
            ("world/session.lock", false, true),
            ("session.lock", false, true),
            ("world/level.dat", false, false),
            ("server.properties", false, false),
        ];
        for (path, is_dir, excluded) in table {
            assert_eq!(
                always_excluded(path, *is_dir),
                *excluded,
                "{path:?} (dir: {is_dir})"
            );
        }
        assert!(always_excluded_path("logs/latest.log", false));
        assert!(always_excluded_path(
            ".lodestone_modpack/objects/ab/cd",
            false
        ));
        assert!(!always_excluded_path("plugins/logs/x.log", false));
    }
}
