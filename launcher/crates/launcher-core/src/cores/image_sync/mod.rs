//! Building and updating a Launcher instance from a server's image
//! (`docs/instance-images.md`, "Server to player", and "M4b decisions"): what replaced the
//! `.mrpack` client pack sync ([`super::sync`], kept only for a core older than milestone
//! 3, which answers 404 on `…/image`).
//!
//! The flow:
//!
//! 1. [`super::player_image::ServerImage::image`] asks the server, naming the views the
//!    instance keeps ([`views::KeptViews::have`], the applied one first): a 204 means the
//!    instance is in step (or holds the current view and switches to it), else a delta from
//!    one of them or the whole view.
//! 2. [`received`] checks what came: a delta is applied to the kept view it names, and the
//!    result, like a whole view, must be exactly the server's client view, with the id the
//!    server named, of content only and a base the Launcher can install.
//! 3. [`base::base_pack`] gets the view's base pack (a `.mrpack` from Modrinth's CDN, cached
//!    by its SHA-512), and [`placed::placed`] works out every file the view places: the
//!    pack's client share, less `removed`, with the view's entries over it ("What a view
//!    places").
//! 4. [`plan::plan`] compares that with the files the instance's `modpack.json` says the
//!    last applied view placed and with what is on disk, through the modpack updater's
//!    three-way merge ([`crate::modpack::merge`]), so the player's own mods and changed
//!    configs stay; the plan, with what the view lacks (withheld files, CurseForge files to
//!    download by hand) is shown before anything changes.
//! 5. [`apply`] places it: a new instance staged whole and renamed into place, or an update
//!    through the modpack updater (staging, a rollback copy, the swap), then the records:
//!    `content.json`, `modpack.json`, the kept view and the link's applied view, in that
//!    order, so a crash anywhere leaves the old link and the next Play plans again.
//!
//! The game and loader are fixed: a view of another Minecraft version or loader is refused
//! for an existing instance (the player makes a new one); only the loader version follows.

pub mod apply;
pub mod base;
pub mod placed;
pub mod plan;
pub mod prepare;
pub mod received;
pub mod views;

pub use lodestone_image::{WithheldReason, display_text};

use super::player_image::ImageError;
use crate::modpack::ModpackError;

/// Why a server's image couldn't be installed. The messages are for people.
#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error(transparent)]
    Image(#[from] ImageError),
    /// The view's rules or schema are newer than this Launcher's.
    #[error("This server runs a newer Lodestone. Update {app} to join it.", app = crate::wording::app_name())]
    UpdateLodestone,
    /// What the server sent can't be installed safely: a view that isn't its client view,
    /// or files outside what a server may place.
    #[error("The server sent files {app} won't install: {0}", app = crate::wording::app_name())]
    Refused(String),
    #[error("The server is built on a CurseForge modpack, which {app} can't install from a server yet.", app = crate::wording::app_name())]
    CurseForgeBase,
    /// The view has CurseForge files that may be downloaded, and CurseForge couldn't be
    /// asked where (offline, an error, no CurseForge connection): nothing is placed, so a
    /// file the player has isn't taken for one they must get by hand.
    #[error(
        "CurseForge couldn't be reached ({0}), and the server has files from it. Try again later."
    )]
    CurseForgeUnavailable(String),
    /// The base pack's version is gone from Modrinth, so its files can't be had.
    #[error(
        "The server's modpack (project {project}, version {version}) is no longer on Modrinth, so its files can't be downloaded. Ask the server's owner to update the modpack."
    )]
    BasePackGone { project: String, version: String },
    /// A `base` entry names a file the base pack doesn't have.
    #[error("The server's modpack has no file {0}, which the server says comes from it.")]
    BaseFileMissing(String),
    /// The view is for another Minecraft version or loader than the instance: those are
    /// fixed when an instance is made.
    #[error(
        "The server now runs {game}, and this instance is for {instance}. Make a new instance for the server."
    )]
    GameChanged { game: String, instance: String },
    /// The instance applied a view from this server, which now answers 404 on
    /// `…/image`: not taken for an older core, whose `.mrpack` fallback may place files
    /// where a view can't ("M4b decisions", "The fallback").
    #[error(
        "The server stopped sharing its files, so they couldn't be checked. Ask the server's owner."
    )]
    StoppedSharing,
    /// The disk lacks the room the files need, before anything is downloaded.
    #[error(
        "There isn't enough free space for the server's files: {needed} bytes are needed and {free} are free."
    )]
    NotEnoughSpace { needed: u64, free: u64 },
    /// The snapshot before the update couldn't be taken, so nothing changed: the player
    /// may update without one ([`apply::ServerUpdate::without_snapshot`]), as a modpack
    /// update offers ("M4b decisions").
    #[error("No snapshot could be taken first, so nothing has changed: {0}")]
    NoSnapshot(String),
    #[error(transparent)]
    Modpack(#[from] ModpackError),
    #[error("{0}")]
    Io(String),
    #[error("cancelled")]
    Cancelled,
}

impl SyncError {
    pub(crate) fn io(what: impl std::fmt::Display, err: std::io::Error) -> Self {
        SyncError::Io(format!("{what}: {err}"))
    }
}
