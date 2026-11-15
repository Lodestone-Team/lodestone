//! States and slots (launcher/docs/friends.md, "States and slots").
//!
//! A person's [`State`] is their signed, numbered list of the cores they share from. It
//! goes into one slot at the service for each friend, sealed with the pair secret and
//! signed for that friend alone. Receivers keep the highest number they've seen and
//! refuse older ones, so a slot can't be rolled back.
//!
//! A state may also list the person's Minecraft accounts ([`StatePlayer`]), so a friend's
//! Host can put them on a server's whitelist. They're signed apart (`players_sig`, over
//! everything the first signature covers and the accounts), so copies from before them
//! still check the first signature and ignore them.
//!
//! It may also say which of its cores are the person's Launcher sharing their open LAN
//! worlds ([`State::launchers`]) rather than a Lodestone core. The same rule holds: they're
//! signed apart (`launchers_sig`), and copies from before them see those cores as cores.

use serde::{Deserialize, Serialize};

use super::{
    crypto::{self, sha256, transcript, PersonKey, PublicKey, Role, Secret, ID_LEN},
    pair::clean_name,
};
use crate::{hex, unhex, Error, Result};

/// How long a state is valid after it was issued. Launchers issue a new one weekly.
pub const STATE_LIFETIME_SECS: u64 = 90 * 24 * 60 * 60;
/// How often a launcher issues a new state while it runs.
pub const REISSUE_AFTER_SECS: u64 = 7 * 24 * 60 * 60;
/// The most cores in a state.
pub const MAX_CORES: usize = 16;
/// A sealed state's largest size (the service takes 4 KiB).
pub const MAX_SEALED: usize = 4096;
/// The most Minecraft accounts in a state.
pub const MAX_PLAYERS: usize = 4;

const VERSION: u32 = 1;

/// A core someone shares from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StateCore {
    /// SHA-256 of the core's friend certificate.
    #[serde(with = "hex32")]
    pub fingerprint: [u8; 32],
    /// The core's name, as its owner's launcher read it (shown, never trusted).
    pub name: String,
}

/// One of a person's Minecraft accounts, for a friend's whitelist.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatePlayer {
    /// The player name: 1 to 16 letters, digits or underscores.
    pub name: String,
    /// The profile UUID (a Microsoft account's), or the offline one for an offline
    /// account.
    #[serde(with = "hex16")]
    pub uuid: [u8; 16],
    /// An offline account: it can join only servers in offline mode.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub offline: bool,
}

impl StatePlayer {
    /// Whether `name` can be a Minecraft player name.
    pub fn is_name(name: &str) -> bool {
        (1..=16).contains(&name.len())
            && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    }
}

/// A person's signed state for one friend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct State {
    /// Higher in each new state.
    pub seq: u64,
    /// Unix seconds.
    pub issued: u64,
    pub expires: u64,
    /// The person's display name.
    pub name: String,
    pub cores: Vec<StateCore>,
    /// The person retired this key: drop them.
    pub retired: bool,
    /// Their Minecraft accounts, at most [`MAX_PLAYERS`].
    pub players: Vec<StatePlayer>,
    /// The fingerprints of the cores in `cores` that are the person's Launcher, sharing
    /// the worlds they open to LAN (launcher/docs/friends.md, "LAN worlds"). Each must
    /// be in `cores`.
    pub launchers: Vec<[u8; 32]>,
}

impl State {
    pub fn is_expired(&self, now: u64) -> bool {
        now >= self.expires
    }
}

#[derive(Serialize, Deserialize)]
struct Wire {
    v: u32,
    seq: u64,
    issued: u64,
    expires: u64,
    name: String,
    cores: Vec<StateCore>,
    retired: bool,
    sig: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    players: Vec<StatePlayer>,
    /// Over [`players_message`]; present when `players` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    players_sig: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty", with = "hex32_list")]
    launchers: Vec<[u8; 32]>,
    /// Over [`launchers_message`]; present when `launchers` is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    launchers_sig: Option<String>,
}

fn message(receiver: &PublicKey, state: &State) -> Vec<u8> {
    let version = VERSION.to_be_bytes();
    let seq = state.seq.to_be_bytes();
    let issued = state.issued.to_be_bytes();
    let expires = state.expires.to_be_bytes();
    let count = u32::try_from(state.cores.len())
        .unwrap_or(u32::MAX)
        .to_be_bytes();
    let mut fields: Vec<&[u8]> = vec![
        &version,
        &receiver.0,
        &seq,
        &issued,
        &expires,
        state.name.as_bytes(),
        &count,
    ];
    for core in &state.cores {
        fields.push(&core.fingerprint);
        fields.push(core.name.as_bytes());
    }
    let retired = [u8::from(state.retired)];
    fields.push(&retired);
    transcript(&fields)
}

/// What `players_sig` signs: everything the state's signature does, then the accounts.
fn players_message(receiver: &PublicKey, state: &State) -> Vec<u8> {
    let base = message(receiver, state);
    let count = u32::try_from(state.players.len())
        .unwrap_or(u32::MAX)
        .to_be_bytes();
    let mut fields: Vec<&[u8]> = vec![&base, &count];
    let flags: Vec<[u8; 1]> = state
        .players
        .iter()
        .map(|player| [u8::from(player.offline)])
        .collect();
    for (player, flag) in state.players.iter().zip(&flags) {
        fields.push(player.name.as_bytes());
        fields.push(&player.uuid);
        fields.push(flag);
    }
    transcript(&fields)
}

/// What `launchers_sig` signs: everything the state's signature does, then the
/// launchers' fingerprints.
fn launchers_message(receiver: &PublicKey, state: &State) -> Vec<u8> {
    let base = message(receiver, state);
    let count = u32::try_from(state.launchers.len())
        .unwrap_or(u32::MAX)
        .to_be_bytes();
    let mut fields: Vec<&[u8]> = vec![&base, &count];
    for fingerprint in &state.launchers {
        fields.push(fingerprint);
    }
    transcript(&fields)
}

/// Every launcher is one of the state's cores, listed once.
fn launchers_in_bounds(state: &State) -> bool {
    state.launchers.iter().enumerate().all(|(i, fp)| {
        state.cores.iter().any(|core| &core.fingerprint == fp) && !state.launchers[..i].contains(fp)
    })
}

fn players_in_bounds(players: &[StatePlayer]) -> bool {
    players.len() <= MAX_PLAYERS && players.iter().all(|p| StatePlayer::is_name(&p.name))
}

/// The slot `sender` keeps for `receiver`: its id at the service.
pub fn slot_id(pair: &Secret, sender: &PublicKey, receiver: &PublicKey) -> [u8; ID_LEN] {
    pair.derive("slot", &transcript(&[&sender.0, &receiver.0]))[..ID_LEN]
        .try_into()
        .expect("16 bytes")
}

/// What proves to the service that a write to the slot is the sender's (Bearer, 64 hex).
pub fn slot_proof(pair: &Secret, sender: &PublicKey, receiver: &PublicKey) -> String {
    hex(&pair.derive("slot-write", &transcript(&[&sender.0, &receiver.0])))
}

/// What the service keeps to check [`slot_proof`]: SHA-256 of its hex.
pub fn slot_proof_hash(proof: &str) -> String {
    hex(&sha256(proof.as_bytes()))
}

/// Signs `state` as `me` for `receiver`, and seals it for their slot.
pub fn seal_state(
    me: &PersonKey,
    receiver: &PublicKey,
    pair: &Secret,
    state: &State,
) -> Result<Vec<u8>> {
    let name = clean_name(&state.name);
    if name != state.name || name.is_empty() {
        return Err(Error::Other("Choose a name your friends will see.".into()));
    }
    if state.cores.len() > MAX_CORES {
        return Err(Error::Other(format!(
            "Friends can see at most {MAX_CORES} of your cores."
        )));
    }
    if !players_in_bounds(&state.players) {
        return Err(Error::Other(format!(
            "Friends can see at most {MAX_PLAYERS} of your Minecraft accounts."
        )));
    }
    if !launchers_in_bounds(state) {
        return Err(Error::Other(
            "A launcher in your state isn't one of its cores.".into(),
        ));
    }
    let mut state = state.clone();
    for core in &mut state.cores {
        core.name = clean_name(&core.name);
    }
    let sig = me.sign("state", &message(receiver, &state));
    let players_sig = (!state.players.is_empty())
        .then(|| hex(&me.sign("state-players", &players_message(receiver, &state))));
    let launchers_sig = (!state.launchers.is_empty())
        .then(|| hex(&me.sign("state-launchers", &launchers_message(receiver, &state))));
    let bytes = serde_json::to_vec(&Wire {
        v: VERSION,
        seq: state.seq,
        issued: state.issued,
        expires: state.expires,
        name: state.name.clone(),
        cores: state.cores.clone(),
        retired: state.retired,
        sig: hex(&sig),
        players: state.players.clone(),
        players_sig,
        launchers: state.launchers.clone(),
        launchers_sig,
    })
    .expect("serializable");
    let id = slot_id(pair, &me.public(), receiver);
    let sealed = crypto::seal(pair, Role::State, &id, &bytes)?;
    if sealed.len() > MAX_SEALED {
        return Err(Error::Other("Your friend state is too large.".into()));
    }
    Ok(sealed)
}

/// Opens `sender`'s state from their slot for `me`, and checks its signature and that
/// it was made for `me`. It may have expired: see [`State::is_expired`]. Comparing
/// `seq` with the last one seen is the caller's job.
pub fn open_state(
    sender: &PublicKey,
    me: &PublicKey,
    pair: &Secret,
    sealed: &[u8],
) -> Result<State> {
    let bad = |what: &str| Error::Other(format!("A friend's state didn't check out ({what})."));
    let id = slot_id(pair, sender, me);
    let bytes =
        crypto::open(pair, Role::State, &id, sealed).ok_or_else(|| bad("it couldn't be opened"))?;
    let wire: Wire = serde_json::from_slice(&bytes).map_err(|_| bad("garbled"))?;
    if wire.v != VERSION {
        return Err(bad("a newer version of Lodestone made it"));
    }
    let state = State {
        seq: wire.seq,
        issued: wire.issued,
        expires: wire.expires,
        name: wire.name,
        cores: wire.cores,
        retired: wire.retired,
        players: wire.players,
        launchers: wire.launchers,
    };
    let sig = unhex(&wire.sig).ok_or_else(|| bad("signature"))?;
    if !sender.verify("state", &message(me, &state), &sig) {
        return Err(bad("signature"));
    }
    if !state.players.is_empty() {
        let players_sig = wire
            .players_sig
            .as_deref()
            .and_then(unhex)
            .ok_or_else(|| bad("signature"))?;
        if !sender.verify("state-players", &players_message(me, &state), &players_sig)
            || !players_in_bounds(&state.players)
        {
            return Err(bad("accounts"));
        }
    }
    if !state.launchers.is_empty() {
        let launchers_sig = wire
            .launchers_sig
            .as_deref()
            .and_then(unhex)
            .ok_or_else(|| bad("signature"))?;
        if !sender.verify(
            "state-launchers",
            &launchers_message(me, &state),
            &launchers_sig,
        ) || !launchers_in_bounds(&state)
        {
            return Err(bad("launchers"));
        }
    }
    if state.cores.len() > MAX_CORES
        || state.expires > state.issued.saturating_add(STATE_LIFETIME_SECS)
        || clean_name(&state.name) != state.name
        || state
            .cores
            .iter()
            .any(|core| clean_name(&core.name) != core.name)
    {
        return Err(bad("out of bounds"));
    }
    Ok(state)
}

mod hex16 {
    use serde::{Deserialize as _, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 16], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&crate::hex(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 16], D::Error> {
        let text = String::deserialize(deserializer)?;
        crate::unhex(&text)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| serde::de::Error::custom("expected 32 hex digits"))
    }
}

mod hex32_list {
    use serde::{Deserialize as _, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(list: &[[u8; 32]], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(list.iter().map(|bytes| crate::hex(bytes)))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Vec<[u8; 32]>, D::Error> {
        Vec::<String>::deserialize(deserializer)?
            .iter()
            .map(|text| {
                crate::unhex(text)
                    .and_then(|bytes| bytes.try_into().ok())
                    .ok_or_else(|| serde::de::Error::custom("expected 64 hex digits"))
            })
            .collect()
    }
}

mod hex32 {
    use serde::{Deserialize as _, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&crate::hex(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(deserializer)?;
        crate::unhex(&text)
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or_else(|| serde::de::Error::custom("expected 64 hex digits"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(seq: u64) -> State {
        State {
            seq,
            issued: 1_800_000_000,
            expires: 1_800_000_000 + STATE_LIFETIME_SECS,
            name: "Ana".into(),
            cores: vec![StateCore {
                fingerprint: [7; 32],
                name: "Ana's PC".into(),
            }],
            retired: false,
            players: Vec::new(),
            launchers: Vec::new(),
        }
    }

    fn player(name: &str) -> StatePlayer {
        StatePlayer {
            name: name.into(),
            uuid: [9; 16],
            offline: false,
        }
    }

    #[test]
    fn accounts_are_signed_and_older_copies_still_open_the_state() {
        let ana = PersonKey::generate().unwrap();
        let ben = PersonKey::generate().unwrap();
        let pair = Secret::random().unwrap();
        let mut with = state(4);
        with.players = vec![
            player("Ana_mc"),
            StatePlayer {
                offline: true,
                ..player("ana")
            },
        ];
        let sealed = seal_state(&ana, &ben.public(), &pair, &with).unwrap();
        let opened = open_state(&ana.public(), &ben.public(), &pair, &sealed).unwrap();
        assert_eq!(opened, with);

        // a copy from before accounts: the same bytes parse into its fields, and its
        // signature check covers only those
        let id = slot_id(&pair, &ana.public(), &ben.public());
        let bytes = crypto::open(&pair, Role::State, &id, &sealed).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let old = State {
            players: Vec::new(),
            ..with.clone()
        };
        let sig = unhex(json["sig"].as_str().unwrap()).unwrap();
        assert!(ana
            .public()
            .verify("state", &message(&ben.public(), &old), &sig));

        // accounts swapped in without their signature are refused
        let mut forged = json.clone();
        forged["players"] = serde_json::json!([{ "name": "Notch", "uuid": "09".repeat(16) }]);
        let resealed = crypto::seal(
            &pair,
            Role::State,
            &id,
            &serde_json::to_vec(&forged).unwrap(),
        )
        .unwrap();
        assert!(open_state(&ana.public(), &ben.public(), &pair, &resealed).is_err());

        // too many, or a name that can't be a player's
        let mut many = state(5);
        many.players = (0..=MAX_PLAYERS)
            .map(|i| player(&format!("p{i}")))
            .collect();
        assert!(seal_state(&ana, &ben.public(), &pair, &many).is_err());
        let mut odd = state(6);
        odd.players = vec![player("not a name")];
        assert!(seal_state(&ana, &ben.public(), &pair, &odd).is_err());
    }

    #[test]
    fn launchers_are_signed_and_older_copies_still_open_the_state() {
        let ana = PersonKey::generate().unwrap();
        let ben = PersonKey::generate().unwrap();
        let pair = Secret::random().unwrap();
        let mut with = state(4);
        with.cores.push(StateCore {
            fingerprint: [8; 32],
            name: "Lodestone Launcher".into(),
        });
        with.launchers = vec![[8; 32]];
        with.players = vec![player("Ana_mc")];
        let sealed = seal_state(&ana, &ben.public(), &pair, &with).unwrap();
        let opened = open_state(&ana.public(), &ben.public(), &pair, &sealed).unwrap();
        assert_eq!(opened, with);

        // a copy from before launchers reads the same bytes as a state with two cores:
        // its fields parse (unknown ones are ignored) and its signature checks
        #[derive(serde::Deserialize)]
        struct OldWire {
            v: u32,
            cores: Vec<StateCore>,
            sig: String,
            #[serde(default)]
            players: Vec<StatePlayer>,
            players_sig: Option<String>,
        }
        let id = slot_id(&pair, &ana.public(), &ben.public());
        let bytes = crypto::open(&pair, Role::State, &id, &sealed).unwrap();
        let old: OldWire = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(old.v, VERSION);
        assert_eq!(old.cores.len(), 2);
        let as_old = State {
            launchers: Vec::new(),
            ..with.clone()
        };
        assert!(ana.public().verify(
            "state",
            &message(&ben.public(), &as_old),
            &unhex(&old.sig).unwrap()
        ));
        assert_eq!(old.players.len(), 1);
        assert!(ana.public().verify(
            "state-players",
            &players_message(&ben.public(), &as_old),
            &unhex(old.players_sig.as_deref().unwrap()).unwrap()
        ));

        // a state from before launchers (none listed) has neither field, so its bytes
        // are what they always were, and it opens with no launchers
        let plain = seal_state(&ana, &ben.public(), &pair, &state(5)).unwrap();
        let plain_bytes = crypto::open(&pair, Role::State, &id, &plain).unwrap();
        let json: serde_json::Value = serde_json::from_slice(&plain_bytes).unwrap();
        assert!(json.get("launchers").is_none() && json.get("launchers_sig").is_none());
        assert!(open_state(&ana.public(), &ben.public(), &pair, &plain)
            .unwrap()
            .launchers
            .is_empty());

        // launchers added without their signature, or with another state's, are refused
        let mut json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let reseal = |json: &serde_json::Value| {
            crypto::seal(&pair, Role::State, &id, &serde_json::to_vec(json).unwrap()).unwrap()
        };
        let sig = json["launchers_sig"].take();
        assert!(open_state(&ana.public(), &ben.public(), &pair, &reseal(&json)).is_err());
        json["launchers_sig"] = sig;
        json["launchers"] = serde_json::json!(["07".repeat(32)]);
        assert!(open_state(&ana.public(), &ben.public(), &pair, &reseal(&json)).is_err());

        // a launcher must be one of the cores
        let mut stray = state(6);
        stray.launchers = vec![[3; 32]];
        assert!(seal_state(&ana, &ben.public(), &pair, &stray).is_err());
    }

    #[test]
    fn states_open_only_for_their_receiver() {
        let ana = PersonKey::generate().unwrap();
        let ben = PersonKey::generate().unwrap();
        let carl = PersonKey::generate().unwrap();
        let pair = Secret::random().unwrap();
        let sealed = seal_state(&ana, &ben.public(), &pair, &state(3)).unwrap();
        let opened = open_state(&ana.public(), &ben.public(), &pair, &sealed).unwrap();
        assert_eq!(opened, state(3));
        assert!(!opened.is_expired(1_800_000_001));
        assert!(opened.is_expired(opened.expires));
        // not for Carl, even with the same pair secret (a malicious friend re-sealing it)
        assert!(open_state(&ana.public(), &carl.public(), &pair, &sealed).is_err());
        // not as from Ben
        assert!(open_state(&ben.public(), &ana.public(), &pair, &sealed).is_err());
        // Ben can't forge a state from Ana even though he can seal for her slot
        let forged = seal_state(&ben, &ben.public(), &pair, &state(9)).unwrap();
        assert!(open_state(&ana.public(), &ben.public(), &pair, &forged).is_err());
    }

    #[test]
    fn slots_differ_by_direction() {
        let pair = Secret::random().unwrap();
        let a = PublicKey([1; 32]);
        let b = PublicKey([2; 32]);
        assert_ne!(slot_id(&pair, &a, &b), slot_id(&pair, &b, &a));
        assert_ne!(slot_proof(&pair, &a, &b), slot_proof(&pair, &b, &a));
        assert_eq!(slot_proof_hash(&slot_proof(&pair, &a, &b)).len(), 64);
    }

    #[test]
    fn states_stay_in_bounds() {
        let ana = PersonKey::generate().unwrap();
        let ben = PersonKey::generate().unwrap();
        let pair = Secret::random().unwrap();
        let mut long = state(1);
        long.expires = long.issued + STATE_LIFETIME_SECS + 1;
        let sealed = seal_state(&ana, &ben.public(), &pair, &long).unwrap();
        assert!(open_state(&ana.public(), &ben.public(), &pair, &sealed).is_err());
        let mut many = state(1);
        many.cores = vec![many.cores[0].clone(); MAX_CORES + 1];
        assert!(seal_state(&ana, &ben.public(), &pair, &many).is_err());
        let mut sneaky = state(1);
        sneaky.name = "Ana\u{202E}".into();
        assert!(seal_state(&ana, &ben.public(), &pair, &sneaky).is_err());
    }
}
