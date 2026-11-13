//! Which backups to delete, and when the next scheduled backup is due. Pure functions over
//! timestamps, so they can be tested with fixed clocks.

use std::collections::HashSet;

use chrono::{DateTime, Datelike};

use super::types::{BackupKind, BackupPolicy};

/// BeforeRestore backups younger than this are always kept.
pub const BEFORE_RESTORE_KEEP_SECS: i64 = 30 * 24 * 3600;
/// Of the older BeforeRestore backups, the newest this many are kept.
pub const BEFORE_RESTORE_KEEP_OLD: usize = 5;
/// A newest backup more than this far in the future means the clock was wrong when it was
/// taken; it no longer delays the schedule.
pub const FUTURE_SLACK_SECS: i64 = 5 * 60;
/// After a failed scheduled backup, try again after at most this long.
pub const RETRY_AFTER_FAILURE_SECS: i64 = 30 * 60;

/// What retention needs to know about a backup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionItem<'a> {
    pub id: &'a str,
    pub kind: BackupKind,
    /// Unix seconds.
    pub created: i64,
    pub pinned: bool,
}

/// The ids of the backups retention deletes, from all of a server's backups.
///
/// - Manual and pinned backups are never deleted.
/// - Scheduled: keep the newest `keep_last`; then, in UTC hour, day and ISO week buckets,
///   the newest backup of each of the most recent `keep_hourly` / `keep_daily` /
///   `keep_weekly` buckets that have one. Backups dated after `now` (the clock was wrong)
///   are kept and not counted.
/// - BeforeRestore: keep those younger than 30 days and the newest 5 older ones, and the
///   newest one when it is newer than every other backup (it is then the only copy of the
///   server's latest state).
/// - `just_created`, the backup this run made, is never deleted.
pub fn to_delete(
    items: &[RetentionItem],
    now: i64,
    policy: &BackupPolicy,
    just_created: Option<&str>,
) -> Vec<String> {
    let mut keep: HashSet<&str> = HashSet::new();
    if let Some(id) = just_created {
        keep.insert(id);
    }

    // newest first; ties by id, which starts with the time
    let newest_first = |list: &mut Vec<&RetentionItem>| {
        list.sort_by(|a, b| b.created.cmp(&a.created).then_with(|| b.id.cmp(a.id)));
    };

    let mut scheduled: Vec<&RetentionItem> = items
        .iter()
        .filter(|i| i.kind == BackupKind::Scheduled && !i.pinned)
        .collect();
    newest_first(&mut scheduled);
    let (future, past): (Vec<&RetentionItem>, Vec<&RetentionItem>) =
        scheduled.iter().copied().partition(|i| i.created > now);
    keep.extend(future.iter().map(|i| i.id));
    keep.extend(past.iter().take(policy.keep_last as usize).map(|i| i.id));
    keep_bucketed(&past, policy.keep_hourly, &mut keep, |t| t.div_euclid(3600));
    keep_bucketed(&past, policy.keep_daily, &mut keep, |t| t.div_euclid(86400));
    keep_bucketed(
        &past,
        policy.keep_weekly,
        &mut keep,
        |t| match DateTime::from_timestamp(t, 0) {
            Some(date) => {
                let week = date.iso_week();
                week.year() as i64 * 100 + week.week() as i64
            }
            None => t.div_euclid(7 * 86400),
        },
    );

    let mut before_restore: Vec<&RetentionItem> = items
        .iter()
        .filter(|i| i.kind == BackupKind::BeforeRestore && !i.pinned)
        .collect();
    newest_first(&mut before_restore);
    let (young, old): (Vec<&RetentionItem>, Vec<&RetentionItem>) = before_restore
        .iter()
        .copied()
        .partition(|i| now - i.created < BEFORE_RESTORE_KEEP_SECS);
    keep.extend(young.iter().map(|i| i.id));
    keep.extend(old.iter().take(BEFORE_RESTORE_KEEP_OLD).map(|i| i.id));
    if let Some(newest) = before_restore.first() {
        let newer_than_all_others = items
            .iter()
            .filter(|i| i.kind != BackupKind::BeforeRestore)
            .all(|i| i.created < newest.created);
        if newer_than_all_others {
            keep.insert(newest.id);
        }
    }

    scheduled
        .iter()
        .chain(before_restore.iter())
        .filter(|i| !keep.contains(i.id))
        .map(|i| i.id.to_string())
        .collect()
}

/// Keeps the newest backup of each of the `count` most recent buckets that have one.
/// `items` is newest first.
fn keep_bucketed<'a>(
    items: &[&RetentionItem<'a>],
    count: u32,
    keep: &mut HashSet<&'a str>,
    bucket: impl Fn(i64) -> i64,
) {
    let mut last_bucket = None;
    let mut buckets = 0;
    for item in items {
        if buckets >= count {
            break;
        }
        let b = bucket(item.created);
        if last_bucket != Some(b) {
            last_bucket = Some(b);
            buckets += 1;
            keep.insert(item.id);
        }
    }
}

/// The last scheduled attempt for a server, successful or not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LastAttempt {
    /// Unix seconds.
    pub at: i64,
    pub failed: bool,
}

/// When the next scheduled backup is due (unix seconds), or `None` when the schedule is off.
///
/// `newest` is the newest Scheduled or Manual backup's `created`. The next backup is due an
/// interval after that, and an interval after the last attempt (only `min(interval, 30 min)`
/// after a failed one). A newest backup more than 5 minutes in the future is ignored, so a
/// clock that was wrong doesn't stop the schedule.
pub fn next_due(
    policy: &BackupPolicy,
    newest: Option<i64>,
    last_attempt: Option<LastAttempt>,
    now: i64,
) -> Option<i64> {
    if !policy.enabled {
        return None;
    }
    let interval = policy.interval_minutes as i64 * 60;
    let from_newest = newest
        .filter(|created| *created <= now + FUTURE_SLACK_SECS)
        .map(|created| created + interval);
    let from_attempt = last_attempt.map(|attempt| {
        let wait = if attempt.failed {
            interval.min(RETRY_AFTER_FAILURE_SECS)
        } else {
            interval
        };
        attempt.at + wait
    });
    Some(match (from_newest, from_attempt) {
        (Some(a), Some(b)) => a.max(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => now,
    })
}

/// Whether a scheduled backup should run now. `active`: players were on since the last
/// backup (or the core started since).
pub fn due(
    policy: &BackupPolicy,
    newest: Option<i64>,
    last_attempt: Option<LastAttempt>,
    now: i64,
    active: bool,
) -> bool {
    if policy.only_when_active && !active {
        return false;
    }
    next_due(policy, newest, last_attempt, now).is_some_and(|next| now >= next)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3600;
    const DAY: i64 = 86400;
    /// Monday 2026-11-09 00:00:00 UTC.
    const MONDAY: i64 = 1_762_646_400 + 365 * DAY;

    fn policy(keep_last: u32, hourly: u32, daily: u32, weekly: u32) -> BackupPolicy {
        BackupPolicy {
            keep_last,
            keep_hourly: hourly,
            keep_daily: daily,
            keep_weekly: weekly,
            ..BackupPolicy::default()
        }
    }

    fn item(id: &str, kind: BackupKind, created: i64) -> RetentionItem<'_> {
        RetentionItem {
            id,
            kind,
            created,
            pinned: false,
        }
    }

    fn sorted(mut ids: Vec<String>) -> Vec<String> {
        ids.sort();
        ids
    }

    #[test]
    fn monday_constant_is_a_monday() {
        let date = DateTime::from_timestamp(MONDAY, 0).unwrap();
        assert_eq!(date.weekday(), chrono::Weekday::Mon);
        assert_eq!(date.timestamp() % DAY, 0);
    }

    #[test]
    fn keeps_last_n() {
        let ids: Vec<String> = (0..10).map(|i| format!("s{i}")).collect();
        let items: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| item(id, BackupKind::Scheduled, MONDAY + i as i64 * 60))
            .collect();
        let now = MONDAY + DAY;
        let deleted = to_delete(&items, now, &policy(3, 0, 0, 0), None);
        assert_eq!(
            sorted(deleted),
            vec!["s0", "s1", "s2", "s3", "s4", "s5", "s6"]
        );
    }

    #[test]
    fn keeps_newest_of_each_bucket() {
        // two backups an hour for 3 hours, keep_last 1, keep_hourly 2
        let items = vec![
            item("h0a", BackupKind::Scheduled, MONDAY + 10),
            item("h0b", BackupKind::Scheduled, MONDAY + 20),
            item("h1a", BackupKind::Scheduled, MONDAY + HOUR + 10),
            item("h1b", BackupKind::Scheduled, MONDAY + HOUR + 20),
            item("h2a", BackupKind::Scheduled, MONDAY + 2 * HOUR + 10),
            item("h2b", BackupKind::Scheduled, MONDAY + 2 * HOUR + 20),
        ];
        let now = MONDAY + 3 * HOUR;
        let deleted = to_delete(&items, now, &policy(1, 2, 0, 0), None);
        // kept: h2b (last and hour 2), h1b (hour 1)
        assert_eq!(sorted(deleted), vec!["h0a", "h0b", "h1a", "h2a"]);
    }

    #[test]
    fn daily_and_weekly_buckets() {
        // one backup a day for 15 days from a Monday, at noon
        let ids: Vec<String> = (0..15).map(|d| format!("d{d:02}")).collect();
        let items: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(d, id)| {
                item(
                    id,
                    BackupKind::Scheduled,
                    MONDAY + d as i64 * DAY + 12 * HOUR,
                )
            })
            .collect();
        let now = MONDAY + 15 * DAY;
        let deleted = to_delete(&items, now, &policy(1, 0, 3, 3), None);
        // daily keeps d14, d13, d12; weekly keeps the newest of week 3 (d14), week 2 (d13,
        // a Sunday) and week 1 (d06, a Sunday)
        let kept: Vec<&str> = ids
            .iter()
            .map(|s| s.as_str())
            .filter(|id| !deleted.iter().any(|d| d == id))
            .collect();
        assert_eq!(kept, vec!["d06", "d12", "d13", "d14"]);
    }

    #[test]
    fn manual_and_pinned_are_never_deleted() {
        let mut pinned = item("pinned", BackupKind::Scheduled, MONDAY);
        pinned.pinned = true;
        let items = vec![
            item("manual", BackupKind::Manual, MONDAY - 100 * DAY),
            pinned,
            item("old", BackupKind::Scheduled, MONDAY + 1),
            item("new", BackupKind::Scheduled, MONDAY + 2),
        ];
        let deleted = to_delete(&items, MONDAY + DAY, &policy(1, 0, 0, 0), None);
        assert_eq!(deleted, vec!["old"]);
    }

    #[test]
    fn never_deletes_the_new_one() {
        // the new backup's clock is behind the others (the clock went back)
        let items = vec![
            item("a", BackupKind::Scheduled, MONDAY + 100),
            item("b", BackupKind::Scheduled, MONDAY + 200),
            item("new", BackupKind::Scheduled, MONDAY + 50),
        ];
        let deleted = to_delete(&items, MONDAY + 300, &policy(1, 0, 0, 0), Some("new"));
        assert_eq!(deleted, vec!["a"]);
    }

    #[test]
    fn future_dated_are_kept_and_not_counted() {
        let items = vec![
            item("future", BackupKind::Scheduled, MONDAY + 10 * DAY),
            item("a", BackupKind::Scheduled, MONDAY - 2 * HOUR),
            item("b", BackupKind::Scheduled, MONDAY - HOUR),
        ];
        let deleted = to_delete(&items, MONDAY, &policy(1, 0, 0, 0), None);
        // keep_last 1 counts only b; the future one is kept too
        assert_eq!(deleted, vec!["a"]);
    }

    #[test]
    fn before_restore_rules() {
        let now = MONDAY + 100 * DAY;
        let ids: Vec<String> = (0..8).map(|i| format!("br{i}")).collect();
        // 8 old ones (older than 30 days), 2 young ones
        let mut items: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| item(id, BackupKind::BeforeRestore, MONDAY + i as i64 * DAY))
            .collect();
        items.push(item("young1", BackupKind::BeforeRestore, now - 10 * DAY));
        items.push(item("young2", BackupKind::BeforeRestore, now - 2 * DAY));
        items.push(item("manual", BackupKind::Manual, now - DAY));
        let deleted = to_delete(&items, now, &policy(6, 24, 7, 4), None);
        // the newest 5 old ones (br3..br7) and the young ones stay
        assert_eq!(sorted(deleted), vec!["br0", "br1", "br2"]);
    }

    #[test]
    fn newest_before_restore_newer_than_everything_is_kept() {
        let now = MONDAY + 400 * DAY;
        // six old BeforeRestore backups; the newest is newer than every other backup
        let ids: Vec<String> = (0..6).map(|i| format!("br{i}")).collect();
        let mut items: Vec<_> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| item(id, BackupKind::BeforeRestore, MONDAY + i as i64 * DAY))
            .collect();
        items.push(item("manual", BackupKind::Manual, MONDAY - DAY));
        let deleted = to_delete(&items, now, &policy(6, 24, 7, 4), None);
        assert_eq!(deleted, vec!["br0"]);
        // with only one old one, nothing goes
        let one = vec![item("br", BackupKind::BeforeRestore, MONDAY)];
        assert!(to_delete(&one, now, &policy(1, 0, 0, 0), None).is_empty());
    }

    #[test]
    fn due_table() {
        let mut on = BackupPolicy {
            enabled: true,
            interval_minutes: 60,
            only_when_active: true,
            ..BackupPolicy::default()
        };
        let now = MONDAY;
        let ok = |at| Some(LastAttempt { at, failed: false });
        let failed = |at| Some(LastAttempt { at, failed: true });
        // (newest, last attempt, active, due)
        let table: Vec<(Option<i64>, Option<LastAttempt>, bool, bool)> = vec![
            (None, None, true, true),
            (None, None, false, false),
            (Some(now - HOUR), None, true, true),
            (Some(now - HOUR + 1), None, true, false),
            // a skip ("nothing changed") counts as an attempt
            (Some(now - 5 * HOUR), ok(now - 10), true, false),
            (Some(now - 5 * HOUR), ok(now - HOUR), true, true),
            // after a failure, try again after min(interval, 30 min)
            (Some(now - 5 * HOUR), failed(now - 29 * 60), true, false),
            (Some(now - 5 * HOUR), failed(now - 30 * 60), true, true),
            // a newest backup slightly in the future still delays
            (Some(now + 4 * 60), None, true, false),
            // far in the future: ignored
            (Some(now + 10 * DAY), None, true, true),
            (Some(now + 10 * DAY), ok(now - 10), true, false),
        ];
        for (i, (newest, attempt, active, expected)) in table.into_iter().enumerate() {
            assert_eq!(
                due(&on, newest, attempt, now, active),
                expected,
                "row {i}: newest {newest:?}, attempt {attempt:?}, active {active}"
            );
        }
        // with a 15 min interval, a failure waits 15 min, not 30
        on.interval_minutes = 15;
        assert!(due(&on, None, failed(now - 15 * 60), now, true));
        // only_when_active off: inactive servers are due too
        on.only_when_active = false;
        assert!(due(&on, None, None, now, false));
        // disabled: never
        on.enabled = false;
        assert!(!due(&on, None, None, now, true));
        assert_eq!(next_due(&on, None, None, now), None);
    }
}
