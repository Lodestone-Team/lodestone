//! Modrinth (<https://modrinth.com>): the API types, modrinth.com link parsing and search
//! facets. Calling the API is up to the caller.

pub mod links;
pub mod search;
pub mod types;

pub use search::{ClientEnvironment, ClientProjectType, ClientSearchParams, SearchParams};
pub use types::*;

/// The only host that version files (mods, plugins and modpacks) are downloaded from.
pub const CDN_HOST: &str = "cdn.modrinth.com";
