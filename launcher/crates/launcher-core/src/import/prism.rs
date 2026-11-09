//! Prism Launcher and MultiMC, which share the instance format.
//!
//! - Data folder: Prism `~/Library/Application Support/PrismLauncher` (`%APPDATA%` on
//!   Windows) (`prismlauncher.cfg`, or an older `polymc.cfg`/`multimc.cfg`); MultiMC keeps
//!   its data in `MultiMC.app/Data`, or beside `MultiMC.exe` on Windows (`multimc.cfg`). `InstanceDir` (default `instances`) and
//!   `IconsDir` (default `icons`) are relative to it (PrismLauncher `launcher/Application.cpp`).
//! - `instance.cfg`: Prism writes QSettings INI (a `[General]` section, values with `;`,
//!   `,`, `=` or edge spaces quoted, `\"`, `\\`, `\n` escapes, a leading `@` doubled,
//!   `launcher/settings/INIFile.cpp`); MultiMC writes `key=value` lines without a section,
//!   escaping `\n`, `\t`, `\\` and `\#` (MultiMC/Launcher `launcher/settings/INIFile.cpp`).
//!   Keys from `BaseInstance.cpp` and `MinecraftInstance.cpp`: `name`, `iconKey`,
//!   `OverrideMemory` + `MinMemAlloc`/`MaxMemAlloc` (MiB), `OverrideJavaArgs` + `JvmArgs`,
//!   `OverrideJavaLocation` + `JavaPath`; MultiMC's legacy `IntendedVersion`/`ForgeVersion`.
//!   `JvmArgs` is split into arguments by `Commandline::splitArgs` (`launcher/Commandline.cpp`).
//! - `mmc-pack.json`: `components[].{uid, version}` with bare loader versions (Forge
//!   `47.4.28`, `10.13.4.1614`), `launcher/minecraft/PackProfile.cpp`, `Component.cpp`.
//! - The game folder (`MinecraftInstance::gameRoot`): Prism uses `.minecraft` only if it
//!   exists and `minecraft` doesn't; MultiMC uses `minecraft` only if it exists and
//!   `.minecraft` doesn't.
//! - Icons: `<IconsDir>/<iconKey>.<ext>` (`launcher/icons/IconList.cpp`, `IconUtils.cpp`);
//!   `default` and the built-in keys have no file.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde_json::Value;

use super::{
    Found, IconSource, Launcher, Source, WantedLoader, WantedVersion, folder_name, read_json,
    read_text, subfolders,
};
use crate::game::{
    launch::{join_args, split_args},
    loaders::LoaderKind,
};

/// The settings files of a Prism or MultiMC data folder, in the order Prism reads them.
pub fn config_files(source: Source) -> &'static [&'static str] {
    match source {
        Source::MultiMc => &["multimc.cfg"],
        _ => &["prismlauncher.cfg", "polymc.cfg", "multimc.cfg"],
    }
}

/// Parses an `instance.cfg` (or the launcher's `.cfg`): the keys of `[General]` and of no
/// section, unescaped.
pub fn parse_cfg(text: &str) -> HashMap<String, String> {
    let mut values = HashMap::new();
    let mut section: Option<String> = None;
    for line in text.lines() {
        let line = line.trim_start_matches('\u{feff}').trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = Some(line[1..line.len() - 1].trim().to_string());
            continue;
        }
        if section
            .as_deref()
            .is_some_and(|s| !s.eq_ignore_ascii_case("General"))
        {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        values.insert(key.trim().to_string(), unescape(value.trim()));
    }
    values
}

/// Prism and MultiMC write a backslash as `\\`, so a backslash before a character neither
/// escapes means the value was typed by hand, usually with a Windows path like
/// `C:\Games\x`: that value is kept as it is (Prism itself would drop those backslashes).
fn unescape(value: &str) -> String {
    let (value, quoted) = match value.strip_prefix('"').and_then(|v| v.strip_suffix('"')) {
        Some(inner) => (inner, true),
        None => (value, false),
    };
    let value = if !quoted && value.starts_with("@@") {
        &value[1..]
    } else {
        value
    };
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\'
            && !matches!(
                chars.next(),
                None | Some('\\' | '"' | 'n' | 't' | 'r' | '0' | '#')
            )
        {
            return value.to_string();
        }
    }
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('0') => {}
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Prism's `JvmArgs` as text that [`split_args`] reads into the same arguments. Prism's
/// `Commandline::splitArgs` takes `\"` and `\\` inside quotes as escapes and
/// `split_args` doesn't, so where they'd differ the arguments are joined again. A quote
/// that an escape leaves open (`"C:\Games\"`) means the escape wasn't meant: the text is
/// kept, which `split_args` reads as typed.
fn jvm_args(text: &str) -> String {
    match split_like_prism(text) {
        Some(args) if args != split_args(text) => join_args(&args),
        _ => text.to_string(),
    }
}

/// Prism's `Commandline::splitArgs` (`launcher/Commandline.cpp`), the same on every
/// platform: whitespace outside quotes separates arguments, `"` or `'` quote, and inside
/// quotes a backslash escapes only the matching quote or another backslash; empty
/// arguments are dropped. `None` if a quote is left open.
fn split_like_prism(text: &str) -> Option<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match quote {
            Some(q) if c == '\\' && chars.peek().is_some_and(|&n| n == q || n == '\\') => {
                current.extend(chars.next());
            }
            Some(q) if c == q => quote = None,
            Some(_) => current.push(c),
            None if c.is_whitespace() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            None if c == '"' || c == '\'' => quote = Some(c),
            None => current.push(c),
        }
    }
    if quote.is_some() {
        return None;
    }
    if !current.is_empty() {
        args.push(current);
    }
    Some(args)
}

fn is_true(values: &HashMap<String, String>, key: &str) -> bool {
    values
        .get(key)
        .is_some_and(|v| v.eq_ignore_ascii_case("true") || v == "1")
}

/// Reads the instances of a Prism (or MultiMC) data folder.
pub fn read(root: &Path, source: Source) -> Launcher {
    let mut launcher = Launcher::new(source, root);
    let cfg = config_files(source)
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
        .and_then(|path| read_text(&path, 1024 * 1024).ok())
        .map(|text| parse_cfg(&text))
        .unwrap_or_default();
    let dir = |key: &str, default: &str| {
        let value = cfg
            .get(key)
            .map(String::as_str)
            .filter(|v| !v.is_empty())
            .unwrap_or(default);
        let path = PathBuf::from(value);
        if path.is_absolute() {
            path
        } else {
            root.join(path)
        }
    };
    let instances_dir = dir("InstanceDir", "instances");
    let icons_dir = dir("IconsDir", "icons");
    for dir in subfolders(&instances_dir) {
        if dir.join("instance.cfg").is_file() {
            match read_instance(&dir, source, Some(&icons_dir)) {
                Ok(found) => launcher.instances.push(found),
                Err(err) => launcher.problems.push(err),
            }
        }
    }
    launcher
}

/// Reads one instance folder (with `instance.cfg`).
pub fn read_instance(
    dir: &Path,
    source: Source,
    icons_dir: Option<&Path>,
) -> Result<Found, String> {
    let cfg = parse_cfg(&read_text(&dir.join("instance.cfg"), 1024 * 1024)?);
    let name = cfg
        .get("name")
        .filter(|n| !n.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| folder_name(dir));
    let (dot, plain) = (dir.join(".minecraft"), dir.join("minecraft"));
    let game_dir = match source {
        Source::MultiMc => {
            if plain.exists() && !dot.exists() {
                plain
            } else {
                dot
            }
        }
        _ => {
            if dot.exists() && !plain.exists() {
                dot
            } else {
                plain
            }
        }
    };
    let mut notes = Vec::new();
    let (version, loader) = if dir.join("mmc-pack.json").is_file() {
        let pack = read_json(&dir.join("mmc-pack.json"), 4 * 1024 * 1024)?;
        components(&pack, &mut notes)?
    } else if let Some(version) = cfg
        .get("IntendedVersion")
        .or_else(|| cfg.get("MinecraftVersion"))
    {
        // MultiMC's instances from before components.
        let loader = cfg
            .get("ForgeVersion")
            .filter(|v| !v.is_empty())
            .map(|v| WantedLoader::new(LoaderKind::Forge, Some(v)));
        (WantedVersion::Exact(version.clone()), loader)
    } else {
        return Err(format!(
            "{}: no Minecraft version in mmc-pack.json or instance.cfg",
            dir.display()
        ));
    };
    let mut found = Found::new(source, &name, game_dir, version);
    found.loader = loader;
    found.notes = notes;
    if is_true(&cfg, "OverrideMemory") {
        let number = |key: &str| cfg.get(key).and_then(|v| v.trim().parse::<u64>().ok());
        found.set_memory(number("MinMemAlloc"), number("MaxMemAlloc"));
    }
    if is_true(&cfg, "OverrideJavaArgs") {
        found.set_jvm_args(cfg.get("JvmArgs").map(|args| jvm_args(args)).as_deref());
    }
    if is_true(&cfg, "OverrideJavaLocation") {
        found.note(
            "The instance's own Java isn't carried over; Mojang's Java for the version is used.",
        );
    }
    if let (Some(key), Some(icons_dir)) = (cfg.get("iconKey"), icons_dir) {
        found.icon = icon(icons_dir, key);
    }
    Ok(found)
}

/// The Minecraft version and loader from `mmc-pack.json`'s components.
fn components(
    pack: &Value,
    notes: &mut Vec<String>,
) -> Result<(WantedVersion, Option<WantedLoader>), String> {
    let list = pack
        .get("components")
        .and_then(Value::as_array)
        .ok_or("mmc-pack.json has no components")?;
    let mut game = None;
    let mut loader = None;
    for component in list {
        if component.get("disabled").and_then(Value::as_bool) == Some(true) {
            continue;
        }
        let uid = component
            .get("uid")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let version = component
            .get("version")
            .or_else(|| component.get("cachedVersion"))
            .and_then(Value::as_str);
        let kind = match uid {
            "net.minecraft" => {
                game = version.map(str::to_string);
                continue;
            }
            "net.fabricmc.fabric-loader" => LoaderKind::Fabric,
            "org.quiltmc.quilt-loader" => LoaderKind::Quilt,
            "net.minecraftforge" => LoaderKind::Forge,
            "net.neoforged" => LoaderKind::NeoForge,
            "com.mumfrey.liteloader" => {
                notes.push("LiteLoader isn't a loader Lodestone Launcher installs; LiteLoader mods won't load.".into());
                continue;
            }
            _ => continue,
        };
        loader = Some(WantedLoader::new(kind, version));
    }
    let game = game.ok_or("mmc-pack.json has no net.minecraft component")?;
    // NeoForge for 1.20.1 is a Forge 47.1 fork; this launcher runs it as Forge.
    if let Some(l) = &mut loader
        && l.kind == LoaderKind::NeoForge
        && game == "1.20.1"
    {
        l.kind = LoaderKind::Forge;
        notes
            .push("NeoForge for Minecraft 1.20.1 runs as Forge 47.1 in Lodestone Launcher.".into());
    }
    Ok((WantedVersion::Exact(game), loader))
}

/// The icon file for `key` in `icons_dir` (or a folder in it), if it is one the image
/// decoder reads.
fn icon(icons_dir: &Path, key: &str) -> Option<IconSource> {
    if key.is_empty() || key == "default" || key.contains(['/', '\\']) || key.starts_with('.') {
        return None;
    }
    let mut dirs = vec![icons_dir.to_path_buf()];
    dirs.extend(subfolders(icons_dir));
    for dir in dirs {
        for ext in ["png", "jpg", "jpeg", "gif", "webp"] {
            let path = dir.join(format!("{key}.{ext}"));
            if let Some(path) = super::file_inside(icons_dir, &path) {
                return Some(IconSource::File(path));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cfg_formats() {
        let prism = parse_cfg(
            "[General]\nConfigVersion=1.3\nname=\"My Pack, v2\"\nJvmArgs=\"-Dfoo=bar -Dq=\\\"x\\\"\"\nOverrideMemory=true\nnotes=@@home\n\n[Other]\nname=wrong\n",
        );
        assert_eq!(prism["name"], "My Pack, v2");
        assert_eq!(prism["JvmArgs"], "-Dfoo=bar -Dq=\"x\"");
        assert_eq!(prism["notes"], "@home");
        assert!(is_true(&prism, "OverrideMemory"));
        let multimc = parse_cfg("InstanceType=OneSix\nname=Old\\#1\nnotes=a\\nb\n");
        assert_eq!(multimc["name"], "Old#1");
        assert_eq!(multimc["notes"], "a\nb");
    }

    /// An `instance.cfg` line's `JvmArgs` as imported, and how the launch splits it.
    fn imported(line: &str) -> (String, Vec<String>) {
        let text = jvm_args(&parse_cfg(line)["JvmArgs"]);
        let args = split_args(&text);
        (text, args)
    }

    #[test]
    fn windows_paths_in_jvm_args() {
        // As Prism writes it, and as typed by hand: the text is kept.
        for line in [
            r#"[General]\nConfigVersion=1.3\nJvmArgs=-Dfoo=\"C:\\Path With Spaces\\x\" -Xss2m"#,
            r#"JvmArgs=-Dfoo="C:\Path With Spaces\x" -Xss2m"#,
        ] {
            let line = line.replace(r"\n", "\n");
            assert_eq!(
                imported(&line),
                (
                    r#"-Dfoo="C:\Path With Spaces\x" -Xss2m"#.into(),
                    vec![r"-Dfoo=C:\Path With Spaces\x".into(), "-Xss2m".into()]
                )
            );
        }
        // UNC paths: unquoted, Prism keeps backslashes as they are.
        for line in [
            r"JvmArgs=-Dunc=\\\\server\\share\\dir -Xss2m",
            r"JvmArgs=-Dunc=\\server\share\dir -Xss2m",
        ] {
            assert_eq!(imported(line).1, [r"-Dunc=\\server\share\dir", "-Xss2m"]);
        }
        // Quoted, Prism needs `\\` for each of the two leading backslashes.
        assert_eq!(
            imported(r#"JvmArgs="\"-Dunc=\\\\\\\\server\\share x\" -Xss2m""#),
            (
                r#""-Dunc=\\server\share x" -Xss2m"#.into(),
                vec![r"-Dunc=\\server\share x".into(), "-Xss2m".into()]
            )
        );
        // A trailing backslash: escaped as Prism needs it, or typed as it is.
        for line in [
            r#"JvmArgs=-Ddir=\"C:\\My Games\\\\\" -Xss2m"#,
            r#"JvmArgs=-Ddir="C:\My Games\" -Xss2m"#,
        ] {
            assert_eq!(imported(line).1, [r"-Ddir=C:\My Games\", "-Xss2m"]);
        }
    }

    #[test]
    fn posix_quoting_in_jvm_args() {
        assert_eq!(
            imported(r#"JvmArgs=-Dq=\"say \\\"hi\\\"\" '-Dp=/Users/me/My Pack' a\\ b"#),
            (
                r#""-Dq=say "'"'"hi"'"'"" "-Dp=/Users/me/My Pack" a\ b"#.into(),
                vec![
                    r#"-Dq=say "hi""#.into(),
                    "-Dp=/Users/me/My Pack".into(),
                    r"a\".into(),
                    "b".into()
                ]
            )
        );
        // Text that splits the same either way is kept as it was.
        assert_eq!(
            imported(r#"JvmArgs=-Xss2m -Dp=\"/a b\" -Dq='it''s'"#).0,
            r#"-Xss2m -Dp="/a b" -Dq='it''s'"#
        );
    }
}
