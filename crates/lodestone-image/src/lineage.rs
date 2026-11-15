//! The content lineage: which content images come first, and how merged operations label
//! one image. The core and the Launcher record content images the same way (every record
//! takes the image current before it as its parent), so both keep and list them by these
//! rules.
//!
//! - [`newest_content_first`]: the order retention keeps and a history lists content images
//!   in. Retention keeps the first [`KEEP_CONTENT`] of them, so a history shows what is kept.
//! - [`join_labels`]: one image's label from the labels of the operations merged into it.

use std::collections::{HashMap, HashSet};

use crate::manifest::{Kind, MAX_TEXT_LEN};
use crate::store::ManifestHeader;

/// How many of the newest content images retention keeps beyond those something names
/// (the current image and its parent, and every image a data image links to).
pub const KEEP_CONTENT: usize = 30;

/// The content images among `images`, newest first: by when they were recorded, and those
/// recorded in the same second (a restore and the record before it are often a second
/// apart at most) by the lineage. Every record takes the image current before it as its
/// parent, so going back from the instance's `current` image, parent by parent, lists the
/// images newest first. Returning to earlier content saves an image again under the id it
/// had, with the image it came from as its new parent, so parents can go round in a circle:
/// the walk stops at an image it has seen. Images the walk doesn't reach (the parent they
/// were recorded after was saved again since) come after those it does, the one with more
/// ancestors first; then by id, so the order never depends on how `images` was listed.
pub fn newest_content_first<'a>(
    images: &'a [ManifestHeader],
    current: Option<&str>,
) -> Vec<&'a ManifestHeader> {
    let content: Vec<&ManifestHeader> = images.iter().filter(|i| i.kind == Kind::Content).collect();
    let by_id: HashMap<&str, &ManifestHeader> =
        content.iter().map(|i| (i.id.as_str(), *i)).collect();
    let parent_of = |image: &ManifestHeader| image.parent.as_deref().and_then(|p| by_id.get(p));
    // How far back from the current image each is.
    let mut back: HashMap<&str, usize> = HashMap::new();
    let mut at = current.and_then(|id| by_id.get(id)).copied();
    while let Some(image) = at {
        if back.contains_key(image.id.as_str()) {
            break;
        }
        back.insert(image.id.as_str(), back.len());
        at = parent_of(image).copied();
    }
    // How many ancestors one the walk doesn't reach has.
    let depth = |image: &ManifestHeader| {
        let mut seen = HashSet::new();
        let mut at = image;
        while seen.insert(at.id.as_str()) {
            match parent_of(at) {
                Some(parent) => at = parent,
                None => break,
            }
        }
        seen.len()
    };
    let mut ordered: Vec<(&ManifestHeader, usize, usize)> = content
        .into_iter()
        .map(|i| {
            let back = back.get(i.id.as_str()).copied().unwrap_or(usize::MAX);
            let depth = if back == usize::MAX { depth(i) } else { 0 };
            (i, back, depth)
        })
        .collect();
    ordered.sort_by(|(a, a_back, da), (b, b_back, db)| {
        b.created
            .cmp(&a.created)
            .then(a_back.cmp(b_back))
            .then(db.cmp(da))
            .then(a.id.cmp(&b.id))
    });
    ordered.into_iter().map(|(image, _, _)| image).collect()
}

/// An image's label from the labels of the operations merged into it: each one's, in
/// order, control characters made spaces and trimmed, without repeats in a row, joined
/// with "; " and cut to what an image may hold ([`MAX_TEXT_LEN`] characters, the last an
/// ellipsis when cut). A list of each operation's label rather than a count, so a history
/// says what happened. `None` when no label has any text.
pub fn join_labels(labels: &[String]) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();
    for label in labels {
        let clean: String = label
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let clean = clean.trim().to_string();
        if !clean.is_empty() && parts.last() != Some(&clean) {
            parts.push(clean);
        }
    }
    if parts.is_empty() {
        return None;
    }
    let joined = parts.join("; ");
    if joined.chars().count() <= MAX_TEXT_LEN {
        return Some(joined);
    }
    let mut cut: String = joined.chars().take(MAX_TEXT_LEN - 1).collect();
    cut.push('…');
    Some(cut)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(id: &str, parent: Option<&str>, created: i64) -> ManifestHeader {
        ManifestHeader {
            id: id.into(),
            kind: Kind::Content,
            parent: parent.map(str::to_string),
            created,
            label: None,
            content: None,
        }
    }

    fn ids(ordered: &[&ManifestHeader]) -> Vec<String> {
        ordered.iter().map(|h| h.id.clone()).collect()
    }

    #[test]
    fn newest_first_by_time_then_by_lineage_within_a_second() {
        // a <- b <- c, all in one second: the lineage decides, from the current image.
        let headers = vec![
            header("a", None, 10),
            header("c", Some("b"), 10),
            header("b", Some("a"), 10),
        ];
        assert_eq!(
            ids(&newest_content_first(&headers, Some("c"))),
            ["c", "b", "a"]
        );
        // Time wins over the lineage when the seconds differ (a clock that went back).
        let headers = vec![
            header("a", None, 30),
            header("b", Some("a"), 20),
            header("c", Some("b"), 25),
        ];
        assert_eq!(
            ids(&newest_content_first(&headers, Some("c"))),
            ["a", "c", "b"]
        );
    }

    #[test]
    fn circular_parents_end_the_walk() {
        // Going back to earlier content saves `a` again with `b` as its parent.
        let headers = vec![header("a", Some("b"), 5), header("b", Some("a"), 5)];
        assert_eq!(ids(&newest_content_first(&headers, Some("a"))), ["a", "b"]);
        assert_eq!(ids(&newest_content_first(&headers, Some("b"))), ["b", "a"]);
        // An image its own parent (never written, but a manifest from elsewhere may say so).
        let headers = vec![header("x", Some("x"), 1)];
        assert_eq!(ids(&newest_content_first(&headers, Some("x"))), ["x"]);
    }

    #[test]
    fn images_the_walk_misses_come_after_with_more_ancestors_first() {
        // current: c <- a. `d <- e` is a branch the walk never reaches; e has more
        // ancestors, so it comes first among them. All in one second.
        let headers = vec![
            header("a", None, 7),
            header("c", Some("a"), 7),
            header("d", None, 7),
            header("e", Some("d"), 7),
        ];
        assert_eq!(
            ids(&newest_content_first(&headers, Some("c"))),
            ["c", "a", "e", "d"]
        );
        // No current image: ancestors, then ids.
        assert_eq!(
            ids(&newest_content_first(&headers, None)),
            ["c", "e", "a", "d"]
        );
    }

    #[test]
    fn only_content_images_and_any_listing_order() {
        let mut data = header("z", None, 99);
        data.kind = Kind::Data;
        data.content = Some("a".into());
        let headers = vec![header("b", Some("a"), 1), data, header("a", None, 1)];
        let mut reversed = headers.clone();
        reversed.reverse();
        assert_eq!(ids(&newest_content_first(&headers, Some("b"))), ["b", "a"]);
        assert_eq!(ids(&newest_content_first(&reversed, Some("b"))), ["b", "a"]);
    }

    #[test]
    fn labels_join_and_fit() {
        assert_eq!(join_labels(&[]), None);
        assert_eq!(join_labels(&[" ".into(), "\n".into()]), None);
        assert_eq!(
            join_labels(&["Added JEI".into(), "Added JEI".into(), "Removed X\n".into()]),
            Some("Added JEI; Removed X".into())
        );
        // Repeats only in a row are dropped.
        assert_eq!(
            join_labels(&["a".into(), "b".into(), "a".into()]),
            Some("a; b; a".into())
        );
        // Control characters can't break a label over lines.
        assert_eq!(
            join_labels(&["Added\nevil\u{7}mod".into()]),
            Some("Added evil mod".into())
        );
        let long: Vec<String> = (0..500).map(|n| format!("Updated mod {n}")).collect();
        let joined = join_labels(&long).unwrap();
        assert_eq!(joined.chars().count(), MAX_TEXT_LEN);
        assert!(joined.ends_with('…'));
        // Cut by characters, never inside one.
        let wide = vec!["é".repeat(MAX_TEXT_LEN + 5)];
        assert_eq!(
            join_labels(&wide).unwrap().chars().count(),
            MAX_TEXT_LEN
        );
    }
}
