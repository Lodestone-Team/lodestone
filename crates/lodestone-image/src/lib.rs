//! Instance images: one format for every copy of a Minecraft instance that Lodestone keeps
//! or moves. Server backups, Launcher snapshots, modpack history, what a server sends its
//! players so they can join, and sharing between Hosts, Launchers and friends all become
//! images. `docs/instance-images.md` is the design; this crate is its shared core, used by
//! Lodestone Core and Lodestone Launcher alike, so both sides agree byte for byte.
//!
//! An image ([`Image`]) is a full listing of an instance: every file by path and SHA-256
//! with where it comes from (the base pack, a Modrinth or CurseForge version, or bytes only
//! the store has), plus the game, the settings that travel and the worlds. Nothing in it
//! refers to an earlier image, so deleting one image never breaks another.
//!
//! - [`manifest`]: the image and its parts, and reading a manifest safely
//!   ([`Image::from_json`]): unknown fields are kept, other schemas are refused.
//! - [`identity`]: an image's id is the SHA-256 of its canonical form, so the same files,
//!   sources, base, game and settings give the same id wherever they were recorded
//!   ([`Image::seal`], [`Image::verify`]).
//! - [`paths`]: canonical paths ([`CanonicalPath`]), what is never recorded, where a file
//!   may be placed, and names that clash on case-insensitive disks.
//! - [`layout`]: the two-way mapping between canonical paths and a client's or a server's
//!   disk, including Paper's split worlds.
//! - [`settings`]: the typed settings an image may carry, by scope. Local settings (Java,
//!   arguments, window, play time) have no field, so they can never get into an image.
//! - [`mod@env`]: which sides a file is for, from the owner's mark, the pack, Modrinth and the
//!   jar, in that order.
//! - [`mod@view`]: the server's and the client's view of an image, computed and canonical, and
//!   the check that the two views lose nothing.
//! - [`delta`]: the changes between any two images, and applying them.
//! - [`store`]: the content-addressed store on one machine: objects, manifests and stat
//!   caches, snapshots of a folder into it, placing an image's files on a disk, and
//!   collecting what no image names.
//! - [`container`]: export files, one zip carrying an image to another computer, and the
//!   strict reader that takes one in.
//! - [`share`]: rules both programs follow for images that move between computers: what
//!   an export places, plausible versions, taking files out, JVM crash names.
//! - [`lineage`]: the order content images are kept and listed in, and merged labels.
//! - [`hex`]: hex digests.
//!
//! The crate makes no network calls: callers download. [`store`] and [`container`] read and
//! write files, walk folders and flush disks, so they block: call them on a blocking
//! thread, never on an async or UI thread. The rest touches no files. Untrusted input (a
//! manifest from another computer, a path from a disk, an export) never panics; it gives
//! an error.

pub mod container;
pub mod delta;
pub mod env;
pub mod hex;
pub mod identity;
pub mod layout;
pub mod lineage;
pub mod manifest;
pub mod paths;
pub mod settings;
mod sha256;
pub mod share;
pub mod store;
mod strings;
pub mod view;

use serde::{Deserialize, Serialize};

pub use delta::{apply, diff, Delta, DeltaError, FieldChange};
pub use env::{Env, EnvFrom, EnvInputs, EnvValue};
pub use identity::IdentityError;
pub use layout::{DiskLayout, ServerLayout};
pub use manifest::{
    display_text, Base, Entry, Game, Image, Kind, Loader, ManifestError, Producer, Source,
    Withheld, WithheldReason, World, RULES, SCHEMA,
};
pub use paths::{CanonicalPath, PathError, Placement};
pub use settings::Settings;
pub use view::{view, ViewError};

/// The two sides of a game: a dedicated server and a player's client.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Client,
    Server,
}

impl Side {
    /// Both sides, client first.
    pub const BOTH: [Side; 2] = [Side::Client, Side::Server];

    /// The other side.
    pub fn other(self) -> Side {
        match self {
            Side::Client => Side::Server,
            Side::Server => Side::Client,
        }
    }
}
