//! An instance's screenshots (`minecraft/screenshots/`, where F2 saves them): the list,
//! thumbnails, and deleting (to the Trash).

use std::{
    fs, io,
    path::{Path, PathBuf},
    time::SystemTime,
};

use crate::content::{InstanceDirs, is_plain_name};

pub const SCREENSHOTS: &str = "screenshots";
/// Images larger than this aren't decoded.
pub const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024;
/// Nor images with more pixels than this on a side.
const MAX_SIDE: u32 = 16_384;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Screenshot {
    pub file_name: String,
    pub path: PathBuf,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

pub fn dir(dirs: &InstanceDirs) -> PathBuf {
    dirs.game.join(SCREENSHOTS)
}

fn is_image_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    is_plain_name(name)
        && (lower.ends_with(".png") || lower.ends_with(".jpg") || lower.ends_with(".jpeg"))
}

/// The screenshots, newest first. Links and other files are left out.
pub fn list(dirs: &InstanceDirs) -> Vec<Screenshot> {
    let Ok(entries) = fs::read_dir(dir(dirs)) else {
        return Vec::new();
    };
    let mut list: Vec<Screenshot> = entries
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter_map(|e| {
            let file_name = e.file_name().to_str()?.to_string();
            if !is_image_name(&file_name) {
                return None;
            }
            let metadata = e.metadata().ok()?;
            Some(Screenshot {
                path: e.path(),
                file_name,
                size: metadata.len(),
                modified: metadata.modified().ok(),
            })
        })
        .collect();
    list.sort_by(|a, b| {
        b.modified
            .cmp(&a.modified)
            .then_with(|| b.file_name.cmp(&a.file_name))
    });
    list
}

/// Decodes an image file, refusing files and images that are too large.
pub fn decode(path: &Path) -> Result<image::DynamicImage, String> {
    let metadata = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    if !metadata.is_file() {
        return Err("not a file".into());
    }
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err("the image is larger than 64 MB".into());
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let mut reader = image::ImageReader::new(io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(1024 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().map_err(|e| e.to_string())
}

/// The image scaled to fit in `max` × `max` pixels (never enlarged), as RGBA.
pub fn thumbnail(path: &Path, max: u32) -> Result<image::RgbaImage, String> {
    let image = decode(path)?;
    let image = if image.width() > max || image.height() > max {
        image.thumbnail(max, max)
    } else {
        image
    };
    Ok(image.to_rgba8())
}

/// The image as PNG bytes, for the clipboard: the file itself when it is a PNG.
pub fn png_bytes(path: &Path) -> Result<Vec<u8>, String> {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("png"))
    {
        let metadata = fs::metadata(path).map_err(|e| e.to_string())?;
        if metadata.len() > MAX_IMAGE_BYTES {
            return Err("the image is larger than 64 MB".into());
        }
        return fs::read(path).map_err(|e| e.to_string());
    }
    let image = decode(path)?;
    let mut out = io::Cursor::new(Vec::new());
    image
        .write_to(&mut out, image::ImageFormat::Png)
        .map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// The screenshot `file_name`, if it is one.
pub fn path(dirs: &InstanceDirs, file_name: &str) -> Option<PathBuf> {
    if !is_image_name(file_name) {
        return None;
    }
    let path = dir(dirs).join(file_name);
    fs::symlink_metadata(&path)
        .is_ok_and(|m| m.is_file())
        .then_some(path)
}

/// Moves a screenshot to the Trash, with `remove` (tests don't use the Trash).
pub fn delete_with(
    dirs: &InstanceDirs,
    file_name: &str,
    remove: impl FnOnce(&Path) -> Result<(), String>,
) -> Result<(), String> {
    let path =
        path(dirs, file_name).ok_or_else(|| format!("There is no screenshot {file_name}."))?;
    remove(&path)
}

pub fn delete(dirs: &InstanceDirs, file_name: &str) -> Result<(), String> {
    delete_with(dirs, file_name, |path| {
        trash::delete(path).map_err(|e| e.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_thumbnails_and_deletes() {
        let temp = tempfile::tempdir().unwrap();
        let dirs = InstanceDirs::new(temp.path());
        assert!(list(&dirs).is_empty());
        let shots = dir(&dirs);
        fs::create_dir_all(&shots).unwrap();
        let image = image::RgbaImage::from_pixel(800, 400, image::Rgba([10, 20, 30, 255]));
        image.save(shots.join("2026-11-08_10.00.00.png")).unwrap();
        image.save(shots.join("old.png")).unwrap();
        let old = SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000);
        fs::File::options()
            .write(true)
            .open(shots.join("old.png"))
            .unwrap()
            .set_modified(old)
            .unwrap();
        fs::write(shots.join("notes.txt"), "x").unwrap();
        fs::write(shots.join(".hidden.png"), "x").unwrap();
        fs::create_dir(shots.join("folder.png")).unwrap();
        let listed: Vec<String> = list(&dirs).into_iter().map(|s| s.file_name).collect();
        assert_eq!(listed, ["2026-11-08_10.00.00.png", "old.png"]);

        let thumb = thumbnail(&shots.join("old.png"), 200).unwrap();
        assert_eq!((thumb.width(), thumb.height()), (200, 100));
        let small = thumbnail(&shots.join("old.png"), 2000).unwrap();
        assert_eq!(small.width(), 800);
        assert!(thumbnail(&shots.join("notes.txt"), 10).is_err());
        assert_eq!(&png_bytes(&shots.join("old.png")).unwrap()[..4], b"\x89PNG");

        let mut removed = Vec::new();
        delete_with(&dirs, "old.png", |p| {
            removed.push(p.to_path_buf());
            Ok(())
        })
        .unwrap();
        assert_eq!(removed, [shots.join("old.png")]);
        for bad in ["../x.png", "notes.txt", "missing.png", "folder.png"] {
            assert!(delete_with(&dirs, bad, |_| Ok(())).is_err(), "{bad}");
        }
    }
}
