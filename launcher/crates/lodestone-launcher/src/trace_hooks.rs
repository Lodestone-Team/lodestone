//! The launcher's global hook for the debug traces (see [`launcher_core::trace`]): the
//! shortcuts typed and the actions they ran. Log records reach the traces through
//! the log file's logger ([`launcher_core::logging`]).

use gpui::{App, Keystroke};
use launcher_core::trace;
use serde_json::json;

/// "cmd-shift-z": the modifiers, then the key.
fn keys(keystroke: &Keystroke) -> String {
    let m = &keystroke.modifiers;
    let mut out = String::new();
    for (on, name) in [
        (m.control, "ctrl-"),
        (m.alt, "alt-"),
        (m.shift, "shift-"),
        (m.platform, "cmd-"),
        (m.function, "fn-"),
    ] {
        if on {
            out.push_str(name);
        }
    }
    out.push_str(&keystroke.key);
    out
}

/// Installs the keystroke observer. Call once, at start.
pub fn install(cx: &mut App) {
    cx.observe_keystrokes(|event, _, _| {
        let keystroke = &event.keystroke;
        let modified = keystroke.modifiers.control
            || keystroke.modifiers.platform
            || keystroke.modifiers.alt
            || keystroke.modifiers.function;
        match &event.action {
            // A key that ran an action: the action's name and the keys, never text.
            Some(action) => {
                let name = action.name();
                trace::emit(
                    "ui.action",
                    || json!({ "action": name, "keys": keys(keystroke) }),
                );
            }
            // A shortcut nothing bound: live only. Plain typing isn't traced.
            None if modified => {
                trace::emit("ui.key", || json!({ "keys": keys(keystroke) }));
            }
            None => {}
        }
    })
    .detach();
}
