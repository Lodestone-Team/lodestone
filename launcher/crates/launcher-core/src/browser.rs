//! Opening a page in a window of its own in the user's default browser.
//!
//! The apps open pages the usual way (the system's handler), which adds a tab to
//! whichever browser window the user had. For a run of pages (the download pages of
//! the files only CurseForge's website has), [`open_in_new_window`] opens the first in
//! a new window instead: the pages opened after it land there too, as a browser adds
//! tabs to the window used last, and at the end the user closes one window rather than
//! picking tabs out of their own.
//!
//! That takes starting the browser itself with its "new window" option, so it only
//! works for browsers known to have one, and where the default browser's program can
//! be found: on Windows. Anywhere else, or with another browser, it does nothing and
//! says so, and the caller opens the page the usual way.

use std::path::{Path, PathBuf};

/// The option the browsers below take for a page in a new window. Chromium's browsers
/// document it; Firefox's take it with one dash or two.
const NEW_WINDOW: &str = "--new-window";

/// The programs known to take [`NEW_WINDOW`], by file name without its extension, in
/// lowercase.
const KNOWN: [&str; 14] = [
    "chrome",
    "chromium",
    "msedge",
    "brave",
    "vivaldi",
    "opera",
    "arc",
    "thorium",
    "firefox",
    "librewolf",
    "waterfox",
    "floorp",
    "zen",
    "palemoon",
];

/// Whether `program` is a browser known to open a page in a new window when asked.
fn takes_new_window(program: &Path) -> bool {
    program
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| KNOWN.contains(&stem.to_ascii_lowercase().as_str()))
}

/// Whether `url` is a plain HTTPS address: the only kind handed to a browser's command
/// line here. It can't be taken for an option (it starts with `https://`), and it goes
/// as one argument, never through a shell.
fn is_web_page(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|parsed| {
        parsed.scheme() == "https" && parsed.host_str().is_some() && url.starts_with("https://")
    })
}

/// The program that opens `https` links for this user, if the system says.
#[cfg(windows)]
pub fn default_browser() -> Option<PathBuf> {
    use std::os::windows::ffi::OsStringExt as _;
    use windows_sys::Win32::UI::Shell::{ASSOCF_NONE, ASSOCSTR_EXECUTABLE, AssocQueryStringW};

    let wide = |text: &str| -> Vec<u16> { text.encode_utf16().chain(std::iter::once(0)).collect() };
    let (scheme, verb) = (wide("https"), wide("open"));
    let mut buffer = [0u16; 1024];
    let mut length = buffer.len() as u32;
    // SAFETY: the strings are NUL-terminated and outlive the call, and `length` is the
    // buffer's size in characters, which the call never writes past.
    let result = unsafe {
        AssocQueryStringW(
            ASSOCF_NONE,
            ASSOCSTR_EXECUTABLE,
            scheme.as_ptr(),
            verb.as_ptr(),
            buffer.as_mut_ptr(),
            &mut length,
        )
    };
    if result != 0 {
        return None;
    }
    let end = buffer.iter().position(|c| *c == 0).unwrap_or(buffer.len());
    let path = PathBuf::from(std::ffi::OsString::from_wide(&buffer[..end]));
    path.is_file().then_some(path)
}

/// The program that opens `https` links for this user. Not found on this system: pages
/// open the usual way.
#[cfg(not(windows))]
pub fn default_browser() -> Option<PathBuf> {
    None
}

/// Opens `url` in a new window of the user's default browser. `false` when that can't
/// be done (an address that isn't HTTPS, a browser that isn't known to have the
/// option, a system where the browser's program isn't found, or the browser didn't
/// start): the caller then opens the page the usual way.
pub fn open_in_new_window(url: &str) -> bool {
    if !is_web_page(url) {
        return false;
    }
    let Some(browser) = default_browser().filter(|program| takes_new_window(program)) else {
        return false;
    };
    std::process::Command::new(browser)
        .arg(NEW_WINDOW)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_browsers_known_to_have_the_option() {
        for known in [
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Mozilla Firefox\Firefox.exe",
            "/usr/bin/brave",
        ] {
            assert!(takes_new_window(Path::new(known)), "{known}");
        }
        for other in [
            r"C:\Windows\System32\OpenWith.exe",
            r"C:\Tools\chrome-wrapper.exe",
            "",
        ] {
            assert!(!takes_new_window(Path::new(other)), "{other}");
        }
    }

    #[test]
    fn only_https_pages_go_to_a_command_line() {
        assert!(is_web_page(
            "https://www.curseforge.com/minecraft/mc-mods/jei/download/1"
        ));
        for refused in [
            "http://www.curseforge.com/minecraft/mc-mods/jei",
            "--new-window",
            "file:///C:/Windows/System32/calc.exe",
            "HTTPS://www.curseforge.com/",
            "https://",
            " https://www.curseforge.com/",
        ] {
            assert!(!is_web_page(refused), "{refused}");
            assert!(!open_in_new_window(refused), "{refused}");
        }
    }
}
