//! A backup policy's exclude patterns as an image's excludes.
//!
//! Policy patterns (`backups::pattern`) match a server's disk paths, ignoring letter case
//! on Windows and macOS; an image's excludes match canonical paths, case-sensitively
//! (`lodestone_image::store::excludes`). Both have the same syntax, so most patterns carry
//! over as they are: a server's non-world files have the same path in both. Worlds move:
//! `world/x` on the disk is `saves/world/x`, and a Paper server's `world_nether/DIM-1/x` is
//! `saves/world/DIM-1/x` (its other files `saves/world/.bukkit_nether/x`).
//!
//! The one rule: a translated pattern may leave out less than the policy did, never more.
//! Leaving out less makes an image a little larger; leaving out more would lose files the
//! owner wanted kept. So a pattern whose translation could reach other files (one that
//! could match `saves`, or a Paper level whose canonical folder also holds the Nether and
//! the End) is translated only in the shapes known to be safe, and otherwise reported as
//! untranslated (and left out of the image's excludes). Matching becomes case-sensitive,
//! which can only leave out less. The tests check the rule over every pattern shape.

use lodestone_image::store::excludes::{excludes_error, glob_matches, pattern_error};
use lodestone_image::ServerLayout;

/// A policy's patterns as an image's excludes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Translated {
    /// For the image: canonical, valid together, without repeats, in policy order.
    pub canonical: Vec<String>,
    /// Policy patterns (or parts of them) with no safe canonical form.
    pub untranslated: Vec<String>,
}

/// The Paper family's split folders, where a vanilla world keeps the Nether and the End.
const NETHER: &str = "DIM-1";
const END: &str = "DIM1";
const BUKKIT_NETHER: &str = ".bukkit_nether";
const BUKKIT_END: &str = ".bukkit_the_end";

/// Translates a policy's patterns for a server with `layout` (see the module).
pub fn canonical_excludes(policy: &[String], layout: &ServerLayout) -> Translated {
    canonical_excludes_where(policy, layout, |_| true)
}

/// [`canonical_excludes`], keeping only the canonical patterns `keep` accepts (a content
/// image's: those that can reach content). The others are left out before the patterns are
/// cut to an image's budgets, so they never push out one that is kept.
pub fn canonical_excludes_where(
    policy: &[String],
    layout: &ServerLayout,
    keep: impl Fn(&str) -> bool,
) -> Translated {
    let mut out = Translated::default();
    for pattern in policy {
        match translate(pattern, layout) {
            Some(patterns) if !patterns.is_empty() => {
                for canonical in patterns {
                    if pattern_error(&canonical).is_some() {
                        // Too long once `saves/<world>/` is added, say.
                        push_unique(&mut out.untranslated, pattern.clone());
                    } else if keep(&canonical) {
                        push_unique(&mut out.canonical, canonical);
                    }
                }
            }
            _ => push_unique(&mut out.untranslated, pattern.clone()),
        }
    }
    // Within an image's budgets: the last patterns go first.
    while excludes_error(&out.canonical).is_some() {
        match out.canonical.pop() {
            Some(dropped) => out.untranslated.push(dropped),
            None => break,
        }
    }
    out
}

fn push_unique(list: &mut Vec<String>, item: String) {
    if !list.contains(&item) {
        list.push(item);
    }
}

/// The canonical patterns for one policy pattern; `None` (or empty) when none is safe.
fn translate(pattern: &str, layout: &ServerLayout) -> Option<Vec<String>> {
    if pattern_error(pattern).is_some() {
        return None;
    }
    let suffix = if pattern.ends_with('/') { "/" } else { "" };
    let body = pattern.trim_start_matches('/').trim_end_matches('/');
    let anchored = pattern.starts_with('/') || body.contains('/');
    let parts: Vec<&str> = body.split('/').collect();
    let level = layout.level_name();
    let split = layout.is_split();
    // Names a canonical path has that no server disk path has in the same place: a match
    // there would leave out what the policy never did.
    let canonical_only = |glob: &str| {
        glob_matches(glob, "saves")
            || (split && (glob_matches(glob, BUKKIT_NETHER) || glob_matches(glob, BUKKIT_END)))
    };

    if !anchored {
        let glob = parts[0];
        if glob.chars().all(|c| c == '*') {
            // Everything, on both sides.
            return Some(vec![format!("{glob}{suffix}")]);
        }
        // The split level's canonical folder holds the Nether and the End too.
        if canonical_only(glob) || (split && glob_matches(glob, level)) {
            return None;
        }
        let mut out = vec![format!("{glob}{suffix}")];
        if split {
            // A name at any depth includes the split folders at the top.
            for (folder, dimension, bukkit) in split_folders(level) {
                if glob_matches(glob, &folder) {
                    out.push(format!("saves/{level}/{dimension}{suffix}"));
                    out.push(format!("saves/{level}/{bukkit}{suffix}"));
                }
            }
        }
        return Some(out);
    }

    let first = parts[0];
    let rest = &parts[1..];
    if first == "**" {
        // At any depth. Only a match that starts at `saves`, at a split level's folder or
        // at its `.bukkit_*` folders could reach what the disk pattern didn't.
        let Some(next) = rest.first() else {
            return Some(vec![format!("**{suffix}")]);
        };
        if canonical_only(next) || (split && glob_matches(next, level)) {
            return None;
        }
        return Some(vec![format!("{body}{suffix}")]);
    }

    let mut out = Vec::new();
    let joined = |head: &[&str], tail: &[&str]| -> String {
        let all: Vec<&str> = head.iter().chain(tail.iter()).copied().collect();
        format!("{}{suffix}", all.join("/"))
    };
    // Non-world files keep their paths. Only a single part needs its leading `/` to stay
    // anchored.
    let is_world_name = layout.worlds().any(|world| world == first)
        || (split && (first == nether_folder(level) || first == end_folder(level)));
    // As it is, the pattern also matches canonical `saves/<world>/…` when its first part
    // matches `saves`: safe only when its second part can't be a world there.
    let as_is_safe = !glob_matches(first, "saves")
        || rest.first().is_some_and(|next| {
            *next != "**" && !layout.worlds().any(|world| glob_matches(next, world))
        });
    if !is_world_name && as_is_safe {
        if rest.is_empty() {
            out.push(format!("/{first}{suffix}"));
        } else {
            out.push(joined(&parts, &[]));
        }
    }
    // The worlds `first` names.
    for world in layout.worlds() {
        if !glob_matches(first, world) {
            continue;
        }
        if split && world == level {
            // `saves/<level>/…` also holds the Nether and the End: only a first part that
            // is plainly none of their folders maps one to one.
            match rest.first() {
                Some(next) if is_plain(next) && !is_split_name(next) => {
                    out.push(joined(&["saves", world], rest));
                }
                _ => {}
            }
        } else {
            out.push(joined(&["saves", world], rest));
        }
    }
    if split {
        for (folder, dimension, bukkit) in split_folders(level) {
            if !glob_matches(first, &folder) {
                continue;
            }
            match rest.first() {
                None => {
                    out.push(joined(&["saves", level, dimension], &[]));
                    out.push(joined(&["saves", level, bukkit], &[]));
                }
                Some(&"**") => {
                    out.push(joined(&["saves", level, dimension], rest));
                    out.push(joined(&["saves", level, bukkit], rest));
                }
                Some(next) => {
                    if glob_matches(next, dimension) {
                        out.push(joined(&["saves", level, dimension], &rest[1..]));
                    }
                    // Files in the split folder other than its dimension: an exact
                    // dimension name never lands here.
                    if *next != dimension {
                        out.push(joined(&["saves", level, bukkit], rest));
                    }
                }
            }
        }
    }
    Some(out)
}

/// A split level's folders, each with the canonical folders its files go to.
fn split_folders(level: &str) -> [(String, &'static str, &'static str); 2] {
    [
        (nether_folder(level), NETHER, BUKKIT_NETHER),
        (end_folder(level), END, BUKKIT_END),
    ]
}

fn nether_folder(level: &str) -> String {
    format!("{level}_nether")
}

fn end_folder(level: &str) -> String {
    format!("{level}_the_end")
}

/// A part without wildcards.
fn is_plain(part: &str) -> bool {
    !part.contains('*')
}

/// Whether `part` is (in any letter case) one of the folders the split moves.
fn is_split_name(part: &str) -> bool {
    [NETHER, END, BUKKIT_NETHER, BUKKIT_END]
        .iter()
        .any(|name| part.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backups::pattern::reference::Matcher;
    use lodestone_image::store::excludes::Excludes;
    use lodestone_image::DiskLayout;

    fn s(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn paper() -> ServerLayout {
        ServerLayout::new("world", ["creative".to_string()], true).unwrap()
    }

    fn vanilla() -> ServerLayout {
        ServerLayout::new("world", ["creative".to_string()], false).unwrap()
    }

    #[test]
    fn vanilla_patterns() {
        let t = canonical_excludes(
            &s(&[
                "dynmap/web/tiles/",
                "/plugins/dynmap/web/tiles/",
                "*.bak",
                "world/region/",
                "/creative/",
                "/backups/",
                "*/data/",
                "saves/",
                "**/cache/",
                "**/saves/x",
            ]),
            &vanilla(),
        );
        assert_eq!(
            t.canonical,
            s(&[
                "dynmap/web/tiles/",
                "plugins/dynmap/web/tiles/",
                "*.bak",
                "saves/world/region/",
                "saves/creative/",
                "/backups/",
                "*/data/",
                "saves/creative/data/",
                "saves/world/data/",
                "**/cache/",
            ])
        );
        assert_eq!(t.untranslated, s(&["saves/", "**/saves/x"]));
    }

    #[test]
    fn paper_patterns() {
        let t = canonical_excludes(
            &s(&[
                "world/region/",
                "world/DIM-1/",
                "world/*/x",
                "/world",
                "world_nether/DIM-1/region/",
                "world_nether/",
                "/world_the_end",
                "world_nether/paper-world.yml",
                "world_the_end/*",
                "world_nether/**/poi/",
                "world",
                ".bukkit_nether",
                "DIM-1",
            ]),
            &paper(),
        );
        assert_eq!(
            t.canonical,
            s(&[
                "saves/world/region/",
                "saves/world/DIM-1/region/",
                "world_nether/",
                "saves/world/DIM-1/",
                "saves/world/.bukkit_nether/",
                "saves/world/DIM1",
                "saves/world/.bukkit_the_end",
                "saves/world/.bukkit_nether/paper-world.yml",
                "saves/world/.bukkit_the_end/*",
                "saves/world/DIM-1/**/poi/",
                "saves/world/.bukkit_nether/**/poi/",
                "DIM-1",
            ])
        );
        assert_eq!(
            t.untranslated,
            s(&[
                "world/DIM-1/",
                "world/*/x",
                "/world",
                "world",
                ".bukkit_nether"
            ])
        );
    }

    /// Disk paths of a server, files and folders, to check translations against.
    fn disk_paths() -> Vec<(String, bool)> {
        let mut paths = Vec::new();
        let tops = [
            "world",
            "world_nether",
            "world_the_end",
            "creative",
            "plugins",
            "saves",
            "dynmap",
            "World",
            "x",
        ];
        let middles = [
            "region", "DIM-1", "DIM1", "data", "poi", "cache", "x", "saves",
        ];
        let leaves = ["r.0.0.mca", "level.dat", "x", "a.bak", "paper-world.yml"];
        for top in tops {
            paths.push((top.to_string(), true));
            paths.push((top.to_string(), false));
            for middle in middles {
                paths.push((format!("{top}/{middle}"), true));
                for leaf in leaves {
                    paths.push((format!("{top}/{middle}/{leaf}"), false));
                    paths.push((format!("{top}/{middle}/{middle}/{leaf}"), false));
                }
            }
            for leaf in leaves {
                paths.push((format!("{top}/{leaf}"), false));
            }
        }
        paths
    }

    #[test]
    fn translations_never_leave_out_more_than_the_policy() {
        let patterns = s(&[
            "world",
            "/world",
            "world/",
            "/world/",
            "World",
            "world_nether",
            "/world_nether",
            "world_nether/",
            "/world_the_end/",
            "creative",
            "/creative/x",
            "saves",
            "/saves/",
            "sav*",
            "*",
            "*/",
            "/*",
            "**",
            "**/x",
            "**/region/",
            "**/saves/x",
            "**/world/x",
            "**/DIM-1/region",
            "*/region/",
            "w*/region/",
            "*d/DIM-1/",
            "*/DIM-1/",
            "*/level.dat",
            "world/*/x",
            "world/**/x",
            "world/region/",
            "world/DIM-1/",
            "world/DIM1/x",
            "world_nether/DIM-1/region/",
            "world_nether/*",
            "world_nether/**",
            "world_nether/**/x",
            "world_nether/x",
            "world_nether/D*",
            "world_the_end/DIM1/",
            "DIM-1",
            "DIM1/",
            ".bukkit_nether",
            "*.bak",
            "x",
            "x/",
            "/x",
            "dynmap/x",
            "plugins/*/x",
            "**/cache/",
            "*/*/x",
            "region",
            "*_nether/*",
        ]);
        for layout in [
            paper(),
            vanilla(),
            ServerLayout::new("w", [], true).unwrap(),
        ] {
            let disk = DiskLayout::Server(layout.clone());
            for pattern in &patterns {
                let policy = vec![pattern.clone()];
                let canonical = canonical_excludes(&policy, &layout).canonical;
                let image = Excludes::new(&canonical);
                let backup = Matcher::with_case(&policy, false);
                for (path, is_dir) in disk_paths() {
                    let probe = if is_dir {
                        format!("{path}/probe")
                    } else {
                        path.clone()
                    };
                    let Ok(Some(mapped)) = disk.from_disk(&probe) else {
                        continue;
                    };
                    let mapped = if is_dir {
                        match mapped.as_str().strip_suffix("/probe") {
                            Some(folder) => folder.to_string(),
                            None => continue,
                        }
                    } else {
                        mapped.to_string()
                    };
                    if image.excludes(&mapped, is_dir) {
                        assert!(
                            backup.is_excluded_path(&path, is_dir),
                            "{pattern:?} as {canonical:?} leaves out {mapped} ({path}) on {layout:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn budgets_and_bad_patterns() {
        let t = canonical_excludes(&s(&["!x", "a\\b", ""]), &vanilla());
        assert!(t.canonical.is_empty());
        assert_eq!(t.untranslated.len(), 3);
        // Too long once inside `saves/<world>/`.
        let long = format!("world/{}", "x".repeat(250));
        let t = canonical_excludes(std::slice::from_ref(&long), &vanilla());
        assert!(t.canonical.is_empty());
        assert_eq!(t.untranslated, vec![long]);
        // Repeats are dropped.
        let t = canonical_excludes(&s(&["*.bak", "*.bak"]), &vanilla());
        assert_eq!(t.canonical, s(&["*.bak"]));
    }
}
