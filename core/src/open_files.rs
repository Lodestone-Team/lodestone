//! How many files the core may have open at once.
//!
//! macOS starts a program from launchd or Terminal with a soft limit of 256 open files, and
//! the core, as Lodestone Host's child, inherits it. That is easily reached: every player
//! download holds a socket and a file or two of the image store, a record holds the files
//! it walks, and the servers' consoles hold pipes. So the core raises its soft limit at
//! startup, which needs no privilege, to [`WANTED`] or the hard limit if that is lower. The
//! servers it starts inherit the raised limit, which is why it is capped: in containers and
//! under systemd the hard limit is often a million or more, and some programs a server may
//! start loop over every possible descriptor (to close them before running a child) or use
//! `select`, and slow down or break with a limit that large.

/// The soft limit the core asks for: ample for its 64 downloads and its records, and what
/// macOS allows at most (it refuses a soft limit above `OPEN_MAX`, 10240, even under an
/// unlimited hard one).
#[cfg(unix)]
const WANTED: libc::rlim_t = 10240;

/// Raises the soft limit on open files to [`WANTED`], within the hard limit; never lowers
/// it. A failure only leaves the limit as it was.
#[cfg(unix)]
pub(crate) fn raise_limit() {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit only writes the struct it is given, which lives for the call.
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) } != 0 {
        return;
    }
    let wanted = limit.rlim_max.min(WANTED);
    if wanted <= limit.rlim_cur {
        return;
    }
    let raised = libc::rlimit {
        rlim_cur: wanted,
        rlim_max: limit.rlim_max,
    };
    // SAFETY: setrlimit only reads the struct it is given, which lives for the call.
    if unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raised) } != 0 {
        tracing::debug!(
            "Could not raise the open file limit from {} to {wanted}: {}",
            limit.rlim_cur,
            std::io::Error::last_os_error()
        );
    }
}

/// Windows has no such per-process limit to raise.
#[cfg(not(unix))]
pub(crate) fn raise_limit() {}

#[cfg(all(test, unix))]
mod tests {
    fn limit() -> libc::rlimit {
        let mut limit = libc::rlimit {
            rlim_cur: 0,
            rlim_max: 0,
        };
        // SAFETY: as in `raise_limit`.
        assert_eq!(
            unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut limit) },
            0
        );
        limit
    }

    #[test]
    fn the_soft_limit_only_goes_up_within_the_hard_one() {
        let before = limit();
        super::raise_limit();
        let after = limit();
        assert!(after.rlim_cur >= before.rlim_cur);
        assert!(after.rlim_cur <= after.rlim_max);
        assert!(after.rlim_cur <= before.rlim_cur.max(super::WANTED));
        assert_eq!(after.rlim_max, before.rlim_max);
        super::raise_limit();
        assert_eq!(limit().rlim_cur, after.rlim_cur);
    }
}
