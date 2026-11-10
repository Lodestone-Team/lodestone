//! Records and knocks (launcher/docs/friends.md, "Finding a friend's core").
//!
//! A core publishes, for each friend, a [`seal_record`]: its addresses, sealed with the
//! core secret it shares with that friend, under a lookup id that changes each period.
//! A friend knocks on that id with [`seal_knock`]: its own addresses, a random id and the
//! time. The core opens knocks with [`open_knock`] and lets each in once, while it's
//! fresh ([`ReplayGuard`]).

use std::{collections::HashMap, net::SocketAddr};

use serde::{Deserialize, Serialize};

use super::crypto::{self, lookup, period, secs_to_next_period, Role, Secret, ID_LEN};
use crate::{hex, unhex, Error, Result};

/// The most addresses in a record or a knock (the service allows 8 too).
pub const MAX_CANDIDATES: usize = 8;
/// How far a knock's time may be from the core's clock.
pub const KNOCK_SKEW_SECS: u64 = 10 * 60;
/// How long a knock's id is remembered, to refuse it again: longer than the whole
/// window a knock is let in (twice [`KNOCK_SKEW_SECS`]), so a replay can't outlast it.
pub const KNOCK_MEMORY_SECS: u64 = 2 * KNOCK_SKEW_SECS + 5 * 60;
/// The most knock ids remembered for one friend. A friend knocks only so often (the
/// service limits each address to 30 a minute), so this is plenty, and one friend
/// filling it only stops their own knocks.
pub const KNOCK_MEMORY_PER_FRIEND: usize = 256;
/// How old a record may be: the core renews its records at least daily.
pub const RECORD_MAX_AGE_SECS: u64 = 26 * 60 * 60;
/// Before a period ends, the core also publishes the next period's ids.
pub const PUBLISH_NEXT_WITHIN_SECS: u64 = 60 * 60;
/// A sealed record or knock's largest size (the service takes 1 KiB).
pub const MAX_SEALED: usize = 1024;

const VERSION: u32 = 1;

#[derive(Serialize, Deserialize)]
struct RecordWire {
    v: u32,
    issued: u64,
    candidates: Vec<SocketAddr>,
}

#[derive(Serialize, Deserialize)]
struct KnockWire {
    v: u32,
    id: String,
    issued: u64,
    candidates: Vec<SocketAddr>,
}

/// A friend's knock, opened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Knock {
    pub id: [u8; 16],
    pub issued: u64,
    pub candidates: Vec<SocketAddr>,
}

fn sealed_size(sealed: Vec<u8>) -> Result<Vec<u8>> {
    if sealed.len() > MAX_SEALED {
        return Err(Error::Other("Too many addresses to publish".into()));
    }
    Ok(sealed)
}

/// The core's record for one friend at `lookup`.
pub fn seal_record(
    core_secret: &Secret,
    lookup: &[u8; ID_LEN],
    candidates: &[SocketAddr],
    now: u64,
) -> Result<Vec<u8>> {
    let candidates: Vec<SocketAddr> = candidates.iter().take(MAX_CANDIDATES).copied().collect();
    let bytes = serde_json::to_vec(&RecordWire {
        v: VERSION,
        issued: now,
        candidates,
    })
    .expect("serializable");
    sealed_size(crypto::seal(core_secret, Role::Record, lookup, &bytes)?)
}

/// Opens a core's record: its addresses, if it's genuine and not too old.
pub fn open_record(
    core_secret: &Secret,
    lookup: &[u8; ID_LEN],
    sealed: &[u8],
    now: u64,
) -> Result<Vec<SocketAddr>> {
    let bad = || Error::Other("Your friend's core sent addresses that didn't check out.".into());
    let bytes = crypto::open(core_secret, Role::Record, lookup, sealed).ok_or_else(bad)?;
    let wire: RecordWire = serde_json::from_slice(&bytes).map_err(|_| bad())?;
    if wire.v != VERSION
        || wire.issued > now + KNOCK_SKEW_SECS
        || now.saturating_sub(wire.issued) > RECORD_MAX_AGE_SECS
        || wire.candidates.is_empty()
    {
        return Err(bad());
    }
    let mut candidates = wire.candidates;
    candidates.truncate(MAX_CANDIDATES);
    Ok(candidates)
}

/// A knock on `lookup` with this side's addresses. Returns the knock (its id gives the
/// probe tag) and the sealed form.
pub fn seal_knock(
    core_secret: &Secret,
    lookup: &[u8; ID_LEN],
    candidates: &[SocketAddr],
    now: u64,
) -> Result<(Knock, Vec<u8>)> {
    let knock = Knock {
        id: crate::random()?,
        issued: now,
        candidates: candidates.iter().take(MAX_CANDIDATES).copied().collect(),
    };
    let bytes = serde_json::to_vec(&KnockWire {
        v: VERSION,
        id: hex(&knock.id),
        issued: knock.issued,
        candidates: knock.candidates.clone(),
    })
    .expect("serializable");
    let sealed = sealed_size(crypto::seal(core_secret, Role::Knock, lookup, &bytes)?)?;
    Ok((knock, sealed))
}

/// Opens a knock on `lookup`: `None` if it isn't from the friend this core secret is
/// shared with. Check it with a [`ReplayGuard`] before acting on it.
pub fn open_knock(core_secret: &Secret, lookup: &[u8; ID_LEN], sealed: &[u8]) -> Option<Knock> {
    let bytes = crypto::open(core_secret, Role::Knock, lookup, sealed)?;
    let wire: KnockWire = serde_json::from_slice(&bytes).ok()?;
    if wire.v != VERSION {
        return None;
    }
    let mut candidates = wire.candidates;
    candidates.truncate(MAX_CANDIDATES);
    Some(Knock {
        id: unhex(&wire.id)?.try_into().ok()?,
        issued: wire.issued,
        candidates,
    })
}

/// Lets each knock in once, and only while it's fresh. Keep one per friend.
#[derive(Default, Debug)]
pub struct ReplayGuard {
    /// Knock ids and when they were seen.
    seen: HashMap<[u8; 16], u64>,
}

impl ReplayGuard {
    /// Whether `knock` is new and its time near `now`. Remembers it if so.
    pub fn admit(&mut self, knock: &Knock, now: u64) -> bool {
        self.seen
            .retain(|_, at| now.saturating_sub(*at) < KNOCK_MEMORY_SECS);
        if knock.issued.abs_diff(now) > KNOCK_SKEW_SECS || self.seen.contains_key(&knock.id) {
            return false;
        }
        if self.seen.len() >= KNOCK_MEMORY_PER_FRIEND {
            return false;
        }
        self.seen.insert(knock.id, now);
        true
    }
}

/// The lookup ids a core publishes a friend's record under at `now`: this period's,
/// and the next period's within [`PUBLISH_NEXT_WITHIN_SECS`] of it starting.
pub fn publish_lookups(core_secret: &Secret, now: u64) -> Vec<[u8; ID_LEN]> {
    let current = period(core_secret, now);
    let mut ids = vec![lookup(core_secret, current)];
    if secs_to_next_period(core_secret, now) <= PUBLISH_NEXT_WITHIN_SECS {
        ids.push(lookup(core_secret, current + 1));
    }
    ids
}

/// The lookup ids a friend tries at `now`, in order: this period's, then the previous
/// and next periods' (in case the two clocks differ).
pub fn knock_lookups(core_secret: &Secret, now: u64) -> [[u8; ID_LEN]; 3] {
    let current = period(core_secret, now);
    [
        lookup(core_secret, current),
        lookup(core_secret, current.saturating_sub(1)),
        lookup(core_secret, current + 1),
    ]
}

/// When a core should next renew its records: at the next period change, or an hour
/// before it (to add the next period's ids), whichever comes first. In seconds from now.
pub fn secs_to_republish(core_secret: &Secret, now: u64) -> u64 {
    let next = secs_to_next_period(core_secret, now);
    if next > PUBLISH_NEXT_WITHIN_SECS {
        next - PUBLISH_NEXT_WITHIN_SECS
    } else {
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    fn addrs() -> Vec<SocketAddr> {
        vec!["203.0.113.5:40000".parse().unwrap(), "[2001:db8::1]:40000".parse().unwrap()]
    }

    #[test]
    fn records_round_trip_and_age() {
        let secret = Secret::random().unwrap();
        let id = lookup(&secret, 1);
        let sealed = seal_record(&secret, &id, &addrs(), NOW).unwrap();
        assert_eq!(open_record(&secret, &id, &sealed, NOW + 5).unwrap(), addrs());
        assert!(open_record(&secret, &id, &sealed, NOW + RECORD_MAX_AGE_SECS + 1).is_err());
        assert!(open_record(&secret, &lookup(&secret, 2), &sealed, NOW).is_err());
        // a knock handed back as a record doesn't open
        let (_, knock) = seal_knock(&secret, &id, &addrs(), NOW).unwrap();
        assert!(open_record(&secret, &id, &knock, NOW).is_err());
    }

    #[test]
    fn knocks_are_let_in_once_while_fresh() {
        let secret = Secret::random().unwrap();
        let id = lookup(&secret, 1);
        let (knock, sealed) = seal_knock(&secret, &id, &addrs(), NOW).unwrap();
        let opened = open_knock(&secret, &id, &sealed).unwrap();
        assert_eq!(opened, knock);
        let mut guard = ReplayGuard::default();
        assert!(guard.admit(&opened, NOW + 3));
        assert!(!guard.admit(&opened, NOW + 4), "a replay");
        let (late, _) = seal_knock(&secret, &id, &addrs(), NOW).unwrap();
        assert!(!guard.admit(&late, NOW + KNOCK_SKEW_SECS + 1), "too old");
        assert!(open_knock(&Secret::random().unwrap(), &id, &sealed).is_none());
        // forgotten after a while, but by then it's too old anyway
        assert!(!guard.admit(&opened, NOW + KNOCK_MEMORY_SECS + 10));
        // the latest moment the knock's time is still accepted: still remembered
        let mut guard = ReplayGuard::default();
        assert!(guard.admit(&opened, NOW - KNOCK_SKEW_SECS));
        assert!(!guard.admit(&opened, NOW + KNOCK_SKEW_SECS), "a replay at the edge");
    }

    #[test]
    fn friends_find_what_cores_publish() {
        let secret = Secret::random().unwrap();
        for step in 0..48u64 {
            let now = NOW + step * 1800;
            let published = publish_lookups(&secret, now);
            let tried = knock_lookups(&secret, now);
            assert_eq!(published[0], tried[0]);
            // a friend whose clock is 20 minutes ahead still finds the core
            let ahead = knock_lookups(&secret, now + 1200);
            assert!(ahead.iter().any(|id| published.contains(id)));
            let behind = knock_lookups(&secret, now.saturating_sub(1200));
            assert!(behind.iter().any(|id| published.contains(id)));
        }
        assert!(secs_to_republish(&secret, NOW) >= 1);
    }
}
