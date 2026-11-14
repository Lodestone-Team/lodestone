//! Which mod loader a mod's metadata says it loads on. An instance's loader is fixed
//! once it is made (see [`super::copy_as`] for moving to another one), so this only
//! decides whether a file added to an instance fits it.

use std::cmp::Ordering;

use lodestone_minecraft::versions::compare_versions;

use super::scan::ModFormat;
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
