//! The cryptography under the friend list: Ed25519 for people's identities (`ring`),
//! X25519 for pairing (`aws-lc-rs`, whose keys can be stored, unlike `ring`'s),
//! HKDF-SHA256 to derive every other key, and ChaCha20-Poly1305 to seal what passes
//! through the introduction service.
//!
//! Every derivation and signature is domain-separated by a label, and everything signed
//! or derived from several values is a length-prefixed [`transcript`], so a key or
//! signature made for one purpose is never valid for another.

use std::fmt;

use aws_lc_rs::{
    agreement::{self, UnparsedPublicKey, X25519},
    encoding::{AsBigEndian as _, Curve25519SeedBin},
};
use ring::{
    aead, hkdf,
    rand::SystemRandom,
    signature::{self, Ed25519KeyPair, KeyPair as _},
};
use sha2::{Digest as _, Sha256};
use zeroize::Zeroize as _;

use crate::{hex, unhex, Error, Result};

/// A person's public key (Ed25519), or an X25519 public key.
pub const PUBLIC_KEY_LEN: usize = 32;
/// An Ed25519 signature.
pub const SIGNATURE_LEN: usize = 64;
/// Every secret here: pair secrets, core secrets, link secrets.
pub const SECRET_LEN: usize = 32;
/// A lookup id or slot id at the service.
pub const ID_LEN: usize = 16;
/// The nonce at the start of a sealed blob.
const NONCE_LEN: usize = 12;
/// What sealing adds to a message: the nonce and the tag.
pub const SEAL_OVERHEAD: usize = NONCE_LEN + 16;

/// Starts every signed message and every derivation.
const DOMAIN: &[u8] = b"lodestone-friend-v1";

/// Values joined so that no two lists give the same bytes: each is its length (4 bytes,
/// big-endian) and then its bytes.
pub fn transcript(fields: &[&[u8]]) -> Vec<u8> {
    let mut out = Vec::with_capacity(fields.iter().map(|f| f.len() + 4).sum());
    for field in fields {
        out.extend_from_slice(&u32::try_from(field.len()).expect("small").to_be_bytes());
        out.extend_from_slice(field);
    }
    out
}

pub fn sha256(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

// ---------------------------------------------------------------------------
// Secrets

/// 32 secret bytes. Never printed, wiped when dropped.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(pub(crate) [u8; SECRET_LEN]);

impl Secret {
    /// A new random secret.
    pub fn random() -> Result<Self> {
        Ok(Self(crate::random()?))
    }

    pub fn from_bytes(bytes: [u8; SECRET_LEN]) -> Self {
        Self(bytes)
    }

    pub fn from_hex(text: &str) -> Option<Self> {
        Some(Self(unhex(text)?.try_into().ok()?))
    }

    /// The secret in hex, to store (in the Keychain, or a file only its user can read).
    pub fn to_hex(&self) -> String {
        hex(&self.0)
    }

    pub fn as_bytes(&self) -> &[u8; SECRET_LEN] {
        &self.0
    }

    /// A key for `label`, bound to `context`: HKDF-SHA256 with this secret.
    pub fn derive(&self, label: &str, context: &[u8]) -> [u8; SECRET_LEN] {
        derive(&self.0, label, context)
    }

    /// [`Secret::derive`], as a secret.
    pub fn child(&self, label: &str, context: &[u8]) -> Secret {
        Secret(self.derive(label, context))
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(..)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        // Best effort: copies made on the way may remain, but the stored value goes.
        self.0.zeroize();
    }
}

struct Len(usize);

impl hkdf::KeyType for Len {
    fn len(&self) -> usize {
        self.0
    }
}

/// HKDF-SHA256: salt [`DOMAIN`], input `secret`, info `label | 0 | context`.
pub(crate) fn derive(secret: &[u8], label: &str, context: &[u8]) -> [u8; SECRET_LEN] {
    let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, DOMAIN);
    let info = [label.as_bytes(), &[0u8], context];
    let mut out = [0u8; SECRET_LEN];
    salt.extract(secret)
        .expand(&info, Len(SECRET_LEN))
        .and_then(|okm| okm.fill(&mut out))
        .expect("32 bytes is a valid HKDF-SHA256 length");
    out
}

// ---------------------------------------------------------------------------
// People's keys

/// A person's public key: their id in the friend list.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PublicKey(pub [u8; PUBLIC_KEY_LEN]);

impl PublicKey {
    pub fn from_hex(text: &str) -> Option<Self> {
        Some(Self(unhex(text)?.try_into().ok()?))
    }

    pub fn to_hex(&self) -> String {
        hex(&self.0)
    }

    /// Whether `sig` is this key's signature of `message` for `context`.
    pub fn verify(&self, context: &str, message: &[u8], sig: &[u8]) -> bool {
        sig.len() == SIGNATURE_LEN
            && signature::UnparsedPublicKey::new(&signature::ED25519, &self.0)
                .verify(&signed_message(context, message), sig)
                .is_ok()
    }

    /// The first 16 bytes of the key's SHA-256: what a friend link carries to check the
    /// sender's card against.
    pub fn short_hash(&self) -> [u8; 16] {
        sha256(&self.0)[..16].try_into().expect("16 bytes")
    }

    /// What people compare to check they have each other's real key:
    /// 80 bits of the key's SHA-256 as four groups of four Crockford base32 characters,
    /// such as `K7Q2-M9XD-4TRW-H3BE`. 80 bits keeps a forged key with the same code
    /// out of reach (2^80 tries).
    pub fn safety_code(&self) -> String {
        let digest = Sha256::new()
            .chain_update(DOMAIN)
            .chain_update(b"\0safety code\0")
            .chain_update(self.0)
            .finalize();
        let code = crockford(&digest[..10], 16);
        let mut out = String::with_capacity(19);
        for (i, c) in code.chars().enumerate() {
            if i > 0 && i % 4 == 0 {
                out.push('-');
            }
            out.push(c);
        }
        out
    }
}

impl fmt::Debug for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PublicKey({})", self.safety_code())
    }
}

pub(crate) const CROCKFORD: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// The first `chars` characters (5 bits each) of `bytes` in Crockford base32.
pub(crate) fn crockford(bytes: &[u8], chars: usize) -> String {
    assert!(chars * 5 <= bytes.len() * 8);
    (0..chars)
        .map(|i| {
            let bit = i * 5;
            let mut value: u16 = u16::from(bytes[bit / 8]) << 8;
            if bit / 8 + 1 < bytes.len() {
                value |= u16::from(bytes[bit / 8 + 1]);
            }
            let index = (value >> (11 - bit % 8)) & 31;
            CROCKFORD[index as usize] as char
        })
        .collect()
}

/// `DOMAIN | 0 | context | 0 | message`.
fn signed_message(context: &str, message: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(DOMAIN.len() + context.len() + message.len() + 2);
    out.extend_from_slice(DOMAIN);
    out.push(0);
    out.extend_from_slice(context.as_bytes());
    out.push(0);
    out.extend_from_slice(message);
    out
}

/// A person's identity: their Ed25519 key pair. Its PKCS#8 form is what's stored, in the
/// Keychain only. Never printed.
pub struct PersonKey {
    pkcs8: Vec<u8>,
    pair: Ed25519KeyPair,
}

impl PersonKey {
    pub fn generate() -> Result<Self> {
        let pkcs8 = Ed25519KeyPair::generate_pkcs8(&SystemRandom::new())
            .map_err(|_| Error::Other("This computer gave no random numbers".into()))?;
        Self::from_pkcs8(pkcs8.as_ref())
    }

    pub fn from_pkcs8(bytes: &[u8]) -> Result<Self> {
        let pair = Ed25519KeyPair::from_pkcs8(bytes)
            .map_err(|_| Error::Other("The saved friend identity is damaged".into()))?;
        Ok(Self {
            pkcs8: bytes.to_vec(),
            pair,
        })
    }

    /// The key pair in PKCS#8, to store in the Keychain.
    pub fn pkcs8(&self) -> &[u8] {
        &self.pkcs8
    }

    pub fn public(&self) -> PublicKey {
        PublicKey(
            self.pair
                .public_key()
                .as_ref()
                .try_into()
                .expect("Ed25519 public keys are 32 bytes"),
        )
    }

    /// Signs `message` for `context` (see [`PublicKey::verify`]).
    pub fn sign(&self, context: &str, message: &[u8]) -> [u8; SIGNATURE_LEN] {
        self.pair
            .sign(&signed_message(context, message))
            .as_ref()
            .try_into()
            .expect("Ed25519 signatures are 64 bytes")
    }
}

impl fmt::Debug for PersonKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PersonKey({})", self.public().safety_code())
    }
}

impl Drop for PersonKey {
    fn drop(&mut self) {
        self.pkcs8.zeroize();
    }
}

// ---------------------------------------------------------------------------
// Key exchange

/// An X25519 key pair for one pairing. Its seed is stored (sealed) only until the
/// pairing ends, then deleted: that's what keeps old pairings secret (forward secrecy).
pub struct ExchangeKey(agreement::PrivateKey);

impl ExchangeKey {
    pub fn generate() -> Result<Self> {
        agreement::PrivateKey::generate(&X25519)
            .map(Self)
            .map_err(|_| Error::Other("Couldn't make a key".into()))
    }

    pub fn from_seed(seed: &Secret) -> Result<Self> {
        agreement::PrivateKey::from_private_key(&X25519, seed.as_bytes())
            .map(Self)
            .map_err(|_| Error::Other("A saved pairing key is damaged".into()))
    }

    /// The private key's 32 bytes, to keep until the pairing ends.
    pub fn seed(&self) -> Result<Secret> {
        let seed: Curve25519SeedBin<'_> = self
            .0
            .as_be_bytes()
            .map_err(|_| Error::Other("Couldn't read a key".into()))?;
        Ok(Secret(
            seed.as_ref()
                .try_into()
                .map_err(|_| Error::Other("Couldn't read a key".into()))?,
        ))
    }

    pub fn public(&self) -> Result<[u8; PUBLIC_KEY_LEN]> {
        let public = self
            .0
            .compute_public_key()
            .map_err(|_| Error::Other("Couldn't read a key".into()))?;
        public
            .as_ref()
            .try_into()
            .map_err(|_| Error::Other("Couldn't read a key".into()))
    }

    /// The shared secret with `peer`'s public key. Refuses keys that give an all-zero
    /// result (low-order points), which would make the secret known to anyone.
    pub fn agree(&self, peer: &[u8; PUBLIC_KEY_LEN]) -> Result<Secret> {
        let refused = || Error::Other("Your friend's pairing key isn't valid".into());
        let shared: [u8; SECRET_LEN] = agreement::agree(
            &self.0,
            UnparsedPublicKey::new(&X25519, peer),
            refused(),
            |shared| shared.try_into().map_err(|_| refused()),
        )?;
        if shared.iter().all(|b| *b == 0) {
            return Err(refused());
        }
        Ok(Secret(shared))
    }
}

impl fmt::Debug for ExchangeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExchangeKey(..)")
    }
}

// ---------------------------------------------------------------------------
// Sealing

/// What a sealed blob is, so one kind can never be opened as another (a core's record
/// handed back as a knock, say). Part of both the key and the associated data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Role {
    /// A core's addresses, published for one friend.
    Record = 1,
    /// A friend's addresses, knocking on a core.
    Knock = 2,
    /// The inviter's card, in a friend-link mailbox.
    Card = 3,
    /// The answer to a card.
    Reply = 4,
    /// A person's signed state, in their slot for a friend.
    State = 5,
    /// The launcher's friend file on disk.
    File = 6,
}

/// Seals `plain` with a key derived from `secret` for `role` and `context` (a lookup id,
/// a mailbox code). The blob is `nonce | ciphertext | tag`, with a random nonce.
pub fn seal(secret: &Secret, role: Role, context: &[u8], plain: &[u8]) -> Result<Vec<u8>> {
    let key = seal_key(secret, role, context);
    let nonce: [u8; NONCE_LEN] = crate::random()?;
    let mut out = Vec::with_capacity(plain.len() + SEAL_OVERHEAD);
    out.extend_from_slice(&nonce);
    let mut body = plain.to_vec();
    key.seal_in_place_append_tag(
        aead::Nonce::assume_unique_for_key(nonce),
        aead::Aad::from(associated(role, context)),
        &mut body,
    )
    .map_err(|_| Error::Other("Couldn't seal a message".into()))?;
    out.extend_from_slice(&body);
    body.zeroize();
    Ok(out)
}

/// Opens a blob from [`seal`]: `None` if it was sealed with another secret, role or
/// context, or changed on the way.
pub fn open(secret: &Secret, role: Role, context: &[u8], blob: &[u8]) -> Option<Vec<u8>> {
    if blob.len() < SEAL_OVERHEAD {
        return None;
    }
    let key = seal_key(secret, role, context);
    let nonce: [u8; NONCE_LEN] = blob[..NONCE_LEN].try_into().ok()?;
    let mut body = blob[NONCE_LEN..].to_vec();
    let plain = key
        .open_in_place(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(associated(role, context)),
            &mut body,
        )
        .ok()?
        .to_vec();
    body.zeroize();
    Some(plain)
}

fn seal_key(secret: &Secret, role: Role, context: &[u8]) -> aead::LessSafeKey {
    let key = secret.derive("seal", &transcript(&[&[role as u8], context]));
    aead::LessSafeKey::new(
        aead::UnboundKey::new(&aead::CHACHA20_POLY1305, &key).expect("32-byte key"),
    )
}

fn associated(role: Role, context: &[u8]) -> Vec<u8> {
    transcript(&[DOMAIN, &[role as u8], context])
}

// ---------------------------------------------------------------------------
// Cores and lookups

/// Seconds in a lookup period.
pub const PERIOD_SECS: u64 = 24 * 60 * 60;

/// The secret one core shares with one friend: derived from the pair secret, the core
/// owner's key, the friend's key and the core's certificate fingerprint, so it differs
/// by core and by direction. A core only ever holds these, never the pair secret, so a
/// compromised core reveals nothing about the friendship's other cores.
pub fn core_secret(
    pair: &Secret,
    owner: &PublicKey,
    friend: &PublicKey,
    core_fingerprint: &[u8; 32],
) -> Secret {
    pair.child("core", &transcript(&[&owner.0, &friend.0, core_fingerprint]))
}

/// When this pair's ids change, as seconds into the day: each pair at its own time, so
/// a core's ids don't all change at once.
pub fn period_offset(core_secret: &Secret) -> u64 {
    let bytes = core_secret.derive("offset", &[]);
    u64::from_be_bytes(bytes[..8].try_into().expect("8 bytes")) % PERIOD_SECS
}

/// The period `unix_secs` falls in for this pair.
pub fn period(core_secret: &Secret, unix_secs: u64) -> u64 {
    (unix_secs + period_offset(core_secret)) / PERIOD_SECS
}

/// Seconds until this pair's next period starts.
pub fn secs_to_next_period(core_secret: &Secret, unix_secs: u64) -> u64 {
    let into = (unix_secs + period_offset(core_secret)) % PERIOD_SECS;
    PERIOD_SECS - into
}

/// Where a core's record for one friend is at the service during `period`.
pub fn lookup(core_secret: &Secret, period: u64) -> [u8; ID_LEN] {
    core_secret.derive("lookup", &period.to_be_bytes())[..ID_LEN]
        .try_into()
        .expect("16 bytes")
}

/// The probe tag for one knock: derived per knock, so watching the network doesn't
/// link one join of a core to the next.
pub fn probe_tag(core_secret: &Secret, knock_id: &[u8]) -> [u8; 8] {
    core_secret.derive("probe", knock_id)[..8]
        .try_into()
        .expect("8 bytes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signatures_are_bound_to_their_context() {
        let key = PersonKey::generate().unwrap();
        let public = key.public();
        let sig = key.sign("card", b"hello");
        assert!(public.verify("card", b"hello", &sig));
        assert!(!public.verify("hello", b"hello", &sig));
        assert!(!public.verify("card", b"hullo", &sig));
        let other = PersonKey::generate().unwrap();
        assert!(!other.public().verify("card", b"hello", &sig));
        assert!(!public.verify("card", b"hello", &sig[..63]));

        let back = PersonKey::from_pkcs8(key.pkcs8()).unwrap();
        assert_eq!(back.public(), public);
        assert!(!format!("{key:?}").contains(&hex(key.pkcs8())));
    }

    #[test]
    fn transcripts_never_collide() {
        assert_ne!(transcript(&[b"ab", b"c"]), transcript(&[b"a", b"bc"]));
        assert_ne!(transcript(&[b"", b"a"]), transcript(&[b"a"]));
    }

    #[test]
    fn safety_codes_look_right() {
        let code = PersonKey::generate().unwrap().public().safety_code();
        assert_eq!(code.len(), 19);
        assert_eq!(code.matches('-').count(), 3);
        assert!(code
            .chars()
            .all(|c| c == '-' || CROCKFORD.contains(&(c as u8))));
        assert_eq!(PublicKey([1; 32]).safety_code(), PublicKey([1; 32]).safety_code());
        assert_ne!(PublicKey([1; 32]).safety_code(), PublicKey([2; 32]).safety_code());
        assert_eq!(crockford(&[0xff, 0xff], 3), "ZZZ");
        assert_eq!(crockford(&[0x00, 0x00], 3), "000");
        assert_eq!(crockford(&[0b00001_000, 0b01_000000], 2), "11");
    }

    #[test]
    fn sealed_blobs_open_only_as_what_they_are() {
        let secret = Secret::random().unwrap();
        let blob = seal(&secret, Role::Record, b"lookup-a", b"addresses").unwrap();
        assert_eq!(blob.len(), 9 + SEAL_OVERHEAD);
        assert_eq!(
            open(&secret, Role::Record, b"lookup-a", &blob).as_deref(),
            Some(&b"addresses"[..])
        );
        // another role, context or secret
        assert!(open(&secret, Role::Knock, b"lookup-a", &blob).is_none());
        assert!(open(&secret, Role::Record, b"lookup-b", &blob).is_none());
        assert!(open(&Secret::random().unwrap(), Role::Record, b"lookup-a", &blob).is_none());
        // changed on the way
        let mut changed = blob.clone();
        *changed.last_mut().unwrap() ^= 1;
        assert!(open(&secret, Role::Record, b"lookup-a", &changed).is_none());
        assert!(open(&secret, Role::Record, b"lookup-a", &blob[..10]).is_none());
        // a fresh nonce each time
        assert_ne!(blob, seal(&secret, Role::Record, b"lookup-a", b"addresses").unwrap());
    }

    #[test]
    fn exchanges_agree_and_keys_survive_storing() {
        let a = ExchangeKey::generate().unwrap();
        let b = ExchangeKey::generate().unwrap();
        let ab = a.agree(&b.public().unwrap()).unwrap();
        assert_eq!(ab, b.agree(&a.public().unwrap()).unwrap());
        let stored = ExchangeKey::from_seed(&a.seed().unwrap()).unwrap();
        assert_eq!(stored.public().unwrap(), a.public().unwrap());
        assert_eq!(stored.agree(&b.public().unwrap()).unwrap(), ab);
        // a low-order point gives zeros: refused
        assert!(a.agree(&[0u8; 32]).is_err());
        let mut one = [0u8; 32];
        one[0] = 1;
        assert!(a.agree(&one).is_err());
    }

    #[test]
    fn core_secrets_differ_by_core_and_direction() {
        let pair = Secret::random().unwrap();
        let ana = PublicKey([1; 32]);
        let ben = PublicKey([2; 32]);
        let a = core_secret(&pair, &ana, &ben, &[1; 32]);
        assert_ne!(a, core_secret(&pair, &ana, &ben, &[2; 32]));
        assert_ne!(a, core_secret(&pair, &ben, &ana, &[1; 32]));
        assert_ne!(a.0, pair.0);
        assert_eq!(lookup(&a, 20_000), lookup(&a, 20_000));
        assert_ne!(lookup(&a, 20_000), lookup(&a, 20_001));
        assert_eq!(format!("{pair:?}"), "Secret(..)");
    }

    #[test]
    fn periods_turn_at_each_pairs_own_time() {
        let a = Secret::random().unwrap();
        let start = 1_800_000_000;
        let next = secs_to_next_period(&a, start);
        assert!(next >= 1 && next <= PERIOD_SECS);
        assert_eq!(period(&a, start + next), period(&a, start) + 1);
        assert_eq!(period(&a, start + next - 1), period(&a, start));
    }
}
