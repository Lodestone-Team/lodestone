//! Copying another launcher's game folder: planned first (what is copied, its size, what
//! is left out), then copied with progress and cancel.
//!
//! Caches, logs and the files launchers keep for themselves are left out; so are the
//! worlds with [`CopyOptions::without_worlds`]. A symbolic link is followed only when what
//! it points to is inside the game folder (it is copied as a file or folder); any other
//! link is left out. Every path goes through [`safe_relative_path`], and the source is
//! only read.

use std::{
    fs::{self, File},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

use super::{ImportError, ImportProgress, io_error};
use crate::game::{download::CancelToken, libraries::safe_relative_path};

/// Top-level folders that are caches, logs or the launcher's own files, never the
/// player's (lowercase).
const SKIPPED_DIRS: &[&str] = &[
    "logs",
    "crash-reports",
    "debug",
    ".cache",
    "cache",
    "natives",
    "libraries",
    "assets",
    "versions",
    "bin",
    "runtime",
    "webcache",
    "webcache2",
    "downloads",
    "staging",
    "launcher",
    "quickplay",
    ".fabric",
    ".quilt",
    ".mixin.out",
    "jars",
    "__macosx",
];

/// Files anywhere that are never copied (lowercase).
const SKIPPED_FILES: &[&str] = &[".ds_store", "thumbs.db", "desktop.ini"];

/// Top-level files of launchers that keep them in the game folder (lowercase): the
/// official launcher's (accounts and tokens among them), MultiMC's and Prism's instance
/// files when the game folder is the instance folder, and the other launchers' own.
const SKIPPED_TOP_FILES: &[&str] = &[
    "launcher_profiles.json",
    "launcher_accounts.json",
    "launcher_accounts_microsoft_store.json",
    "launcher_msa_credentials.bin",
    "launcher_msa_credentials_microsoft_store.bin",
    "launcher_settings.json",
    "launcher_ui_state.json",
    "launcher_cef_log.txt",
    "launcher_log.txt",
    "launcher_entitlements.json",
    "launcher_gamer_pics.json",
    "launcher_product_state.json",
    "launcher_quick_play.json",
    "launcher_skins.json",
    "clientid.txt",
    "clientid_v2.txt",
    "treatment_tags.json",
    "updatesourcecache.json",
    "instance.json",
    "minecraftinstance.json",
    "config.json.bak",
    "profile.json",
    "packinfo.json",
    "usercache.json",
    "usernamecache.json",
    "instance.png",
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CopyOptions {
    /// Leave `saves/` out.
    pub without_worlds: bool,
}

/// A file to copy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlannedFile {
    /// The file to read (a link's target when it is one).
    pub from: PathBuf,
    /// Where it goes in the new game folder.
    pub relative: PathBuf,
    pub size: u64,
}

/// What an import copies.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CopyPlan {
    pub files: Vec<PlannedFile>,
    /// Folders to create, so empty ones (such as `mods/`) carry over.
    pub dirs: Vec<PathBuf>,
    pub total_bytes: u64,
    /// The size of `saves/`, for "Without worlds".
    pub worlds_bytes: u64,
    /// Left out, by relative path, and why.
    pub skipped: Vec<(String, String)>,
}

/// Why `relative` (a path in the game folder) is left out, if it is.
pub fn skip_reason(relative: &Path, is_dir: bool, options: &CopyOptions) -> Option<&'static str> {
    let name = relative.file_name()?.to_string_lossy().to_ascii_lowercase();
    let top = relative.components().count() == 1;
    if SKIPPED_FILES.contains(&name.as_str()) {
        return Some("system file");
    }
    if top && is_dir && SKIPPED_DIRS.contains(&name.as_str()) {
        return Some("cache or logs");
    }
    if top && is_dir && options.without_worlds && name == "saves" {
        return Some("worlds left out");
    }
    if top && !is_dir {
        if SKIPPED_TOP_FILES.contains(&name.as_str())
            || name.starts_with("launcher_")
            || name.starts_with("nativeupdaterlog")
            || name.contains("credentials")
        {
            return Some("launcher file");
        }
        if name.ends_with(".log") || name.ends_with(".log.gz") || name.starts_with("hs_err_pid") {
            return Some("log");
        }
    }
    if !is_dir && (name.ends_with(".tmp") || name.ends_with(".part")) {
        return Some("unfinished download");
    }
    None
}

/// Plans copying `game_dir`.
pub fn plan(game_dir: &Path, options: &CopyOptions) -> Result<CopyPlan, ImportError> {
    let root = fs::canonicalize(game_dir).map_err(io_error(game_dir))?;
    if !root.is_dir() {
        return Err(ImportError::Invalid(format!(
            "{} is not a folder",
            game_dir.display()
        )));
    }
    let mut plan = CopyPlan::default();
    let mut ancestors = vec![root.clone()];
    walk(
        &root,
        &root,
        Path::new(""),
        options,
        &mut ancestors,
        &mut plan,
    )?;
    plan.files.sort_by(|a, b| a.relative.cmp(&b.relative));
    plan.total_bytes = plan.files.iter().map(|f| f.size).sum();
    plan.worlds_bytes = plan
        .files
        .iter()
        .filter(|f| f.relative.starts_with("saves"))
        .map(|f| f.size)
        .sum();
    Ok(plan)
}

fn walk(
    root: &Path,
    dir: &Path,
    relative_dir: &Path,
    options: &CopyOptions,
    ancestors: &mut Vec<PathBuf>,
    plan: &mut CopyPlan,
) -> Result<(), ImportError> {
    let mut entries: Vec<_> = fs::read_dir(dir)
        .map_err(io_error(dir))?
        .collect::<Result<_, _>>()
        .map_err(io_error(dir))?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        let shown = relative_dir.join(entry.file_name());
        let shown_text = shown.to_string_lossy().into_owned();
        let Some(name) = entry.file_name().to_str().map(str::to_string) else {
            plan.skipped
                .push((shown_text, "the name isn't valid UTF-8".into()));
            continue;
        };
        let relative_text = match relative_dir.to_str() {
            Some("") => name.clone(),
            Some(dir) => format!("{dir}/{name}"),
            None => continue,
        };
        let Ok(relative) = safe_relative_path(&relative_text) else {
            plan.skipped.push((shown_text, "unsafe name".into()));
            continue;
        };
        let kind = entry.file_type().map_err(io_error(&path))?;
        // A link counts as what it points to, if that is inside the game folder.
        let (target, is_dir) = if kind.is_symlink() {
            match fs::canonicalize(&path) {
                Ok(target) if target.starts_with(root) && target != root => {
                    let is_dir = target.is_dir();
                    (target, is_dir)
                }
                _ => {
                    plan.skipped
                        .push((relative_text, "link to outside the game folder".into()));
                    continue;
                }
            }
        } else if kind.is_dir() || kind.is_file() {
            (path.clone(), kind.is_dir())
        } else {
            plan.skipped
                .push((relative_text, "not a file or folder".into()));
            continue;
        };
        if let Some(reason) = skip_reason(&relative, is_dir, options) {
            plan.skipped.push((relative_text, reason.into()));
            continue;
        }
        if is_dir {
            let canonical = fs::canonicalize(&target).map_err(io_error(&target))?;
            // A link to a folder that contains it would copy forever.
            if ancestors.contains(&canonical) {
                plan.skipped
                    .push((relative_text, "a link to a folder that contains it".into()));
                continue;
            }
            plan.dirs.push(relative.clone());
            ancestors.push(canonical.clone());
            let walked = walk(root, &canonical, &relative, options, ancestors, plan);
            ancestors.pop();
            walked?;
        } else {
            let size = fs::metadata(&target).map_err(io_error(&target))?.len();
            plan.files.push(PlannedFile {
                from: target,
                relative,
                size,
            });
        }
    }
    Ok(())
}

/// Copies the planned files into `dest` (the new game folder, which may exist and be
/// empty), counting bytes into `progress`. Stops with [`ImportError::Cancelled`] when
/// `cancel` is set; the caller removes what was copied.
pub fn copy(
    plan: &CopyPlan,
    dest: &Path,
    progress: &ImportProgress,
    cancel: &CancelToken,
) -> Result<(), ImportError> {
    progress
        .bytes_total
        .store(plan.total_bytes, Ordering::Relaxed);
    progress
        .files_total
        .store(plan.files.len() as u64, Ordering::Relaxed);
    progress.bytes_done.store(0, Ordering::Relaxed);
    progress.files_done.store(0, Ordering::Relaxed);
    fs::create_dir_all(dest).map_err(io_error(dest))?;
    let dest_root = fs::canonicalize(dest).map_err(io_error(dest))?;
    for dir in &plan.dirs {
        let path = dest_root.join(dir);
        fs::create_dir_all(&path).map_err(io_error(&path))?;
    }
    let mut buffer = vec![0u8; 1024 * 1024];
    for file in &plan.files {
        if cancel.is_cancelled() {
            return Err(ImportError::Cancelled);
        }
        let target = dest_root.join(&file.relative);
        let parent = target.parent().unwrap_or(&dest_root);
        fs::create_dir_all(parent).map_err(io_error(parent))?;
        // The folders were made here, but check: nothing is written outside the new
        // instance.
        let canonical_parent = fs::canonicalize(parent).map_err(io_error(parent))?;
        if !canonical_parent.starts_with(&dest_root) || fs::symlink_metadata(&target).is_ok() {
            return Err(ImportError::Invalid(format!(
                "{} would be written outside the new instance",
                file.relative.display()
            )));
        }
        let mut reader = File::open(&file.from).map_err(io_error(&file.from))?;
        let mut writer = File::create(&target).map_err(io_error(&target))?;
        loop {
            if cancel.is_cancelled() {
                return Err(ImportError::Cancelled);
            }
            let n = reader.read(&mut buffer).map_err(io_error(&file.from))?;
            if n == 0 {
                break;
            }
            writer.write_all(&buffer[..n]).map_err(io_error(&target))?;
            progress.bytes_done.fetch_add(n as u64, Ordering::Relaxed);
        }
        writer.flush().map_err(io_error(&target))?;
        drop(writer);
        // Keep the modification time, so the game and the Content tab see the same file.
        if let Ok(modified) = fs::metadata(&file.from).and_then(|m| m.modified()) {
            let _ = File::options()
                .write(true)
                .open(&target)
                .and_then(|f| f.set_modified(modified));
        }
        progress.files_done.fetch_add(1, Ordering::Relaxed);
    }
    Ok(())
}

/// "1.2 GB".
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["bytes", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1000.0 && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} bytes")
    } else if value < 10.0 {
        format!("{value:.1} {}", UNITS[unit])
    } else {
        format!("{value:.0} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skip_rules() {
        let options = CopyOptions::default();
        let skip = |p: &str, dir: bool| skip_reason(Path::new(p), dir, &options);
        assert_eq!(skip("logs", true), Some("cache or logs"));
        assert_eq!(skip("mods", true), None);
        assert_eq!(skip("config/logs", true), None, "only top-level folders");
        assert_eq!(skip("launcher_accounts.json", false), Some("launcher file"));
        assert_eq!(
            skip("launcher_msa_credentials.bin", false),
            Some("launcher file")
        );
        assert_eq!(skip("latest.log", false), Some("log"));
        assert_eq!(skip("options.txt", false), None);
        assert_eq!(skip("servers.dat", false), None);
        assert_eq!(skip("saves", true), None);
        assert_eq!(skip("mods/.DS_Store", false), Some("system file"));
        let without = CopyOptions {
            without_worlds: true,
        };
        assert_eq!(
            skip_reason(Path::new("saves"), true, &without),
            Some("worlds left out")
        );
    }

    #[test]
    fn sizes() {
        assert_eq!(format_size(512), "512 bytes");
        assert_eq!(format_size(1_500_000), "1.5 MB");
        assert_eq!(format_size(23_000_000_000), "23 GB");
    }
}
