//! Friends: finding and joining the servers friends share with you, without links
//! (launcher/docs/friends.md has the design).
//!
//! - [`crypto`]: people's keys, derived secrets, X25519, sealing and lookup ids.
//! - [`pair`]: friend links, cards, replies and the pair secret.
//! - [`state`]: people's signed lists of cores, and the slots that carry them.
//! - [`meet`]: records and knocks.
//! - [`client`]: the service's v2 calls.
//! - [`protocol`]: friend connections' stream kinds and hello.
//! - [`host`]: a core's friend host (the `host` feature).
//! - [`join`]: joining a friend's core (the `join` feature).

pub mod client;
pub mod crypto;
pub mod meet;
pub mod pair;
pub mod protocol;
pub mod state;

#[cfg(feature = "host")]
pub(crate) mod gate;
#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "join")]
pub mod join;

pub use crypto::{PersonKey, PublicKey, Secret};
