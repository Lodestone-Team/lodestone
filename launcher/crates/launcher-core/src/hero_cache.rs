//! Display-size copies of the images the Play tab's banner shows: screenshots,
//! slideshow images and backgrounds, scaled down to [`HERO_MAX`] and saved as JPEG in
//! `<data>/cache/hero/`. The banner never decodes an original (a 4K PNG screenshot
//! takes far longer to read than the copy): the first time an image is needed, a
//! copy is made on a background thread, and only the copy is read afterwards.
//!
//! A copy is named after a hash of its source's path, size and modification time, so
//! a changed source gets a new copy. Beside each copy a `.src` file records the
//! source's path, so copies whose source is gone can be pruned. The folder is kept
//! under [`HERO_CACHE_LIMIT`], least recently used first.

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use sha1::{Digest as _, Sha1};

use crate::{paths::DataDir, screenshots, storage};

/// The largest copy: the banner at 2× on a typical window.
pub const HERO_MAX: (u32, u32) = (1920, 1080);
/// The copies' JPEG quality.
pub const HERO_QUALITY: u8 = 85;
/// The folder is pruned to this size, oldest first.
pub const HERO_CACHE_LIMIT: u64 = 200 * 1024 * 1024;

/// The folder of display-size copies, `<data>/cache/hero`.
#[derive(Clone, Debug)]
pub struct HeroCache {
    dir: PathBuf,
}

impl HeroCache {
    pub fn new(data: &DataDir) -> Self {
        Self::at(data.cache_dir().join("hero"))
    }

    /// A cache in `dir`, for tests.
    pub fn at(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Where the copy of `source` is kept, as it is now: `None` if the source can't
    /// be read.
    fn copy_path(&self, source: &Path) -> Option<PathBuf> {
        let metadata = fs::metadata(source).ok()?;
        let modified = metadata
            .modified()
            .ok()
            .and_then(|m| m.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let mut hash = Sha1::new();
        hash.update(source.to_string_lossy().as_bytes());
        hash.update(metadata.len().to_le_bytes());
        hash.update(modified.to_le_bytes());
        let digest = hash.finalize();
        let name: String = digest[..12].iter().map(|b| format!("{b:02x}")).collect();
        Some(self.dir.join(format!("{name}.jpg")))
    }

    /// The display-size copy of `source`, made now if there is none yet. Blocking:
    /// call it on a background thread.
    pub fn copy_of(&self, source: &Path) -> Result<PathBuf, String> {
        let copy = self
            .copy_path(source)
            .ok_or_else(|| format!("{} can't be read", source.display()))?;
        if copy.is_file() {
            // Recently used: kept longest when the folder is pruned.
            if let Ok(file) = fs::File::options().write(true).open(&copy) {
                let _ = file.set_modified(SystemTime::now());
            }
            return Ok(copy);
        }
        let image = screenshots::decode(source)?;
        let jpeg = display_jpeg(image)?;
        fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        storage::write_atomic(&copy, &jpeg).map_err(|e| e.to_string())?;
        let _ = fs::write(
            copy.with_extension("src"),
            source.to_string_lossy().as_bytes(),
        );
        self.prune_to(HERO_CACHE_LIMIT);
        Ok(copy)
    }

    /// The copies and their source records, with their sizes and modification times.
    fn entries(&self) -> Vec<(PathBuf, u64, SystemTime)> {
        fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let path = entry.path();
                (path.extension()? == "jpg").then_some(())?;
                let metadata = entry.metadata().ok()?;
                let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
                Some((path, metadata.len(), modified))
            })
            .collect()
    }

    /// How much space the copies take, in bytes.
    pub fn size(&self) -> u64 {
        self.entries().iter().map(|(_, size, _)| size).sum()
    }

    /// Removes every copy; they are made again when needed.
    pub fn clear(&self) -> io::Result<()> {
        match fs::remove_dir_all(&self.dir) {
            Err(err) if err.kind() != io::ErrorKind::NotFound => Err(err),
            _ => Ok(()),
        }
    }

    /// Removes the copies whose source is gone, then the least recently used until
    /// the folder fits [`HERO_CACHE_LIMIT`].
    pub fn prune(&self) {
        for (copy, _, _) in self.entries() {
            let record = copy.with_extension("src");
            let gone = match fs::read_to_string(&record) {
                Ok(source) => !Path::new(&source).is_file(),
                Err(_) => true,
            };
            if gone {
                remove(&copy);
            }
        }
        self.prune_to(HERO_CACHE_LIMIT);
    }

    fn prune_to(&self, limit: u64) {
        let mut entries = self.entries();
        let mut total: u64 = entries.iter().map(|(_, size, _)| size).sum();
        if total <= limit {
            return;
        }
        entries.sort_by_key(|(_, _, modified)| *modified);
        for (copy, size, _) in entries {
            if total <= limit {
                break;
            }
            remove(&copy);
            total = total.saturating_sub(size);
        }
    }
}

/// Removes a copy and its source record.
fn remove(copy: &Path) {
    let _ = fs::remove_file(copy);
    let _ = fs::remove_file(copy.with_extension("src"));
}

/// `image` scaled down to fit [`HERO_MAX`], as a JPEG.
pub fn display_jpeg(image: image::DynamicImage) -> Result<Vec<u8>, String> {
    let (max_w, max_h) = HERO_MAX;
    let image = if image.width() > max_w || image.height() > max_h {
        image.resize(max_w, max_h, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    let rgb = image::DynamicImage::ImageRgb8(image.to_rgb8());
    let mut jpeg = io::Cursor::new(Vec::new());
    rgb.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(
        &mut jpeg,
        HERO_QUALITY,
    ))
    .map_err(|e| e.to_string())?;
    Ok(jpeg.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(path: &Path, w: u32, h: u32) {
        image::RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x ^ y) % 239) as u8])
        })
        .save(path)
        .unwrap();
    }

    #[test]
    fn copies_are_made_once_at_display_size_and_follow_their_source() {
        let dir = tempfile::tempdir().unwrap();
        let cache = HeroCache::at(dir.path().join("cache/hero"));
        let source = dir.path().join("shot.png");
        picture(&source, 3840, 2160);

        let copy = cache.copy_of(&source).unwrap();
        let decoded = image::open(&copy).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (1920, 1080));
        assert_eq!(cache.copy_of(&source).unwrap(), copy, "the same copy again");
        assert!(cache.size() > 0);

        // a changed source gets a new copy
        picture(&source, 1000, 500);
        let changed = cache.copy_of(&source).unwrap();
        assert_ne!(changed, copy);
        let decoded = image::open(&changed).unwrap();
        assert_eq!(
            (decoded.width(), decoded.height()),
            (1000, 500),
            "never enlarged"
        );

        // copies of a source that is gone are pruned
        fs::remove_file(&source).unwrap();
        cache.prune();
        assert_eq!(cache.size(), 0);

        picture(&source, 800, 400);
        cache.copy_of(&source).unwrap();
        cache.clear().unwrap();
        assert_eq!(cache.size(), 0);
        cache.clear().unwrap();
    }

    #[test]
    fn the_folder_is_pruned_to_its_limit_oldest_first() {
        let dir = tempfile::tempdir().unwrap();
        let cache = HeroCache::at(dir.path().join("hero"));
        let mut copies = Vec::new();
        for (i, age) in [300u64, 200, 100].into_iter().enumerate() {
            let source = dir.path().join(format!("{i}.png"));
            picture(&source, 400, 300);
            let copy = cache.copy_of(&source).unwrap();
            fs::File::options()
                .write(true)
                .open(&copy)
                .unwrap()
                .set_modified(SystemTime::now() - std::time::Duration::from_secs(age))
                .unwrap();
            copies.push(copy);
        }
        let one = fs::metadata(&copies[2]).unwrap().len();
        cache.prune_to(one);
        assert!(!copies[0].exists() && !copies[1].exists());
        assert!(copies[2].exists(), "the newest stays");
    }

    /// Times reading a 4K screenshot against reading its copy. Run with
    /// `cargo test --release -p launcher-core hero_cache -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn time_a_4k_screenshot() {
        let dir = tempfile::tempdir().unwrap();
        let cache = HeroCache::at(dir.path().join("hero"));
        let source = dir.path().join("4k.png");
        // Noise-like detail, closer to a game screenshot than a flat fill.
        image::RgbImage::from_fn(3840, 2160, |x, y| {
            let n = x.wrapping_mul(2654435761) ^ y.wrapping_mul(40503);
            image::Rgb([
                (n >> 3) as u8,
                (n >> 11) as u8 ^ (x / 9) as u8,
                (y / 5) as u8,
            ])
        })
        .save(&source)
        .unwrap();
        let time = |f: &dyn Fn()| {
            let start = std::time::Instant::now();
            for _ in 0..5 {
                f();
            }
            start.elapsed().as_secs_f64() * 1000.0 / 5.0
        };
        let original = time(&|| {
            let image = screenshots::decode(&source).unwrap();
            let _ = image.resize(1920, 1080, image::imageops::FilterType::Triangle);
        });
        let made = std::time::Instant::now();
        let copy = cache.copy_of(&source).unwrap();
        let made = made.elapsed().as_secs_f64() * 1000.0;
        let cached = time(&|| {
            let _ = screenshots::decode(&copy).unwrap();
        });
        println!(
            "4K PNG {} KB: decode + scale {original:.0} ms; copy made once in {made:.0} ms, {} KB; copy decode {cached:.0} ms",
            fs::metadata(&source).unwrap().len() / 1024,
            fs::metadata(&copy).unwrap().len() / 1024,
        );
    }
}
