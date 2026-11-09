//! The system's display accessibility settings: Reduce Motion, Reduce Transparency
//! and Increase Contrast (macOS: System Settings > Accessibility > Display).
//!
//! GPUI doesn't read them, and the launcher has no Objective-C crate, so they are
//! read through the Objective-C runtime that AppKit already loads. They are asked for
//! at most every couple of seconds: a change shows within that, and drawing never
//! waits on it.

use std::{
    cell::Cell,
    time::{Duration, Instant},
};

/// What the system asks of the launcher's look.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Prefs {
    pub reduce_motion: bool,
    pub reduce_transparency: bool,
    pub increase_contrast: bool,
}

/// How long a reading is kept before the system is asked again.
const FRESH: Duration = Duration::from_secs(2);

thread_local! {
    static READ: Cell<Option<(Prefs, Instant)>> = const { Cell::new(None) };
    /// Set by the screenshots, so they don't depend on the machine's settings.
    static FORCED: Cell<Option<Prefs>> = const { Cell::new(None) };
}

/// The system's settings, as last read.
pub fn prefs() -> Prefs {
    if let Some(forced) = FORCED.with(Cell::get) {
        return forced;
    }
    let now = Instant::now();
    if let Some((prefs, at)) = READ.with(Cell::get)
        && now.duration_since(at) < FRESH
    {
        return prefs;
    }
    let prefs = read();
    READ.with(|read| read.set(Some((prefs, now))));
    prefs
}

/// Uses `prefs` instead of the system's (`None` to read the system's again).
#[cfg_attr(not(feature = "screenshot"), allow(dead_code))]
pub fn force(prefs: Option<Prefs>) {
    FORCED.with(|forced| forced.set(prefs));
}

#[cfg(target_os = "macos")]
fn read() -> Prefs {
    use std::ffi::{c_char, c_void};

    type Id = *mut c_void;
    type Sel = *mut c_void;

    #[link(name = "objc")]
    unsafe extern "C" {
        fn objc_getClass(name: *const c_char) -> Id;
        fn sel_registerName(name: *const c_char) -> Sel;
        fn objc_msgSend();
    }

    // SAFETY: `objc_msgSend` is called through function types that match each method:
    // `+[NSWorkspace sharedWorkspace]` takes no arguments and returns an object, and
    // `-accessibilityDisplayShouldReduceMotion`, `…ReduceTransparency` and
    // `…IncreaseContrast` take none and return a `BOOL` (read as a byte). All exist
    // since macOS 10.12; a missing class and a null workspace are checked before any
    // other message is sent.
    unsafe {
        let class = objc_getClass(c"NSWorkspace".as_ptr());
        if class.is_null() {
            return Prefs::default();
        }
        let send_id = std::mem::transmute::<
            unsafe extern "C" fn(),
            unsafe extern "C" fn(Id, Sel) -> Id,
        >(objc_msgSend);
        // `BOOL` is a `signed char` on Intel Macs and a `bool` on Apple silicon; read it
        // as a byte and compare, so no value is ever reinterpreted as a Rust `bool`.
        let send_bool = std::mem::transmute::<
            unsafe extern "C" fn(),
            unsafe extern "C" fn(Id, Sel) -> i8,
        >(objc_msgSend);
        let workspace = send_id(class, sel_registerName(c"sharedWorkspace".as_ptr()));
        if workspace.is_null() {
            return Prefs::default();
        }
        let flag =
            |name: &std::ffi::CStr| send_bool(workspace, sel_registerName(name.as_ptr())) != 0;
        Prefs {
            reduce_motion: flag(c"accessibilityDisplayShouldReduceMotion"),
            reduce_transparency: flag(c"accessibilityDisplayShouldReduceTransparency"),
            increase_contrast: flag(c"accessibilityDisplayShouldIncreaseContrast"),
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn read() -> Prefs {
    Prefs::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn forced_prefs_win() {
        let all = Prefs {
            reduce_motion: true,
            reduce_transparency: true,
            increase_contrast: true,
        };
        force(Some(all));
        assert_eq!(prefs(), all);
        force(None);
    }
}
