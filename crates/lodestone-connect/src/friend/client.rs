//! The introduction service's v2 calls, for friends (services/connect/README.md has the
//! protocol): pairing mailboxes, slots, and meeting (records, knocks, presence).
//!
//! Everything sent is already sealed or a hash: the service never gets a secret it could
//! use to read anything.

use std::net::IpAddr;

use serde_json::{json, Value};

use super::crypto::ID_LEN;
use crate::{
    hex,
    service::{call, Service},
    unhex, Error, Result,
};

fn hex_field(value: &Value, name: &str) -> Result<Vec<u8>> {
    value[name]
        .as_str()
        .and_then(unhex)
        .ok_or_else(|| Error::Unreachable(format!("the service sent no {name}")))
}

fn str_field(value: &Value, name: &str) -> Result<String> {
    value[name]
        .as_str()
        .map(str::to_string)
        .ok_or_else(|| Error::Unreachable(format!("the service sent no {name}")))
}

/// `None` for a 404 answer.
fn found<T>(result: Result<T>) -> Result<Option<T>> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(Error::Service { status: 404, .. }) => Ok(None),
        Err(err) => Err(err),
    }
}

impl Service {
    // -----------------------------------------------------------------------
    // Pairing mailboxes

    /// Opens the mailbox `code` with the sealed card. Returns the mailbox's owner
    /// secret. An error with code `taken` means the code is in use (make a new link).
    pub async fn pair_open(&self, code: &str, access_hash: &str, card: &[u8]) -> Result<String> {
        let answer = call(
            self,
            "PUT",
            &format!("/v2/pairs/{code}"),
            None,
            Some(json!({ "access": access_hash, "card": hex(card) })),
        )
        .await?;
        str_field(&answer, "secret")
    }

    /// The sealed card in mailbox `code`. `None` if there's no such mailbox (the link
    /// was used or expired).
    pub async fn pair_card(&self, code: &str, access: &str) -> Result<Option<Vec<u8>>> {
        found(
            call(self, "GET", &format!("/v2/pairs/{code}"), Some(access), None)
                .await
                .and_then(|answer| hex_field(&answer, "card")),
        )
    }

    /// Puts a sealed reply in mailbox `code`; returns how many replies it has now.
    pub async fn pair_reply(&self, code: &str, access: &str, reply: &[u8]) -> Result<u64> {
        let answer = call(
            self,
            "POST",
            &format!("/v2/pairs/{code}/replies"),
            Some(access),
            Some(json!({ "reply": hex(reply) })),
        )
        .await?;
        Ok(answer["count"].as_u64().unwrap_or(1))
    }

    /// The sealed replies in mailbox `code`, for its owner. `None` if it's gone.
    pub async fn pair_replies(&self, code: &str, secret: &str) -> Result<Option<Vec<Vec<u8>>>> {
        found(
            call(self, "GET", &format!("/v2/pairs/{code}/replies"), Some(secret), None)
                .await
                .map(|answer| {
                    answer["replies"]
                        .as_array()
                        .map(|list| list.iter().filter_map(|r| unhex(r.as_str()?)).collect())
                        .unwrap_or_default()
                }),
        )
    }

    /// Deletes mailbox `code`.
    pub async fn pair_close(&self, code: &str, secret: &str) -> Result<()> {
        match call(self, "DELETE", &format!("/v2/pairs/{code}"), Some(secret), None).await {
            Ok(_) | Err(Error::Service { status: 404, .. }) => Ok(()),
            Err(err) => Err(err),
        }
    }

    // -----------------------------------------------------------------------
    // Slots

    /// Writes a sealed state to slot `id`, proving it's the sender's with `proof`.
    pub async fn slot_write(&self, id: &[u8; ID_LEN], proof: &str, sealed: &[u8]) -> Result<()> {
        call(
            self,
            "PUT",
            &format!("/v2/slots/{}", hex(id)),
            Some(proof),
            Some(json!({ "sealed": hex(sealed) })),
        )
        .await
        .map(|_| ())
    }

    /// The sealed state in slot `id`, if any.
    pub async fn slot_read(&self, id: &[u8; ID_LEN]) -> Result<Option<Vec<u8>>> {
        found(
            call(self, "GET", &format!("/v2/slots/{}", hex(id)), None, None)
                .await
                .and_then(|answer| hex_field(&answer, "sealed")),
        )
    }

    /// Deletes slot `id`.
    pub async fn slot_clear(&self, id: &[u8; ID_LEN], proof: &str) -> Result<()> {
        match call(self, "DELETE", &format!("/v2/slots/{}", hex(id)), Some(proof), None).await {
            Ok(_) | Err(Error::Service { status: 404, .. }) => Ok(()),
            Err(err) => Err(err),
        }
    }

    // -----------------------------------------------------------------------
    // Meeting: the friend's side

    /// Knocks on `lookup` with a sealed knock; returns the core's sealed record, or
    /// `None` if no core is publishing there now.
    pub async fn knock(&self, lookup: &[u8; ID_LEN], sealed: &[u8]) -> Result<Option<Vec<u8>>> {
        found(
            call(
                self,
                "POST",
                &format!("/v2/lookups/{}/knocks", hex(lookup)),
                None,
                Some(json!({ "sealed": hex(sealed) })),
            )
            .await
            .and_then(|answer| hex_field(&answer, "sealed")),
        )
    }

    /// Whether a core is publishing at `lookup` now.
    pub async fn is_publishing(&self, lookup: &[u8; ID_LEN]) -> Result<bool> {
        match call(self, "HEAD", &format!("/v2/lookups/{}", hex(lookup)), None, None).await {
            Ok(_) => Ok(true),
            Err(Error::Service { status: 404, .. }) => Ok(false),
            Err(err) => Err(err),
        }
    }
}

// ---------------------------------------------------------------------------
// Meeting: the core's side

/// A meeting session at the service.
#[derive(Clone, PartialEq, Eq)]
pub struct MeetSession {
    pub session: String,
    /// Proves to the service that this is the session's core. Never shared.
    pub secret: String,
}

impl std::fmt::Debug for MeetSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeetSession")
            .field("session", &self.session)
            .finish_non_exhaustive()
    }
}

/// A knock as the service passes it on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeliveredKnock {
    pub lookup: [u8; ID_LEN],
    pub sealed: Vec<u8>,
    /// The address the service saw the knock come from.
    pub seen: Option<IpAddr>,
}

#[cfg(feature = "host")]
fn records_json(records: &[([u8; ID_LEN], Vec<u8>)]) -> Value {
    json!({
        "records": records
            .iter()
            .map(|(lookup, sealed)| json!({ "lookup": hex(lookup), "sealed": hex(sealed) }))
            .collect::<Vec<_>>()
    })
}

#[cfg(feature = "host")]
pub use meet_host::*;

#[cfg(feature = "host")]
mod meet_host {
    use futures_util::{SinkExt as _, StreamExt as _};
    use std::pin::Pin;
    use tokio_tungstenite::{tungstenite::Message, WebSocketStream};

    use super::*;
    use crate::service::{Io, CLOSE_ENDED, PING_EVERY};

    impl Service {
        /// Opens a meeting session with these records (lookup id, sealed record).
        pub async fn meet_open(&self, records: &[([u8; ID_LEN], Vec<u8>)]) -> Result<MeetSession> {
            let answer = call(self, "PUT", "/v2/meet", None, Some(records_json(records))).await?;
            Ok(MeetSession {
                session: str_field(&answer, "session")?,
                secret: str_field(&answer, "secret")?,
            })
        }

        /// Replaces a session's records. A 404 error means the session ended: open
        /// another.
        pub async fn meet_update(
            &self,
            session: &MeetSession,
            records: &[([u8; ID_LEN], Vec<u8>)],
        ) -> Result<()> {
            call(
                self,
                "PUT",
                &format!("/v2/meet/{}", session.session),
                Some(&session.secret),
                Some(records_json(records)),
            )
            .await
            .map(|_| ())
        }

        /// Ends a session: its records go at once.
        pub async fn meet_close(&self, session: &MeetSession) -> Result<()> {
            match call(
                self,
                "DELETE",
                &format!("/v2/meet/{}", session.session),
                Some(&session.secret),
                None,
            )
            .await
            {
                Ok(_) | Err(Error::Service { status: 404, .. }) => Ok(()),
                Err(err) => Err(err),
            }
        }

        /// Waits for knocks on a session's records. `None` if the session has ended.
        pub async fn meet_watch(&self, session: &MeetSession) -> Result<Option<MeetWatch>> {
            let path = format!("/v2/meet/{}/knocks", session.session);
            Ok(self.websocket(&path, &session.secret).await?.map(|ws| {
                let mut ping = tokio::time::interval(PING_EVERY);
                ping.reset();
                MeetWatch { ws, ping }
            }))
        }
    }

    /// What a waiting core hears.
    #[derive(Debug, PartialEq)]
    pub enum MeetEvent {
        Knocks(Vec<DeliveredKnock>),
        /// The session ended (quiet too long, a day old, or deleted).
        Ended,
    }

    /// A core's WebSocket, waiting for knocks.
    pub struct MeetWatch {
        ws: WebSocketStream<Pin<Box<dyn Io>>>,
        ping: tokio::time::Interval,
    }

    impl MeetWatch {
        /// The next knocks, or the end of the session. Keeps the socket alive meanwhile.
        pub async fn next(&mut self) -> Result<MeetEvent> {
            let lost = |err: String| Error::Unreachable(err);
            loop {
                tokio::select! {
                    _ = self.ping.tick() => self.ws.send(Message::text("ping")).await.map_err(|err| lost(err.to_string()))?,
                    message = self.ws.next() => match message.ok_or_else(|| lost("the service closed the connection".into()))?.map_err(|err| lost(err.to_string()))? {
                        Message::Text(text) => {
                            if text.as_str() == "pong" {
                                continue;
                            }
                            let value: Value = serde_json::from_str(text.as_str()).map_err(|err| lost(err.to_string()))?;
                            let knocks = value["knocks"]
                                .as_array()
                                .ok_or_else(|| lost("a garbled message from the service".into()))?
                                .iter()
                                .filter_map(|knock| {
                                    Some(DeliveredKnock {
                                        lookup: unhex(knock["lookup"].as_str()?)?.try_into().ok()?,
                                        sealed: unhex(knock["sealed"].as_str()?)?,
                                        seen: knock["seen"].as_str().and_then(|ip| ip.parse().ok()),
                                    })
                                })
                                .collect();
                            return Ok(MeetEvent::Knocks(knocks));
                        }
                        Message::Close(Some(frame)) if u16::from(frame.code) == CLOSE_ENDED => {
                            return Ok(MeetEvent::Ended);
                        }
                        Message::Close(frame) => {
                            return Err(lost(format!("the service closed the connection ({frame:?})")));
                        }
                        _ => {}
                    },
                }
            }
        }
    }
}
