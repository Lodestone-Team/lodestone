//! Server backups: incremental, content-addressed copies of a server folder that each
//! restore on their own.
//!
//! - [`store`]: objects, manifests, walking a server folder with a stat cache, GC.
//! - [`restore`]: owned paths, staging, the journaled swap and startup recovery.
//! - [`retention`]: which backups to delete, and when the next scheduled one is due.
//! - [`pattern`]: the exclude patterns and the paths every backup leaves out.
//! - [`live`]: pausing saves while a running server is copied.
//! - [`types`]: the stored and API types.
//! - [`service`]: [`BackupService`], the locks and jobs around all of that, which the
//!   HTTP handlers and the scheduler call.
//! - [`scheduler`]: the background task that takes scheduled backups.

pub mod live;
pub mod pattern;
pub mod restore;
pub mod retention;
pub mod scheduler;
pub mod service;
pub mod store;
pub mod types;

pub use service::BackupService;

pub use types::{
    BackupInfo, BackupKind, BackupList, BackupPolicy, BackupUsage, FileEntry, JobKind, Manifest,
    RestoreRequest, RestoreScope, RunningJob, SkipReason, SkippedFile,
};
