//! Invite links. Three forms name the same invite:
//!
//! - `https://connect.lodestone.cc/j/<code>#<fingerprint>.<token>`: the one people
//!   share. It is clickable in chat apps, and the service's page at `/j/<code>` offers
//!   to open it in Lodestone Launcher. A test service on `ip:port` uses `http://`.
//! - `lodestone://join?invite=<the https link, percent-encoded>`: the deep link that
//!   page opens.
//! - `lhp2://connect.lodestone.cc/<code>#<fingerprint>.<token>`: the hole-punching
//!   prototype's form, still accepted.
//!
//! `<fingerprint>` is the SHA-256 of the host's certificate and `<token>` 16 random
//! bytes, both in lower-case hex. They come after `#`, which browsers never send, so the
//! introduction service never sees them.

use std::{fmt, str::FromStr};

use crate::{hex, service::Service, tunnel::TOKEN_LEN, unhex};

/// The start of a deep link that opens Lodestone Launcher on an invite.
pub const DEEP_LINK_PREFIX: &str = "lodestone://join";

/// An invite to a shared server.
#[derive(Clone, PartialEq, Eq)]
pub struct Invite {
    /// The introduction service that knows the code.
    pub service: Service,
    /// The invite's code at the service.
    pub code: String,
    /// The SHA-256 of the host's certificate.
    pub fingerprint: [u8; 32],
    /// Proves to the host that the friend has the link.
    pub token: [u8; TOKEN_LEN],
}

/// Why a text isn't an invite.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("That isn't a Lodestone invite link: {0}")]
pub struct LinkError(pub &'static str);

impl Invite {
    /// Reads any of the three forms. Spaces around the link are ignored.
    pub fn parse(text: &str) -> Result<Self, LinkError> {
        let text = text.trim();
        if let Some(query) = text.strip_prefix(DEEP_LINK_PREFIX) {
            return Self::parse_deep_link(query);
        }
        let (service, rest) = if let Some(rest) = text.strip_prefix("https://") {
            let (place, rest) = rest.split_once('/').ok_or(LinkError("no code"))?;
            let service = Service::host_port(place, 443, true).ok_or(LinkError("bad address"))?;
            let rest = rest.strip_prefix("j/").ok_or(LinkError("no /j/ in it"))?;
            (service, rest)
        } else if let Some(rest) = text.strip_prefix("http://") {
            let (place, rest) = rest.split_once('/').ok_or(LinkError("no code"))?;
            let service = Service::host_port(place, 80, false).ok_or(LinkError("bad address"))?;
            let rest = rest.strip_prefix("j/").ok_or(LinkError("no /j/ in it"))?;
            (service, rest)
        } else if let Some(rest) = text.strip_prefix("lhp2://") {
            let (place, rest) = rest.split_once('/').ok_or(LinkError("no code"))?;
            let service = Service::from_link(place).ok_or(LinkError("bad address"))?;
            (service, rest)
        } else {
            return Err(LinkError("it should start with https://"));
        };
        let (code, secret) = rest
            .split_once('#')
            .ok_or(LinkError("the part after # is missing"))?;
        let code = code.trim_end_matches('/');
        if code.is_empty() || code.len() > 32 || !code.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(LinkError("bad code"));
        }
        let (fingerprint, token) = secret
            .split_once('.')
            .ok_or(LinkError("the part after # is cut short"))?;
        Ok(Self {
            service,
            code: code.to_ascii_uppercase(),
            fingerprint: unhex(fingerprint)
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or(LinkError("bad certificate fingerprint"))?,
            token: unhex(token)
                .and_then(|bytes| bytes.try_into().ok())
                .ok_or(LinkError("bad token"))?,
        })
    }

    fn parse_deep_link(rest: &str) -> Result<Self, LinkError> {
        // `lodestone://join?invite=...` (a trailing slash before `?` is allowed)
        let query = rest
            .trim_start_matches('/')
            .strip_prefix('?')
            .ok_or(LinkError("the deep link has no invite"))?;
        let invite = url::form_urlencoded::parse(query.as_bytes())
            .find(|(name, _)| name == "invite")
            .map(|(_, value)| value.into_owned())
            .ok_or(LinkError("the deep link has no invite"))?;
        if invite.starts_with(DEEP_LINK_PREFIX) {
            return Err(LinkError("a deep link inside a deep link"));
        }
        Self::parse(&invite)
    }

    /// The link to share: `https://connect.lodestone.cc/j/<code>#<fingerprint>.<token>`.
    pub fn link(&self) -> String {
        format!(
            "{}/j/{}#{}.{}",
            self.service.base_url(),
            self.code,
            hex(&self.fingerprint),
            hex(&self.token)
        )
    }

    /// The deep link that opens Lodestone Launcher on this invite:
    /// `lodestone://join?invite=<the link, percent-encoded>`.
    pub fn deep_link(&self) -> String {
        let query: String = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("invite", &self.link())
            .finish();
        format!("{DEEP_LINK_PREFIX}?{query}")
    }

    /// The prototype's form: `lhp2://connect.lodestone.cc/<code>#<fingerprint>.<token>`.
    pub fn lhp2(&self) -> String {
        format!(
            "lhp2://{}/{}#{}.{}",
            self.service.link(),
            self.code,
            hex(&self.fingerprint),
            hex(&self.token)
        )
    }
}

impl FromStr for Invite {
    type Err = LinkError;

    fn from_str(text: &str) -> Result<Self, LinkError> {
        Self::parse(text)
    }
}

impl fmt::Display for Invite {
    /// The full link, token included: share it, don't log it.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.link())
    }
}

impl fmt::Debug for Invite {
    /// Leaves out the token.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Invite")
            .field("service", &self.service)
            .field("code", &self.code)
            .field("fingerprint", &hex(&self.fingerprint))
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invite() -> Invite {
        Invite {
            service: Service::default(),
            code: "ABCDEFGH23".into(),
            fingerprint: [0xab; 32],
            token: [0x01; TOKEN_LEN],
        }
    }

    #[test]
    fn the_three_forms_round_trip() {
        let invite = invite();
        let secret = format!("{}.{}", "ab".repeat(32), "01".repeat(16));
        assert_eq!(
            invite.link(),
            format!("https://connect.lodestone.cc/j/ABCDEFGH23#{secret}")
        );
        assert_eq!(
            invite.lhp2(),
            format!("lhp2://connect.lodestone.cc/ABCDEFGH23#{secret}")
        );
        assert!(invite.deep_link().starts_with(
            "lodestone://join?invite=https%3A%2F%2Fconnect.lodestone.cc%2Fj%2FABCDEFGH23%23"
        ));
        for text in [invite.link(), invite.lhp2(), invite.deep_link()] {
            assert_eq!(Invite::parse(&text), Ok(invite.clone()), "{text}");
        }
        assert_eq!(
            Invite::parse(&format!("  {}\n", invite.link())),
            Ok(invite.clone())
        );
    }

    #[test]
    fn a_test_service_on_an_address_is_plain_http() {
        let text = format!(
            "http://127.0.0.1:8787/j/ABCDEFGH23#{}.{}",
            "ab".repeat(32),
            "01".repeat(16)
        );
        let invite = Invite::parse(&text).unwrap();
        assert!(!invite.service.tls);
        assert_eq!(invite.service.port, 8787);
        assert_eq!(invite.link(), text);
        let lhp2 = format!(
            "lhp2://16.146.26.94:8080/ABCDEFGH23#{}.{}",
            "ab".repeat(32),
            "01".repeat(16)
        );
        let old = Invite::parse(&lhp2).unwrap();
        assert_eq!(old.service.link(), "16.146.26.94:8080");
        assert_eq!(old.lhp2(), lhp2);
    }

    #[test]
    fn broken_links_are_refused() {
        let secret = format!("{}.{}", "ab".repeat(32), "01".repeat(16));
        for text in [
            "".to_string(),
            "https://connect.lodestone.cc/j/ABCDEFGH23".to_string(),
            format!("https://connect.lodestone.cc/ABCDEFGH23#{secret}"),
            format!("https://connect.lodestone.cc/j/AB/../x#{secret}"),
            format!("https://evil.example@x/j/ABC#{secret}"),
            format!(
                "https://connect.lodestone.cc/j/ABC#{}.{}",
                "ab".repeat(31),
                "01".repeat(16)
            ),
            "lodestone://join?other=1".to_string(),
            format!("ftp://connect.lodestone.cc/j/ABC#{secret}"),
        ] {
            assert!(Invite::parse(&text).is_err(), "{text}");
        }
    }

    #[test]
    fn debug_leaves_out_the_token() {
        let shown = format!("{:?}", invite());
        assert!(!shown.contains(&"01".repeat(16)), "{shown}");
    }
}
