//! The Minecraft and Modrinth logic that Lodestone Core and Lodestone Launcher share: API
//! types, version selection, link parsing, search facets, `.mrpack` and jar metadata
//! reading and writing, `.properties` files, NBT (worlds' `level.dat`), and the colours and
//! styles of server console lines.
//!
//! This crate makes no network calls and needs no async runtime. The HTTP calls, caches
//! and rate limiting stay in the programs that use it.
//!
//! Features:
//! - `zip` (default): reading and writing `.mrpack` files and jars.
//! - `ts-rs`: TypeScript bindings for the types Lodestone Core sends to its clients.

pub mod console_format;
mod error;
pub mod hash;
pub mod jar;
pub mod level;
pub mod modrinth;
pub mod mrpack;
pub mod nbt;
pub mod properties;
pub mod util;
pub mod versions;

pub use error::Error;
