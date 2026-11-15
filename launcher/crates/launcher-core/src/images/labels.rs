//! What each Launcher action calls the content image it makes.
//!
//! Labels are data that travel inside images (another Lodestone shows them as they were
//! written), not interface text, so they are plain English, as the core writes its own.
//! The history shows them through `display_text`; names in them (files, packs, servers)
//! may hold any character, and [`lodestone_image::lineage::join_labels`] turns control
//! characters into spaces and cuts the whole to what an image may hold.

use crate::content::ContentKind;
use crate::content::install::InstallOutcome;
use crate::instances::Loader;

/// The first image of an instance that had none when the Launcher began recording it.
pub const INITIAL_LABEL: &str = "Content when the Launcher began recording";
/// An image the check before a launch records: content changed outside the Launcher.
pub const OUTSIDE_LABEL: &str = "Changed by hand";
/// The record before a modpack update, of what the update is about to change.
pub const BEFORE_MODPACK_UPDATE: &str = "Before a modpack update";
/// The record before a server's update is applied, of what it is about to change.
pub const BEFORE_SERVER_UPDATE: &str = "Before a server update";

/// A new instance.
pub fn created(name: &str) -> String {
    format!("Created {name}")
}

/// A copy of the instance `from` (duplicated, or copied with another version or loader).
pub fn copied(from: &str) -> String {
    format!("Copied from {from}")
}

/// An instance imported from another launcher.
pub fn imported(launcher: &str) -> String {
    format!("Imported from {launcher}")
}

/// An instance made from a modpack.
pub fn installed_pack(title: &str, version: &str) -> String {
    with_version(format!("Installed {title}"), version)
}

/// An instance made from a server's mods.
pub fn created_from_server(server: &str) -> String {
    format!("Created from {server}")
}

/// An instance made from a snapshot.
pub const FROM_SNAPSHOT: &str = "Made from a snapshot";

/// What an install from Modrinth did: "Added <title>", or "Updated <title>" when it took an
/// older file's place, or counts for several files.
pub fn installed(outcome: &InstallOutcome, kind: ContentKind) -> Option<String> {
    let replaced = !outcome.removed.is_empty();
    match outcome.installed.as_slice() {
        [] => None,
        [(title, file)] => {
            let name = if title.trim().is_empty() { file } else { title };
            Some(format!(
                "{} {name}",
                if replaced { "Updated" } else { "Added" }
            ))
        }
        many => Some(count(
            if replaced { "Updated" } else { "Added" },
            many.len(),
            kind,
        )),
    }
}

/// One file added.
pub fn added(name: &str) -> String {
    format!("Added {name}")
}

/// Files added from the computer.
pub fn added_files(files: &[String], kind: ContentKind) -> Option<String> {
    match files {
        [] => None,
        [file] => Some(added(file)),
        many => Some(count("Added", many.len(), kind)),
    }
}

/// "Added 3 mods", "Updated 2 resource packs".
pub fn count(verb: &str, n: usize, kind: ContentKind) -> String {
    let noun = match (kind, n) {
        (ContentKind::Mod, 1) => "mod",
        (ContentKind::Mod, _) => "mods",
        (ContentKind::ResourcePack, 1) => "resource pack",
        (ContentKind::ResourcePack, _) => "resource packs",
        (ContentKind::ShaderPack, 1) => "shader pack",
        (ContentKind::ShaderPack, _) => "shader packs",
    };
    format!("{verb} {n} {noun}")
}

/// A file removed from the instance.
pub fn removed(file: &str) -> String {
    format!("Removed {file}")
}

/// Several files removed at once.
pub fn removed_many(files: &[String], kind: ContentKind) -> Option<String> {
    match files {
        [] => None,
        [file] => Some(removed(file)),
        many => Some(count("Removed", many.len(), kind)),
    }
}

/// A file turned on or off.
pub fn enabled(file: &str, on: bool) -> String {
    format!("{} {file}", if on { "Enabled" } else { "Disabled" })
}

/// The resource pack list or order changed.
pub const RESOURCE_PACKS_CHANGED: &str = "Changed the resource packs";

/// The shader pack in use changed.
pub fn shader(file: Option<&str>) -> String {
    match file {
        Some(file) => format!("Turned on the shader {file}"),
        None => "Turned off shaders".into(),
    }
}

/// The game the instance plays changed in place: `minecraft` is the new Minecraft
/// version when it changed, `loader` the new loader and its version when either changed.
/// `None` when neither did.
pub fn game_changed(minecraft: Option<&str>, loader: Option<(Loader, Option<&str>)>) -> Option<String> {
    let loader_text = |loader: Loader, version: Option<&str>| match version {
        Some(version) if loader != Loader::None => format!("{} {version}", loader.label()),
        _ => loader.label().to_string(),
    };
    Some(match (minecraft, loader) {
        (None, None) => return None,
        (Some(minecraft), None) => format!("Changed Minecraft to {minecraft}"),
        (Some(minecraft), Some((loader, version))) => format!(
            "Changed Minecraft to {minecraft} with {}",
            loader_text(loader, version)
        ),
        (None, Some((loader, Some(version)))) if loader != Loader::None => {
            format!("Changed {} to {version}", loader.label())
        }
        (None, Some((loader, version))) => format!("Changed to {}", loader_text(loader, version)),
    })
}

/// The recommended memory changed (a pack's or a server's update brings it).
pub const MEMORY_CHANGED: &str = "Changed the recommended memory";

/// The instance's pack updated to another version.
pub fn pack_updated(title: &str, version: &str) -> String {
    with_version(format!("Updated {title} to"), version)
}

/// A modpack's files added to the instance.
pub fn addon_added(title: &str) -> String {
    format!("Added the modpack {title}")
}

/// An added modpack's files taken off again.
pub fn addon_removed(title: &str) -> String {
    format!("Removed the modpack {title}")
}

/// Content installed from a share code.
pub const SHARE_CODE: &str = "Installed from a share code";

/// The instance's mods put in step with a server's.
pub fn server_updated(server: &str) -> String {
    format!("Updated from {server}")
}

/// The instance went back to one of its snapshots.
pub fn reverted(snapshot: &str) -> String {
    format!("Went back to snapshot \u{201c}{snapshot}\u{201d}")
}

/// "Find it for me" ended: the mods it found stay off, or every mod it turned off is back.
pub fn mod_finder_ended(kept_off: &[String]) -> String {
    match kept_off {
        [] => "Find it for me put the mods back".into(),
        [file] => format!("Find it for me turned off {file}"),
        many => format!("Find it for me turned off {} mods", many.len()),
    }
}

fn with_version(text: String, version: &str) -> String {
    let version = version.trim();
    if version.is_empty() {
        text.trim_end_matches(" to").to_string()
    } else {
        format!("{text} {version}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::install::InstallOutcome;

    fn outcome(installed: &[(&str, &str)], removed: &[&str]) -> InstallOutcome {
        InstallOutcome {
            installed: installed
                .iter()
                .map(|(t, f)| (t.to_string(), f.to_string()))
                .collect(),
            removed: removed.iter().map(|f| f.to_string()).collect(),
            skipped: Vec::new(),
        }
    }

    #[test]
    fn installs_say_what_they_did() {
        let kind = ContentKind::Mod;
        assert_eq!(installed(&outcome(&[], &[]), kind), None);
        assert_eq!(
            installed(&outcome(&[("JEI", "jei.jar")], &[]), kind).as_deref(),
            Some("Added JEI")
        );
        assert_eq!(
            installed(&outcome(&[("JEI", "jei-2.jar")], &["jei-1.jar"]), kind).as_deref(),
            Some("Updated JEI")
        );
        assert_eq!(
            installed(&outcome(&[(" ", "x.jar")], &[]), kind).as_deref(),
            Some("Added x.jar")
        );
        assert_eq!(
            installed(
                &outcome(&[("a", "a"), ("b", "b")], &[]),
                ContentKind::ResourcePack
            )
            .as_deref(),
            Some("Added 2 resource packs")
        );
        assert_eq!(
            installed(&outcome(&[("a", "a"), ("b", "b")], &["c"]), kind).as_deref(),
            Some("Updated 2 mods")
        );
        assert_eq!(
            added_files(&["x.zip".into()], ContentKind::ShaderPack).as_deref(),
            Some("Added x.zip")
        );
        assert_eq!(
            count("Removed", 1, ContentKind::ShaderPack),
            "Removed 1 shader pack"
        );
    }

    #[test]
    fn other_labels() {
        assert_eq!(enabled("a.jar", false), "Disabled a.jar");
        let game = |minecraft, loader| game_changed(minecraft, loader).unwrap_or_default();
        assert_eq!(
            game(None, Some((Loader::Fabric, Some("0.16.10")))),
            "Changed Fabric to 0.16.10"
        );
        assert_eq!(game(None, Some((Loader::None, None))), "Changed to Vanilla");
        assert_eq!(game(None, Some((Loader::Quilt, None))), "Changed to Quilt");
        assert_eq!(game(Some("1.21.1"), None), "Changed Minecraft to 1.21.1");
        assert_eq!(
            game(Some("1.21.1"), Some((Loader::NeoForge, Some("21.1.5")))),
            "Changed Minecraft to 1.21.1 with NeoForge 21.1.5"
        );
        assert_eq!(
            game(Some("1.21.1"), Some((Loader::None, None))),
            "Changed Minecraft to 1.21.1 with Vanilla"
        );
        assert_eq!(game_changed(None, None), None);
        assert_eq!(pack_updated("FO", "6.4.0"), "Updated FO to 6.4.0");
        assert_eq!(pack_updated("FO", ""), "Updated FO");
        assert_eq!(installed_pack("FO", ""), "Installed FO");
        assert_eq!(
            reverted("Before"),
            "Went back to snapshot \u{201c}Before\u{201d}"
        );
        assert_eq!(shader(None), "Turned off shaders");
    }
}
