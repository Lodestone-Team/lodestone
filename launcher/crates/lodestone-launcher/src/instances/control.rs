//! What the control API reads of an instance's run: its state and progress, and its
//! console rows since a cursor. Nothing here changes the run.

use launcher_core::game::{download::ProgressSnapshot, install::Stage};

#[cfg(unix)]
use super::LogRow;
use super::{Instances, RunState};

/// An instance's run, as the control API reports it.
#[derive(Clone, Debug, PartialEq)]
pub struct RunStatus {
    /// `idle`, `preparing` or `running`.
    #[cfg_attr(not(unix), allow(dead_code))] // read by the control API only
    pub state: &'static str,
    /// While preparing: the stage and the download counts.
    pub progress: Option<(Stage, ProgressSnapshot)>,
    /// While running: the game's process id and how long it has run.
    pub pid: Option<u32>,
    #[cfg_attr(not(unix), allow(dead_code))] // read by the control API only
    pub running_secs: Option<u64>,
}

impl Instances {
    pub(crate) fn run_status(&self, id: &str) -> RunStatus {
        let idle = RunStatus {
            state: "idle",
            progress: None,
            pid: None,
            running_secs: None,
        };
        match self.runs.get(id).map(|run| &run.state) {
            None | Some(RunState::Idle) => idle,
            Some(RunState::Preparing { progress, .. }) => RunStatus {
                state: "preparing",
                progress: Some(progress.snapshot()),
                ..idle
            },
            Some(RunState::Running { process, started }) => RunStatus {
                state: "running",
                pid: Some(process.pid()),
                running_secs: Some(started.elapsed().as_secs()),
                ..idle
            },
        }
    }

    /// The console rows after `cursor` (as returned by the last call; 0 at first), and
    /// the new cursor. Rows dropped from the console's start are skipped.
    #[cfg(unix)]
    pub(crate) fn log_since(&self, id: &str, cursor: u64) -> (Vec<LogRow>, u64) {
        let Some(log) = self.runs.get(id).map(|run| &run.log) else {
            return (Vec::new(), cursor);
        };
        let new = log.pushed.saturating_sub(cursor) as usize;
        let skip = log.rows.len().saturating_sub(new);
        (log.rows.iter().skip(skip).cloned().collect(), log.pushed)
    }

    /// The last `count` console rows, and the cursor after them.
    #[cfg(unix)]
    pub(crate) fn log_tail(&self, id: &str, count: usize) -> (Vec<LogRow>, u64) {
        let Some(log) = self.runs.get(id).map(|run| &run.log) else {
            return (Vec::new(), 0);
        };
        let skip = log.rows.len().saturating_sub(count);
        (log.rows.iter().skip(skip).cloned().collect(), log.pushed)
    }

    /// Whether "Switch to Fabric" or another loader switch is running.
    #[cfg(unix)]
    pub(crate) fn loader_switch_running(&self) -> bool {
        self.loader_switch.is_some()
    }
}

#[cfg(test)]
mod tests {
    use launcher_core::game::log4j::{LogLine, Stream};

    use super::super::GameLog;

    #[test]
    fn the_cursor_counts_rows_not_events() {
        let mut log = GameLog::default();
        log.push(LogLine::plain(Stream::Stdout, "one\ntwo"));
        assert_eq!(log.pushed, 2);
        log.clear();
        log.push(LogLine::plain(Stream::Stdout, "three"));
        assert_eq!(log.pushed, 3, "clearing keeps the count");
    }
}
