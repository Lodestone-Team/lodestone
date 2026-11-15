//! A backup policy's `exclude` patterns (a small subset of gitignore), as the owner writes
//! them: on the server's disk paths. A backup leaves out their canonical translation
//! (`crate::images::excludes`), which never leaves out more; what every backup leaves out
//! whatever the policy is the image store's rule (`lodestone_image::paths::always_excluded`).
//!
//! Paths here are relative to the server folder, `/`-separated, with no leading `/`.
//!
//! Pattern syntax:
//! - `name` matches a file or folder with that name at any depth.
//! - A pattern with a `/` in it (other than a trailing one) matches from the server folder:
//!   `plugins/dynmap/web/tiles`. A leading `/` is allowed and means the same.
//! - `*` matches any run of characters within one part of a path.
//! - `**` as a whole part matches any number of parts, none included.
//! - A trailing `/` matches folders only.
//!
//! A pattern that matches a folder leaves out everything in it.
//!
//! Backups never match these patterns themselves: they match the canonical translation in
//! the image store's snapshot walk. [`reference`] matches them as written, for tests.

#[cfg(test)]
pub(crate) mod reference;

/// At most this many patterns in a policy.
pub const MAX_PATTERNS: usize = 100;

/// Why a pattern isn't accepted, as a sentence for the user: the image store's own rule
/// for excludes, so a policy that validates always translates to valid excludes.
pub use lodestone_image::store::excludes::pattern_error;

/// The patterns a new policy starts with: web map tiles, which are large, change all the
/// time and can be rendered again.
pub fn default_patterns() -> Vec<String> {
    [
        "dynmap/web/tiles/",
        "plugins/dynmap/web/tiles/",
        "bluemap/web/maps/",
        "plugins/BlueMap/web/maps/",
        "squaremap/web/tiles/",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}
