//! Key bindings in `options.txt`, in the three forms the game has used:
//!
//! - before data version 1344 (1.12.2 and older): LWJGL 2 key codes, `key_key.forward:17`,
//!   and mouse buttons as negative numbers, `key_key.attack:-100`;
//! - 1344 to 1445 (1.13 snapshots): `key.w`, `key.mouse.left`;
//! - 1446 and later: `key.keyboard.w`, `key.mouse.left`.
//!
//! The game upgrades older files itself (`OptionsKeyLwjgl3Fix` at 1344 and
//! `OptionsKeyTranslationFix` at 1446); this module does the same conversion, and the
//! reverse for older instances. The code table is the one in the game's
//! `OptionsKeyLwjgl3Fix` (read from the 1.21.11 client), so a binding converts exactly as
//! the game would convert it.

/// The data version of `OptionsKeyLwjgl3Fix`: files older than this use key codes.
pub const NAMES_SINCE: u32 = 1344;
/// The data version of `OptionsKeyTranslationFix`: files older than this (and at least
/// [`NAMES_SINCE`]) use `key.<name>` without `keyboard.`.
pub const KEYBOARD_PREFIX_SINCE: u32 = 1446;

/// LWJGL 2 key codes and the key names the game gives them (after `key.keyboard.`).
const KEYS: &[(i32, &str)] = &[
    (0, "unknown"),
    (1, "escape"),
    (2, "1"),
    (3, "2"),
    (4, "3"),
    (5, "4"),
    (6, "5"),
    (7, "6"),
    (8, "7"),
    (9, "8"),
    (10, "9"),
    (11, "0"),
    (12, "minus"),
    (13, "equal"),
    (14, "backspace"),
    (15, "tab"),
    (16, "q"),
    (17, "w"),
    (18, "e"),
    (19, "r"),
    (20, "t"),
    (21, "y"),
    (22, "u"),
    (23, "i"),
    (24, "o"),
    (25, "p"),
    (26, "left.bracket"),
    (27, "right.bracket"),
    (28, "enter"),
    (29, "left.control"),
    (30, "a"),
    (31, "s"),
    (32, "d"),
    (33, "f"),
    (34, "g"),
    (35, "h"),
    (36, "j"),
    (37, "k"),
    (38, "l"),
    (39, "semicolon"),
    (40, "apostrophe"),
    (41, "grave.accent"),
    (42, "left.shift"),
    (43, "backslash"),
    (44, "z"),
    (45, "x"),
    (46, "c"),
    (47, "v"),
    (48, "b"),
    (49, "n"),
    (50, "m"),
    (51, "comma"),
    (52, "period"),
    (53, "slash"),
    (54, "right.shift"),
    (55, "keypad.multiply"),
    (56, "left.alt"),
    (57, "space"),
    (58, "caps.lock"),
    (59, "f1"),
    (60, "f2"),
    (61, "f3"),
    (62, "f4"),
    (63, "f5"),
    (64, "f6"),
    (65, "f7"),
    (66, "f8"),
    (67, "f9"),
    (68, "f10"),
    (69, "num.lock"),
    (70, "scroll.lock"),
    (71, "keypad.7"),
    (72, "keypad.8"),
    (73, "keypad.9"),
    (74, "keypad.subtract"),
    (75, "keypad.4"),
    (76, "keypad.5"),
    (77, "keypad.6"),
    (78, "keypad.add"),
    (79, "keypad.1"),
    (80, "keypad.2"),
    (81, "keypad.3"),
    (82, "keypad.0"),
    (83, "keypad.decimal"),
    (87, "f11"),
    (88, "f12"),
    (100, "f13"),
    (101, "f14"),
    (102, "f15"),
    (103, "f16"),
    (104, "f17"),
    (105, "f18"),
    (113, "f19"),
    (141, "keypad.equal"),
    (156, "keypad.enter"),
    (157, "right.control"),
    (181, "keypad.divide"),
    (183, "print.screen"),
    (184, "right.alt"),
    (197, "pause"),
    (199, "home"),
    (200, "up"),
    (201, "page.up"),
    (203, "left"),
    (205, "right"),
    (207, "end"),
    (208, "down"),
    (209, "page.down"),
    (210, "insert"),
    (211, "delete"),
    (219, "left.win"),
    (220, "right.win"),
];

/// The key names the game knows (after `key.keyboard.`), sorted: those of the GLFW
/// releases (1.13 to 1.21.x) and the longer list of 26.x, read from `InputConstants` in
/// the 1.20.1, 1.21.10 and 26.3 clients. The game binds any other name to nothing.
const NAMES: &[&str] = &[
    "0",
    "1",
    "2",
    "3",
    "4",
    "5",
    "6",
    "7",
    "8",
    "9",
    "a",
    "ac.back",
    "ac.bookmarks",
    "ac.close",
    "ac.exit",
    "ac.forward",
    "ac.home",
    "ac.new",
    "ac.open",
    "ac.print",
    "ac.properties",
    "ac.refresh",
    "ac.save",
    "ac.search",
    "ac.stop",
    "again",
    "alternate.erase",
    "apostrophe",
    "application",
    "b",
    "backslash",
    "backspace",
    "c",
    "call",
    "cancel",
    "caps.lock",
    "channel.down",
    "channel.up",
    "clear",
    "clear.again",
    "comma",
    "copy",
    "crsel",
    "currency.subunit",
    "currency.unit",
    "cut",
    "d",
    "decimal.separator",
    "delete",
    "down",
    "e",
    "end",
    "end.call",
    "enter",
    "enter2",
    "equal",
    "escape",
    "execute",
    "exsel",
    "f",
    "f1",
    "f10",
    "f11",
    "f12",
    "f13",
    "f14",
    "f15",
    "f16",
    "f17",
    "f18",
    "f19",
    "f2",
    "f20",
    "f21",
    "f22",
    "f23",
    "f24",
    "f25",
    "f3",
    "f4",
    "f5",
    "f6",
    "f7",
    "f8",
    "f9",
    "find",
    "g",
    "grave.accent",
    "h",
    "help",
    "home",
    "i",
    "insert",
    "international1",
    "international2",
    "international3",
    "international4",
    "international5",
    "international6",
    "international7",
    "international8",
    "international9",
    "j",
    "k",
    "keypad.0",
    "keypad.00",
    "keypad.000",
    "keypad.1",
    "keypad.2",
    "keypad.3",
    "keypad.4",
    "keypad.5",
    "keypad.6",
    "keypad.7",
    "keypad.8",
    "keypad.9",
    "keypad.a",
    "keypad.add",
    "keypad.ampersand",
    "keypad.at",
    "keypad.b",
    "keypad.backspace",
    "keypad.binary",
    "keypad.c",
    "keypad.clear",
    "keypad.clear.entry",
    "keypad.colon",
    "keypad.comma",
    "keypad.d",
    "keypad.decimal",
    "keypad.divide",
    "keypad.double.ampersand",
    "keypad.double.vertical.bar",
    "keypad.e",
    "keypad.enter",
    "keypad.equal",
    "keypad.equals.as400",
    "keypad.exclamation",
    "keypad.f",
    "keypad.greater",
    "keypad.hash",
    "keypad.hexadecimal",
    "keypad.left.brace",
    "keypad.left.parenthesis",
    "keypad.less",
    "keypad.memory.add",
    "keypad.memory.clear",
    "keypad.memory.divide",
    "keypad.memory.multiply",
    "keypad.memory.recall",
    "keypad.memory.store",
    "keypad.memory.subtract",
    "keypad.multiply",
    "keypad.octal",
    "keypad.percent",
    "keypad.period",
    "keypad.plus.minus",
    "keypad.power",
    "keypad.right.brace",
    "keypad.right.parenthesis",
    "keypad.space",
    "keypad.subtract",
    "keypad.tab",
    "keypad.vertical.bar",
    "keypad.xor",
    "l",
    "lang1",
    "lang2",
    "lang3",
    "lang4",
    "lang5",
    "lang6",
    "lang7",
    "lang8",
    "lang9",
    "left",
    "left.alt",
    "left.bracket",
    "left.command",
    "left.control",
    "left.meta",
    "left.option",
    "left.shift",
    "left.win",
    "left.windows",
    "m",
    "media.eject",
    "media.fast.forward",
    "media.next.track",
    "media.pause",
    "media.play",
    "media.play.pause",
    "media.previous.track",
    "media.record",
    "media.rewind",
    "media.select",
    "media.stop",
    "menu",
    "minus",
    "mode",
    "mute",
    "n",
    "num.lock",
    "o",
    "oper",
    "out",
    "p",
    "page.down",
    "page.up",
    "paste",
    "pause",
    "period",
    "power",
    "print.screen",
    "prior",
    "q",
    "r",
    "right",
    "right.alt",
    "right.bracket",
    "right.command",
    "right.control",
    "right.meta",
    "right.option",
    "right.shift",
    "right.win",
    "right.windows",
    "s",
    "scroll.lock",
    "select",
    "semicolon",
    "separator",
    "slash",
    "sleep",
    "soft.left",
    "soft.right",
    "space",
    "stop",
    "sys.req",
    "t",
    "tab",
    "thousands.separator",
    "u",
    "undo",
    "unknown",
    "up",
    "v",
    "volume.down",
    "volume.up",
    "w",
    "wake",
    "world.1",
    "world.2",
    "x",
    "y",
    "z",
];

/// Whether `name` is a binding the game can read back as a key: a key name it knows
/// (`key.keyboard.w`), a mouse button (`key.mouse.left`, `key.mouse.4`) or a scan code
/// (`scancode.42`).
pub fn is_known(name: &str) -> bool {
    if let Some(key) = name.strip_prefix("key.keyboard.") {
        return NAMES.binary_search(&key).is_ok();
    }
    if let Some(button) = name.strip_prefix("key.mouse.") {
        return matches!(button, "left" | "right" | "middle")
            || (!button.starts_with(['0', '+']) && button.parse::<u8>().is_ok_and(|n| n >= 4));
    }
    name.strip_prefix("scancode.")
        .is_some_and(|code| !code.is_empty() && code.bytes().all(|b| b.is_ascii_digit()))
}

/// A binding's value in the current form (`key.keyboard.w`, `key.mouse.left`), from
/// the value in a file of data version `dv`. `None` when the value can't be read.
pub fn to_canonical(raw: &str, dv: u32) -> Option<String> {
    let raw = raw.trim();
    if dv < NAMES_SINCE {
        let code: i32 = raw.parse().ok()?;
        return Some(code_to_name(code));
    }
    if raw.starts_with("key.mouse.") || raw.starts_with("key.keyboard.") {
        return Some(raw.to_string());
    }
    if raw.starts_with("scancode.") {
        return Some(raw.to_string());
    }
    // `key.w` (1.13 snapshots) or a plain number from a file whose version line lies.
    if let Some(name) = raw.strip_prefix("key.") {
        return Some(format!("key.keyboard.{name}"));
    }
    raw.parse::<i32>().ok().map(code_to_name)
}

/// The value to write into a file of data version `dv`. `None` when that form has no
/// way to say it (a key LWJGL 2 had no code for, a scan code).
pub fn from_canonical(name: &str, dv: u32) -> Option<String> {
    let name = name.trim();
    if dv < NAMES_SINCE {
        return name_to_code(name).map(|code| code.to_string());
    }
    if name.starts_with("key.mouse.") || name.starts_with("scancode.") {
        return Some(name.to_string());
    }
    let key = name.strip_prefix("key.keyboard.")?;
    if dv < KEYBOARD_PREFIX_SINCE {
        Some(format!("key.{key}"))
    } else {
        Some(name.to_string())
    }
}

/// What `OptionsKeyLwjgl3Fix` and `OptionsKeyTranslationFix` make of a key code.
pub fn code_to_name(code: i32) -> String {
    if code < 0 {
        let button = code + 100;
        return match button {
            0 => "key.mouse.left".to_string(),
            1 => "key.mouse.right".to_string(),
            2 => "key.mouse.middle".to_string(),
            n => format!("key.mouse.{}", n + 1),
        };
    }
    let name = KEYS
        .iter()
        .find(|(c, _)| *c == code)
        .map_or("unknown", |(_, name)| name);
    format!("key.keyboard.{name}")
}

/// The LWJGL 2 code of a key name, the reverse of [`code_to_name`].
pub fn name_to_code(name: &str) -> Option<i32> {
    if let Some(button) = name.strip_prefix("key.mouse.") {
        let index = match button {
            "left" => 0,
            "right" => 1,
            "middle" => 2,
            n => n.parse::<i32>().ok().filter(|n| *n >= 4)? - 1,
        };
        return Some(index - 100);
    }
    let key = name.strip_prefix("key.keyboard.")?;
    KEYS.iter().find(|(_, n)| *n == key).map(|(code, _)| *code)
}

/// A short label for a binding: `W`, `Left Shift`, `Mouse 4`, `Not bound`.
pub fn label(name: &str) -> String {
    match name {
        "key.mouse.left" => return "Left Button".into(),
        "key.mouse.right" => return "Right Button".into(),
        "key.mouse.middle" => return "Middle Button".into(),
        "key.keyboard.unknown" => return "Not bound".into(),
        _ => {}
    }
    if let Some(n) = name.strip_prefix("key.mouse.") {
        return format!("Button {n}");
    }
    let Some(key) = name.strip_prefix("key.keyboard.") else {
        return name.to_string();
    };
    if key.chars().count() == 1 {
        return key.to_uppercase();
    }
    key.split('.')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_has_no_duplicates() {
        for (i, (code, name)) in KEYS.iter().enumerate() {
            assert!(
                KEYS[i + 1..].iter().all(|(c, n)| c != code && n != name),
                "{code} {name}"
            );
        }
        assert_eq!(KEYS.len(), 112);
    }

    #[test]
    fn codes_convert_like_the_game() {
        // The defaults of 1.8.9/1.12.2 and what 1.13+ writes for them.
        for (code, name) in [
            (17, "key.keyboard.w"),
            (30, "key.keyboard.a"),
            (31, "key.keyboard.s"),
            (32, "key.keyboard.d"),
            (57, "key.keyboard.space"),
            (42, "key.keyboard.left.shift"),
            (29, "key.keyboard.left.control"),
            (16, "key.keyboard.q"),
            (18, "key.keyboard.e"),
            (20, "key.keyboard.t"),
            (15, "key.keyboard.tab"),
            (53, "key.keyboard.slash"),
            (60, "key.keyboard.f2"),
            (63, "key.keyboard.f5"),
            (87, "key.keyboard.f11"),
            (2, "key.keyboard.1"),
            (10, "key.keyboard.9"),
            (0, "key.keyboard.unknown"),
            (-100, "key.mouse.left"),
            (-99, "key.mouse.right"),
            (-98, "key.mouse.middle"),
            (-97, "key.mouse.4"),
            (-96, "key.mouse.5"),
        ] {
            assert_eq!(code_to_name(code), name, "{code}");
            assert_eq!(name_to_code(name), Some(code), "{name}");
            assert_eq!(to_canonical(&code.to_string(), 1343).as_deref(), Some(name));
            assert_eq!(from_canonical(name, 1343), Some(code.to_string()));
            assert_eq!(from_canonical(name, 3465).as_deref(), Some(name));
            assert_eq!(to_canonical(name, 3465).as_deref(), Some(name));
        }
        // A code the table doesn't have becomes unknown, as in the game.
        assert_eq!(code_to_name(250), "key.keyboard.unknown");
    }

    #[test]
    fn the_short_names_of_the_13_snapshots() {
        assert_eq!(
            from_canonical("key.keyboard.w", 1400).as_deref(),
            Some("key.w")
        );
        assert_eq!(
            from_canonical("key.mouse.left", 1400).as_deref(),
            Some("key.mouse.left")
        );
        assert_eq!(
            to_canonical("key.w", 1400).as_deref(),
            Some("key.keyboard.w")
        );
        assert_eq!(
            to_canonical("key.left.shift", 1445).as_deref(),
            Some("key.keyboard.left.shift")
        );
    }

    #[test]
    fn keys_old_versions_cant_bind_are_not_written() {
        // `menu` and `world.1` have no LWJGL 2 code in the game's table.
        assert_eq!(from_canonical("key.keyboard.menu", 1343), None);
        assert_eq!(from_canonical("key.keyboard.world.1", 922), None);
        assert_eq!(from_canonical("scancode.42", 1343), None);
        assert_eq!(from_canonical("key.mouse.3", 1343), None);
        assert_eq!(to_canonical("not a number", 1343), None);
    }

    #[test]
    fn known_names() {
        assert!(NAMES.windows(2).all(|pair| pair[0] < pair[1]), "sorted");
        // Every key of the LWJGL 2 table is one the game knows.
        for (_, name) in KEYS {
            assert!(is_known(&format!("key.keyboard.{name}")), "{name}");
        }
        for name in [
            "key.keyboard.left.shift",
            "key.keyboard.menu",
            "key.mouse.left",
            "key.mouse.4",
            "key.mouse.8",
            "scancode.42",
        ] {
            assert!(is_known(name), "{name}");
        }
        for name in [
            "key.keyboard.wboguskey.keyboard.w",
            "key.keyboard.",
            "key.keyboard.W",
            "key.mouse.3",
            "key.mouse.04",
            "key.mouse.x",
            "scancode.",
            "scancode.-1",
            "w",
        ] {
            assert!(!is_known(name), "{name}");
        }
    }

    #[test]
    fn labels() {
        assert_eq!(label("key.keyboard.w"), "W");
        assert_eq!(label("key.keyboard.left.shift"), "Left Shift");
        assert_eq!(label("key.keyboard.f3"), "F3");
        assert_eq!(label("key.mouse.4"), "Button 4");
        assert_eq!(label("key.mouse.left"), "Left Button");
        assert_eq!(label("key.keyboard.unknown"), "Not bound");
    }
}
