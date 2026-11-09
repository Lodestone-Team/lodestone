//! Links between Lodestone users and their Minecraft (Java Edition) accounts.
//!
//! A link is made by Lodestone Launcher, the only client that holds a token proving
//! that its user owns a Minecraft account. It sends the account's Minecraft access token
//! once, in the body of an authenticated request; the core asks Minecraft's services
//! (`GET https://api.minecraftservices.com/minecraft/profile`) whose profile the token
//! belongs to, and links that profile's UUID and name. The token is used for that one
//! request only: it is never stored, logged or sent anywhere else.
//!
//! Links are kept in `stores/minecraft_links.json` (0600). A Minecraft account is linked
//! to at most one Lodestone user: proving it again from another user moves the link.

use std::path::PathBuf;
use std::time::Duration;

use color_eyre::eyre::{eyre, Context};
use serde::{Deserialize, Serialize};
use tracing::{error, info};
use ts_rs::TS;

use crate::{
    auth::user_id::UserId,
    error::{Error, ErrorKind},
};

/// Where Minecraft's services describe the profile of an access token.
pub const MINECRAFT_PROFILE_URL: &str = "https://api.minecraftservices.com/minecraft/profile";
/// The most Minecraft accounts one user may link.
pub const MAX_LINKS_PER_USER: usize = 10;
/// Access tokens longer than this are refused without being sent anywhere.
const MAX_TOKEN_BYTES: usize = 8 * 1024;
/// How long Minecraft's services may take to answer.
const VERIFY_TIMEOUT: Duration = Duration::from_secs(15);
/// Profile replies larger than this are refused.
const MAX_PROFILE_BYTES: usize = 256 * 1024;

/// A Lodestone user's Minecraft account.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct MinecraftLink {
    pub uid: UserId,
    /// The account's UUID, lowercase with dashes.
    pub minecraft_uuid: String,
    /// The account's name when it was linked.
    pub minecraft_name: String,
    /// Whether Minecraft's services confirmed that the user owns the account. Always
    /// true for links made by this core; kept so that a future unverified kind of link
    /// can be told apart.
    pub verified: bool,
    #[ts(type = "number")]
    pub linked_at: i64,
}

/// A link with its user's name, for the owner's list of every link.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export)]
pub struct MinecraftLinkInfo {
    pub uid: UserId,
    /// The Lodestone user's name.
    pub username: String,
    pub minecraft_uuid: String,
    pub minecraft_name: String,
    pub verified: bool,
    #[ts(type = "number")]
    pub linked_at: i64,
}

impl MinecraftLinkInfo {
    pub fn new(link: MinecraftLink, username: String) -> Self {
        Self {
            uid: link.uid,
            username,
            minecraft_uuid: link.minecraft_uuid,
            minecraft_name: link.minecraft_name,
            verified: link.verified,
            linked_at: link.linked_at,
        }
    }
}

/// `POST /user/minecraft_links`. No `Debug` that could print the token: see the manual
/// implementation.
#[derive(Deserialize, TS)]
#[ts(export)]
pub struct LinkMinecraftAccount {
    /// The UUID of the account the launcher has selected, with or without dashes. It
    /// must be the profile the token belongs to.
    pub minecraft_uuid: String,
    /// The account's name as the launcher knows it. Only checked for shape: the name
    /// stored is the one Minecraft's services return.
    pub minecraft_name: String,
    /// The account's Minecraft access token. Used once to ask for the profile, then
    /// dropped.
    pub access_token: String,
}

impl std::fmt::Debug for LinkMinecraftAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LinkMinecraftAccount")
            .field("minecraft_uuid", &self.minecraft_uuid)
            .field("minecraft_name", &self.minecraft_name)
            .field("access_token", &"<redacted>")
            .finish()
    }
}

#[derive(Serialize, Deserialize, Default)]
struct StoredLinks {
    links: Vec<MinecraftLink>,
}

/// The profile part of Minecraft's answer.
#[derive(Deserialize)]
struct Profile {
    id: String,
    name: String,
}

fn bad_request(message: impl Into<String>) -> Error {
    Error {
        kind: ErrorKind::BadRequest,
        source: eyre!(message.into()),
    }
}

/// A Minecraft UUID, with or without dashes, as lowercase with dashes.
pub fn normalize_uuid(input: &str) -> Result<String, Error> {
    let parsed = uuid::Uuid::try_parse(input.trim())
        .map_err(|_| bad_request("That isn't a Minecraft account UUID"))?;
    Ok(parsed.hyphenated().to_string())
}

/// Whether `name` could be a Minecraft Java account name: 1 to 16 letters, digits or
/// underscores. (Old accounts can have names of 1 or 2 characters.)
pub fn is_valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

/// The links, as kept in memory and in `stores/minecraft_links.json`.
pub struct MinecraftLinks {
    path: PathBuf,
    links: tokio::sync::Mutex<Vec<MinecraftLink>>,
    /// The file could not be read at startup; it is kept until a link changes.
    corrupt: std::sync::atomic::AtomicBool,
    profile_url: String,
    http: reqwest::Client,
}

impl MinecraftLinks {
    /// Loads the links from `path`. `profile_url` is [`MINECRAFT_PROFILE_URL`], except in
    /// tests, which use a server on a loopback port.
    pub async fn load(path: PathBuf, profile_url: impl Into<String>) -> Self {
        let mut corrupt = false;
        let links = match tokio::fs::read(&path).await {
            Ok(data) => match serde_json::from_slice::<StoredLinks>(&data) {
                Ok(stored) => stored.links,
                Err(e) => {
                    error!(
                        "{} is corrupt ({e}); no Minecraft account is linked. The file is \
                         kept as it is until a link is made or removed",
                        path.display()
                    );
                    corrupt = true;
                    Vec::new()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => {
                error!(
                    "Failed to read {} ({e}); no Minecraft account is linked",
                    path.display()
                );
                corrupt = true;
                Vec::new()
            }
        };
        let http = reqwest::Client::builder()
            .timeout(VERIFY_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .unwrap_or_default();
        Self {
            path,
            links: tokio::sync::Mutex::new(links),
            corrupt: corrupt.into(),
            profile_url: profile_url.into(),
            http,
        }
    }

    async fn save(&self, links: &[MinecraftLink]) -> Result<(), Error> {
        let data = serde_json::to_vec_pretty(&StoredLinks {
            links: links.to_vec(),
        })
        .context("Failed to serialize the Minecraft links")?;
        crate::persist::write_atomic(&self.path, data, Some(0o600))
            .await
            .context(format!("Failed to write {}", self.path.display()))?;
        if self
            .corrupt
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            info!(
                "Replaced the corrupt {} with the current Minecraft links",
                self.path.display()
            );
        }
        Ok(())
    }

    /// The links of `uid`, oldest first.
    pub async fn for_user(&self, uid: &UserId) -> Vec<MinecraftLink> {
        self.links
            .lock()
            .await
            .iter()
            .filter(|link| link.uid == *uid)
            .cloned()
            .collect()
    }

    /// Every link, oldest first.
    pub async fn all(&self) -> Vec<MinecraftLink> {
        self.links.lock().await.clone()
    }

    /// Asks Minecraft's services whose profile `access_token` belongs to. The token is
    /// only put in this request's `Authorization` header, and never in an error.
    async fn verify(&self, access_token: &str) -> Result<Profile, Error> {
        let response = self
            .http
            .get(&self.profile_url)
            .bearer_auth(access_token)
            .header(reqwest::header::ACCEPT, "application/json")
            .send()
            .await
            .map_err(|e| Error {
                kind: ErrorKind::External,
                // without the URL's query or the headers: reqwest never prints those
                source: eyre!(
                    "Couldn't reach Minecraft's services to check the account ({})",
                    e.without_url()
                ),
            })?;
        let status = response.status();
        match status.as_u16() {
            200 => {}
            401 | 403 => {
                return Err(bad_request(
                    "Minecraft's services refused the account's token. Sign in to the \
                     account again in the launcher and retry",
                ))
            }
            404 => return Err(bad_request("This Microsoft account doesn't own Minecraft")),
            429 => {
                return Err(Error {
                    kind: ErrorKind::TooManyRequests,
                    source: eyre!("Minecraft's services are busy. Try again in a minute"),
                })
            }
            _ => {
                return Err(Error {
                    kind: ErrorKind::External,
                    source: eyre!("Minecraft's services answered {status}"),
                })
            }
        }
        let body = response.bytes().await.map_err(|e| Error {
            kind: ErrorKind::External,
            source: eyre!(
                "Couldn't read the answer of Minecraft's services ({})",
                e.without_url()
            ),
        })?;
        if body.len() > MAX_PROFILE_BYTES {
            return Err(Error {
                kind: ErrorKind::External,
                source: eyre!("Minecraft's services sent an answer that is too large"),
            });
        }
        serde_json::from_slice::<Profile>(&body).map_err(|_| Error {
            kind: ErrorKind::External,
            source: eyre!("Minecraft's services sent an answer Lodestone doesn't understand"),
        })
    }

    /// Links the account in `request` to `uid` once Minecraft's services confirm that the
    /// token belongs to it. Returns the link.
    pub async fn link(
        &self,
        uid: &UserId,
        request: LinkMinecraftAccount,
        now: i64,
    ) -> Result<MinecraftLink, Error> {
        let LinkMinecraftAccount {
            minecraft_uuid,
            minecraft_name,
            access_token,
        } = request;
        let minecraft_uuid = normalize_uuid(&minecraft_uuid)?;
        if !is_valid_name(minecraft_name.trim()) {
            return Err(bad_request("That isn't a Minecraft account name"));
        }
        if access_token.is_empty() || access_token.len() > MAX_TOKEN_BYTES {
            return Err(bad_request("That isn't a Minecraft access token"));
        }
        if access_token
            .bytes()
            .any(|b| b.is_ascii_control() || b == b' ' || !b.is_ascii())
        {
            return Err(bad_request("That isn't a Minecraft access token"));
        }
        let at_limit = {
            let links = self.links.lock().await;
            let own = links.iter().filter(|l| l.uid == *uid).count();
            let relink = links
                .iter()
                .any(|l| l.uid == *uid && l.minecraft_uuid == minecraft_uuid);
            own >= MAX_LINKS_PER_USER && !relink
        };
        if at_limit {
            return Err(bad_request(format!(
                "You can link at most {MAX_LINKS_PER_USER} Minecraft accounts"
            )));
        }
        let profile = self.verify(&access_token).await;
        drop(access_token);
        let profile = profile?;
        let verified_uuid = normalize_uuid(&profile.id).map_err(|_| Error {
            kind: ErrorKind::External,
            source: eyre!("Minecraft's services sent an answer Lodestone doesn't understand"),
        })?;
        if verified_uuid != minecraft_uuid {
            return Err(bad_request(
                "The token belongs to a different Minecraft account than the one selected",
            ));
        }
        if !is_valid_name(&profile.name) {
            return Err(Error {
                kind: ErrorKind::External,
                source: eyre!("Minecraft's services sent an answer Lodestone doesn't understand"),
            });
        }
        let link = MinecraftLink {
            uid: uid.clone(),
            minecraft_uuid,
            minecraft_name: profile.name,
            verified: true,
            linked_at: now,
        };
        let mut links = self.links.lock().await;
        let mut updated: Vec<MinecraftLink> = links
            .iter()
            .filter(|l| l.minecraft_uuid != link.minecraft_uuid)
            .cloned()
            .collect();
        if updated.iter().filter(|l| l.uid == *uid).count() >= MAX_LINKS_PER_USER {
            return Err(bad_request(format!(
                "You can link at most {MAX_LINKS_PER_USER} Minecraft accounts"
            )));
        }
        updated.push(link.clone());
        self.save(&updated).await?;
        *links = updated;
        info!(
            "Linked Minecraft account {} to a Lodestone user",
            link.minecraft_uuid
        );
        Ok(link)
    }

    /// Removes the link of `minecraft_uuid`, if it belongs to `owner_uid` (or to anyone,
    /// for `None`). Returns whether a link was removed.
    pub async fn remove(
        &self,
        minecraft_uuid: &str,
        owner_uid: Option<&UserId>,
    ) -> Result<bool, Error> {
        let minecraft_uuid = normalize_uuid(minecraft_uuid)?;
        let mut links = self.links.lock().await;
        let matches = |l: &MinecraftLink| {
            l.minecraft_uuid == minecraft_uuid && owner_uid.is_none_or(|uid| l.uid == *uid)
        };
        if !links.iter().any(matches) {
            return Ok(false);
        }
        let updated: Vec<MinecraftLink> = links.iter().filter(|l| !matches(l)).cloned().collect();
        self.save(&updated).await?;
        *links = updated;
        Ok(true)
    }

    /// Removes every link of `uid`, for a deleted user.
    pub async fn remove_user(&self, uid: &UserId) -> Result<(), Error> {
        let mut links = self.links.lock().await;
        if !links.iter().any(|l| l.uid == *uid) {
            return Ok(());
        }
        let updated: Vec<MinecraftLink> = links.iter().filter(|l| l.uid != *uid).cloned().collect();
        self.save(&updated).await?;
        *links = updated;
        Ok(())
    }
}
