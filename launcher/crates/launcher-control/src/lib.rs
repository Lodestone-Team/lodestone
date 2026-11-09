//! The control protocol of Lodestone Launcher.
//!
//! A running launcher serves a local endpoint that `lodestone-launcher-cli` (and any
//! other program of the same user) uses to do what the launcher's window does. This
//! crate holds what both sides share, without GPUI:
//!
//! - [`protocol`]: the messages, newline-delimited JSON with a protocol version;
//! - [`paths`]: where the socket is, from the launcher's data directory;
//! - [`server`] and [`client`]: the Unix domain socket ([`transport`]), whose server
//!   refuses connections from other users and whose client talks only to a launcher of
//!   its own user ([`peer_uid`]).
//!
//! On macOS and Linux the socket is created with mode 0600 and the user on the other
//! end is checked with `getpeereid` or `SO_PEERCRED`. Windows (10 1803 and later) has
//! `AF_UNIX` sockets too: the socket file inherits the data folder's permissions, and
//! the other end's user is the SID of the process the socket names.

pub mod paths;
pub mod protocol;

pub mod client;
mod peer;
pub mod server;
pub mod transport;

pub use peer::{UserId, current_uid, peer_uid};
pub use protocol::{ErrorObject, Message, PROTOCOL_VERSION, Request, codes};
