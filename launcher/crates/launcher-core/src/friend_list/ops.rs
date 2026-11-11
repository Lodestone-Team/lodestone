//! What the friend list does over the network (launcher/docs/friends.md): friend links,
//! states and slots, presence, a friend's servers, and tunnels to them.
//!
//! Each call reads what it needs from the book, does its network part on a Tokio thread
//! ([`super::on_tokio`]), then changes the book through [`Friends::modify`], which
//! reloads it under a lock first (the Host writes the same file).

use lodestone_connect::{
    Service, SharedServer,
    friend::{
        crypto::{PersonKey, PublicKey, core_secret},
        join::{FriendCore, is_online, join_friend},
        pair::{FriendLink, Invitation, answer_with_player, open_card},
        state::{
            REISSUE_AFTER_SECS, STATE_LIFETIME_SECS, State, StateCore, open_state, seal_state,
            slot_id, slot_proof,
        },
    },
    join::JoinOptions,
};

use super::{
    Book, Friend, FriendState, FriendStatus, Friends, FriendsError, MAX_FRIENDS, PendingLink,
    StatePlayer, hex, now, on_tokio, profile_of,
};
use crate::cores::connect::{GamePort, JoinError, Progress, Tunnel, open_friend};

/// How far ahead of this computer's clock a friend's state may say it was issued (their
/// clock may be ahead).
pub const MAX_ISSUED_AHEAD_SECS: u64 = 24 * 60 * 60;

/// How far a state's number may be above the time it was issued. Numbers follow the
/// clock (a new state is numbered at least the time it was made, in seconds), so only
/// changes faster than one a second put a number ahead, and never by a day. A state
/// numbered further ahead is refused: someone with a stolen key could otherwise number
/// a state so high that the owner's later states all look older. (A retired state is
/// taken whatever its number: retiring is final.)
pub const MAX_SEQ_AHEAD_SECS: u64 = 24 * 60 * 60;

/// Why a friend's (not retired) state can't be believed, if it can't.
pub fn implausible_state(state: &State, now: u64) -> Option<&'static str> {
    if state.issued > now.saturating_add(MAX_ISSUED_AHEAD_SECS) {
        return Some("their state says it was made more than a day from now");
    }
    if state.seq > state.issued.saturating_add(MAX_SEQ_AHEAD_SECS) {
        return Some("their state's number is too far ahead of when it was made");
    }
    None
}

/// What a refresh reports when the service asked it to slow down.
const SLOW_DOWN: &str =
    "The introduction service asked to slow down. The rest are checked next time.";

/// Whether the service turned a request down for coming too often (`429`).
fn is_rate_limited(err: &lodestone_connect::Error) -> bool {
    matches!(err, lodestone_connect::Error::Service { status: 429, .. })
}

/// How many times a state is sent again when a newer one was made while it was being
/// sent (by the other app, or a retirement).
const REPUBLISH_TRIES: usize = 3;

/// A friend link's card, opened and checked: who sent it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkPreview {
    /// The sender's name.
    pub name: String,
    /// Their safety code, to compare.
    pub safety_code: String,
    /// Their key, 64 hex digits.
    pub key: String,
    /// Already a friend.
    pub already_friends: bool,
    /// Friends with the same name (or Minecraft profile): shown, in case someone is
    /// pretending.
    pub same_name_as: Vec<String>,
    /// The Minecraft profile the sender says they play as, for their skin (from
    /// launchers that send it). Their claim, signed by their key: nobody checks it.
    pub player: Option<StatePlayer>,
}

/// An answer to one of this person's friend links, waiting for them to accept it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkReply {
    pub code: String,
    pub key: String,
    pub name: String,
    pub safety_code: String,
    /// How many people answered this link. More than one means someone else had it.
    pub answers: usize,
    pub same_name_as: Vec<String>,
    /// The Minecraft profile they say they play as (see [`LinkPreview::player`]).
    pub player: Option<StatePlayer>,
}

/// What [`Friends::refresh`] found.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Refreshed {
    /// Friends whose first state arrived: the friendship is confirmed.
    pub confirmed: Vec<String>,
    /// Friends who retired their key.
    pub retired: Vec<String>,
    /// Cores friends no longer list (their fingerprints, hex): stop trusting their
    /// servers' mods.
    pub dropped_cores: Vec<String>,
    /// Friends who renamed themselves to a name another friend has: (their key, the
    /// other friends' names). Shown, in case someone is pretending.
    pub same_name: Vec<(String, Vec<String>)>,
    /// Friends whose state couldn't be read, and why.
    pub problems: Vec<(String, String)>,
}

/// A friend's core and whether it's sharing with this person now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CorePresence {
    /// The friend's key.
    pub friend: String,
    pub core: StateCore,
    pub online: bool,
}

fn refused(message: impl Into<String>) -> FriendsError {
    FriendsError::Refused(message.into())
}

fn pkcs8_of(me: &PersonKey) -> Vec<u8> {
    me.pkcs8().to_vec()
}

/// A friend link's text, checked: a link to `service`, the one the friend list uses
/// (over HTTPS, or plain HTTP to this computer for testing). A link naming any other
/// place is refused, so nobody can make the launcher send its requests elsewhere.
fn checked_link(service: &Service, text: &str) -> Result<FriendLink, FriendsError> {
    let link = FriendLink::parse(text).map_err(|err| refused(err.to_string()))?;
    if &link.service != service {
        return Err(refused(format!(
            "That friend link is for {}, not {}. Ask for a new one.",
            link.service.base_url(),
            service.base_url()
        )));
    }
    Ok(link)
}

/// Opens a friend link and checks its card: who it's from. `mine` is this person's key
/// and book when they have a friend list; without one, this makes nothing (no key),
/// so a link opened from outside the launcher can be looked at before anything is
/// made.
pub async fn preview_link(
    service: &Service,
    mine: Option<(PublicKey, &Book)>,
    text: &str,
) -> Result<LinkPreview, FriendsError> {
    let link = checked_link(service, text)?;
    let card = fetch_card(&link).await?;
    let key = card.key.to_hex();
    let (already_friends, same_name_as) = match mine {
        Some((me, _)) if card.key == me => {
            return Err(refused("That's your own friend link."));
        }
        Some((_, book)) => (
            book.friend(&key).is_some(),
            book.lookalikes(&card.name, card.player.as_ref(), &key)
                .iter()
                .map(|f| f.display_name().to_string())
                .collect(),
        ),
        None => (false, Vec::new()),
    };
    Ok(LinkPreview {
        name: card.name.clone(),
        safety_code: card.key.safety_code(),
        already_friends,
        same_name_as,
        key,
        player: card.player.clone(),
    })
}

impl Friends {
    fn checked_link(&self, text: &str) -> Result<FriendLink, FriendsError> {
        checked_link(&self.service, text)
    }

    // -----------------------------------------------------------------------
    // Friend links

    /// Makes a friend link and opens its mailbox. Returns the link to send.
    pub async fn create_link(&mut self) -> Result<String, FriendsError> {
        self.reload()?;
        if self.book.name.is_empty() {
            return Err(refused("Choose a name your friends will see first."));
        }
        let (mut invitation, card) = Invitation::create_with_player(
            &self.me,
            &self.book.name,
            self.book.profile(),
            self.service.clone(),
            now(),
        )?;
        let service = self.service.clone();
        let code = invitation.link.code.clone();
        let access_hash = invitation.link.access_hash();
        let secret =
            on_tokio(move || async move { service.pair_open(&code, &access_hash, &card).await })
                .await??;
        invitation.mailbox_secret = Some(secret);
        let link = invitation.link.link();
        let pending = PendingLink {
            code: invitation.link.code.clone(),
            invitation: invitation.to_json(),
            expires: invitation.expires,
        };
        self.modify(|book| {
            book.links.push(pending);
            Ok(())
        })?;
        Ok(link)
    }

    /// Cancels a friend link: its mailbox and its keys go.
    pub async fn cancel_link(&mut self, code: &str) -> Result<(), FriendsError> {
        let removed = self.modify(|book| {
            Ok(book
                .links
                .iter()
                .position(|l| l.code == code)
                .map(|at| book.links.remove(at)))
        })?;
        let Some(link) = removed else {
            return Ok(());
        };
        if let Ok(invitation) = Invitation::from_json(&link.invitation)
            && let Some(secret) = invitation.mailbox_secret.clone()
        {
            let service = invitation.link.service.clone();
            let code = link.code.clone();
            let _ = on_tokio(move || async move { service.pair_close(&code, &secret).await }).await;
        }
        Ok(())
    }

    /// The answers waiting on this person's friend links. Expired links are cancelled.
    pub async fn link_replies(&mut self) -> Result<Vec<LinkReply>, FriendsError> {
        self.reload()?;
        let now = now();
        let expired: Vec<String> = self
            .book
            .links
            .iter()
            .filter(|l| now >= l.expires)
            .map(|l| l.code.clone())
            .collect();
        for code in expired {
            self.cancel_link(&code).await?;
        }
        let me = self.me();
        let mut out = Vec::new();
        for link in self.book.links.clone() {
            let invitation = Invitation::from_json(&link.invitation)?;
            let Some(secret) = invitation.mailbox_secret.clone() else {
                continue;
            };
            let service = invitation.link.service.clone();
            let code = link.code.clone();
            let replies =
                on_tokio(move || async move { service.pair_replies(&code, &secret).await })
                    .await??
                    .unwrap_or_default();
            let answers = replies.len();
            for sealed in replies {
                // a reply that doesn't check out is somebody's garbage: skip it
                let Ok(reply) = invitation.open_reply(&me, &sealed) else {
                    continue;
                };
                let key = reply.key.to_hex();
                out.push(LinkReply {
                    code: link.code.clone(),
                    same_name_as: self
                        .book
                        .lookalikes(&reply.name, reply.player.as_ref(), &key)
                        .iter()
                        .map(|f| f.display_name().to_string())
                        .collect(),
                    key,
                    name: reply.name.clone(),
                    safety_code: reply.key.safety_code(),
                    answers,
                    player: reply.player.clone(),
                });
            }
        }
        Ok(out)
    }

    /// Accepts `key`'s answer to the link `code`: they become a friend, the link and its
    /// keys go, and this person's first state (their confirmation) goes to the friend.
    pub async fn accept_reply(&mut self, code: &str, key: &str) -> Result<(), FriendsError> {
        self.reload()?;
        let link = self
            .book
            .links
            .iter()
            .find(|l| l.code == code)
            .cloned()
            .ok_or_else(|| refused("That friend link has been used or cancelled."))?;
        let invitation = Invitation::from_json(&link.invitation)?;
        let secret = invitation
            .mailbox_secret
            .clone()
            .ok_or_else(|| refused("That friend link has no mailbox."))?;
        let service = invitation.link.service.clone();
        let fetch_code = code.to_string();
        let replies =
            on_tokio(move || async move { service.pair_replies(&fetch_code, &secret).await })
                .await??
                .unwrap_or_default();
        let me = self.me();
        let reply = replies
            .iter()
            .filter_map(|sealed| invitation.open_reply(&me, sealed).ok())
            .find(|reply| reply.key.to_hex() == key)
            .ok_or_else(|| refused("That answer is no longer there."))?;
        let pair = invitation.accept(&reply)?;
        let friend = Friend {
            key: key.to_string(),
            name: reply.name.clone(),
            pair: pair.to_hex(),
            status: FriendStatus::Active,
            added_at: now(),
            player: reply.player.clone(),
            ..Friend::default()
        };
        self.modify(|book| {
            if book.friend(key).is_some() {
                return Err(refused("You're already friends."));
            }
            if book.friends.len() >= MAX_FRIENDS {
                return Err(refused(format!(
                    "You can have at most {MAX_FRIENDS} friends."
                )));
            }
            book.friends.push(friend);
            Ok(())
        })?;
        // the link has done its job: its keys and mailbox go
        self.cancel_link(code).await?;
        self.publish_state_to(key).await
    }

    /// Opens a friend link and checks its card: who it's from.
    pub async fn preview_link(&mut self, text: &str) -> Result<LinkPreview, FriendsError> {
        self.reload()?;
        let service = self.service.clone();
        preview_link(&service, Some((self.me(), &self.book)), text).await
    }

    /// Answers a friend link: the sender becomes a pending friend until their first
    /// state arrives, and this person's state goes to them.
    pub async fn answer_link(&mut self, text: &str) -> Result<String, FriendsError> {
        self.reload()?;
        if self.book.name.is_empty() {
            return Err(refused("Choose a name your friends will see first."));
        }
        let link = self.checked_link(text)?;
        let card = fetch_card(&link).await?;
        let key = card.key.to_hex();
        if self.book.friend(&key).is_some() {
            return Err(refused("You're already friends."));
        }
        if self.book.friends.len() >= MAX_FRIENDS {
            return Err(refused(format!(
                "You can have at most {MAX_FRIENDS} friends."
            )));
        }
        let answer =
            answer_with_player(&self.me, &self.book.name, self.book.profile(), &link, &card)?;
        let service = link.service.clone();
        let code = link.code.clone();
        let access = link.access();
        let sealed = answer.sealed.clone();
        on_tokio(move || async move { service.pair_reply(&code, &access, &sealed).await })
            .await??;
        let friend = Friend {
            key: key.clone(),
            name: card.name.clone(),
            pair: answer.pair.to_hex(),
            status: FriendStatus::Pending,
            added_at: now(),
            player: card.player.clone(),
            ..Friend::default()
        };
        self.modify(|book| {
            if book.friend(&key).is_none() {
                book.friends.push(friend);
            }
            Ok(())
        })?;
        // No state goes to them yet: nothing of this person's (not even their cores'
        // names) is sent until the sender confirms with their own first state, which
        // `refresh` picks up and answers.
        Ok(key)
    }

    // -----------------------------------------------------------------------
    // States

    /// This person's current state, as the book has it (retired once the identity is).
    fn my_state(&self) -> State {
        let retired = self.book.retired;
        let issued = if self.book.state_issued == 0 {
            now()
        } else {
            self.book.state_issued
        };
        State {
            seq: self.book.state_seq.max(1),
            issued,
            expires: issued + STATE_LIFETIME_SECS,
            name: self.book.name.clone(),
            cores: if retired {
                Vec::new()
            } else {
                self.book.state_cores()
            },
            retired,
            players: if retired {
                Vec::new()
            } else {
                self.book.players.clone()
            },
            launchers: if retired {
                Vec::new()
            } else {
                self.book.state_launchers()
            },
        }
    }

    /// Makes sure the current state is numbered and dated, reissuing it once it's a week
    /// old.
    fn freshen_state(&mut self) -> Result<(), FriendsError> {
        self.modify(|book| {
            let now = now();
            if book.state_seq == 0
                || book.state_issued == 0
                || now.saturating_sub(book.state_issued) >= REISSUE_AFTER_SECS
            {
                if book.state_issued != 0 || book.state_seq == 0 {
                    book.new_state();
                }
                book.state_issued = now;
            }
            Ok(())
        })
    }

    async fn write_state(&self, friend: &Friend, state: &State) -> Result<(), FriendsError> {
        let key = friend
            .public_key()
            .ok_or_else(|| FriendsError::Damaged("a friend's key".into()))?;
        let pair = friend
            .pair_secret()
            .ok_or_else(|| FriendsError::Damaged("a friend's secret".into()))?;
        let me = self.me();
        let sealed = seal_state(&self.me, &key, &pair, state)?;
        let id = slot_id(&pair, &me, &key);
        let proof = slot_proof(&pair, &me, &key);
        let service = self.service.clone();
        on_tokio(move || async move { service.slot_write(&id, &proof, &sealed).await }).await??;
        Ok(())
    }

    /// Writes this person's state to one friend's slot.
    ///
    /// The slot write happens outside the file's lock, so the other app (or a
    /// retirement) may make a newer state meanwhile and send it first; this write
    /// would then put the older one back. So afterwards it looks again under the lock,
    /// and sends the newer state if there is one.
    pub async fn publish_state_to(&mut self, key: &str) -> Result<(), FriendsError> {
        if self.book.retired {
            self.reload_retired()?;
        } else {
            self.freshen_state()?;
        }
        let friend = self
            .book
            .friend(key)
            .cloned()
            .ok_or_else(|| refused("No such friend."))?;
        for _ in 0..REPUBLISH_TRIES {
            let state = self.my_state();
            self.write_state(&friend, &state).await?;
            let current = self.modify_book(true, |book| {
                if let Some(friend) = book.friend_mut(key) {
                    friend.sent_seq = friend.sent_seq.max(state.seq);
                }
                Ok((book.state_seq.max(1), book.retired))
            })?;
            if current == (state.seq, state.retired) {
                return Ok(());
            }
            // a newer state was made while this one was being sent: send that
            if !self.book.retired {
                self.freshen_state()?;
            }
        }
        Ok(())
    }

    /// Reads the book again, retired or not.
    fn reload_retired(&mut self) -> Result<(), FriendsError> {
        match self.reload() {
            Ok(()) | Err(FriendsError::Retired) => Ok(()),
            Err(err) => Err(err),
        }
    }

    /// Writes this person's state to every confirmed friend who doesn't have the current
    /// one (pending friends get nothing until they confirm).
    /// Returns the friends it couldn't reach, and why.
    pub async fn publish_states(&mut self) -> Result<Vec<(String, String)>, FriendsError> {
        self.freshen_state()?;
        let seq = self.my_state().seq;
        let mut problems = Vec::new();
        let behind: Vec<String> = self
            .book
            .friends
            .iter()
            .filter(|f| f.status == FriendStatus::Active && f.sent_seq < seq)
            .map(|f| f.key.clone())
            .collect();
        for key in behind {
            if let Err(err) = self.publish_state_to(&key).await {
                problems.push((key, err.to_string()));
            }
        }
        Ok(problems)
    }

    /// Reads every friend's state from their slot: confirms pending friends, notes cores
    /// they dropped and keys they retired, and sends this person's state where it's
    /// behind.
    pub async fn refresh(&mut self) -> Result<Refreshed, FriendsError> {
        self.reload()?;
        let me = self.me();
        let mut out = Refreshed::default();
        let mut read: Vec<(String, State)> = Vec::new();
        let mut friends: Vec<Friend> = self
            .book
            .friends
            .iter()
            .filter(|f| f.status != FriendStatus::Retired)
            .cloned()
            .collect();
        // The service limits slot reads per address. Start at another friend each
        // time, so a long list that runs into the limit isn't always cut at the same
        // friends.
        if !friends.is_empty() {
            let start = (now() / 60) as usize % friends.len();
            friends.rotate_left(start);
        }
        for friend in friends {
            let (Some(key), Some(pair)) = (friend.public_key(), friend.pair_secret()) else {
                continue;
            };
            let id = slot_id(&pair, &key, &me);
            let service = self.service.clone();
            let sealed = match on_tokio(move || async move { service.slot_read(&id).await }).await?
            {
                Ok(Some(sealed)) => sealed,
                Ok(None) => continue,
                Err(err) if is_rate_limited(&err) => {
                    // the rest are read next time; nothing is wrong with them
                    out.problems.push((String::new(), SLOW_DOWN.to_string()));
                    break;
                }
                Err(err) => {
                    out.problems.push((friend.key.clone(), err.to_string()));
                    continue;
                }
            };
            match open_state(&key, &me, &pair, &sealed) {
                Ok(state) => read.push((friend.key.clone(), state)),
                Err(err) => out.problems.push((friend.key.clone(), err.to_string())),
            }
        }
        let now = now();
        self.modify(|book| {
            for (key, state) in read {
                // a friend who takes on another friend's name (or Minecraft profile)
                // is flagged, as at adding
                let player = profile_of(&state.players);
                let renamed = book.friend(&key).is_some_and(|f| {
                    !state.retired && (f.name != state.name || f.minecraft() != player)
                });
                let same_name: Vec<String> = if renamed {
                    book.lookalikes(&state.name, player, &key)
                        .iter()
                        .map(|f| f.display_name().to_string())
                        .collect()
                } else {
                    Vec::new()
                };
                let Some(entry) = book.friend_mut(&key) else {
                    // removed meanwhile, in this app or the other
                    continue;
                };
                if entry.status == FriendStatus::Retired {
                    continue;
                }
                let known = entry.state.as_ref().map_or(0, |s| s.seq);
                // A retired state is final, and taken whatever its number: someone with
                // a stolen key can't block it by numbering a state of theirs higher.
                if !state.retired {
                    if state.seq <= known {
                        // the same, or older (a rollback): keep what we have
                        continue;
                    }
                    if let Some(why) = implausible_state(&state, now) {
                        out.problems.push((key.clone(), why.to_string()));
                        continue;
                    }
                }
                if !same_name.is_empty() {
                    out.same_name.push((key.clone(), same_name));
                }
                if state.retired {
                    entry.status = FriendStatus::Retired;
                    out.retired.push(key.clone());
                } else if entry.status == FriendStatus::Pending {
                    entry.status = FriendStatus::Active;
                    out.confirmed.push(key.clone());
                }
                if let Some(old) = &entry.state {
                    for core in &old.cores {
                        if state.retired
                            || !state
                                .cores
                                .iter()
                                .any(|c| c.fingerprint == core.fingerprint)
                        {
                            out.dropped_cores.push(hex(&core.fingerprint));
                        }
                    }
                }
                entry.name = state.name.clone();
                entry.state = Some(FriendState {
                    seq: state.seq.max(known),
                    expires: state.expires,
                    players: if state.retired {
                        Vec::new()
                    } else {
                        state.players
                    },
                    launchers: if state.retired {
                        Vec::new()
                    } else {
                        state.launchers.iter().map(|fp| hex(fp)).collect()
                    },
                    cores: if state.retired {
                        Vec::new()
                    } else {
                        state.cores
                    },
                });
            }
            Ok(())
        })?;
        out.problems.extend(self.publish_states().await?);
        Ok(out)
    }

    /// Retires one of this person's cores (a lost or stolen computer, or one they no
    /// longer share from): it leaves their state and is never listed again, and the new
    /// state goes to friends, whose launchers drop it and stop trusting its servers'
    /// mods. Returns the friends it couldn't reach, and why.
    pub async fn retire_core(
        &mut self,
        fingerprint: &str,
    ) -> Result<Vec<(String, String)>, FriendsError> {
        let fingerprint = fingerprint.trim().to_ascii_lowercase();
        if !self
            .book
            .my_cores
            .iter()
            .any(|c| c.fingerprint == fingerprint)
        {
            self.reload()?;
            if !self
                .book
                .my_cores
                .iter()
                .any(|c| c.fingerprint == fingerprint)
            {
                return Err(refused("You don't share from that host."));
            }
        }
        self.retire_my_core(&fingerprint)?;
        self.publish_states().await
    }

    /// Removes a friend: their entry and this person's slot for them go. This person's
    /// cores drop them the next time Lodestone Host syncs them.
    pub async fn remove(&mut self, key: &str) -> Result<(), FriendsError> {
        let removed = self.modify(|book| {
            Ok(book
                .friends
                .iter()
                .position(|f| f.key == key)
                .map(|at| book.friends.remove(at)))
        })?;
        let Some(friend) = removed else {
            return Ok(());
        };
        if let (Some(their), Some(pair)) = (friend.public_key(), friend.pair_secret()) {
            let me = self.me();
            let id = slot_id(&pair, &me, &their);
            let proof = slot_proof(&pair, &me, &their);
            let service = self.service.clone();
            let _ = on_tokio(move || async move { service.slot_clear(&id, &proof).await }).await;
        }
        Ok(())
    }

    /// Retires this person's friend identity, for a key that may be stolen or a fresh
    /// start. In order:
    ///
    /// 1. the book is marked retired, so nothing (this app or Lodestone Host) uses the
    ///    identity again, even if a later step fails;
    /// 2. a final state saying so, signed with this key, goes to every friend (their
    ///    launchers drop this person and remove them from their cores);
    /// 3. the book is emptied down to that mark: friends, pair secrets and links go.
    ///
    /// The app then deletes the person key ([`super::forget_identity`]). The emptied
    /// file stays, so Lodestone Host sees the retirement and clears its core; the
    /// Launcher deletes it, and the file key, when the person starts a new identity.
    /// Returns the friends it couldn't reach, and why; it retires anyway.
    pub async fn retire_identity(&mut self) -> Result<Vec<(String, String)>, FriendsError> {
        self.modify_book(true, |book| {
            if !book.retired {
                book.retired = true;
                book.new_state();
                book.state_issued = now();
            }
            Ok(())
        })?;
        let mut problems = Vec::new();
        let friends: Vec<String> = self
            .book
            .friends
            .iter()
            .filter(|f| f.status != FriendStatus::Retired)
            .map(|f| f.key.clone())
            .collect();
        for key in friends {
            if let Err(err) = self.publish_state_to(&key).await {
                problems.push((key, err.to_string()));
            }
        }
        self.modify_book(true, |book| {
            *book = Book {
                name: book.name.clone(),
                state_seq: book.state_seq,
                state_issued: book.state_issued,
                retired: true,
                generation: book.generation,
                ..Book::default()
            };
            Ok(())
        })?;
        Ok(problems)
    }

    // -----------------------------------------------------------------------
    // Joining

    /// The core secret this person shares with `friend`'s core `fingerprint`.
    pub fn friend_core(&self, friend: &Friend, fingerprint: &[u8; 32]) -> Option<FriendCore> {
        let their = friend.public_key()?;
        let pair = friend.pair_secret()?;
        Some(FriendCore {
            fingerprint: *fingerprint,
            secret: core_secret(&pair, &their, &self.me(), fingerprint),
        })
    }

    /// Which of each friend's cores are sharing with this person now.
    pub async fn presence(&self) -> Result<Vec<CorePresence>, FriendsError> {
        let now = now();
        let mut checks: Vec<(String, StateCore, FriendCore)> = Vec::new();
        for friend in &self.book.friends {
            for core in friend.joinable_cores(now) {
                if let Some(target) = self.friend_core(friend, &core.fingerprint) {
                    checks.push((friend.key.clone(), core.clone(), target));
                }
            }
        }
        // as for slot reads: start somewhere else each time, and stop when the service
        // asks to slow down (the cores not checked stay unknown, not offline)
        if !checks.is_empty() {
            let start = (now / 60) as usize % checks.len();
            checks.rotate_left(start);
        }
        let service = self.service.clone();
        let results = on_tokio(move || async move {
            let mut out = Vec::new();
            for (friend, core, target) in checks {
                let online = match is_online(&service, &target).await {
                    Ok(online) => online,
                    Err(err) if is_rate_limited(&err) => break,
                    Err(_) => false,
                };
                out.push(CorePresence {
                    friend,
                    core,
                    online,
                });
            }
            out
        })
        .await?;
        Ok(results)
    }

    /// The servers `friend`'s core shares with this person: opens a tunnel, asks, and
    /// closes it.
    pub async fn servers(
        &self,
        friend: &str,
        fingerprint: &[u8; 32],
    ) -> Result<Vec<SharedServer>, FriendsError> {
        let friend = self
            .book
            .friend(friend)
            .ok_or_else(|| refused("No such friend."))?;
        if !friend
            .joinable_cores(now())
            .iter()
            .any(|c| &c.fingerprint == fingerprint)
        {
            return Err(refused("Your friend doesn't list that host."));
        }
        let target = self
            .friend_core(friend, fingerprint)
            .ok_or_else(|| FriendsError::Damaged("a friend's key".into()))?;
        let service = self.service.clone();
        let me = pkcs8_of(&self.me);
        on_tokio(move || async move {
            let me = PersonKey::from_pkcs8(&me)?;
            let joined = join_friend(&service, &me, &target, JoinOptions::default()).await?;
            let servers = joined.servers().await;
            joined.close().await;
            servers
        })
        .await?
        .map_err(Into::into)
    }

    /// A tunnel to `instance` on `friend`'s core, for Play, with the game port as
    /// `game_port` says (the server's saved one, so Minecraft's server list keeps
    /// working).
    pub async fn open_tunnel(
        &self,
        friend: &str,
        fingerprint: &[u8; 32],
        instance: &str,
        game_port: GamePort,
        progress: Progress,
    ) -> Result<Tunnel, JoinError> {
        let friend = self
            .book
            .friend(friend)
            .ok_or_else(|| JoinError::Failed("No such friend.".into()))?;
        if !friend
            .joinable_cores(now())
            .iter()
            .any(|c| &c.fingerprint == fingerprint)
        {
            return Err(JoinError::Failed(
                "Your friend doesn't list that host any more.".into(),
            ));
        }
        let target = self
            .friend_core(friend, fingerprint)
            .ok_or_else(|| JoinError::Failed("A friend's key is damaged.".into()))?;
        open_friend(
            self.service.clone(),
            &self.me,
            target,
            instance.to_string(),
            game_port,
            progress,
        )
        .await
    }
}

async fn fetch_card(
    link: &FriendLink,
) -> Result<lodestone_connect::friend::pair::Card, FriendsError> {
    let service: Service = link.service.clone();
    let code = link.code.clone();
    let access = link.access();
    let sealed = on_tokio(move || async move { service.pair_card(&code, &access).await })
        .await??
        .ok_or_else(|| {
            refused("This friend link has been used or has expired. Ask for a new one.")
        })?;
    Ok(open_card(link, &sealed, now())?)
}
