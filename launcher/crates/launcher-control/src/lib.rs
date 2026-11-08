//! The control protocol of Lodestone Launcher.
//!
//! A running launcher serves a local endpoint that `lodestone-launcher-cli` (and any
//! other program of the same user) uses to do what the launcher's window does. This
//! crate holds what both sides share, without GPUI:
//!
//! - [`protocol`]: the messages, newline-delimited JSON with a protocol version;
//! - [`paths`]: where the socket is, from the launcher's data directory;
//! - [`server`] and [`client`]: the Unix domain socket, created with mode 0600, whose
//!   server refuses connections from other users (checked with `getpeereid` or
//!   `SO_PEERCRED`).
//!
//! The socket is Unix only. On Windows the protocol types build, but there is no
//! transport yet (a named pipe would be the equivalent).

pub mod paths;
pub mod protocol;

#[cfg(unix)]
pub mod client;
#[cfg(unix)]
mod peer;
#[cfg(unix)]
pub mod server;

#[cfg(unix)]
pub use peer::{current_uid, peer_uid};
pub use protocol::{ErrorObject, Message, PROTOCOL_VERSION, Request, codes};
