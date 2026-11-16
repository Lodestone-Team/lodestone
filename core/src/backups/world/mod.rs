//! World backups: plain `.zip` files of a server's world, kept beside its snapshots
//! (`docs/instance-images.md`, "World backups").
//!
//! A snapshot (what the rest of [`crate::backups`] calls a backup) is a data image of the
//! whole server folder in the image store. A world backup is the other thing people ask
//! for: the level, and on Paper and Spigot its split `_nether` and `_the_end` folders, as
//! a standard zip that opens outside Lodestone. No delta, no image store: deleting one
//! frees its whole size. They are made only when asked (and before a world backup is
//! restored), never on a schedule, and never pruned.
//!
//! - [`writer`]: the zip of a world as it is on disk.
//! - [`reader`]: the checked plan of restoring a zip, and its extraction.
//! - [`store`]: the zips and their sidecars in `<lodestone>/world-backups/<uuid>/`.
//! - [`types`]: the sidecar and the API types.
//! - [`service`]: [`WorldBackupService`], the jobs and locks around all of that, sharing
//!   the snapshots' job lock and their swap and journal ([`super::restore`]).

pub mod reader;
pub mod service;
pub mod store;
pub mod types;
pub mod writer;

pub use service::WorldBackupService;
pub use types::{WorldBackupInfo, WorldBackupKind, WorldBackupList, WorldBackupRecord};
