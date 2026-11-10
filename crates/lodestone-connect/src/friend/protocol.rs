//! What both ends of a friend connection agree on: the stream kinds and the hello.

use super::crypto::{transcript, PublicKey};

/// The label of the TLS exporter a friend hello signs.
pub const HELLO_LABEL: &[u8] = b"lodestone-friend hello";
/// The signature context of a friend hello.
pub const HELLO_CONTEXT: &str = "friend hello";

/// What a friend stream carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum FriendStreamKind {
    /// The first stream: the friend's key and signature. Answered with [`crate::tunnel::HELLO_ANSWER`].
    Hello = 3,
    /// HTTP/1.1 to the routes about the instances shared with this friend.
    Info = 4,
    /// Minecraft's connection to a shared instance; then a length byte and its uuid.
    Game = 5,
}

/// The longest instance uuid a game stream may name.
pub const MAX_INSTANCE_LEN: usize = 64;

/// What a friend signs in their hello.
pub fn hello_message(
    exporter: &[u8; 32],
    core_fingerprint: &[u8; 32],
    friend: &PublicKey,
) -> Vec<u8> {
    transcript(&[exporter, core_fingerprint, &friend.0])
}
