//! The background task that takes scheduled backups.
//!
//! It looks at every server a minute after startup, then every 30 seconds: it notes which
//! servers have players on, and starts a scheduled backup for each server whose policy says
//! one is due ([`super::retention::due`]). Each backup runs on its own task, so a long one
//! doesn't hold up the others. It ends when the service is cancelled at shutdown.
//!
//! While no server has scheduled backups on (they're off by default), it stops looking
//! until a schedule is saved, so an idle core isn't woken for nothing.
//!
//! A server that stopped (or crashed) in the last minute isn't backed up while stopped: a
//! backup of a stopped server holds off starts, and the restart after a crash comes 5
//! seconds later. The instance records when it stopped, so a server that started and
//! crashed between two looks waits too. (The restart also waits a while for a backup that
//! began anyway.)

use std::collections::HashMap;
use std::time::Duration;

use chrono::Utc;
use tracing::warn;

use crate::prelude::GameInstance;
use crate::traits::t_player::TPlayerManagement;
use crate::traits::t_server::{State, TServer};
use crate::types::InstanceUuid;
use crate::AppState;

/// The first look, after startup has settled.
const FIRST_TICK: Duration = Duration::from_secs(60);
/// How often the servers are looked at after that.
const TICK: Duration = Duration::from_secs(30);
/// How long after stopping a server isn't backed up while stopped.
const AFTER_STOP: Duration = Duration::from_secs(60);

/// Runs until the backup service is cancelled.
pub async fn run(state: AppState) {
    let cancel = state.backups.cancel_token();
    tokio::select! {
        _ = cancel.cancelled() => return,
        _ = tokio::time::sleep(FIRST_TICK) => {}
    }
    let mut interval = tokio::time::interval(TICK);
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut seen = HashMap::new();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return,
            _ = interval.tick() => {}
        }
        let any_scheduled;
        (seen, any_scheduled) = tick(&state, &seen).await;
        if !any_scheduled {
            tokio::select! {
                _ = cancel.cancelled() => return,
                _ = state.backups.schedule_changed() => {}
            }
            // the states seen before the wait are stale; `stopped_ago` still covers a crash
            seen.clear();
            interval.reset();
        }
    }
}

/// Whether a stopped server may have just crashed and be about to restart, so isn't backed
/// up yet: it wasn't stopped at the last look (`previous`), or it became stopped less than
/// [`AFTER_STOP`] ago (`stopped_ago`, from the instance).
fn just_stopped(previous: Option<State>, now: State, stopped_ago: Option<Duration>) -> bool {
    let stopped = |state: State| matches!(state, State::Stopped | State::Error);
    stopped(now)
        && (previous.is_some_and(|previous| !stopped(previous))
            || stopped_ago.is_some_and(|ago| ago < AFTER_STOP))
}

/// One look at every server. `seen` is each server's state at the last look; returns the
/// states now, and whether any server may have scheduled backups on (a server skipped
/// before its policy was read counts as one).
pub(crate) async fn tick(
    state: &AppState,
    seen: &HashMap<InstanceUuid, State>,
) -> (HashMap<InstanceUuid, State>, bool) {
    let mut now_seen = HashMap::new();
    let mut any_scheduled = false;
    let service = state.backups.clone();
    let now = Utc::now().timestamp();
    // clones, so no map guard is held across an await
    let instances: Vec<GameInstance> = state
        .instances
        .iter()
        .map(|entry| entry.value().clone())
        .collect();
    for instance in instances {
        let GameInstance::MinecraftInstance(instance) = instance;
        let uuid = crate::traits::t_configurable::TConfigurable::uuid(&instance).await;
        let server_state = instance.state().await;
        now_seen.insert(uuid.clone(), server_state);
        if server_state == State::Running && instance.get_player_count().await.unwrap_or(0) > 0 {
            service.mark_active(&uuid);
        }
        if matches!(server_state, State::Starting | State::Stopping)
            || just_stopped(
                seen.get(&uuid).copied(),
                server_state,
                instance.stopped_ago(),
            )
            || service.job_running(&uuid)
        {
            any_scheduled = true;
            continue;
        }
        let policy = match service.policy(&uuid).await {
            Ok(policy) => policy,
            Err(e) => {
                warn!("Failed to read the backup schedule of {uuid}: {}", e.source);
                any_scheduled = true;
                continue;
            }
        };
        any_scheduled |= policy.enabled;
        if !service.is_due(&uuid, &policy, now).await {
            continue;
        }
        // set before it runs, so the next tick doesn't start it again
        service.set_last_attempt(&uuid, now, false);
        let (state, service) = (state.clone(), service.clone());
        state.backups.clone().spawn(async move {
            let result = service.scheduled_backup(&state, instance).await;
            if let Err(e) = &result {
                warn!("Scheduled backup of {uuid} failed: {}", e.source);
            }
            service.set_last_attempt(&uuid, now, result.is_err());
        });
    }
    (now_seen, any_scheduled)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_server_that_just_stopped_waits_a_look() {
        let never = None;
        // it may have crashed, and restart in 5 seconds
        assert!(just_stopped(Some(State::Running), State::Stopped, never));
        assert!(just_stopped(Some(State::Stopping), State::Stopped, never));
        assert!(just_stopped(Some(State::Starting), State::Error, never));
        // stopped at the last look too, or not seen before (the first look, a new server)
        assert!(!just_stopped(Some(State::Stopped), State::Stopped, never));
        assert!(!just_stopped(Some(State::Error), State::Stopped, never));
        assert!(!just_stopped(None, State::Stopped, never));
        // running servers are backed up live
        assert!(!just_stopped(Some(State::Stopped), State::Running, never));
        assert!(!just_stopped(Some(State::Running), State::Running, never));
    }

    /// Stopped at both looks, but it started and crashed in between: it waits a minute
    /// from the crash, which the instance recorded.
    #[test]
    fn a_server_that_crashed_between_two_looks_waits_a_minute() {
        let ago = |secs| Some(Duration::from_secs(secs));
        assert!(just_stopped(Some(State::Stopped), State::Stopped, ago(10)));
        assert!(just_stopped(None, State::Error, ago(59)));
        assert!(!just_stopped(Some(State::Stopped), State::Stopped, ago(60)));
        assert!(!just_stopped(
            Some(State::Stopped),
            State::Stopped,
            ago(600)
        ));
        // a running server is backed up live, whenever it last stopped
        assert!(!just_stopped(Some(State::Stopped), State::Running, ago(10)));
    }
}
