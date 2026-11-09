//! Lodestone Connect: a friend joins a server shared from Lodestone Core with an invite
//! link, over a direct, encrypted tunnel. No relay carries the game.
//!
//! 1. Each side collects its candidate addresses ([`gather`]): its address on the local
//!    network, its IPv6 address, the public address STUN servers see ([`stun`]), and a
//!    port its router forwards on request (UPnP or NAT-PMP, [`portmap`]). One UDP socket
//!    serves IPv4 and IPv6.
//! 2. The host registers its candidates with the introduction service
//!    (connect.lodestone.cc, [`service`]) and gets a code. The invite link
//!    ([`Invite`]) carries the code, the SHA-256 of the host's certificate and a random
//!    token. The part after `#` never reaches the service.
//! 3. A friend opens the link: the service swaps the two sides' candidates, and both
//!    send small probes to every address of the other ([`probe`]) until one gets through.
//! 4. The friend opens a QUIC connection to the host on that path, accepting only the
//!    certificate from the link. Each QUIC stream starts with the token and a kind
//!    ([`StreamKind`]): the game (Minecraft's TCP connection), or the server's info.
//!
//! The `host` feature shares a server ([`host::Host`]); the `join` feature joins one
//! ([`join::join`]). README.md describes the stream protocol.

// Without `join`, some of the shared parts are unused.
#![cfg_attr(not(feature = "join"), allow(dead_code))]

pub mod addr;
pub mod gather;
pub mod info;
mod link;
pub mod portmap;
pub mod probe;
pub mod service;
pub mod stun;
pub mod tunnel;

#[cfg(feature = "host")]
pub mod host;
#[cfg(feature = "join")]
pub mod join;

pub use addr::ConnectionKind;
pub use gather::{Candidate, CandidateKind, GatherOptions};
pub use info::SharedServer;
pub use link::{Invite, LinkError, DEEP_LINK_PREFIX};
pub use service::{Service, DEFAULT_SERVICE};
pub use tunnel::{StreamKind, TunnelStream, TOKEN_LEN};

/// What can go wrong while sharing or joining.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// The introduction service answered with an error.
    #[error("The introduction service said: {message} ({code})")]
    Service {
        status: u16,
        code: String,
        message: String,
    },
    /// The introduction service couldn't be reached, or its answer made no sense.
    #[error("Couldn't reach the introduction service: {0}")]
    Unreachable(String),
    #[error(transparent)]
    Link(#[from] LinkError),
    /// This computer has no address to offer (it seems to be offline).
    #[error("Found no address to offer. Is this computer online?")]
    NoAddress,
    /// No probe got through: these two networks need a relay.
    #[error("{0}")]
    NoPath(String),
    /// The host's certificate isn't the one in the invite.
    #[error("The host's certificate doesn't match the invite. Ask for a new link.")]
    WrongHost,
    /// The QUIC tunnel failed or closed.
    #[error("The tunnel failed: {0}")]
    Tunnel(String),
    /// Anything else, as a sentence.
    #[error("{0}")]
    Other(String),
}

impl Error {
    /// Whether the service said the invite doesn't exist (or has ended).
    pub fn is_not_found(&self) -> bool {
        matches!(self, Error::Service { status: 404, .. })
    }

    /// The service's error code, such as `not_found` or `host_offline`.
    pub fn service_code(&self) -> Option<&str> {
        match self {
            Error::Service { code, .. } => Some(code),
            _ => None,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn unhex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok())
        .collect()
}

pub(crate) fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    ring::rand::SecureRandom::fill(&ring::rand::SystemRandom::new(), &mut bytes)
        .map_err(|_| Error::Other("This computer gave no random numbers".into()))?;
    Ok(bytes)
}
