//! How many files the core may have open at once.
//!
//! macOS starts a program from launchd or Terminal with a soft limit of 256 open files, and
//! the core, as Lodestone Host's child, inherits it. That is easily reached: every player
//! download holds a socket and a file or two of the image store, a record holds the files
//! it walks, and the servers' consoles hold pipes. So the core raises its soft limit to its
//! hard limit at startup, which needs no privilege. The servers it starts inherit the
//! raised limit.

/// Raises the soft limit on open files as far as the hard limit allows. A failure only
/// leaves the limit as it was.
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
    #[allow(unused_mut)]
    let mut wanted = limit.rlim_max;
    // macOS refuses a soft limit above OPEN_MAX (10240), even under an unlimited hard one.
    #[cfg(target_vendor = "apple")]
    {
        wanted = wanted.min(10240);
    }
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
        assert_eq!(after.rlim_max, before.rlim_max);
        super::raise_limit();
        assert_eq!(limit().rlim_cur, after.rlim_cur);
    }
}
