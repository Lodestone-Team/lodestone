//! The platform the game runs on, as Mojang's version files name it: the operating
//! system, the CPU architecture of the Java runtime, and the OS version.
//!
//! On Apple silicon, versions before 1.14 run in an x86_64 Java under Rosetta (see
//! [`super::libraries::java_arch`]). The platform the version's rules are evaluated
//! against is then the x86_64 one, because that is what the game's Java reports.

use std::{fmt, path::Path};

/// An operating system, by its name in Mojang's rules (`os.name`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Os {
    MacOs,
    Windows,
    Linux,
}

impl Os {
    /// The name in Mojang's rules and `natives` maps.
    pub fn rule_name(self) -> &'static str {
        match self {
            Os::MacOs => "osx",
            Os::Windows => "windows",
            Os::Linux => "linux",
        }
    }

    /// The OS this build of the launcher runs on.
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Os::Windows
        } else if cfg!(target_os = "linux") {
            Os::Linux
        } else {
            Os::MacOs
        }
    }

    /// The separator between the entries of a Java classpath (`-cp`) on this OS.
    pub fn classpath_separator(self) -> &'static str {
        match self {
            Os::Windows => ";",
            Os::MacOs | Os::Linux => ":",
        }
    }
}

/// A CPU architecture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arch {
    X86_64,
    Arm64,
    /// 32-bit x86, which Mojang's rules call `x86`.
    X86,
}

impl Arch {
    /// The architecture of this build of the launcher.
    pub fn current() -> Self {
        match std::env::consts::ARCH {
            "aarch64" => Arch::Arm64,
            "x86" => Arch::X86,
            _ => Arch::X86_64,
        }
    }

    /// The value Mojang's `os.arch` rules compare with.
    pub fn rule_name(self) -> &'static str {
        match self {
            Arch::X86_64 => "x86_64",
            Arch::Arm64 => "arm64",
            Arch::X86 => "x86",
        }
    }

    /// `${arch}` in a native classifier: the JVM's bitness.
    pub fn bits(self) -> &'static str {
        match self {
            Arch::X86 => "32",
            Arch::X86_64 | Arch::Arm64 => "64",
        }
    }
}

impl fmt::Display for Arch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.rule_name())
    }
}

/// The OS, the architecture of the Java that runs the game, and the OS version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Platform {
    pub os: Os,
    pub arch: Arch,
    /// The OS version, such as `14.6.1` on macOS or `10.0.22631` on Windows 11. Mojang's
    /// `os.version` patterns and `versionRange`s are matched against it.
    pub os_version: String,
}

impl Platform {
    pub fn new(os: Os, arch: Arch, os_version: impl Into<String>) -> Self {
        Self {
            os,
            arch,
            os_version: os_version.into(),
        }
    }

    /// This machine, with the launcher's own architecture.
    pub fn current() -> Self {
        Self::new(
            Os::current(),
            Arch::current(),
            os_version().unwrap_or_default(),
        )
    }

    /// The same machine with Java built for `arch`.
    pub fn with_arch(&self, arch: Arch) -> Self {
        Self {
            arch,
            ..self.clone()
        }
    }

    /// The key of this platform in Mojang's Java runtime manifest.
    pub fn runtime_key(&self) -> &'static str {
        match (self.os, self.arch) {
            (Os::MacOs, Arch::Arm64) => "mac-os-arm64",
            (Os::MacOs, _) => "mac-os",
            (Os::Windows, Arch::Arm64) => "windows-arm64",
            (Os::Windows, Arch::X86) => "windows-x86",
            (Os::Windows, Arch::X86_64) => "windows-x64",
            (Os::Linux, Arch::X86) => "linux-i386",
            // Mojang has no Linux arm64 runtimes; this key finds none.
            (Os::Linux, Arch::Arm64) => "linux-arm64",
            (Os::Linux, Arch::X86_64) => "linux",
        }
    }
}

/// The macOS product version (`14.6.1`), from `kern.osproductversion`.
#[cfg(target_os = "macos")]
pub fn os_version() -> Option<String> {
    sysctl_string(c"kern.osproductversion")
}

/// The Windows version as `major.minor.build` (`10.0.22631`), the form the official
/// launcher matches Mojang's rules against: `^10\.` patterns and `versionRange`s such
/// as `min 10.0.17134`. Windows 11 still reports 10.0.
///
/// This uses `RtlGetVersion`, because `GetVersionEx` reports Windows 8 (6.2) to
/// programs without a compatibility manifest.
#[cfg(windows)]
pub fn os_version() -> Option<String> {
    use windows_sys::{
        Wdk::System::SystemServices::RtlGetVersion,
        Win32::System::SystemInformation::OSVERSIONINFOW,
    };
    let mut info = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a writable OSVERSIONINFOW whose size field is set, as
    // RtlGetVersion requires.
    let status = unsafe { RtlGetVersion(&mut info) };
    (status == 0)
        .then(|| windows_version(info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber))
}

#[cfg(not(any(target_os = "macos", windows)))]
pub fn os_version() -> Option<String> {
    None
}

/// A Windows version as the official launcher reports it.
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn windows_version(major: u32, minor: u32, build: u32) -> String {
    format!("{major}.{minor}.{build}")
}

/// The total physical memory in bytes.
#[cfg(target_os = "macos")]
pub fn total_memory() -> Option<u64> {
    let mut value: u64 = 0;
    let mut size = std::mem::size_of::<u64>();
    // SAFETY: `hw.memsize` is a 64-bit integer; `value` and `size` describe a buffer
    // of exactly that size, and the name is a NUL-terminated C string.
    let result = unsafe {
        libc::sysctlbyname(
            c"hw.memsize".as_ptr(),
            (&mut value as *mut u64).cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    (result == 0 && size == std::mem::size_of::<u64>()).then_some(value)
}

/// The total physical memory in bytes: the installed RAM, as Windows Settings shows
/// it. What Windows can use is a little less (hardware reserves some), and would read
/// as "15 GB" on a 16 GB PC; that is the fallback when the firmware doesn't say.
#[cfg(windows)]
pub fn total_memory() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{
        GetPhysicallyInstalledSystemMemory, GlobalMemoryStatusEx, MEMORYSTATUSEX,
    };
    let mut kilobytes: u64 = 0;
    // SAFETY: `kilobytes` is a writable u64, which is all the call writes.
    if unsafe { GetPhysicallyInstalledSystemMemory(&mut kilobytes) } != 0 && kilobytes > 0 {
        return Some(kilobytes.saturating_mul(1024));
    }
    let mut status = MEMORYSTATUSEX {
        dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
        ..Default::default()
    };
    // SAFETY: `status` is a writable MEMORYSTATUSEX whose length field is set, as
    // GlobalMemoryStatusEx requires.
    let ok = unsafe { GlobalMemoryStatusEx(&mut status) };
    (ok != 0 && status.ullTotalPhys > 0).then_some(status.ullTotalPhys)
}

/// The total memory in bytes that Linux can use, as `free` and `/proc/meminfo`'s
/// `MemTotal` show it: the installed RAM less what the firmware, an integrated GPU and
/// the kernel reserve, so a 16 GB PC can read as 15 GB. The installed size is only in
/// the DMI tables, which only root can read.
///
/// This uses the `sysinfo` system call rather than reading `/proc/meminfo`, because it
/// works where `/proc` isn't mounted (as in some sandboxes) and needs no parsing.
#[cfg(target_os = "linux")]
pub fn total_memory() -> Option<u64> {
    // SAFETY: `sysinfo` is a C struct of integers, for which all zeroes is valid.
    let mut info: libc::sysinfo = unsafe { std::mem::zeroed() };
    // SAFETY: `info` is a writable `sysinfo`, which is all the call writes.
    if unsafe { libc::sysinfo(&mut info) } != 0 {
        return None;
    }
    // `totalram` counts units of `mem_unit` bytes: 1 on 64-bit systems, the page size
    // on 32-bit ones with more memory than a `long` counts in bytes. (It is a `c_ulong`,
    // already a u64 on 64-bit systems.)
    #[allow(clippy::useless_conversion)]
    let units = u64::from(info.totalram);
    let bytes = units.checked_mul(u64::from(info.mem_unit.max(1)))?;
    (bytes > 0).then_some(bytes)
}

/// The total physical memory in bytes, from `hw.physmem`.
#[cfg(target_os = "freebsd")]
pub fn total_memory() -> Option<u64> {
    let mut value: libc::c_ulong = 0;
    let mut size = std::mem::size_of::<libc::c_ulong>();
    // SAFETY: `hw.physmem` is an unsigned long; `value` and `size` describe a buffer of
    // exactly that size, and the name is a NUL-terminated C string.
    let result = unsafe {
        libc::sysctlbyname(
            c"hw.physmem".as_ptr(),
            (&mut value as *mut libc::c_ulong).cast(),
            &mut size,
            std::ptr::null(),
            0,
        )
    };
    // A u64 already on 64-bit systems.
    #[allow(clippy::useless_conversion)]
    let value = u64::from(value);
    (result == 0 && size == std::mem::size_of::<libc::c_ulong>() && value > 0).then_some(value)
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "linux",
    target_os = "freebsd",
    windows
)))]
pub fn total_memory() -> Option<u64> {
    None
}

#[cfg(target_os = "macos")]
fn sysctl_string(name: &std::ffi::CStr) -> Option<String> {
    let mut buffer = [0u8; 256];
    let mut size = buffer.len();
    // SAFETY: `buffer` is writable for `size` bytes, and the name is NUL-terminated.
    let result = unsafe {
        libc::sysctlbyname(
            name.as_ptr(),
            buffer.as_mut_ptr().cast(),
            &mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if result != 0 {
        return None;
    }
    let bytes = &buffer[..size.min(buffer.len())];
    let end = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8(bytes[..end].to_vec()).ok()
}

/// Whether x86_64 programs can run on this Mac. On Intel Macs they always can; on
/// Apple silicon they need Rosetta 2.
pub fn can_run_x86_64() -> bool {
    if !cfg!(target_os = "macos") || Arch::current() != Arch::Arm64 {
        return Arch::current() == Arch::X86_64;
    }
    rosetta_installed()
}

/// Whether Rosetta 2 is installed: runs `/usr/bin/true` as x86_64 through `arch`.
/// It is a system binary that does nothing, so this starts no third-party code.
pub fn rosetta_installed() -> bool {
    std::process::Command::new("/usr/bin/arch")
        .args(["-x86_64", "/usr/bin/true"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

/// The architecture a Mac program at `path` runs as on Apple silicon, from its Mach-O
/// header: `X86_64` for an Intel-only program (which runs under Rosetta 2), `Arm64`
/// for an arm64 one or a universal one that has arm64 (macOS runs a universal program
/// natively). `None` when the file can't be read or isn't a Mac program (a script, say),
/// and for Apple's stubs in `/usr/bin` (`/usr/bin/java` is universal, but runs whichever
/// JDK is chosen in the system, which may be Intel).
///
/// It is read, never run: a Java override is the player's file, and starting it to ask
/// would run it before they pressed Play.
pub fn mac_binary_arch(path: &Path) -> Option<Arch> {
    let resolved = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    if path.starts_with("/usr/bin") || resolved.starts_with("/usr/bin") {
        return None;
    }
    let mut header = [0u8; 4096];
    let mut file = std::fs::File::open(path).ok()?;
    let mut read = 0;
    while read < header.len() {
        match std::io::Read::read(&mut file, &mut header[read..]) {
            Ok(0) => break,
            Ok(n) => read += n,
            Err(err) if err.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => return None,
        }
    }
    mach_o_arch(&header[..read])
}

/// [`mac_binary_arch`] on the first bytes of a file.
fn mach_o_arch(bytes: &[u8]) -> Option<Arch> {
    const CPU_TYPE_X86_64: u32 = 0x0100_0007;
    const CPU_TYPE_ARM64: u32 = 0x0100_000c;
    let word = |at: usize, big_endian: bool| -> Option<u32> {
        let bytes: [u8; 4] = bytes.get(at..at + 4)?.try_into().ok()?;
        Some(if big_endian {
            u32::from_be_bytes(bytes)
        } else {
            u32::from_le_bytes(bytes)
        })
    };
    let arch = |cpu: u32| match cpu {
        CPU_TYPE_X86_64 => Some(Arch::X86_64),
        CPU_TYPE_ARM64 => Some(Arch::Arm64),
        _ => None,
    };
    match word(0, true)? {
        // A universal ("fat") file, always big-endian: the architectures it holds,
        // 20 bytes each (32 for the 64-bit form), the CPU type first.
        magic @ (0xcafe_babe | 0xcafe_babf) => {
            let count = word(4, true)? as usize;
            // Java class files share 0xcafebabe; their next word, a version, is never
            // a small count.
            if count == 0 || count > 16 {
                return None;
            }
            let size = if magic == 0xcafe_babe { 20 } else { 32 };
            let cpus: Vec<Arch> = (0..count)
                .filter_map(|i| word(8 + i * size, true).and_then(arch))
                .collect();
            if cpus.contains(&Arch::Arm64) {
                Some(Arch::Arm64)
            } else {
                cpus.first().copied()
            }
        }
        // A thin 64-bit Mach-O file, little-endian on both Intel and Apple silicon.
        0xcffa_edfe => word(4, false).and_then(arch),
        _ => None,
    }
}

/// What to tell the user when an x86_64 Java is needed but Rosetta is missing.
pub const ROSETTA_MISSING: &str = "This Minecraft version needs an Intel (x86_64) Java, which runs on Apple silicon through Rosetta 2. Rosetta isn't installed. Install it by running this in Terminal, then press Play again:\n\nsoftwareupdate --install-rosetta";

#[cfg(test)]
mod tests {
    use super::*;

    /// Headers as Apple's tools write them: thin Intel, thin arm64, universal (Intel
    /// and arm64e, like /usr/bin/java), and files that are not Mac programs.
    #[test]
    fn reads_the_architecture_of_mac_programs() {
        let thin = |cpu: u32| {
            let mut bytes = vec![0xcf, 0xfa, 0xed, 0xfe];
            bytes.extend(cpu.to_le_bytes());
            bytes.extend([0u8; 24]);
            bytes
        };
        assert_eq!(mach_o_arch(&thin(0x0100_0007)), Some(Arch::X86_64));
        assert_eq!(mach_o_arch(&thin(0x0100_000c)), Some(Arch::Arm64));
        let fat = |cpus: &[u32]| {
            let mut bytes = vec![0xca, 0xfe, 0xba, 0xbe];
            bytes.extend((cpus.len() as u32).to_be_bytes());
            for cpu in cpus {
                bytes.extend(cpu.to_be_bytes());
                bytes.extend([0u8; 16]);
            }
            bytes
        };
        assert_eq!(
            mach_o_arch(&fat(&[0x0100_0007, 0x0100_000c])),
            Some(Arch::Arm64)
        );
        assert_eq!(mach_o_arch(&fat(&[0x0100_0007])), Some(Arch::X86_64));
        // A Java class file (0xcafebabe, then its version) and a script.
        assert_eq!(
            mach_o_arch(&[0xca, 0xfe, 0xba, 0xbe, 0x00, 0x00, 0x00, 0x34]),
            None
        );
        assert_eq!(mach_o_arch(b"#!/bin/sh\nexec java \"$@\"\n"), None);
        assert_eq!(mach_o_arch(&[]), None);
        assert_eq!(mac_binary_arch(Path::new("/nonexistent/java")), None);
        // Apple's stub runs whichever JDK is chosen: unknown, whatever its header.
        #[cfg(target_os = "macos")]
        assert_eq!(mac_binary_arch(Path::new("/usr/bin/java")), None);
    }

    #[test]
    fn runtime_keys() {
        let mac = |arch| Platform::new(Os::MacOs, arch, "14.0").runtime_key();
        assert_eq!(mac(Arch::Arm64), "mac-os-arm64");
        assert_eq!(mac(Arch::X86_64), "mac-os");
        assert_eq!(
            Platform::new(Os::Windows, Arch::X86_64, "10.0").runtime_key(),
            "windows-x64"
        );
        assert_eq!(
            Platform::new(Os::Linux, Arch::X86_64, "6.1").runtime_key(),
            "linux"
        );
    }

    #[test]
    fn rule_names() {
        assert_eq!(Os::MacOs.rule_name(), "osx");
        assert_eq!(Arch::X86.rule_name(), "x86");
        assert_eq!(Arch::Arm64.bits(), "64");
        assert_eq!(Arch::X86.bits(), "32");
    }

    #[test]
    fn windows_versions_match_mojang_rules() {
        use crate::game::rules::{Features, Rule, allows};
        let version = windows_version(10, 0, 22631);
        assert_eq!(version, "10.0.22631");
        let windows = Platform::new(Os::Windows, Arch::X86_64, version);
        let rules = |value| serde_json::from_value::<Vec<Rule>>(value).unwrap();
        let f = Features::default();
        // 1.13 to 1.21's `-Dos.name=Windows 10 -Dos.version=10.0` arguments.
        let windows_10 = rules(serde_json::json!([
            {"action": "allow", "os": {"name": "windows", "version": "^10\\."}}
        ]));
        assert!(allows(&windows_10, &windows, &f));
        // 26.3's ZGC arguments, for Windows 10 1803 and later.
        let zgc = rules(serde_json::json!([
            {"action": "allow", "os": {"name": "windows", "versionRange": {"min": "10.0.17134"}}}
        ]));
        assert!(allows(&zgc, &windows, &f));
        assert!(!allows(
            &zgc,
            &Platform::new(Os::Windows, Arch::X86_64, windows_version(10, 0, 16299)),
            &f
        ));
    }

    #[cfg(windows)]
    #[test]
    fn reads_the_windows_version_and_memory() {
        let version = os_version().unwrap();
        let parts: Vec<u32> = version.split('.').map(|p| p.parse().unwrap()).collect();
        assert_eq!(parts.len(), 3, "{version}");
        // Not the 6.2 that GetVersionEx reports without a manifest.
        assert!(parts[0] >= 10, "{version}");
        assert!(total_memory().unwrap() >= 1024 * 1024 * 1024);
    }

    #[test]
    fn classpath_separators() {
        assert_eq!(Os::Windows.classpath_separator(), ";");
        assert_eq!(Os::MacOs.classpath_separator(), ":");
        assert_eq!(Os::Linux.classpath_separator(), ":");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn reads_the_linux_memory() {
        assert!(total_memory().unwrap() >= 512 * 1024 * 1024);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn reads_the_macos_version_and_memory() {
        let version = os_version().unwrap();
        assert!(
            version.split('.').all(|part| part.parse::<u32>().is_ok()),
            "{version}"
        );
        assert!(total_memory().unwrap() >= 1024 * 1024 * 1024);
    }
}
