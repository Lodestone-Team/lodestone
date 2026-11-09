//! A process's memory, CPU time, threads and open files, for the launcher's
//! performance monitor. Each value is `None` where the system doesn't say.
//!
//! Memory is the number Activity Monitor (macOS) or Task Manager (Windows) shows: the
//! physical footprint on macOS, the working set on Windows, the resident size
//! elsewhere. On Windows "open files" is the process's handle count, which Task
//! Manager calls Handles: files, but also events, threads, registry keys and so on.

use std::time::Duration;

/// One reading of a process.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ProcessStats {
    /// Bytes.
    pub memory: Option<u64>,
    /// User and system time used since it started.
    pub cpu_time: Option<Duration>,
    pub threads: Option<u32>,
    /// Open file descriptors; on Windows, open handles of every kind.
    pub open_files: Option<u32>,
}

/// Reads process `pid`, or `None` when it is gone or not this user's.
pub fn sample(pid: u32) -> Option<ProcessStats> {
    imp::sample(pid)
}

/// The share of one core `pid` used between two readings `wall` apart, in percent.
/// Over 100 when it ran on several cores.
pub fn cpu_percent(before: &ProcessStats, after: &ProcessStats, wall: Duration) -> Option<f64> {
    let used = after.cpu_time?.checked_sub(before.cpu_time?)?;
    if wall.is_zero() {
        return None;
    }
    Some(used.as_secs_f64() / wall.as_secs_f64() * 100.0)
}

/// The number of logical cores the system has: what CPU time is shared between, so
/// a process using all of them reads `cores() * 100` percent of one core.
pub fn cores() -> usize {
    imp::cores().unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
}

#[cfg(target_os = "macos")]
mod imp {
    use std::{mem, sync::OnceLock, time::Duration};

    use super::ProcessStats;

    pub fn cores() -> Option<usize> {
        None
    }

    /// `mach_timebase_info_data_t`. libc's copy is deprecated in favour of a crate the
    /// launcher doesn't use, so it is declared here.
    #[repr(C)]
    struct Timebase {
        numer: u32,
        denom: u32,
    }

    unsafe extern "C" {
        fn mach_timebase_info(info: *mut Timebase) -> libc::c_int;
    }

    /// Mach time units per nanosecond, as (numer, denom): CPU times come in them.
    fn timebase() -> (u64, u64) {
        static TIMEBASE: OnceLock<(u64, u64)> = OnceLock::new();
        *TIMEBASE.get_or_init(|| {
            let mut info = Timebase { numer: 0, denom: 0 };
            // SAFETY: `info` is valid for writes and laid out as the C struct.
            let ok = unsafe { mach_timebase_info(&mut info) } == 0;
            if ok && info.numer != 0 && info.denom != 0 {
                (u64::from(info.numer), u64::from(info.denom))
            } else {
                (1, 1)
            }
        })
    }

    pub fn sample(pid: u32) -> Option<ProcessStats> {
        let pid = libc::c_int::try_from(pid).ok()?;
        // SAFETY: an all-zero rusage_info_v2 is valid; proc_pid_rusage writes at most
        // its size for RUSAGE_INFO_V2.
        let mut usage: libc::rusage_info_v2 = unsafe { mem::zeroed() };
        let read = unsafe {
            libc::proc_pid_rusage(
                pid,
                libc::RUSAGE_INFO_V2,
                (&mut usage as *mut libc::rusage_info_v2).cast(),
            )
        };
        if read != 0 {
            return None;
        }
        let (numer, denom) = timebase();
        let ticks = usage.ri_user_time.saturating_add(usage.ri_system_time);
        let nanos = u128::from(ticks) * u128::from(numer) / u128::from(denom);
        Some(ProcessStats {
            memory: Some(usage.ri_phys_footprint),
            cpu_time: Some(Duration::from_nanos(
                u64::try_from(nanos).unwrap_or(u64::MAX),
            )),
            threads: threads(pid),
            open_files: open_files(pid),
        })
    }

    fn threads(pid: libc::c_int) -> Option<u32> {
        // SAFETY: an all-zero proc_taskinfo is valid; proc_pidinfo writes at most the
        // size it is given.
        let mut info: libc::proc_taskinfo = unsafe { mem::zeroed() };
        let size = mem::size_of::<libc::proc_taskinfo>() as libc::c_int;
        let read = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDTASKINFO,
                0,
                (&mut info as *mut libc::proc_taskinfo).cast(),
                size,
            )
        };
        (read == size).then(|| u32::try_from(info.pti_threadnum).unwrap_or(0))
    }

    fn open_files(pid: libc::c_int) -> Option<u32> {
        let entry = mem::size_of::<libc::proc_fdinfo>();
        // SAFETY: a null buffer asks only for the size the list needs.
        let needed =
            unsafe { libc::proc_pidinfo(pid, libc::PROC_PIDLISTFDS, 0, std::ptr::null_mut(), 0) };
        if needed <= 0 {
            return None;
        }
        // room for a few opened meanwhile
        let mut list: Vec<libc::proc_fdinfo> =
            Vec::with_capacity(usize::try_from(needed).ok()? / entry + 16);
        let bytes = libc::c_int::try_from(list.capacity() * entry).ok()?;
        // SAFETY: the buffer holds `bytes` bytes; proc_pidinfo returns how many it
        // wrote, whole entries only.
        let wrote = unsafe {
            libc::proc_pidinfo(
                pid,
                libc::PROC_PIDLISTFDS,
                0,
                list.as_mut_ptr().cast(),
                bytes,
            )
        };
        (wrote > 0).then(|| (usize::try_from(wrote).unwrap_or(0) / entry) as u32)
    }
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod imp {
    use std::time::Duration;

    use super::ProcessStats;

    pub fn cores() -> Option<usize> {
        None
    }

    pub fn sample(pid: u32) -> Option<ProcessStats> {
        let dir = format!("/proc/{pid}");
        let status = std::fs::read_to_string(format!("{dir}/status")).ok()?;
        let field = |name: &str| {
            status
                .lines()
                .find_map(|line| line.strip_prefix(name))
                .and_then(|rest| rest.split_whitespace().next())
                .and_then(|value| value.parse::<u64>().ok())
        };
        let stat = std::fs::read_to_string(format!("{dir}/stat")).ok();
        Some(ProcessStats {
            memory: field("VmRSS:").map(|kb| kb * 1024),
            cpu_time: stat.as_deref().and_then(cpu_time),
            threads: field("Threads:").and_then(|n| u32::try_from(n).ok()),
            open_files: std::fs::read_dir(format!("{dir}/fd"))
                .ok()
                .map(|entries| entries.count() as u32),
        })
    }

    /// utime and stime, fields 14 and 15 of /proc/<pid>/stat, after the command
    /// (which may hold spaces) in parentheses.
    fn cpu_time(stat: &str) -> Option<Duration> {
        let rest = &stat[stat.rfind(')')? + 1..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        let utime: u64 = fields.get(11)?.parse().ok()?;
        let stime: u64 = fields.get(12)?.parse().ok()?;
        // SAFETY: sysconf has no preconditions.
        let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
        let ticks = u64::try_from(ticks).ok().filter(|t| *t > 0)?;
        Some(Duration::from_nanos(
            (utime + stime).saturating_mul(1_000_000_000) / ticks,
        ))
    }
}

#[cfg(windows)]
mod imp {
    use std::{mem, time::Duration};

    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, INVALID_HANDLE_VALUE},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            Threading::{
                ALL_PROCESSOR_GROUPS, GetActiveProcessorCount, GetProcessHandleCount,
                GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
            },
        },
    };

    use super::ProcessStats;

    /// Every processor group's logical processors. `available_parallelism` counts
    /// only the launcher's own group (at most 64) and its affinity, while process CPU
    /// times add up across all of them.
    pub fn cores() -> Option<usize> {
        // SAFETY: GetActiveProcessorCount has no preconditions; 0 means it failed.
        let count = unsafe { GetActiveProcessorCount(ALL_PROCESSOR_GROUPS) };
        (count > 0).then_some(count as usize)
    }

    fn hundreds_of_ns(time: &FILETIME) -> u64 {
        (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
    }

    pub fn sample(pid: u32) -> Option<ProcessStats> {
        // SAFETY: OpenProcess has no preconditions; a null handle means it failed.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
        if process.is_null() {
            return None;
        }
        // SAFETY: `process` is open with query rights; every out-pointer is valid
        // for writes and `cb` is the counters' size.
        let stats = unsafe {
            let mut counters: PROCESS_MEMORY_COUNTERS = mem::zeroed();
            counters.cb = mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
            let memory = (K32GetProcessMemoryInfo(process, &mut counters, counters.cb) != 0)
                .then_some(counters.WorkingSetSize as u64);
            let zero = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let (mut created, mut exited, mut kernel, mut user) = (zero, zero, zero, zero);
            let cpu_time =
                (GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) != 0)
                    .then(|| {
                        Duration::from_nanos(
                            (hundreds_of_ns(&kernel) + hundreds_of_ns(&user)).saturating_mul(100),
                        )
                    });
            let mut handles = 0u32;
            let open_files = (GetProcessHandleCount(process, &mut handles) != 0).then_some(handles);
            CloseHandle(process);
            ProcessStats {
                memory,
                cpu_time,
                threads: threads(pid),
                open_files,
            }
        };
        Some(stats)
    }

    /// The process's thread count, from a snapshot of the system's processes (Windows
    /// has no call that asks one process for it).
    fn threads(pid: u32) -> Option<u32> {
        // SAFETY: CreateToolhelp32Snapshot has no preconditions; it returns
        // INVALID_HANDLE_VALUE when it fails.
        let snapshot = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) };
        if snapshot == INVALID_HANDLE_VALUE || snapshot.is_null() {
            return None;
        }
        // SAFETY: an all-zero PROCESSENTRY32W is valid once dwSize is set; the
        // snapshot is open until CloseHandle.
        unsafe {
            let mut entry: PROCESSENTRY32W = mem::zeroed();
            entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut found = None;
            let mut more = Process32FirstW(snapshot, &mut entry) != 0;
            while more {
                if entry.th32ProcessID == pid {
                    found = Some(entry.cntThreads);
                    break;
                }
                more = Process32NextW(snapshot, &mut entry) != 0;
            }
            CloseHandle(snapshot);
            found
        }
    }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "android",
    windows
)))]
mod imp {
    use super::ProcessStats;

    pub fn cores() -> Option<usize> {
        None
    }

    pub fn sample(_pid: u32) -> Option<ProcessStats> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "macos", target_os = "linux", windows))]
    #[test]
    fn this_process_has_memory_threads_and_cpu_time() {
        let before = sample(std::process::id()).expect("reads itself");
        // use some CPU
        let started = std::time::Instant::now();
        let mut x = 0u64;
        while started.elapsed() < Duration::from_millis(50) {
            x = x.wrapping_mul(31).wrapping_add(7);
        }
        std::hint::black_box(x);
        let after = sample(std::process::id()).unwrap();
        assert!(after.memory.unwrap() > 1024 * 1024, "{after:?}");
        assert!(after.threads.unwrap() >= 1);
        assert!(after.open_files.unwrap() >= 1);
        let percent = cpu_percent(&before, &after, started.elapsed()).unwrap();
        assert!(percent > 20.0, "{percent}");
        assert!(after.cpu_time.unwrap() > Duration::ZERO);
    }

    #[test]
    fn there_is_at_least_one_core() {
        assert!(cores() >= 1);
    }

    #[test]
    fn a_missing_process_reads_as_none() {
        // pids don't reach this high
        assert_eq!(sample(u32::MAX - 7), None);
    }

    #[test]
    fn cpu_percent_is_time_used_over_wall_time() {
        let at = |ms| ProcessStats {
            cpu_time: Some(Duration::from_millis(ms)),
            ..ProcessStats::default()
        };
        assert_eq!(
            cpu_percent(&at(100), &at(600), Duration::from_secs(1)),
            Some(50.0)
        );
        assert_eq!(
            cpu_percent(&at(600), &at(100), Duration::from_secs(1)),
            None
        );
        assert_eq!(cpu_percent(&at(0), &at(1), Duration::ZERO), None);
    }
}
