//! Moving a server's images to another computer (`docs/instance-images.md`, "Sharing", and
//! "M6a decisions"): a backup exported as a `.lodeimg` file with its secrets taken out
//! ([`export`]), and a `.lodeimg` made by a Host or a Launcher uploaded ([`uploads`]),
//! read ([`inspect`]) and made into a new server ([`import`]), which ends with what the
//! server didn't get ([`outcome`]). Both ends withhold settings files that hold a
//! credential under a name that says so ([`secrets`]).
//!
//! The file is the shared crate's export container (`lodestone_image::container`), the
//! same one the Launcher writes and reads, so either program opens the other's files.

pub mod export;
pub mod import;
pub mod inspect;
pub mod outcome;
pub mod secrets;
pub mod uploads;

#[cfg(test)]
mod launcher_fixtures;
#[cfg(test)]
mod tests;

/// An image export's file extension, the Launcher's too.
pub const EXTENSION: &str = "lodeimg";

/// The most names of one kind a summary lists; the counts say how many there are.
pub const MAX_NAMES: usize = 64;

/// Empties the temporary folder `dir` an earlier run may have left files in (uploads and
/// exports can be tens of gigabytes each) without making the caller wait: the folder is
/// renamed aside at once (`<name>.old-<random>`) and deleted in the background with any
/// older such folder a run stopped deleting (read-only files too, which stop a delete on
/// Windows). Startup calls it before anything writes there.
pub fn clear_leftovers(dir: &std::path::Path) {
    let (Some(parent), Some(name)) = (dir.parent(), dir.file_name().and_then(|n| n.to_str()))
    else {
        return;
    };
    let prefix = format!("{name}.old-");
    if dir.exists() {
        let aside = parent.join(format!("{prefix}{}", crate::util::rand_alphanumeric(8)));
        if let Err(e) = std::fs::rename(dir, &aside) {
            tracing::warn!(
                "Could not move aside the old files in {}: {e}",
                dir.display()
            );
        }
    }
    let old: Vec<std::path::PathBuf> = std::fs::read_dir(parent)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| {
                    e.file_name()
                        .to_str()
                        .is_some_and(|n| n.starts_with(&prefix))
                })
                .map(|e| e.path())
                .collect()
        })
        .unwrap_or_default();
    if old.is_empty() {
        return;
    }
    std::thread::spawn(move || {
        for path in old {
            if let Err(e) = super::remove_tree(&path) {
                tracing::warn!("Could not remove the old files in {}: {e}", path.display());
            }
        }
    });
}
