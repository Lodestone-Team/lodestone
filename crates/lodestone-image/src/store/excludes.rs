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
//! A pattern that matches a folder leaves out everything in it. All patterns run as one
//! automaton over a path's parts (see [`Excludes`]), and a walk's [`ExcludeCursor`] reads
//! each folder's parts once for everything in it. One name is matched against one glob in
//! time linear in the two lengths, never backtracking (see `Glob`), so reading a part costs
//! O(total pattern bytes + live globs × name bytes) and a snapshot costs that times its
//! paths' parts, however deep the folders or hostile the patterns (`**/**/**/x`,
//! `*a*a*a…`, a thousand patterns from another computer).

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

/// One position in the compiled patterns: before a pattern part, or at a pattern's end.
#[derive(Debug, Clone, PartialEq)]
enum Position {
    /// Before `**`: any number of path parts.
    AnyParts,
    /// Before a name, possibly with `*`.
    Glob(Glob),
    /// After a pattern's last part: it matched.
    End,
}

/// Compiled exclude patterns.
///
/// All patterns are one automaton over path parts: a position per pattern part plus one
/// per pattern's end, and a set of live positions (a bitset). Reading one path part moves
/// every live position at once, so a path of `d` parts costs `d` steps of
/// O(positions / 64 + live globs × (name + glob)) whatever the patterns, and an [`ExcludeCursor`] reuses
/// the steps of the folders a walk is already in.
#[derive(Debug, Clone, Default)]
pub struct Excludes {
    positions: Vec<Position>,
    words: usize,
    /// The live positions before any path part.
    start: Vec<u64>,
    any: Vec<u64>,
    glob: Vec<u64>,
    /// Every pattern's end, and the ends of those that match files too (not `dir/`).
    end_any: Vec<u64>,
    end_file: Vec<u64>,
}

impl Excludes {
    /// Compiles `patterns`; those [`pattern_error`] refuses match nothing.
    pub fn new(patterns: &[String]) -> Excludes {
        let mut positions = Vec::new();
        let mut starts = Vec::new();
        let mut file_ends = Vec::new();
        for pattern in patterns.iter().filter(|p| pattern_error(p).is_none()) {
            starts.push(positions.len());
            let dir_only = compile(pattern, &mut positions);
            if !dir_only {
                file_ends.push(positions.len());
            }
            positions.push(Position::End);
        }
        let words = positions.len().div_ceil(64);
        let mut excludes = Excludes {
            words,
            start: vec![0; words],
            any: vec![0; words],
            glob: vec![0; words],
            end_any: vec![0; words],
            end_file: vec![0; words],
            positions: Vec::new(),
        };
        for (i, position) in positions.iter().enumerate() {
            let mask = match position {
                Position::AnyParts => &mut excludes.any,
                Position::Glob(_) => &mut excludes.glob,
                Position::End => &mut excludes.end_any,
            };
            set(mask, i);
        }
        for i in starts {
            set(&mut excludes.start, i);
        }
        for i in file_ends {
            set(&mut excludes.end_file, i);
        }
        excludes.positions = positions;
        let mut start = std::mem::take(&mut excludes.start);
        excludes.close(&mut start);
        excludes.start = start;
        excludes
    }

    pub fn is_empty(&self) -> bool {
        self.positions.is_empty()
    }

    /// A cursor for checking many paths, as a walk meets them.
    pub fn cursor(&self) -> ExcludeCursor<'_> {
        ExcludeCursor {
            excludes: self,
            path: String::new(),
            ends: Vec::new(),
            states: self.start.clone(),
            rows: vec![Row::default()],
        }
    }

    /// Whether a pattern matches `path` itself (`is_dir`: it is a folder).
    pub fn matches(&self, path: &str, is_dir: bool) -> bool {
        if self.is_empty() {
            return false;
        }
        let mut state = self.start.clone();
        let mut next = vec![0; self.words];
        for part in path.split('/') {
            self.step(&state, part, &mut next);
            std::mem::swap(&mut state, &mut next);
        }
        any(
            &state,
            if is_dir {
                &self.end_any
            } else {
                &self.end_file
            },
        )
    }

    /// Whether a pattern matches `path` or any folder it is in. To check many paths, use a
    /// [`ExcludeCursor`], which reuses the work for the folders they share.
    pub fn excludes(&self, path: &str, is_dir: bool) -> bool {
        self.cursor().excludes(path, is_dir)
    }

    /// Adds the position after each live `**` (which matches no part too).
    fn close(&self, state: &mut [u64]) {
        // A `**` is never followed by another (compile merges them), so one step is enough.
        let mut carry = 0;
        for (word, any) in state.iter_mut().zip(&self.any) {
            let anys = *word & any;
            *word |= (anys << 1) | carry;
            carry = anys >> 63;
        }
    }

    /// The live positions after reading the part `name` from `from`, into `to`.
    fn step(&self, from: &[u64], name: &str, to: &mut [u64]) {
        to.fill(0);
        for w in 0..self.words {
            // A `**` takes the part and stays.
            to[w] |= from[w] & self.any[w];
            let mut globs = from[w] & self.glob[w];
            while globs != 0 {
                let i = w * 64 + globs.trailing_zeros() as usize;
                globs &= globs - 1;
                if let Position::Glob(glob) = &self.positions[i] {
                    if glob.matches(name) {
                        set(to, i + 1);
                    }
                }
            }
        }
        self.close(to);
    }
}

/// What a cursor knows after one part of the path.
#[derive(Debug, Clone, Copy, Default)]
struct Row {
    /// A pattern matches the path so far as a folder, or as a file.
    folder: bool,
    file: bool,
    /// A pattern matches a folder this part is in: everything below is excluded.
    inside_excluded: bool,
}

impl Row {
    fn hit(&self, is_dir: bool) -> bool {
        if is_dir {
            self.folder
        } else {
            self.file
        }
    }
}

/// Checks paths against [`Excludes`], keeping the steps for the parts of the last path, so
/// a walk that checks a folder and then what is in it pays only for the new parts: a chain
/// of `d` nested folders costs O(d) steps, not O(d²).
#[derive(Debug, Clone)]
pub struct ExcludeCursor<'e> {
    excludes: &'e Excludes,
    /// The parts read so far, `/`-separated, and where each ends.
    path: String,
    ends: Vec<usize>,
    /// The live positions before the first part and after each: `ends.len() + 1` rows.
    states: Vec<u64>,
    rows: Vec<Row>,
}

impl ExcludeCursor<'_> {
    pub fn is_empty(&self) -> bool {
        self.excludes.is_empty()
    }

    /// Whether a pattern matches `path` or any folder it is in.
    pub fn excludes(&mut self, path: &str, is_dir: bool) -> bool {
        if self.excludes.is_empty() {
            return false;
        }
        self.move_to(path);
        let last = self.rows[self.rows.len() - 1];
        last.inside_excluded || last.hit(is_dir)
    }

    /// Reads `path`, keeping the rows of the parts it shares with the last one.
    fn move_to(&mut self, path: &str) {
        let mut kept = 0;
        let mut parts = path.split('/');
        let mut rest = None;
        for part in parts.by_ref() {
            if kept < self.ends.len() && self.part(kept) == part {
                kept += 1;
            } else {
                rest = Some(part);
                break;
            }
        }
        self.truncate(kept);
        for part in rest.into_iter().chain(parts) {
            self.push(part);
        }
    }

    fn part(&self, i: usize) -> &str {
        let start = if i == 0 { 0 } else { self.ends[i - 1] + 1 };
        &self.path[start..self.ends[i]]
    }

    fn truncate(&mut self, parts: usize) {
        self.path
            .truncate(if parts == 0 { 0 } else { self.ends[parts - 1] });
        self.ends.truncate(parts);
        self.rows.truncate(parts + 1);
        self.states.truncate((parts + 1) * self.excludes.words);
    }

    fn push(&mut self, part: &str) {
        let excludes = self.excludes;
        let words = excludes.words;
        if !self.ends.is_empty() {
            self.path.push('/');
        }
        self.path.push_str(part);
        self.ends.push(self.path.len());
        let last = self.rows[self.rows.len() - 1];
        let mut row = Row {
            inside_excluded: last.inside_excluded || (self.rows.len() > 1 && last.folder),
            ..Row::default()
        };
        let from = self.states.len() - words;
        self.states.resize(from + 2 * words, 0);
        if !row.inside_excluded {
            // Inside an excluded folder everything is excluded, whatever matches.
            let (before, after) = self.states.split_at_mut(from + words);
            excludes.step(&before[from..], part, after);
            row.folder = any(after, &excludes.end_any);
            row.file = any(after, &excludes.end_file);
        }
        self.rows.push(row);
    }
}

fn set(bits: &mut [u64], i: usize) {
    bits[i / 64] |= 1 << (i % 64);
}

fn any(bits: &[u64], mask: &[u64]) -> bool {
    bits.iter().zip(mask).any(|(b, m)| b & m != 0)
}

/// Adds `pattern`'s positions; returns whether it matches folders only.
fn compile(pattern: &str, positions: &mut Vec<Position>) -> bool {
    let dir_only = pattern.ends_with('/');
    let body = pattern.trim_end_matches('/');
    let anchored = body.contains('/');
    let body = body.trim_start_matches('/');
    if !anchored {
        positions.push(Position::AnyParts);
    }
    for part in body.split('/') {
        if part == "**" {
            // `**/**` is the same as `**`.
            // (The positions before are the previous pattern's, ending in its `End`.)
            if positions.last() != Some(&Position::AnyParts) {
                positions.push(Position::AnyParts);
            }
        } else {
            positions.push(Position::Glob(Glob::new(part)));
        }
    }
    dir_only
}

/// A name pattern with `*` wildcards, split at its stars into literal pieces when compiled.
///
/// Matching never backtracks: without a star the name must equal the glob; with stars, it
/// must start with the first piece and end with the last (without the two overlapping),
/// and each piece between is found in what is left of the name, leftmost first. Taking the
/// leftmost occurrence is always right when the only wildcard is `*`: any match could use
/// it instead and leave more of the name for the pieces after. Each search is
/// `str::find` (Two-Way: linear in the text it scans plus the piece), and it scans only
/// what is left after the last piece found, so one match costs O(name + glob) whatever the
/// glob, against O(name × glob) for a matcher that backtracks to its last star.
///
/// On `str`, which gives the same answers as on characters: the pieces are whole
/// characters, and UTF-8 never finds one inside another character.
#[derive(Debug, Clone, PartialEq)]
struct Glob {
    /// The literal runs between stars, in order (empty runs dropped, except that a glob
    /// without stars is its one piece).
    pieces: Vec<String>,
    /// Whether the glob has a star at all, at its start and at its end.
    star: bool,
    star_start: bool,
    star_end: bool,
}

impl Glob {
    fn new(glob: &str) -> Glob {
        let star = glob.contains('*');
        Glob {
            pieces: if star {
                glob.split('*')
                    .filter(|piece| !piece.is_empty())
                    .map(str::to_string)
                    .collect()
            } else {
                vec![glob.to_string()]
            },
            star,
            star_start: glob.starts_with('*'),
            star_end: glob.ends_with('*'),
        }
    }

    fn matches(&self, name: &str) -> bool {
        if !self.star {
            return self.pieces[0] == name;
        }
        let mut pieces = &self.pieces[..];
        let mut rest = name;
        if !self.star_start {
            // The glob starts with its first piece, so there is one.
            let Some((first, others)) = pieces.split_first() else {
                return rest.is_empty();
            };
            let Some(after) = rest.strip_prefix(first.as_str()) else {
                return false;
            };
            rest = after;
            pieces = others;
        }
        if !self.star_end {
            // The glob ends with a piece after its last star, so there is one left.
            let Some((last, others)) = pieces.split_last() else {
                return rest.is_empty();
            };
            let Some(before) = rest.strip_suffix(last.as_str()) else {
                return false;
            };
            rest = before;
            pieces = others;
        }
        for piece in pieces {
            match rest.find(piece.as_str()) {
                Some(at) => rest = &rest[at + piece.len()..],
                None => return false,
            }
        }
        true
    }
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

    /// Whether `pattern` matches `path` itself, written the slow, obvious way.
    fn naive_matches(pattern: &str, path: &str, is_dir: bool) -> bool {
        fn glob(g: &[char], n: &[char]) -> bool {
            match g.split_first() {
                None => n.is_empty(),
                Some(('*', rest)) => (0..=n.len()).any(|i| glob(rest, &n[i..])),
                Some((c, rest)) => n.first() == Some(c) && glob(rest, &n[1..]),
            }
        }
        fn parts(p: &[&str], n: &[&str]) -> bool {
            match p.split_first() {
                None => n.is_empty(),
                Some((&"**", rest)) => (0..=n.len()).any(|i| parts(rest, &n[i..])),
                Some((g, rest)) => {
                    let chars = |s: &str| s.chars().collect::<Vec<_>>();
                    !n.is_empty() && glob(&chars(g), &chars(n[0])) && parts(rest, &n[1..])
                }
            }
        }
        let dir_only = pattern.ends_with('/');
        let body = pattern.trim_end_matches('/');
        let anchored = body.contains('/');
        let mut p: Vec<&str> = body.trim_start_matches('/').split('/').collect();
        if !anchored {
            p.insert(0, "**");
        }
        let n: Vec<&str> = path.split('/').collect();
        (is_dir || !dir_only) && parts(&p, &n)
    }

    /// Whether `pattern` matches `path` or a folder it is in, the slow, obvious way.
    fn naive_excludes(pattern: &str, path: &str, is_dir: bool) -> bool {
        let parts: Vec<&str> = path.split('/').collect();
        (1..parts.len()).any(|i| naive_matches(pattern, &parts[..i].join("/"), true))
            || naive_matches(pattern, path, is_dir)
    }

    #[test]
    fn the_automaton_and_the_cursor_agree_with_the_obvious_meaning() {
        let mut seed = 0x5EED_u64;
        let mut below = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let globs = ["a", "b", "ab", "*", "a*", "*b", "**", "é*", "*é", "a*b"];
        let names = ["a", "b", "ab", "ba", "aab", "é", "éb", "aéb"];
        for _ in 0..400 {
            let mut patterns = Vec::new();
            for _ in 0..1 + below(3) {
                let parts: Vec<&str> = (0..1 + below(3))
                    .map(|_| globs[below(globs.len())])
                    .collect();
                let mut pattern = parts.join("/");
                if below(3) == 0 {
                    pattern.insert(0, '/');
                }
                if below(3) == 0 {
                    pattern.push('/');
                }
                patterns.push(pattern);
            }
            let compiled = Excludes::new(&patterns);
            let mut cursor = compiled.cursor();
            for _ in 0..20 {
                let path: Vec<&str> = (0..1 + below(5))
                    .map(|_| names[below(names.len())])
                    .collect();
                let path = path.join("/");
                let is_dir = below(2) == 0;
                let excluded = patterns.iter().any(|p| naive_excludes(p, &path, is_dir));
                let itself = patterns.iter().any(|p| naive_matches(p, &path, is_dir));
                let what = format!("{patterns:?} against {path} (folder: {is_dir})");
                assert_eq!(cursor.excludes(&path, is_dir), excluded, "{what}");
                assert_eq!(compiled.excludes(&path, is_dir), excluded, "{what}");
                assert_eq!(compiled.matches(&path, is_dir), itself, "{what}");
            }
        }
    }

    /// `*` matching the slow, obvious way, on characters.
    fn naive_glob(g: &[char], n: &[char]) -> bool {
        match g.split_first() {
            None => n.is_empty(),
            Some(('*', rest)) => (0..=n.len()).any(|i| naive_glob(rest, &n[i..])),
            Some((c, rest)) => n.first() == Some(c) && naive_glob(rest, &n[1..]),
        }
    }

    #[test]
    fn globs_match_as_the_obvious_matcher_does() {
        let mut seed = 0x61_0B_u64;
        let mut below = |n: usize| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            (seed % n as u64) as usize
        };
        let alphabet = ['a', 'b', '*', 'é'];
        for _ in 0..50_000 {
            let glob: String = (0..1 + below(7))
                .map(|_| alphabet[below(alphabet.len())])
                .collect();
            let letters = ['a', 'b', 'é'];
            let name: String = (0..below(9)).map(|_| letters[below(3)]).collect();
            let chars = |s: &str| s.chars().collect::<Vec<_>>();
            assert_eq!(
                Glob::new(&glob).matches(&name),
                naive_glob(&chars(&glob), &chars(&name)),
                "{glob:?} against {name:?}"
            );
        }
    }

    #[test]
    fn star_heavy_patterns_against_long_names_stay_fast() {
        // A thousand unanchored patterns of 256 characters, `*` then a long run of `a` and
        // a tail, against files with 255-byte names of `a`: a matcher that backtracks to
        // its last star costs about glob × name per pattern and part (seconds per file).
        let patterns: Vec<String> = (0..1000)
            .map(|i| format!("*{}{i:03}", "a".repeat(252)))
            .collect();
        // And `a*a*…a*b042`: every star live at once, for a name without a `b`.
        let star_runs: Vec<String> = (0..1000)
            .map(|i| format!("{}b{i:03}", "a*".repeat(126)))
            .collect();
        for patterns in [patterns, star_runs] {
            let e = Excludes::new(&patterns);
            assert!(!e.is_empty());
            let started = std::time::Instant::now();
            let mut cursor = e.cursor();
            for n in 0..200 {
                let path = format!("config/{}{n:05}", "a".repeat(250));
                assert!(!cursor.excludes(&path, false));
            }
            let hit = format!("config/{}999", "a".repeat(252));
            let star_hit = format!("config/{}b999", "a".repeat(252));
            assert!(e.matches(&hit, false) || e.matches(&star_hit, false));
            assert!(
                started.elapsed() < std::time::Duration::from_secs(5),
                "{:?}",
                started.elapsed()
            );
        }
    }

    #[test]
    fn deep_paths_and_many_patterns_stay_fast() {
        // A thousand patterns (an image's most) against a chain of 2000 folders (the most a
        // 4096-byte path holds), checked the way a walk does: each folder, then a file in it.
        let patterns: Vec<String> = (0..1000)
            .map(|i| match i % 4 {
                0 => format!("dynmap{i}/web/tiles"),
                1 => format!("x*x*{i}"),
                2 => format!("**/a/**/b{i}/"),
                _ => format!("cache{i}"),
            })
            .collect();
        let e = Excludes::new(&patterns);
        let started = std::time::Instant::now();
        let mut cursor = e.cursor();
        let mut path = String::from("a");
        for _ in 0..2000 {
            assert!(!cursor.excludes(&path, true));
            assert!(!cursor.excludes(&format!("{path}/f"), false));
            path.push_str("/a");
        }
        // One check of the whole path, without a cursor, too.
        assert!(!e.excludes(&path, false));
        assert!(e.excludes(&format!("{path}/cache7/x"), false));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(10),
            "{:?}",
            started.elapsed()
        );
    }
}
