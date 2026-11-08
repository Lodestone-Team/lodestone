//! Asset indexes and where their objects go.
//!
//! Objects are stored by hash under `assets/objects/<first two hex digits>/<hash>` and
//! downloaded from `resources.download.minecraft.net/<xx>/<hash>`. Old versions read
//! assets by name instead:
//! - a `virtual` index (1.6 to 1.7.2, the `legacy` index) is copied by name into
//!   `assets/virtual/<index id>/`, which is `${game_assets}`;
//! - a `map_to_resources` index (before 1.6, `pre-1.6`) is copied by name into the
//!   instance's `resources/` folder.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use super::libraries::{LibraryError, safe_relative_path};

/// Where asset objects are downloaded from.
pub const RESOURCES_URL: &str = "https://resources.download.minecraft.net";

#[derive(Clone, Debug, Deserialize)]
pub struct AssetIndex {
    #[serde(default, rename = "virtual")]
    pub is_virtual: bool,
    #[serde(default)]
    pub map_to_resources: bool,
    pub objects: BTreeMap<String, AssetObject>,
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct AssetObject {
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AssetError {
    #[error("the asset hash {0:?} is not a SHA-1")]
    BadHash(String),
    #[error(transparent)]
    Path(#[from] LibraryError),
}

impl AssetIndex {
    pub fn parse(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }

    /// Where the legacy copies go, if this index needs them.
    pub fn legacy_layout(&self) -> Option<LegacyLayout> {
        if self.map_to_resources {
            Some(LegacyLayout::Resources)
        } else if self.is_virtual {
            Some(LegacyLayout::Virtual)
        } else {
            None
        }
    }
}

/// How an old version reads assets by name.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegacyLayout {
    /// `assets/virtual/<index id>/<name>`.
    Virtual,
    /// `<game dir>/resources/<name>`.
    Resources,
}

/// Checks that `hash` is 40 lowercase or uppercase hex digits, so it is safe in a path.
pub fn check_hash(hash: &str) -> Result<(), AssetError> {
    if hash.len() == 40 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(AssetError::BadHash(hash.to_string()))
    }
}

/// `objects/<xx>/<hash>`, relative to the assets directory.
pub fn object_path(hash: &str) -> Result<PathBuf, AssetError> {
    check_hash(hash)?;
    let hash = hash.to_ascii_lowercase();
    Ok(Path::new("objects").join(&hash[..2]).join(hash))
}

/// The download URL of an object under `base` (normally [`RESOURCES_URL`]).
pub fn object_url(base: &str, hash: &str) -> Result<String, AssetError> {
    check_hash(hash)?;
    let hash = hash.to_ascii_lowercase();
    Ok(format!(
        "{}/{}/{hash}",
        base.trim_end_matches('/'),
        &hash[..2]
    ))
}

/// Where a legacy copy of the asset `name` goes, given the assets directory, the
/// index id and the game directory. Names come from the index file, so they must be
/// safe relative paths.
pub fn legacy_path(
    layout: LegacyLayout,
    assets_dir: &Path,
    index_id: &str,
    game_dir: &Path,
    name: &str,
) -> Result<PathBuf, AssetError> {
    let name = safe_relative_path(name)?;
    Ok(match layout {
        LegacyLayout::Virtual => virtual_dir(assets_dir, index_id)?.join(name),
        LegacyLayout::Resources => game_dir.join("resources").join(name),
    })
}

/// `assets/virtual/<index id>`.
pub fn virtual_dir(assets_dir: &Path, index_id: &str) -> Result<PathBuf, AssetError> {
    Ok(assets_dir
        .join("virtual")
        .join(safe_relative_path(index_id)?))
}

/// `${game_assets}`: the folder an old version reads assets from by name.
pub fn game_assets_dir(
    layout: Option<LegacyLayout>,
    assets_dir: &Path,
    index_id: &str,
    game_dir: &Path,
) -> Result<PathBuf, AssetError> {
    Ok(match layout {
        Some(LegacyLayout::Resources) => game_dir.join("resources"),
        Some(LegacyLayout::Virtual) | None => virtual_dir(assets_dir, index_id)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn index(name: &str) -> AssetIndex {
        let path = format!("{}/testdata/assets/{name}.json", env!("CARGO_MANIFEST_DIR"));
        AssetIndex::parse(&std::fs::read(path).unwrap()).unwrap()
    }

    #[test]
    fn object_paths_and_urls() {
        let hash = "bdf48ef6b5d0d23bbb02e17d04865216179f510a";
        assert_eq!(
            object_path(hash).unwrap(),
            Path::new("objects/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a")
        );
        assert_eq!(
            object_url(RESOURCES_URL, hash).unwrap(),
            "https://resources.download.minecraft.net/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a"
        );
        assert_eq!(
            object_url("http://127.0.0.1:9/res/", hash).unwrap(),
            "http://127.0.0.1:9/res/bd/bdf48ef6b5d0d23bbb02e17d04865216179f510a"
        );
        for bad in [
            "",
            "../../etc/passwd",
            "zz48ef6b5d0d23bbb02e17d04865216179f510a0",
            "abc",
        ] {
            assert!(object_path(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn layouts_of_the_recorded_indexes() {
        assert_eq!(index("5").legacy_layout(), None);
        assert_eq!(index("legacy").legacy_layout(), Some(LegacyLayout::Virtual));
        assert_eq!(
            index("pre-1.6").legacy_layout(),
            Some(LegacyLayout::Resources)
        );
        assert_eq!(index("5").objects.len(), 4);
    }

    #[test]
    fn legacy_paths() {
        let assets = Path::new("/data/assets");
        let game = Path::new("/data/instances/old/minecraft");
        assert_eq!(
            legacy_path(
                LegacyLayout::Virtual,
                assets,
                "legacy",
                game,
                "icons/icon_16x16.png"
            )
            .unwrap(),
            Path::new("/data/assets/virtual/legacy/icons/icon_16x16.png")
        );
        assert_eq!(
            legacy_path(
                LegacyLayout::Resources,
                assets,
                "pre-1.6",
                game,
                "sound/step/grass1.ogg"
            )
            .unwrap(),
            Path::new("/data/instances/old/minecraft/resources/sound/step/grass1.ogg")
        );
        assert!(legacy_path(LegacyLayout::Virtual, assets, "legacy", game, "../../x").is_err());
        assert!(legacy_path(LegacyLayout::Resources, assets, "pre-1.6", game, "/x").is_err());
        assert!(virtual_dir(assets, "../escape").is_err());
        assert_eq!(
            game_assets_dir(Some(LegacyLayout::Resources), assets, "pre-1.6", game).unwrap(),
            game.join("resources")
        );
        assert_eq!(
            game_assets_dir(Some(LegacyLayout::Virtual), assets, "legacy", game).unwrap(),
            Path::new("/data/assets/virtual/legacy")
        );
    }
}
