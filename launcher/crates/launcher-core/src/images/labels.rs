//! What each Launcher action calls the content image it makes.
//!
//! Labels are data that travel inside images (another Lodestone shows them as they were
//! written), not interface text, so they are plain English, as the core writes its own.
//! The history shows them through `display_text`; names in them (files, packs, servers)
//! may hold any character, and [`lodestone_image::lineage::join_labels`] turns control
//! characters into spaces and cuts the whole to what an image may hold.

use crate::content::install::InstallOutcome;
use crate::content::{ContentKind, enabled_name};
use crate::instances::Loader;

/// The first image of an instance that had none when the Launcher began recording it.
pub const INITIAL_LABEL: &str = "Content when the Launcher began recording";
/// An image the check before a launch records: content changed outside the Launcher.
pub const OUTSIDE_LABEL: &str = "Changed by hand";
/// An image the check before a launch records after a session no end was seen of (the
/// Launcher quit or died while the game ran): what the game wrote and what changed by hand
/// can't be told apart.
pub const WHILE_RUNNING_LABEL: &str = "Changed while the game ran or by hand";
/// The record before a modpack update, of what the update is about to change.
pub const BEFORE_MODPACK_UPDATE: &str = "Before a modpack update";
/// The record before a server's update is applied, of what it is about to change.
pub const BEFORE_SERVER_UPDATE: &str = "Before a server update";
/// The content record before a snapshot, when the content differs from the current image
/// only by what the game wrote while it ran (configs mods rewrite), which a session's end
/// folds into the baseline without recording.
pub const WRITTEN_WHILE_PLAYING: &str = "Changed while playing";

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

/// An instance made from a server's backup: "Download this world", or a server's
/// `.lodeimg` opened in the Launcher.
pub fn downloaded_from(server: &str) -> String {
    format!("Downloaded from {server}")
}

/// A server whose name a file doesn't say.
pub const SERVER: &str = "Server";

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

/// An install that failed or was cancelled after placing some of its files.
pub fn partly_installed(title: &str) -> String {
    let title = title.trim();
    if title.is_empty() {
        "Partly installed content".into()
    } else {
        format!("Partly installed {title}")
    }
}

/// An action that failed or was cancelled after it may have changed files: its label,
/// marked.
pub fn stopped_part_way(label: &str) -> String {
    let label = label.trim();
    if label.is_empty() {
        "Stopped part way".into()
    } else {
        format!("{label} (stopped part way)")
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

/// A file removed from the instance, by its name when on (a disabled mod's `.disabled`
/// left off).
pub fn removed(file: &str) -> String {
    format!("Removed {}", enabled_name(file))
}

/// Several files removed at once.
pub fn removed_many(files: &[String], kind: ContentKind) -> Option<String> {
    match files {
        [] => None,
        [file] => Some(removed(file)),
        many => Some(count("Removed", many.len(), kind)),
    }
}

/// A file turned on or off, by its name when on, whichever name it has on disk.
pub fn enabled(file: &str, on: bool) -> String {
    format!(
        "{} {}",
        if on { "Enabled" } else { "Disabled" },
        enabled_name(file)
    )
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
/// version when it changed, `loader` the new loader and its version when either changed,
/// and `other_loader` whether the loader itself changed (not only its version): "Changed
/// Fabric to 0.16.10" is a new Fabric, "Changed to Quilt 0.16.10" another loader. `None`
/// when nothing did.
pub fn game_changed(
    minecraft: Option<&str>,
    loader: Option<(Loader, Option<&str>)>,
    other_loader: bool,
) -> Option<String> {
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
        (None, Some((loader, Some(version)))) if loader != Loader::None && !other_loader => {
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

/// The content record before an in-place restore that puts content back, of what the
/// restore is about to replace.
pub const BEFORE_RESTORE: &str = "Before going back to a snapshot";

/// A snapshot's label when the user gives it no name.
pub const SNAPSHOT: &str = "Snapshot";

/// The snapshot after a play session (when the setting is on).
pub const AFTER_PLAYING: &str = "After playing";

/// The safety snapshot before going back to the snapshot `name`.
pub fn before_going_back(name: &str) -> String {
    format!("Before going back to \u{201c}{name}\u{201d}")
}

/// The snapshot before a modpack update.
pub fn before_pack_update(title: &str, version: &str) -> String {
    with_version(format!("Before updating {title} to"), version)
}

/// The snapshot before a server's update is applied.
pub fn before_server_update(server: &str) -> String {
    format!("Before updating from {server}")
}

/// The snapshot before a content update: one title, or a count of several.
pub fn before_content_update(titles: &[String]) -> String {
    match titles {
        [] => "Before updating content".into(),
        [title] => format!("Before updating {}", title.trim()),
        many => format!("Before updating {} mods", many.len()),
    }
}

/// The snapshot before the loader's version changes.
pub fn before_loader_change(loader: Loader, version: &str) -> String {
    format!("Before changing {} to {}", loader.label(), version.trim())
}

/// The snapshot before the instance moves to another Minecraft version or loader (its
/// worlds upgrade at the next launch, which can't be undone).
pub fn before_game_change(game_version: &str, loader: Loader, version: Option<&str>) -> String {
    let game = format!("Minecraft {}", game_version.trim());
    match (loader, version.map(str::trim).filter(|v| !v.is_empty())) {
        (Loader::None, _) => format!("Before moving to {game}"),
        (loader, Some(version)) => {
            format!("Before moving to {game} with {} {version}", loader.label())
        }
        (loader, None) => format!("Before moving to {game} with {}", loader.label()),
    }
}

/// A world backed up from the Worlds tab: a manual snapshot.
pub fn backed_up(world: &str) -> String {
    format!("Backed up {world}")
}

/// "Find it for me" ended: the mods it found stay off, or every mod it turned off is back.
/// The second records only when putting them back failed part way (with " (stopped part
/// way)"): one that succeeded records nothing ([`super::Images::search_put_back`]).
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
        // A disabled file is named as it is when on.
        assert_eq!(enabled("a.jar.disabled", true), "Enabled a.jar");
        assert_eq!(removed("a.jar.disabled"), "Removed a.jar");
        assert_eq!(
            removed_many(&["a.jar.disabled".into()], ContentKind::Mod).as_deref(),
            Some("Removed a.jar")
        );
        assert_eq!(partly_installed("JEI"), "Partly installed JEI");
        assert_eq!(partly_installed(" "), "Partly installed content");
        assert_eq!(
            stopped_part_way("Updated FO to 6.4.0"),
            "Updated FO to 6.4.0 (stopped part way)"
        );
        assert_eq!(stopped_part_way(""), "Stopped part way");
        let game = |minecraft, loader| game_changed(minecraft, loader, false).unwrap_or_default();
        assert_eq!(
            game(None, Some((Loader::Fabric, Some("0.16.10")))),
            "Changed Fabric to 0.16.10"
        );
        // Another loader with the same version string isn't a new version of it.
        assert_eq!(
            game_changed(None, Some((Loader::Quilt, Some("0.16.10"))), true).as_deref(),
            Some("Changed to Quilt 0.16.10")
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
        assert_eq!(game_changed(None, None, false), None);
        assert_eq!(pack_updated("FO", "6.4.0"), "Updated FO to 6.4.0");
        assert_eq!(pack_updated("FO", ""), "Updated FO");
        assert_eq!(installed_pack("FO", ""), "Installed FO");
        assert_eq!(
            reverted("Before"),
            "Went back to snapshot \u{201c}Before\u{201d}"
        );
        assert_eq!(shader(None), "Turned off shaders");
    }

    #[test]
    fn snapshot_labels() {
        assert_eq!(
            before_going_back("Before 1.21"),
            "Before going back to \u{201c}Before 1.21\u{201d}"
        );
        assert_eq!(
            before_pack_update("FO", "6.4.0"),
            "Before updating FO to 6.4.0"
        );
        assert_eq!(before_pack_update("FO", " "), "Before updating FO");
        assert_eq!(before_server_update("SMP"), "Before updating from SMP");
        assert_eq!(
            before_content_update(&["Sodium".into()]),
            "Before updating Sodium"
        );
        assert_eq!(
            before_content_update(&["a".into(), "b".into(), "c".into()]),
            "Before updating 3 mods"
        );
        assert_eq!(before_content_update(&[]), "Before updating content");
        assert_eq!(
            before_loader_change(Loader::Fabric, "0.16.10"),
            "Before changing Fabric to 0.16.10"
        );
        assert_eq!(
            before_game_change("1.21.1", Loader::None, None),
            "Before moving to Minecraft 1.21.1"
        );
        assert_eq!(
            before_game_change("1.21.1", Loader::Fabric, Some("0.16.10")),
            "Before moving to Minecraft 1.21.1 with Fabric 0.16.10"
        );
        assert_eq!(backed_up("New World"), "Backed up New World");
    }
}
