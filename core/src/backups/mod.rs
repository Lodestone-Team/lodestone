//! Server snapshots ("backups" in the API and the code): data images of a server folder in
//! the core's image store (`docs/instance-images.md`), each restoring on its own; and world
//! backups ([`world`]), plain zips of its world.
//!
//! - [`take`]: a backup's snapshot and its data image.
//! - [`restore`]: what a restore replaces, staging, the journaled swap and startup
//!   recovery.
//! - [`store`]: the backups' own files beside the images (their records and policy).
//! - [`retention`]: which backups to delete, and when the next scheduled one is due.
//! - [`pattern`]: the policy's exclude patterns.
//! - [`live`]: pausing saves while a running server is read.
//! - [`types`]: the stored records and the API types.
//! - [`service`]: [`BackupService`], the locks and jobs around all of that, which the
//!   HTTP handlers and the scheduler call.
//! - [`scheduler`]: the background task that takes scheduled backups.
//! - [`world`]: world backups, plain zips of a server's world beside its snapshots.

pub mod live;
pub mod pattern;
pub mod restore;
pub mod retention;
pub mod scheduler;
pub mod service;
pub mod store;
pub mod take;
pub mod types;
pub mod world;

pub use service::BackupService;

pub use types::{
    BackupInfo, BackupKind, BackupList, BackupPolicy, BackupRecord, BackupUsage, JobKind, Records,
    RestoreRequest, RestoreScope, RunningJob,
};
