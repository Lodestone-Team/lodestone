//! Settings › Profile › Startup: opening Lodestone Host when the user signs in to the
//! computer, so the core and the servers set to start automatically come back after a
//! restart. It is off until the user turns it on.
//!
//! The system keeps the setting, not `settings.json`, so it is right even after the
//! Host is moved or removed:
//!
//! - Windows: a value under the user's `Run` registry key.
//! - macOS: a launch agent, `~/Library/LaunchAgents/<label>.plist`.
//! - Linux: `~/.config/autostart/<name>.desktop`.
//!
//! Each starts this executable with [`BACKGROUND_FLAG`], which keeps the window out of
//! the way ([`crate::app::hide`]). It counts as on only while it names this
//! executable.

use std::path::Path;

/// Starts the Host without showing its window.
pub const BACKGROUND_FLAG: &str = "--background";

/// The entry's name. A development build has its own, as it has its own data folder
/// ([`crate::core_process::host_dir`]), so it never replaces an installed Host's.
const NAME: &str = if cfg!(debug_assertions) {
    "Lodestone Host Dev"
} else {
    "Lodestone Host"
};

/// Whether this start was asked to stay in the background.
pub fn started_in_background() -> bool {
    std::env::args().any(|arg| arg == BACKGROUND_FLAG)
}

/// Whether this Host opens when the user signs in.
pub fn is_enabled() -> bool {
    std::env::current_exe().is_ok_and(|exe| platform::is_enabled(&exe))
}

/// Turns opening at sign-in on or off for this executable.
pub fn set(enabled: bool) -> std::io::Result<()> {
    if enabled {
        platform::enable(&std::env::current_exe()?)
    } else {
        platform::disable()
    }
}

/// A file that starts the Host at sign-in (macOS and Linux): on while it holds
/// `content`.
#[cfg_attr(windows, allow(dead_code))]
mod entry_file {
    use super::*;

    pub fn is_enabled(path: &Path, content: &str) -> bool {
        std::fs::read_to_string(path).is_ok_and(|found| found == content)
    }

    pub fn enable(path: &Path, content: &str) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)
    }

    pub fn disable(path: &Path) -> std::io::Result<()> {
        match std::fs::remove_file(path) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
            _ => Ok(()),
        }
    }
}

/// The launch agent that opens `exe` at sign-in on macOS.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn launch_agent(label: &str, exe: &Path) -> String {
    let escape = |text: &str| {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    };
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>{BACKGROUND_FLAG}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>ProcessType</key>
    <string>Interactive</string>
</dict>
</plist>
"#,
        escape(label),
        escape(&exe.to_string_lossy()),
    )
}

/// The autostart entry that opens `exe` at sign-in on Linux. In `Exec`, a quoted
/// argument escapes `"`, `` ` ``, `$` and `\`, and `%` is doubled.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn autostart_entry(name: &str, exe: &Path) -> String {
    let mut quoted = String::new();
    for c in exe.to_string_lossy().chars() {
        match c {
            '"' | '`' | '$' | '\\' => {
                quoted.push('\\');
                quoted.push(c);
            }
            '%' => quoted.push_str("%%"),
            _ => quoted.push(c),
        }
    }
    format!(
        "[Desktop Entry]\nType=Application\nName={name}\nExec=\"{quoted}\" {BACKGROUND_FLAG}\nTerminal=false\nX-GNOME-Autostart-enabled=true\n"
    )
}

/// The command in the `Run` key that opens `exe` at sign-in on Windows.
#[cfg_attr(not(windows), allow(dead_code))]
fn run_command(exe: &Path) -> String {
    format!("\"{}\" {BACKGROUND_FLAG}", exe.display())
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;
    use std::path::PathBuf;

    const LABEL: &str = if cfg!(debug_assertions) {
        "cc.lodestone.host.dev.login"
    } else {
        "cc.lodestone.host.login"
    };

    fn path() -> Option<PathBuf> {
        Some(
            dirs::home_dir()?
                .join("Library/LaunchAgents")
                .join(format!("{LABEL}.plist")),
        )
    }

    pub fn is_enabled(exe: &Path) -> bool {
        path().is_some_and(|path| entry_file::is_enabled(&path, &launch_agent(LABEL, exe)))
    }

    pub fn enable(exe: &Path) -> std::io::Result<()> {
        let path = path().ok_or(std::io::ErrorKind::NotFound)?;
        entry_file::enable(&path, &launch_agent(LABEL, exe))
    }

    pub fn disable() -> std::io::Result<()> {
        match path() {
            Some(path) => entry_file::disable(&path),
            None => Ok(()),
        }
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
mod platform {
    use super::*;
    use std::path::PathBuf;

    fn path() -> Option<PathBuf> {
        let file = if cfg!(debug_assertions) {
            "lodestone-host-dev.desktop"
        } else {
            "lodestone-host.desktop"
        };
        Some(dirs::config_dir()?.join("autostart").join(file))
    }

    pub fn is_enabled(exe: &Path) -> bool {
        path().is_some_and(|path| entry_file::is_enabled(&path, &autostart_entry(NAME, exe)))
    }

    pub fn enable(exe: &Path) -> std::io::Result<()> {
        let path = path().ok_or(std::io::ErrorKind::NotFound)?;
        entry_file::enable(&path, &autostart_entry(NAME, exe))
    }

    pub fn disable() -> std::io::Result<()> {
        match path() {
            Some(path) => entry_file::disable(&path),
            None => Ok(()),
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::*;

    /// What Windows starts when the user signs in.
    const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

    pub fn is_enabled(exe: &Path) -> bool {
        registry::read(RUN_KEY, NAME).is_some_and(|command| command == run_command(exe))
    }

    pub fn enable(exe: &Path) -> std::io::Result<()> {
        registry::write(RUN_KEY, NAME, &run_command(exe))
    }

    pub fn disable() -> std::io::Result<()> {
        registry::delete(RUN_KEY, NAME)
    }
}

/// Text values under the user's registry (`HKEY_CURRENT_USER`).
#[cfg(windows)]
mod registry {
    use windows_sys::Win32::{
        Foundation::{ERROR_FILE_NOT_FOUND, ERROR_SUCCESS},
        System::Registry::{
            HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_OPTION_NON_VOLATILE,
            REG_SZ, RegCloseKey, RegCreateKeyExW, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW,
            RegSetValueExW,
        },
    };

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain([0]).collect()
    }

    fn check(code: u32) -> std::io::Result<()> {
        if code == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(std::io::Error::from_raw_os_error(code as i32))
        }
    }

    /// An open key, closed when dropped.
    struct Key(HKEY);

    impl Drop for Key {
        fn drop(&mut self) {
            // SAFETY: the handle came from a call that opened it, and is closed once.
            unsafe { RegCloseKey(self.0) };
        }
    }

    fn open(subkey: &str, access: u32) -> std::io::Result<Key> {
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: `subkey` is NUL-terminated and `key` is a valid place for the handle.
        check(unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                wide(subkey).as_ptr(),
                0,
                access,
                &mut key,
            )
        })?;
        Ok(Key(key))
    }

    /// The text of `name` under `subkey`, if there is one.
    pub fn read(subkey: &str, name: &str) -> Option<String> {
        let key = open(subkey, KEY_QUERY_VALUE).ok()?;
        let name = wide(name);
        let mut kind = 0u32;
        let mut bytes = 0u32;
        // SAFETY: with no buffer, the call only writes the value's type and size.
        check(unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                std::ptr::null(),
                &mut kind,
                std::ptr::null_mut(),
                &mut bytes,
            )
        })
        .ok()?;
        if kind != REG_SZ {
            return None;
        }
        let mut buffer = vec![0u16; (bytes as usize).div_ceil(2)];
        let mut bytes = (buffer.len() * 2) as u32;
        // SAFETY: `buffer` holds `bytes` bytes, the size given to the call.
        check(unsafe {
            RegQueryValueExW(
                key.0,
                name.as_ptr(),
                std::ptr::null(),
                &mut kind,
                buffer.as_mut_ptr().cast(),
                &mut bytes,
            )
        })
        .ok()?;
        buffer.truncate((bytes as usize) / 2);
        while buffer.last() == Some(&0) {
            buffer.pop();
        }
        Some(String::from_utf16_lossy(&buffer))
    }

    /// Sets `name` under `subkey` (made if missing) to `value`.
    pub fn write(subkey: &str, name: &str, value: &str) -> std::io::Result<()> {
        let mut key: HKEY = std::ptr::null_mut();
        // SAFETY: the strings are NUL-terminated and `key` is a valid place for the
        // handle; the optional arguments are null.
        check(unsafe {
            RegCreateKeyExW(
                HKEY_CURRENT_USER,
                wide(subkey).as_ptr(),
                0,
                std::ptr::null(),
                REG_OPTION_NON_VOLATILE,
                KEY_SET_VALUE,
                std::ptr::null(),
                &mut key,
                std::ptr::null_mut(),
            )
        })?;
        let key = Key(key);
        let value = wide(value);
        // SAFETY: `value` is NUL-terminated text of the byte length given.
        check(unsafe {
            RegSetValueExW(
                key.0,
                wide(name).as_ptr(),
                0,
                REG_SZ,
                value.as_ptr().cast(),
                (value.len() * 2) as u32,
            )
        })
    }

    /// Removes `name` under `subkey`. Nothing to remove is fine.
    pub fn delete(subkey: &str, name: &str) -> std::io::Result<()> {
        let key = match open(subkey, KEY_SET_VALUE) {
            Ok(key) => key,
            Err(error) if error.raw_os_error() == Some(ERROR_FILE_NOT_FOUND as i32) => {
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        // SAFETY: `name` is NUL-terminated and the key is open.
        match unsafe { RegDeleteValueW(key.0, wide(name).as_ptr()) } {
            ERROR_SUCCESS | ERROR_FILE_NOT_FOUND => Ok(()),
            code => check(code),
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use windows_sys::Win32::System::Registry::RegDeleteKeyW;

        #[test]
        fn text_values_are_written_read_and_removed() {
            // a key of the test's own: the real Run key is never touched
            let subkey = format!(r"Software\Lodestone\HostTest-{}", std::process::id());
            assert_eq!(read(&subkey, "Host"), None);
            delete(&subkey, "Host").unwrap();

            let command = r#""C:\Program Files\Lodestone Host\lodestone-host.exe" --background"#;
            write(&subkey, "Host", command).unwrap();
            assert_eq!(read(&subkey, "Host").as_deref(), Some(command));
            write(&subkey, "Host", "other").unwrap();
            assert_eq!(read(&subkey, "Host").as_deref(), Some("other"));
            assert_eq!(read(&subkey, "Missing"), None);

            delete(&subkey, "Host").unwrap();
            assert_eq!(read(&subkey, "Host"), None);
            delete(&subkey, "Host").unwrap();
            // SAFETY: the name is NUL-terminated; the key is the test's own.
            unsafe { RegDeleteKeyW(HKEY_CURRENT_USER, wide(&subkey).as_ptr()) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_system_is_told_to_start_the_host_in_the_background() {
        assert_eq!(
            run_command(Path::new(
                r"C:\Users\Sam Lee\AppData\Local\Programs\Lodestone Host\lodestone-host.exe"
            )),
            r#""C:\Users\Sam Lee\AppData\Local\Programs\Lodestone Host\lodestone-host.exe" --background"#
        );

        let agent = launch_agent(
            "cc.lodestone.host.login",
            Path::new("/Applications/Lodestone Host.app/Contents/MacOS/R&D <host>"),
        );
        assert!(agent.contains("<string>cc.lodestone.host.login</string>"));
        assert!(agent.contains(
            "<string>/Applications/Lodestone Host.app/Contents/MacOS/R&amp;D &lt;host&gt;</string>"
        ));
        assert!(agent.contains("<string>--background</string>"));
        assert!(agent.contains("<key>RunAtLoad</key>\n    <true/>"));

        let entry = autostart_entry(
            "Lodestone Host",
            Path::new("/home/sam/My $Apps/100%/lodestone-host"),
        );
        assert!(entry.starts_with("[Desktop Entry]\nType=Application\nName=Lodestone Host\n"));
        assert!(
            entry.contains("Exec=\"/home/sam/My \\$Apps/100%%/lodestone-host\" --background\n")
        );
    }

    #[test]
    fn an_entry_file_is_on_only_while_it_names_this_host() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("autostart").join("host.desktop");
        let content = autostart_entry("Lodestone Host", Path::new("/opt/host"));
        // off until it is turned on
        assert!(!entry_file::is_enabled(&path, &content));
        entry_file::disable(&path).unwrap();

        entry_file::enable(&path, &content).unwrap();
        assert!(entry_file::is_enabled(&path, &content));
        // another copy of the Host isn't this one
        let moved = autostart_entry("Lodestone Host", Path::new("/home/sam/host"));
        assert!(!entry_file::is_enabled(&path, &moved));

        entry_file::disable(&path).unwrap();
        assert!(!path.exists());
    }
}
