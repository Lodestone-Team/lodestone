//! Becoming friends: friend links, the inviter's card and the answer to it, and the pair
//! secret both sides end up with (launcher/docs/friends.md, "Becoming friends").
//!
//! 1. Ana makes an [`Invitation`]: a [`FriendLink`] and her card, sealed with the link's
//!    secret, for the mailbox at the service.
//! 2. Ben opens the card with the link ([`open_card`]), checks it against the key hash
//!    in the link, and [`answer`]s: a sealed reply, and the pair secret.
//! 3. Ana opens each reply ([`Invitation::open_reply`]) and [`Invitation::accept`]s one,
//!    which gives her the same pair secret.
//!
//! The pair secret comes from an X25519 exchange and the link's secret, bound to both
//! messages. Once both sides delete their exchange keys, nothing the service stored,
//! nor the link itself, reveals it.

use serde::{Deserialize, Serialize};

use super::crypto::{
    self, crockford, sha256, transcript, ExchangeKey, PersonKey, PublicKey, Role, Secret,
    PUBLIC_KEY_LEN,
};
use crate::{hex, service::Service, unhex, Error, LinkError, Result};

/// The start of a deep link that opens Lodestone Launcher on a friend link.
pub const FRIEND_DEEP_LINK_PREFIX: &str = "lodestone://friend";
/// How long a friend link works.
pub const LINK_LIFETIME_SECS: u64 = 24 * 60 * 60;
/// How many characters a mailbox code has (50 bits).
pub const CODE_LEN: usize = 10;
/// The longest display name, in characters.
pub const MAX_NAME_CHARS: usize = 32;
/// The most replies a mailbox takes.
pub const MAX_REPLIES: usize = 4;

const VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Names

/// A display name as others will see it: control characters, bidirectional formatting
/// and zero-width characters removed, runs of spaces made one, at most
/// [`MAX_NAME_CHARS`] characters. Empty if nothing is left.
pub fn clean_name(name: &str) -> String {
    let kept: String = name
        .chars()
        .filter(|c| !c.is_control() && !is_invisible(*c))
        .collect();
    kept.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(MAX_NAME_CHARS)
        .collect()
}

/// Characters that show as nothing, or change how the characters around them look:
/// bidirectional formatting, zero-width and joiner characters, soft hyphens, variation
/// selectors, tags and other default-ignorable code points. Any of them can make two
/// names that look the same differ, or hide part of a name.
fn is_invisible(c: char) -> bool {
    matches!(
        c,
        '\u{00AD}'
            | '\u{034F}'
            | '\u{061C}'
            | '\u{115F}'
            | '\u{1160}'
            | '\u{17B4}'
            | '\u{17B5}'
            | '\u{180B}'..='\u{180F}'
            | '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{3164}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FEFF}'
            | '\u{FFA0}'
            | '\u{FFF0}'..='\u{FFF8}'
            | '\u{1BCA0}'..='\u{1BCA3}'
            | '\u{1D173}'..='\u{1D17A}'
            | '\u{E0000}'..='\u{E0FFF}'
    )
}

/// A rough "looks like" form of a name, to warn when a new friend's name looks like an
/// existing friend's: lower case, without spaces, with common Cyrillic and Greek
/// look-alikes and a few digits turned into the Latin letters they resemble. Only for
/// warnings: the safety code is the real check.
pub fn name_skeleton(name: &str) -> String {
    clean_name(name)
        .to_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            'а' | 'α' => 'a',
            'в' | 'β' | 'ь' => 'b',
            'с' | 'ϲ' => 'c',
            'е' | 'ё' | 'ε' => 'e',
            'һ' | 'н' => 'h',
            'і' | 'ї' | 'ι' | '1' | '|' | 'ӏ' | 'i' => 'l',
            'ј' => 'j',
            'к' | 'κ' => 'k',
            'м' => 'm',
            'п' | 'η' => 'n',
            'о' | 'ο' | 'σ' | '0' => 'o',
            'р' | 'ρ' => 'p',
            'ѕ' => 's',
            'т' | 'τ' => 't',
            'υ' | 'μ' => 'u',
            'ν' => 'v',
            'ѡ' | 'ω' => 'w',
            'х' | 'χ' => 'x',
            'у' | 'γ' => 'y',
            other => other,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Links

/// A friend link: `https://connect.lodestone.cc/f/<code>#<key hash>.<secret>`.
///
/// The code names the mailbox at the service and is derived from the secret. The part
/// after `#` (the first 16 bytes of SHA-256 of the sender's key, and the 32-byte link
/// secret) is never sent to the service.
#[derive(Clone, PartialEq, Eq)]
pub struct FriendLink {
    pub service: Service,
    pub code: String,
    /// [`PublicKey::short_hash`] of the sender's key.
    pub key_hash: [u8; 16],
    pub secret: Secret,
}

impl std::fmt::Debug for FriendLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FriendLink")
            .field("service", &self.service.base_url())
            .field("code", &self.code)
            .finish_non_exhaustive()
    }
}

impl FriendLink {
    /// A new link for `sender`'s key at `service`.
    pub fn new(service: Service, sender: &PublicKey) -> Result<Self> {
        let secret = Secret::random()?;
        Ok(Self {
            service,
            code: code_for(&secret),
            key_hash: sender.short_hash(),
            secret,
        })
    }

    /// The link to send.
    pub fn link(&self) -> String {
        format!(
            "{}/f/{}#{}.{}",
            self.service.base_url(),
            self.code,
            hex(&self.key_hash),
            self.secret.to_hex()
        )
    }

    /// `lodestone://friend?link=<the link, percent-encoded>`.
    pub fn deep_link(&self) -> String {
        let link: String = url::form_urlencoded::byte_serialize(self.link().as_bytes()).collect();
        format!("{FRIEND_DEEP_LINK_PREFIX}?link={link}")
    }

    /// Reads a friend link or its deep link. Spaces around it are ignored.
    pub fn parse(text: &str) -> Result<Self, LinkError> {
        let text = text.trim();
        if let Some(query) = text.strip_prefix(FRIEND_DEEP_LINK_PREFIX) {
            let query = query
                .strip_prefix('?')
                .ok_or(LinkError("the deep link has no link in it"))?;
            let link = url::form_urlencoded::parse(query.as_bytes())
                .find(|(name, _)| name == "link")
                .map(|(_, value)| value.into_owned())
                .ok_or(LinkError("the deep link has no link in it"))?;
            if link.starts_with(FRIEND_DEEP_LINK_PREFIX) {
                return Err(LinkError("a deep link inside a deep link"));
            }
            return Self::parse(&link);
        }
        let (service, rest) = if let Some(rest) = text.strip_prefix("https://") {
            let (place, rest) = rest.split_once('/').ok_or(LinkError("no code"))?;
            (
                Service::host_port(place, 443, true).ok_or(LinkError("bad address"))?,
                rest,
            )
        } else if let Some(rest) = text.strip_prefix("http://") {
            // plain HTTP only for a test service on this computer
            let (place, rest) = rest.split_once('/').ok_or(LinkError("no code"))?;
            let service = Service::host_port(place, 80, false).ok_or(LinkError("bad address"))?;
            let loopback = service
                .host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback());
            if !loopback {
                return Err(LinkError("it should start with https://"));
            }
            (service, rest)
        } else {
            return Err(LinkError("it should start with https://"));
        };
        let rest = rest
            .strip_prefix("f/")
            .ok_or(LinkError("that's not a friend link"))?;
        let (code, fragment) = rest
            .split_once('#')
            .ok_or(LinkError("the part after # is missing"))?;
        let code = code.trim_end_matches('/').to_ascii_uppercase();
        let (key_hash, secret) = fragment
            .split_once('.')
            .ok_or(LinkError("the part after # is cut short"))?;
        let key_hash: [u8; 16] = unhex(key_hash)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(LinkError("bad key"))?;
        let secret = Secret::from_hex(secret).ok_or(LinkError("bad secret"))?;
        if code != code_for(&secret) {
            return Err(LinkError("the code doesn't match the rest of the link"));
        }
        Ok(Self {
            service,
            code,
            key_hash,
            secret,
        })
    }

    /// What proves to the service that a request comes from someone with the link
    /// (Bearer, 64 hex digits).
    pub fn access(&self) -> String {
        hex(&self.secret.derive("pair-access", self.code.as_bytes()))
    }

    /// What the service keeps to check [`FriendLink::access`]: SHA-256 of its hex.
    pub fn access_hash(&self) -> String {
        hex(&sha256(self.access().as_bytes()))
    }
}

/// The mailbox code for a link secret: 10 Crockford base32 characters.
pub fn code_for(secret: &Secret) -> String {
    crockford(&secret.derive("pair-code", &[]), CODE_LEN)
}

// ---------------------------------------------------------------------------
// Cards and replies

#[derive(Serialize, Deserialize)]
struct CardWire {
    v: u32,
    key: String,
    exchange: String,
    name: String,
    code: String,
    expires: u64,
    sig: String,
}

#[derive(Serialize, Deserialize)]
struct ReplyWire {
    v: u32,
    key: String,
    exchange: String,
    name: String,
    sig: String,
}

/// The inviter's card, as checked.
#[derive(Clone, Debug)]
pub struct Card {
    pub key: PublicKey,
    pub name: String,
    pub expires: u64,
    exchange: [u8; PUBLIC_KEY_LEN],
    /// The card as sent: what the reply and the pair secret are bound to.
    bytes: Vec<u8>,
}

/// An answer to a card, as checked.
#[derive(Clone, Debug)]
pub struct Reply {
    pub key: PublicKey,
    pub name: String,
    exchange: [u8; PUBLIC_KEY_LEN],
    bytes: Vec<u8>,
}

fn card_message(
    key: &PublicKey,
    exchange: &[u8],
    name: &str,
    code: &str,
    expires: u64,
    link_secret: &Secret,
) -> Vec<u8> {
    transcript(&[
        &VERSION.to_be_bytes(),
        &key.0,
        exchange,
        name.as_bytes(),
        code.as_bytes(),
        &expires.to_be_bytes(),
        &sha256(link_secret.as_bytes()),
    ])
}

fn reply_message(card_hash: &[u8; 32], key: &PublicKey, exchange: &[u8], name: &str) -> Vec<u8> {
    transcript(&[
        &VERSION.to_be_bytes(),
        card_hash,
        &key.0,
        exchange,
        name.as_bytes(),
    ])
}

fn pair_secret(shared: &Secret, link_secret: &Secret, card: &[u8], reply: &[u8]) -> Secret {
    let mut input = Vec::with_capacity(64);
    input.extend_from_slice(shared.as_bytes());
    input.extend_from_slice(link_secret.as_bytes());
    let out = crypto::derive(&input, "pair", &transcript(&[&sha256(card), &sha256(reply)]));
    zeroize::Zeroize::zeroize(&mut input);
    Secret::from_bytes(out)
}

fn bad(what: &str) -> Error {
    Error::Other(format!("This friend link didn't check out ({what}). Ask for a new one."))
}

fn key32(text: &str) -> Option<[u8; 32]> {
    unhex(text)?.try_into().ok()
}

/// Opens the card in a mailbox with the link that named it, and checks it: the key
/// matches the link, the signature is good, it's for this link and hasn't expired.
pub fn open_card(link: &FriendLink, sealed: &[u8], now: u64) -> Result<Card> {
    let bytes = crypto::open(&link.secret, Role::Card, link.code.as_bytes(), sealed)
        .ok_or_else(|| bad("it couldn't be opened"))?;
    let wire: CardWire = serde_json::from_slice(&bytes).map_err(|_| bad("garbled"))?;
    if wire.v != VERSION {
        return Err(bad("a newer version of Lodestone made it"));
    }
    let key = PublicKey(key32(&wire.key).ok_or_else(|| bad("key"))?);
    if key.short_hash() != link.key_hash {
        return Err(bad("it isn't from whoever made the link"));
    }
    let exchange = key32(&wire.exchange).ok_or_else(|| bad("key"))?;
    let sig = unhex(&wire.sig).ok_or_else(|| bad("signature"))?;
    let message = card_message(&key, &exchange, &wire.name, &wire.code, wire.expires, &link.secret);
    if !key.verify("card", &message, &sig) {
        return Err(bad("signature"));
    }
    if wire.code != link.code {
        return Err(bad("it's for another link"));
    }
    if now >= wire.expires {
        return Err(Error::Other(
            "This friend link has expired. Ask for a new one.".into(),
        ));
    }
    let name = clean_name(&wire.name);
    if name.is_empty() || name != wire.name {
        return Err(bad("name"));
    }
    Ok(Card {
        key,
        name,
        expires: wire.expires,
        exchange,
        bytes,
    })
}

/// Ben's answer to a card: the sealed reply for the mailbox, and the pair secret.
pub struct Answer {
    pub sealed: Vec<u8>,
    pub pair: Secret,
}

/// Answers `card` as `me` (with display name `name`): makes an exchange key, signs a
/// reply bound to the card, and computes the pair secret. The exchange key is dropped
/// here, so it's never stored.
pub fn answer(me: &PersonKey, name: &str, link: &FriendLink, card: &Card) -> Result<Answer> {
    let name = clean_name(name);
    if name.is_empty() {
        return Err(Error::Other("Choose a name your friend will see.".into()));
    }
    if me.public() == card.key {
        return Err(Error::Other("That's your own friend link.".into()));
    }
    let exchange = ExchangeKey::generate()?;
    let exchange_public = exchange.public()?;
    let key = me.public();
    let sig = me.sign(
        "reply",
        &reply_message(&sha256(&card.bytes), &key, &exchange_public, &name),
    );
    let bytes = serde_json::to_vec(&ReplyWire {
        v: VERSION,
        key: key.to_hex(),
        exchange: hex(&exchange_public),
        name,
        sig: hex(&sig),
    })
    .expect("serializable");
    let shared = exchange.agree(&card.exchange)?;
    let pair = pair_secret(&shared, &link.secret, &card.bytes, &bytes);
    let sealed = crypto::seal(&link.secret, Role::Reply, link.code.as_bytes(), &bytes)?;
    Ok(Answer { sealed, pair })
}

// ---------------------------------------------------------------------------
// The inviter's side

/// A friend link waiting for answers: everything Ana keeps (sealed, in her friend file)
/// until it's used or expires.
#[derive(Clone)]
pub struct Invitation {
    pub link: FriendLink,
    pub expires: u64,
    /// The mailbox's owner secret at the service, once created.
    pub mailbox_secret: Option<String>,
    exchange_seed: Secret,
    card: Vec<u8>,
}

impl std::fmt::Debug for Invitation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Invitation")
            .field("link", &self.link)
            .field("expires", &self.expires)
            .finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize)]
struct InvitationFile {
    v: u32,
    link: String,
    expires: u64,
    #[serde(default)]
    mailbox_secret: Option<String>,
    exchange_seed: String,
    card: String,
}

impl Invitation {
    /// A new friend link from `me` (display name `name`) at `service`, and the sealed
    /// card to put in its mailbox.
    pub fn create(me: &PersonKey, name: &str, service: Service, now: u64) -> Result<(Self, Vec<u8>)> {
        let name = clean_name(name);
        if name.is_empty() {
            return Err(Error::Other("Choose a name your friend will see.".into()));
        }
        let key = me.public();
        let link = FriendLink::new(service, &key)?;
        let exchange = ExchangeKey::generate()?;
        let exchange_public = exchange.public()?;
        let expires = now + LINK_LIFETIME_SECS;
        let sig = me.sign(
            "card",
            &card_message(&key, &exchange_public, &name, &link.code, expires, &link.secret),
        );
        let card = serde_json::to_vec(&CardWire {
            v: VERSION,
            key: key.to_hex(),
            exchange: hex(&exchange_public),
            name,
            code: link.code.clone(),
            expires,
            sig: hex(&sig),
        })
        .expect("serializable");
        let sealed = crypto::seal(&link.secret, Role::Card, link.code.as_bytes(), &card)?;
        Ok((
            Self {
                link,
                expires,
                mailbox_secret: None,
                exchange_seed: exchange.seed()?,
                card,
            },
            sealed,
        ))
    }

    /// Opens and checks a reply from the mailbox: signed by its sender and bound to this
    /// card. `me` is the inviter, whose own key is refused as a reply.
    pub fn open_reply(&self, me: &PublicKey, sealed: &[u8]) -> Result<Reply> {
        let bad = |what: &str| Error::Other(format!("A reply to your friend link didn't check out ({what})."));
        let bytes = crypto::open(&self.link.secret, Role::Reply, self.link.code.as_bytes(), sealed)
            .ok_or_else(|| bad("it couldn't be opened"))?;
        let wire: ReplyWire = serde_json::from_slice(&bytes).map_err(|_| bad("garbled"))?;
        if wire.v != VERSION {
            return Err(bad("a newer version of Lodestone made it"));
        }
        let key = PublicKey(key32(&wire.key).ok_or_else(|| bad("key"))?);
        if &key == me {
            return Err(bad("it's your own key"));
        }
        let exchange = key32(&wire.exchange).ok_or_else(|| bad("key"))?;
        let sig = unhex(&wire.sig).ok_or_else(|| bad("signature"))?;
        if !key.verify(
            "reply",
            &reply_message(&sha256(&self.card), &key, &exchange, &wire.name),
            &sig,
        ) {
            return Err(bad("signature"));
        }
        let name = clean_name(&wire.name);
        if name.is_empty() || name != wire.name {
            return Err(bad("name"));
        }
        Ok(Reply {
            key,
            name,
            exchange,
            bytes,
        })
    }

    /// The pair secret with the sender of `reply`. Delete the invitation afterwards: its
    /// exchange key must not outlive the pairing.
    pub fn accept(&self, reply: &Reply) -> Result<Secret> {
        let exchange = ExchangeKey::from_seed(&self.exchange_seed)?;
        let shared = exchange.agree(&reply.exchange)?;
        Ok(pair_secret(&shared, &self.link.secret, &self.card, &reply.bytes))
    }

    /// The invitation as JSON, with its secrets: keep it sealed.
    pub fn to_json(&self) -> String {
        serde_json::to_string(&InvitationFile {
            v: VERSION,
            link: self.link.link(),
            expires: self.expires,
            mailbox_secret: self.mailbox_secret.clone(),
            exchange_seed: self.exchange_seed.to_hex(),
            card: hex(&self.card),
        })
        .expect("serializable")
    }

    pub fn from_json(text: &str) -> Result<Self> {
        let bad = || Error::Other("A saved friend link is damaged".into());
        let file: InvitationFile = serde_json::from_str(text).map_err(|_| bad())?;
        if file.v != VERSION {
            return Err(bad());
        }
        Ok(Self {
            link: FriendLink::parse(&file.link).map_err(|_| bad())?,
            expires: file.expires,
            mailbox_secret: file.mailbox_secret,
            exchange_seed: Secret::from_hex(&file.exchange_seed).ok_or_else(bad)?,
            card: unhex(&file.card).ok_or_else(bad)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn two_people_become_friends() {
        let ana = PersonKey::generate().unwrap();
        let ben = PersonKey::generate().unwrap();
        let (invitation, sealed_card) =
            Invitation::create(&ana, "Ana", Service::default(), NOW).unwrap();
        let text = invitation.link.link();
        assert!(text.starts_with("https://connect.lodestone.cc/f/"), "{text}");

        // Ben, from the link alone
        let link = FriendLink::parse(&text).unwrap();
        assert_eq!(FriendLink::parse(&link.deep_link()).unwrap(), link);
        let card = open_card(&link, &sealed_card, NOW + 60).unwrap();
        assert_eq!((card.key, card.name.as_str()), (ana.public(), "Ana"));
        let answer = answer(&ben, "Ben", &link, &card).unwrap();

        // Ana, after a restart
        let invitation = Invitation::from_json(&invitation.to_json()).unwrap();
        let reply = invitation.open_reply(&ana.public(), &answer.sealed).unwrap();
        assert_eq!((reply.key, reply.name.as_str()), (ben.public(), "Ben"));
        assert_eq!(invitation.accept(&reply).unwrap(), answer.pair);
    }

    #[test]
    fn cards_are_checked() {
        let ana = PersonKey::generate().unwrap();
        let (invitation, sealed) = Invitation::create(&ana, "Ana", Service::default(), NOW).unwrap();
        let link = invitation.link.clone();
        // expired
        assert!(open_card(&link, &sealed, NOW + LINK_LIFETIME_SECS).is_err());
        // a link naming someone else's key: the card was swapped
        let mallory = PersonKey::generate().unwrap();
        let mut swapped = link.clone();
        swapped.key_hash = mallory.public().short_hash();
        assert!(open_card(&swapped, &sealed, NOW).is_err());
        // Ana's real card, sealed again under another link (Mallory has the plaintext
        // because she opened Ana's link): the signature covers the first link's secret
        let plain = crypto::open(&link.secret, Role::Card, link.code.as_bytes(), &sealed).unwrap();
        let other = FriendLink::new(Service::default(), &ana.public()).unwrap();
        let resealed = crypto::seal(&other.secret, Role::Card, other.code.as_bytes(), &plain).unwrap();
        assert!(open_card(&other, &resealed, NOW).is_err());
        // your own link
        let card = open_card(&link, &sealed, NOW).unwrap();
        assert!(answer(&ana, "Ana", &link, &card).is_err());
    }

    #[test]
    fn replies_are_bound_to_the_card() {
        let ana = PersonKey::generate().unwrap();
        let ben = PersonKey::generate().unwrap();
        let (first, sealed_first) = Invitation::create(&ana, "Ana", Service::default(), NOW).unwrap();
        let (second, _) = Invitation::create(&ana, "Ana", Service::default(), NOW).unwrap();
        let card = open_card(&first.link, &sealed_first, NOW).unwrap();
        let answer = answer(&ben, "Ben", &first.link, &card).unwrap();
        assert!(first.open_reply(&ana.public(), &answer.sealed).is_ok());
        // the reply to the first link can't be used on the second
        assert!(second.open_reply(&ana.public(), &answer.sealed).is_err());
        // a reply that names Ana herself is refused
        let own = super::answer(&PersonKey::from_pkcs8(ana.pkcs8()).unwrap(), "x", &first.link, &card);
        assert!(own.is_err());
    }

    #[test]
    fn links_parse_strictly() {
        let ana = PersonKey::generate().unwrap();
        let link = FriendLink::new(Service::default(), &ana.public()).unwrap();
        let text = link.link();
        assert_eq!(FriendLink::parse(&format!("  {text}  ")).unwrap(), link);
        // the code must match the secret
        let tampered = text.replacen(&link.code, "0000000000", 1);
        assert!(FriendLink::parse(&tampered).is_err());
        assert!(FriendLink::parse(&text.replace("/f/", "/j/")).is_err());
        assert!(FriendLink::parse(text.split('#').next().unwrap()).is_err());
        assert!(FriendLink::parse("lodestone://friend?link=lodestone%3A%2F%2Ffriend").is_err());
        // plain HTTP only to this computer
        let local = FriendLink::new(Service::parse("http://127.0.0.1:8787").unwrap(), &ana.public()).unwrap();
        assert!(FriendLink::parse(&local.link()).is_ok());
        let lan = local.link().replace("127.0.0.1", "192.168.1.1");
        assert!(FriendLink::parse(&lan).is_err());
        assert_eq!(link.access().len(), 64);
        assert_eq!(link.access_hash().len(), 64);
        assert!(!format!("{link:?}").contains(&link.secret.to_hex()));
    }

    #[test]
    fn names_are_cleaned() {
        assert_eq!(clean_name("  Ana\u{202E}  Smith\u{200B} "), "Ana Smith");
        assert_eq!(clean_name("\u{0007}\u{2066}"), "");
        assert_eq!(clean_name(&"x".repeat(50)).chars().count(), MAX_NAME_CHARS);
        assert_eq!(clean_name("Zoë 🙂"), "Zoë 🙂");
        assert_eq!(clean_name("A\u{ad}na\u{E0041}\u{FE0F}\u{17B4}"), "Ana");
        assert_eq!(name_skeleton("Ana"), name_skeleton("Аna"), "a Cyrillic А");
        assert_eq!(name_skeleton("Bob 1"), name_skeleton("B0b l"));
        assert_ne!(name_skeleton("Ana"), name_skeleton("Ben"));
    }
}
