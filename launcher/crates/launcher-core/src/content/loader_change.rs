//! What changing an instance's mod loader does to its game folder: enabled mods made for
//! another loader stop loading, and worlds that used modded blocks or items can lose
//! them. The Settings tab asks before such a change, and can turn those mods off (with
//! `.disabled`, so it can be undone).

use std::{cmp::Ordering, fs};

use lodestone_minecraft::versions::compare_versions;

use super::{
    ContentError, ContentKind, InstanceDirs,
    files::set_mod_enabled,
    scan::{self, ModFormat},
};
use crate::instances::Loader;

/// Whether a mod with metadata for `formats` loads on `loader` for `game_version`, or
/// `None` when the jar says nothing about its loader (unknown jars are left alone).
///
/// Quilt loads Fabric mods. Forge reads `mods.toml` and `mcmod.info`; NeoForge reads
/// `neoforge.mods.toml`, and before 1.20.5 also `mods.toml`.
pub fn loads_on(formats: &[ModFormat], loader: Loader, game_version: &str) -> Option<bool> {
    if formats.is_empty() {
        return None;
    }
    let neoforge_reads_mods_toml = compare_versions(game_version, "1.20.5") == Ordering::Less
        && game_version.starts_with("1.");
    Some(formats.iter().any(|format| {
        matches!(
            (loader, format),
            (Loader::Fabric, ModFormat::Fabric)
                | (Loader::Quilt, ModFormat::Fabric | ModFormat::Quilt)
                | (Loader::Forge, ModFormat::Forge)
                | (Loader::NeoForge, ModFormat::NeoForge)
        ) || (loader == Loader::NeoForge && *format == ModFormat::Forge && neoforge_reads_mods_toml)
    }))
}

/// A mod that won't load after the change.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MismatchedMod {
    pub file_name: String,
    pub name: String,
    /// The loaders it is made for, such as "Fabric" or "Forge/NeoForge".
    pub made_for: String,
}

/// What a loader change would leave behind.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoaderChangeCheck {
    pub mismatched: Vec<MismatchedMod>,
    /// Worlds in `saves/`.
    pub worlds: usize,
    /// Whether the worlds may lose modded blocks: the change is away from a mod loader.
    pub worlds_at_risk: bool,
}

impl LoaderChangeCheck {
    /// Whether the change needs the user's confirmation.
    pub fn needs_confirmation(&self) -> bool {
        !self.mismatched.is_empty() || (self.worlds > 0 && self.worlds_at_risk)
    }
}

fn format_label(format: ModFormat) -> &'static str {
    match format {
        ModFormat::Fabric => "Fabric",
        ModFormat::Quilt => "Quilt",
        ModFormat::Forge => "Forge",
        ModFormat::NeoForge => "NeoForge",
    }
}

/// The worlds in the game folder: folders in `saves/` with a `level.dat`.
pub fn count_worlds(dirs: &InstanceDirs) -> usize {
    fs::read_dir(dirs.game.join("saves"))
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
                .filter(|e| e.path().join("level.dat").is_file())
                .count()
        })
        .unwrap_or(0)
}

/// Checks a change from `from` to `to` for an instance of `game_version`.
pub fn check(
    dirs: &InstanceDirs,
    from: Loader,
    to: Loader,
    game_version: &str,
) -> Result<LoaderChangeCheck, ContentError> {
    if from == to {
        return Ok(LoaderChangeCheck::default());
    }
    let mut mismatched = Vec::new();
    for file in scan::scan(dirs, ContentKind::Mod)? {
        if !file.enabled_by_name() {
            continue;
        }
        let fits = match to {
            // Vanilla loads no mods at all.
            Loader::None => (!file.meta.mod_formats.is_empty()).then_some(false),
            _ => loads_on(&file.meta.mod_formats, to, game_version),
        };
        if fits == Some(false) {
            let mut made_for: Vec<&str> = file
                .meta
                .mod_formats
                .iter()
                .map(|f| format_label(*f))
                .collect();
            made_for.dedup();
            mismatched.push(MismatchedMod {
                name: file.display_name(),
                made_for: made_for.join("/"),
                file_name: file.file_name,
            });
        }
    }
    Ok(LoaderChangeCheck {
        mismatched,
        worlds: count_worlds(dirs),
        worlds_at_risk: from != Loader::None,
    })
}

/// Turns the mods off by adding `.disabled`. Returns the new names.
pub fn disable_mods(
    dirs: &InstanceDirs,
    file_names: &[String],
) -> Result<Vec<String>, ContentError> {
    file_names
        .iter()
        .map(|name| set_mod_enabled(dirs, name, false))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::scan::tests::write_zip;

    #[test]
    fn mismatch_matrix() {
        use Loader::{Fabric, Forge, NeoForge, Quilt};
        use ModFormat as F;
        let cases: &[(&[ModFormat], Loader, &str, Option<bool>)] = &[
            (&[F::Fabric], Fabric, "1.20.1", Some(true)),
            (&[F::Fabric], Quilt, "1.20.1", Some(true)),
            (&[F::Quilt], Quilt, "1.20.1", Some(true)),
            (&[F::Quilt], Fabric, "1.20.1", Some(false)),
            (&[F::Fabric], Forge, "1.20.1", Some(false)),
            (&[F::Fabric], NeoForge, "1.21.1", Some(false)),
            (&[F::Forge], Fabric, "1.20.1", Some(false)),
            (&[F::Forge], Forge, "1.20.1", Some(true)),
            (&[F::Forge], NeoForge, "1.21.1", Some(false)),
            (&[F::Forge], NeoForge, "1.20.4", Some(true)),
            (&[F::NeoForge], NeoForge, "1.21.1", Some(true)),
            (&[F::NeoForge], Forge, "1.21.1", Some(false)),
            (&[F::NeoForge, F::Forge], Forge, "1.21.1", Some(true)),
            (&[F::Fabric, F::Forge], Forge, "1.20.1", Some(true)),
            (&[], Fabric, "1.20.1", None),
            (&[], Forge, "1.20.1", None),
        ];
        for (formats, loader, version, expected) in cases {
            assert_eq!(
                loads_on(formats, *loader, version),
                *expected,
                "{formats:?} on {loader:?} {version}"
            );
        }
    }

    fn setup() -> (tempfile::TempDir, InstanceDirs) {
        let dir = tempfile::tempdir().unwrap();
        let dirs = InstanceDirs::new(dir.path());
        let mods = dirs.content_dir(ContentKind::Mod);
        write_zip(
            &mods.join("sodium.jar"),
            &[("fabric.mod.json", br#"{"id": "sodium", "name": "Sodium"}"#)],
        );
        write_zip(
            &mods.join("qsl.jar"),
            &[("quilt.mod.json", br#"{"quilt_loader": {"id": "qsl"}}"#)],
        );
        write_zip(
            &mods.join("jei.jar"),
            &[(
                "META-INF/mods.toml",
                b"[[mods]]\nmodId=\"jei\"\ndisplayName=\"JEI\"\n",
            )],
        );
        write_zip(
            &mods.join("off.jar.disabled"),
            &[("fabric.mod.json", br#"{"id": "off"}"#)],
        );
        write_zip(&mods.join("mystery.jar"), &[("a.class", b"")]);
        fs::write(mods.join("broken.jar"), b"not a zip").unwrap();
        (dir, dirs)
    }

    fn names(check: &LoaderChangeCheck) -> Vec<&str> {
        check
            .mismatched
            .iter()
            .map(|m| m.file_name.as_str())
            .collect()
    }

    #[test]
    fn lists_the_enabled_mods_that_stop_loading() {
        let (_dir, dirs) = setup();
        let to_forge = check(&dirs, Loader::Fabric, Loader::Forge, "1.20.1").unwrap();
        assert_eq!(names(&to_forge), ["qsl.jar", "sodium.jar"]);
        assert_eq!(to_forge.mismatched[1].name, "Sodium");
        assert_eq!(to_forge.mismatched[1].made_for, "Fabric");
        let to_fabric = check(&dirs, Loader::Forge, Loader::Fabric, "1.20.1").unwrap();
        assert_eq!(names(&to_fabric), ["jei.jar", "qsl.jar"]);
        let to_quilt = check(&dirs, Loader::Fabric, Loader::Quilt, "1.20.1").unwrap();
        assert_eq!(names(&to_quilt), ["jei.jar"]);
        let to_vanilla = check(&dirs, Loader::Fabric, Loader::None, "1.20.1").unwrap();
        assert_eq!(names(&to_vanilla), ["jei.jar", "qsl.jar", "sodium.jar"]);
        assert_eq!(
            check(&dirs, Loader::Fabric, Loader::Fabric, "1.20.1").unwrap(),
            LoaderChangeCheck::default()
        );
    }

    #[test]
    fn worlds_and_turning_mods_off() {
        let (_dir, dirs) = setup();
        let check_now = |from, to| check(&dirs, from, to, "1.20.1").unwrap();
        assert_eq!(check_now(Loader::Fabric, Loader::Forge).worlds, 0);
        fs::create_dir_all(dirs.game.join("saves/World")).unwrap();
        fs::write(dirs.game.join("saves/World/level.dat"), b"x").unwrap();
        fs::create_dir_all(dirs.game.join("saves/not a world")).unwrap();
        let away = check_now(Loader::Fabric, Loader::None);
        assert_eq!((away.worlds, away.worlds_at_risk), (1, true));
        // from vanilla to a loader, worlds lose nothing
        let to_fabric = check_now(Loader::None, Loader::Fabric);
        assert!(!to_fabric.worlds_at_risk);
        assert!(
            to_fabric
                .mismatched
                .iter()
                .all(|m| m.file_name != "sodium.jar")
        );

        let changed = disable_mods(&dirs, &["sodium.jar".into(), "qsl.jar".into()]).unwrap();
        assert_eq!(changed, ["sodium.jar.disabled", "qsl.jar.disabled"]);
        assert_eq!(
            names(&check_now(Loader::Fabric, Loader::Forge)),
            Vec::<&str>::new()
        );
        assert!(
            check_now(Loader::Fabric, Loader::Forge).needs_confirmation(),
            "the world still asks"
        );
    }
}
