//! Whether the core is shutting down. Once it is, no server starts: not a crashed one
//! restarting, not a restart the API asked for finishing in the background, nor anything
//! else. The shutdown stops the servers it finds running; one that started after it had
//! looked would be left running when the core exits.

use std::sync::atomic::{AtomicBool, Ordering};

use color_eyre::eyre::eyre;

use crate::error::{Error, ErrorKind};

/// One per process: the core runs once in its process (as its own program, or inside
/// Lodestone Host's Tauri app, which restarts the whole app to run it again).
static SHUTTING_DOWN: AtomicBool = AtomicBool::new(false);

/// Marks the core as shutting down, for good: from now on servers refuse to start.
pub(crate) fn begin() {
    SHUTTING_DOWN.store(true, Ordering::SeqCst);
}

/// Whether the core is shutting down.
pub(crate) fn has_begun() -> bool {
    SHUTTING_DOWN.load(Ordering::SeqCst)
}

/// An error once the core is shutting down, for a server about to start.
pub(crate) fn refuse_start() -> Result<(), Error> {
    refuse_start_if(&SHUTTING_DOWN)
}

fn refuse_start_if(shutting_down: &AtomicBool) -> Result<(), Error> {
    if shutting_down.load(Ordering::SeqCst) {
        Err(Error {
            kind: ErrorKind::Conflict,
            source: eyre!("Lodestone Core is shutting down, so the server cannot start"),
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The process-wide flag is never set in tests: other tests would then fail to start
    // servers. A flag of the test's own stands in for it.
    #[test]
    fn servers_refuse_to_start_once_the_shutdown_began() {
        let shutting_down = AtomicBool::new(false);
        assert!(refuse_start_if(&shutting_down).is_ok());
        shutting_down.store(true, Ordering::SeqCst);
        let error = refuse_start_if(&shutting_down).unwrap_err();
        assert!(matches!(error.kind, ErrorKind::Conflict));
        assert!(error.source.to_string().contains("shutting down"));
        assert!(!has_begun(), "tests never begin the process's shutdown");
    }
}
