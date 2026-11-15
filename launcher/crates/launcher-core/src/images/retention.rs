//! Which of an instance's image manifests to keep.
//!
//! The spec keeps the content lineage like a modpack history: the newest
//! [`KEEP_CONTENT`] content images, the current one and its parent, and every image a data
//! image links to. Pruning (after every record that wrote an image) keeps:
//!
//! - every data and full image (snapshots, from milestone 5: their own retention deletes
//!   them, never this), and the content image each links to (`content`);
//! - the images a caller holds with [`Images::keep_while`] (a restore in milestone 5 reads
//!   the image it goes back to, which the record before it could make the 31st);
//! - the instance's current content image and its parent (the image before a change holds
//!   the only copy of what the change took off, even when a clock that went back ranks it
//!   among older images);
//! - the newest [`KEEP_CONTENT`] content images
//!   ([`lodestone_image::lineage::newest_content_first`], the order the history lists).
//!
//! Every other content image is deleted (manifests only: objects go when the collector
//! finds nothing uses them). While any manifest can't be read, nothing is deleted: it may be
//! a data image linking to anything. Headers are read, not whole manifests.
//!
//! Snapshots (data and full images) have retention rules of their own, the core's backup
//! rules with the Launcher's kinds ([`data_to_delete`]): manual, imported and pinned
//! snapshots are never deleted; automatic ones (before a change, after playing) keep the
//! newest [`SnapshotPolicy::keep_last`] and the newest of each recent UTC day and ISO week;
//! the safety snapshots before a restore are kept 30 days, with the newest 5 older ones.

use std::collections::HashSet;
use std::sync::Arc;

use lodestone_image::Kind;
pub use lodestone_image::lineage::KEEP_CONTENT;
use lodestone_image::lineage::newest_content_first;
use lodestone_image::store::ManifestHeader;

use super::snapshots::{SnapshotKind, SnapshotRecord};
use super::{ImageError, Images};
use crate::instances::SnapshotSettings;

/// How many automatic snapshots an instance keeps ([`data_to_delete`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct SnapshotPolicy {
    /// The newest this many.
    pub keep_last: u32,
    /// And the newest of each of the most recent this many UTC days that have one.
    pub keep_daily: u32,
    /// And the newest of each of the most recent this many ISO weeks that have one.
    pub keep_weekly: u32,
}

/// The Launcher's defaults: the newest 5, one a day for a week, one a week for a month.
/// Automatic snapshots come before every content update, modpack update and loader
/// change, so a busy day makes several; this keeps the last few changes close and a
/// month's worth spread out, at most 16 at once.
pub const DEFAULT_POLICY: SnapshotPolicy = SnapshotPolicy {
    keep_last: 5,
    keep_daily: 7,
    keep_weekly: 4,
};

/// Safety snapshots before a restore younger than this are always kept.
pub const BEFORE_RESTORE_KEEP_SECS: i64 = 30 * 24 * 3600;
/// Of the older safety snapshots, the newest this many are kept.
pub const BEFORE_RESTORE_KEEP_OLD: usize = 5;

impl SnapshotPolicy {
    /// The policy of an instance with `settings`: each count it sets (at most
    /// [`SnapshotSettings::MAX_KEEP`]), else the default's.
    pub fn of(settings: Option<&SnapshotSettings>) -> SnapshotPolicy {
        let pick = |own: Option<u32>, default: u32| {
            own.map_or(default, |n| n.min(SnapshotSettings::MAX_KEEP))
        };
        let settings = settings.copied().unwrap_or_default();
        SnapshotPolicy {
            keep_last: pick(settings.keep_last, DEFAULT_POLICY.keep_last),
            keep_daily: pick(settings.keep_daily, DEFAULT_POLICY.keep_daily),
            keep_weekly: pick(settings.keep_weekly, DEFAULT_POLICY.keep_weekly),
        }
    }
}

/// The ids of the snapshot records retention deletes, from all of an instance's records
/// (newest first, as the records file lists them), as the core picks backups ("M2 decisions", Backups):
///
/// - Manual, imported and pinned snapshots are never deleted.
/// - Automatic ones (before a change, after playing): the newest `keep_last`; then, in UTC
///   day and ISO week buckets, the newest of each of the most recent `keep_daily` /
///   `keep_weekly` buckets that have one. Records dated after `now` (the clock was wrong)
///   are kept and not counted.
/// - Safety snapshots before a restore: those younger than 30 days, the newest 5 older
///   ones, and the newest one when it is newer than every other record (it is then the
///   only copy of the instance's latest state).
/// - `just_created`, the record this run made, is never deleted.
pub fn data_to_delete(
    records: &[SnapshotRecord],
    now: i64,
    policy: &SnapshotPolicy,
    just_created: Option<&str>,
) -> Vec<String> {
    use chrono::{DateTime, Datelike};
    let mut keep: HashSet<&str> = HashSet::new();
    keep.extend(just_created);
    // Newest first by time; within a second, in the order given (newest first as taken).
    let newest_first = |list: &mut Vec<&SnapshotRecord>| {
        list.sort_by(|a, b| b.created.cmp(&a.created));
    };
    let mut automatic: Vec<&SnapshotRecord> = records
        .iter()
        .filter(|r| r.kind.automatic() && !r.pinned)
        .collect();
    newest_first(&mut automatic);
    let (future, past): (Vec<&SnapshotRecord>, Vec<&SnapshotRecord>) =
        automatic.iter().copied().partition(|r| r.created > now);
    keep.extend(future.iter().map(|r| r.id.as_str()));
    keep.extend(
        past.iter()
            .take(policy.keep_last as usize)
            .map(|r| r.id.as_str()),
    );
    keep_bucketed(&past, policy.keep_daily, &mut keep, |t| t.div_euclid(86400));
    keep_bucketed(
        &past,
        policy.keep_weekly,
        &mut keep,
        |t| match DateTime::from_timestamp(t, 0) {
            Some(date) => {
                let week = date.iso_week();
                i64::from(week.year()) * 100 + i64::from(week.week())
            }
            None => t.div_euclid(7 * 86400),
        },
    );

    let mut before_restore: Vec<&SnapshotRecord> = records
        .iter()
        .filter(|r| r.kind == SnapshotKind::BeforeRestore && !r.pinned)
        .collect();
    newest_first(&mut before_restore);
    let (young, old): (Vec<&SnapshotRecord>, Vec<&SnapshotRecord>) = before_restore
        .iter()
        .copied()
        .partition(|r| now - r.created < BEFORE_RESTORE_KEEP_SECS);
    keep.extend(young.iter().map(|r| r.id.as_str()));
    keep.extend(
        old.iter()
            .take(BEFORE_RESTORE_KEEP_OLD)
            .map(|r| r.id.as_str()),
    );
    if let Some(newest) = before_restore.first() {
        let newer_than_all_others = records
            .iter()
            .filter(|r| r.kind != SnapshotKind::BeforeRestore)
            .all(|r| r.created < newest.created);
        if newer_than_all_others {
            keep.insert(newest.id.as_str());
        }
    }
    automatic
        .iter()
        .chain(before_restore.iter())
        .filter(|r| !keep.contains(r.id.as_str()))
        .map(|r| r.id.clone())
        .collect()
}

/// Keeps the newest record of each of the `count` most recent buckets that have one.
/// `records` is newest first.
fn keep_bucketed<'a>(
    records: &[&'a SnapshotRecord],
    count: u32,
    keep: &mut HashSet<&'a str>,
    bucket: impl Fn(i64) -> i64,
) {
    let mut last_bucket = None;
    let mut buckets = 0;
    for record in records {
        if buckets >= count {
            break;
        }
        let b = bucket(record.created);
        if last_bucket != Some(b) {
            last_bucket = Some(b);
            buckets += 1;
            keep.insert(record.id.as_str());
        }
    }
}

/// An instance's manifests as retention and the history see them.
#[derive(Debug, Default)]
pub(crate) struct Listing {
    /// The content images' headers, in no order.
    pub content: Vec<ManifestHeader>,
    /// Each data and full image's id, with the content image it links to.
    pub links: Vec<(String, Option<String>)>,
    /// Manifests whose header can't be read, by id, with why.
    pub unreadable: Vec<(String, String)>,
}

impl Images {
    /// Lists the instance's manifests by their headers.
    pub(crate) fn listing(&self, instance: &str) -> Result<Listing, ImageError> {
        let mut listing = Listing::default();
        for id in self.store().manifest_ids(instance)? {
            match self.store().load_header(instance, &id) {
                Ok(header) if header.kind == Kind::Content => listing.content.push(header),
                Ok(header) => listing.links.push((id, header.content)),
                Err(e) => listing.unreadable.push((id, e.to_string())),
            }
        }
        listing.unreadable.sort();
        Ok(listing)
    }

    /// Deletes the instance's content images nothing keeps (see the module), for a caller
    /// holding its record lock; `current` is its current image. Returns how many manifests
    /// went.
    pub(crate) fn prune_locked(
        &self,
        instance: &str,
        current: Option<&str>,
    ) -> Result<usize, ImageError> {
        let listing = self.listing(instance)?;
        if let Some((unreadable, why)) = listing.unreadable.first() {
            super::warn(
                "images.prune",
                instance,
                &format!("not pruning: the image {unreadable} can't be read ({why})"),
            );
            return Ok(0);
        }
        let mut keep: HashSet<String> = self
            .keep
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(instance)
            .map(|ids| ids.iter().cloned().collect())
            .unwrap_or_default();
        keep.extend(current.map(str::to_string));
        keep.extend(
            current
                .and_then(|current| listing.content.iter().find(|i| i.id == current))
                .and_then(|image| image.parent.clone()),
        );
        for (image, content) in &listing.links {
            keep.insert(image.clone());
            keep.extend(content.clone());
        }
        keep.extend(
            newest_content_first(&listing.content, current)
                .into_iter()
                .take(KEEP_CONTENT)
                .map(|image| image.id.clone()),
        );
        let mut deleted = 0;
        for image in &listing.content {
            if keep.contains(&image.id) {
                continue;
            }
            match self.store().delete_manifest(instance, &image.id) {
                Ok(true) => deleted += 1,
                Ok(false) => {}
                Err(e) => super::warn("images.prune", instance, &e.to_string()),
            }
        }
        Ok(deleted)
    }

    /// Keeps the image `id` of `instance` from pruning until the guard is dropped.
    pub fn keep_while(self: &Arc<Self>, instance: &str, id: &str) -> Keep {
        self.keep
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entry(instance.to_string())
            .or_default()
            .push(id.to_string());
        Keep {
            images: self.clone(),
            instance: instance.to_string(),
            id: id.to_string(),
        }
    }
}

/// Keeps one image from pruning while it lives ([`Images::keep_while`]).
pub struct Keep {
    images: Arc<Images>,
    instance: String,
    id: String,
}

impl Drop for Keep {
    fn drop(&mut self) {
        let mut keep = self.images.keep.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(ids) = keep.get_mut(&self.instance) {
            if let Some(i) = ids.iter().position(|id| *id == self.id) {
                ids.swap_remove(i);
            }
            if ids.is_empty() {
                keep.remove(&self.instance);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600;
    const DAY: i64 = 86400;

    fn record(id: &str, kind: SnapshotKind, created: i64) -> SnapshotRecord {
        SnapshotRecord {
            id: id.into(),
            image: "a".repeat(64),
            kind,
            name: id.into(),
            created,
            pinned: false,
            full: false,
            worlds: Vec::new(),
            content: None,
            files: 0,
            withheld: 0,
            bytes: 0,
            new_bytes: 0,
        }
    }

    fn ids(mut deleted: Vec<String>) -> Vec<String> {
        deleted.sort();
        deleted
    }

    #[test]
    fn manual_imported_and_pinned_are_never_deleted() {
        let now = 1_000 * DAY;
        let mut records: Vec<SnapshotRecord> = (0..50)
            .map(|i| record(&format!("m{i:02}"), SnapshotKind::Manual, now - i * DAY))
            .collect();
        records.push(record("imp", SnapshotKind::Imported, now - 400 * DAY));
        let mut pinned = record("pin", SnapshotKind::BeforeChange, now - 400 * DAY);
        pinned.pinned = true;
        records.push(pinned);
        let none = SnapshotPolicy {
            keep_last: 0,
            keep_daily: 0,
            keep_weekly: 0,
        };
        assert!(data_to_delete(&records, now, &none, None).is_empty());
    }

    #[test]
    fn automatic_ones_keep_the_last_and_one_per_day_and_week() {
        let now = 1_000 * DAY + 12 * HOUR;
        // Ten an hour apart today, then one a day for 40 days.
        let mut records: Vec<SnapshotRecord> = (0..10)
            .map(|i| record(&format!("h{i}"), SnapshotKind::BeforeChange, now - i * HOUR))
            .collect();
        records.extend(
            (1..=40).map(|d| record(&format!("d{d:02}"), SnapshotKind::AfterPlay, now - d * DAY)),
        );
        let deleted = data_to_delete(&records, now, &DEFAULT_POLICY, None);
        let kept: Vec<&str> = records
            .iter()
            .map(|r| r.id.as_str())
            .filter(|id| !deleted.iter().any(|d| d == id))
            .collect();
        // The newest 5, the newest of the last 7 days (today's is h0, already kept), and
        // the newest of the last 4 weeks.
        for id in ["h0", "h1", "h2", "h3", "h4", "d01", "d06"] {
            assert!(kept.contains(&id), "{id} kept: {kept:?}");
        }
        assert!(deleted.contains(&"h5".to_string()));
        assert!(deleted.contains(&"d40".to_string()));
        assert!(kept.len() <= 5 + 7 + 4, "{kept:?}");
        // Dated in the future (a wrong clock): kept and not counted.
        let mut with_future = records.clone();
        with_future.push(record("future", SnapshotKind::BeforeChange, now + DAY));
        let deleted = data_to_delete(&with_future, now, &DEFAULT_POLICY, None);
        assert!(!deleted.contains(&"future".to_string()));
        assert!(!deleted.contains(&"h4".to_string()));
        // What this run made stays, whatever the policy.
        let none = SnapshotPolicy {
            keep_last: 0,
            keep_daily: 0,
            keep_weekly: 0,
        };
        let deleted = data_to_delete(&records, now, &none, Some("h0"));
        assert!(!deleted.contains(&"h0".to_string()));
        assert_eq!(deleted.len(), records.len() - 1);
    }

    #[test]
    fn safety_snapshots_are_kept_thirty_days_and_five_more() {
        let now = 1_000 * DAY;
        let mut records: Vec<SnapshotRecord> = (0..20)
            .map(|i| {
                record(
                    &format!("r{i:02}"),
                    SnapshotKind::BeforeRestore,
                    now - (i * 5 + 1) * DAY,
                )
            })
            .collect();
        records.push(record("m", SnapshotKind::Manual, now));
        let deleted = ids(data_to_delete(&records, now, &DEFAULT_POLICY, None));
        // r00..r05 are younger than 30 days (1, 6, ... 26 days), r06..r10 the newest older
        // five; the rest go.
        let expected: Vec<String> = (11..20).map(|i| format!("r{i:02}")).collect();
        assert_eq!(deleted, expected);
        // The newest safety snapshot, newer than every other record, stays however old.
        let lone = vec![
            record("old", SnapshotKind::BeforeRestore, now - 400 * DAY),
            record("older", SnapshotKind::BeforeRestore, now - 500 * DAY),
            record("m", SnapshotKind::Manual, now - 600 * DAY),
        ];
        let deleted = data_to_delete(&lone, now, &DEFAULT_POLICY, None);
        assert!(!deleted.contains(&"old".to_string()));
    }

    #[test]
    fn an_instances_counts_win_and_are_bounded() {
        assert_eq!(SnapshotPolicy::of(None), DEFAULT_POLICY);
        let settings = SnapshotSettings {
            keep_last: Some(2),
            keep_weekly: Some(u32::MAX),
            ..SnapshotSettings::default()
        };
        assert_eq!(
            SnapshotPolicy::of(Some(&settings)),
            SnapshotPolicy {
                keep_last: 2,
                keep_daily: DEFAULT_POLICY.keep_daily,
                keep_weekly: SnapshotSettings::MAX_KEEP,
            }
        );
    }
}
