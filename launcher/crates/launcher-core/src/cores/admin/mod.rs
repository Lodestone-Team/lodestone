//! A core's full API, as a signed-in user of the web dashboard uses it: what Lodestone
//! Host shows of the core it runs (and of other cores it connects to).
//!
//! [`AdminClient`] is a [`CoreClient`] with a sign-in token (a session, from
//! [`super::local_claim::claim_session`] or `POST /user/login`). Each area of the
//! dashboard has its own file, so the people building the Host's pages in parallel
//! each mostly touch their own (see `launcher/docs/host-parity.md`):
//!
//! | File | Area | Work package |
//! |---|---|---|
//! | [`models`] | instances, users, settings, `/info` | shared: add, don't change |
//! | [`events`] | the event stream's `ClientEvent` | shared: complete, add helpers only |
//! | [`instances`] | list, power, create, delete, name, Overview | A |
//! | [`console`] | console buffer, commands | B |
//! | [`files`] | the instance file manager | B |
//! | [`instance_settings`] | settings manifest, auto start, listing, resource pack | C |
//! | [`mods`] | mods, plugins, Modrinth, modpacks | A (modpacks) and C (mods) |
//! | [`backups`] | backups: list, take, restore, download, schedule | backups |
//! | [`users`] | users, permissions, profile, API keys, Minecraft links | D |
//! | [`core_settings`] | global settings, public listing, UPnP | D |
//!
//! Errors are [`CoreError`]s: a 401 is [`CoreError::SignedOut`], a 403
//! [`CoreError::Forbidden`] with the core's reason.

pub mod backups;
pub mod console;
pub mod core_settings;
pub mod events;
pub mod files;
pub mod instance_settings;
pub mod instances;
pub mod models;
pub mod mods;
pub mod users;

use http_client::Method;
use serde::{Serialize, de::DeserializeOwned};

use super::{
    CoreClient, CoreError,
    client::{Auth, MAX_JSON_BYTES, REQUEST_TIMEOUT, check},
};
use crate::Secret;

pub use events::{ClientEvent, EventInner};
pub use models::{FullCoreInfo, InstanceInfo, InstanceState, PublicUser, UserPermission};

/// A core's API with a sign-in token. Cheap to clone.
#[derive(Clone)]
pub struct AdminClient {
    client: CoreClient,
    token: Secret,
}

impl std::fmt::Debug for AdminClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminClient")
            .field("core", &self.client.base().as_str())
            .finish_non_exhaustive()
    }
}

impl AdminClient {
    pub fn new(client: CoreClient, token: Secret) -> Self {
        Self { client, token }
    }

    /// The core's client, for routes that need no sign-in (`/info`) and for streams.
    pub fn core(&self) -> &CoreClient {
        &self.client
    }

    /// The sign-in token. Never log it.
    pub fn token(&self) -> &Secret {
        &self.token
    }

    /// `GET path` (below `/api/v1/`), as JSON.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, CoreError> {
        self.json(Method::GET, path, None::<()>).await
    }

    /// A request with a JSON body (or none), whose reply is JSON.
    pub async fn json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<impl Serialize>,
    ) -> Result<T, CoreError> {
        let bytes = self.send(method, path, body).await?;
        serde_json::from_slice(&bytes).map_err(|err| CoreError::Invalid(err.to_string()))
    }

    /// A request with a JSON body (or none) whose reply doesn't matter.
    pub async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<impl Serialize>,
    ) -> Result<(), CoreError> {
        self.send(method, path, body).await.map(drop)
    }

    /// A request with a JSON body (or none); the reply's bytes.
    pub async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<impl Serialize>,
    ) -> Result<Vec<u8>, CoreError> {
        let body = match body {
            Some(body) => Some((
                serde_json::to_vec(&body).map_err(|err| CoreError::Invalid(err.to_string()))?,
                "application/json".to_string(),
            )),
            None => None,
        };
        self.send_raw(method, path, body, MAX_JSON_BYTES).await
    }

    /// A request with any body (`(bytes, content type)`, for raw file contents and
    /// uploads); the reply's bytes, up to `limit`.
    pub async fn send_raw(
        &self,
        method: Method,
        path: &str,
        body: Option<(Vec<u8>, String)>,
        limit: u64,
    ) -> Result<Vec<u8>, CoreError> {
        let auth = Auth::Bearer(&self.token);
        let response = self
            .client
            .send_typed(method, path, auth, body, None, limit, REQUEST_TIMEOUT)
            .await?;
        check(&response, auth)?;
        Ok(response.body)
    }
}

impl AdminClient {
    /// Like [`AdminClient::send_raw`], with its own time limit: for requests that
    /// download or upload a lot (a modpack) or wait for something (a share link).
    pub async fn send_slow(
        &self,
        method: Method,
        path: &str,
        body: Option<(Vec<u8>, String)>,
        limit: u64,
        timeout: std::time::Duration,
    ) -> Result<Vec<u8>, CoreError> {
        let auth = Auth::Bearer(&self.token);
        let response = self
            .client
            .send_typed(method, path, auth, body, None, limit, timeout)
            .await?;
        check(&response, auth)?;
        Ok(response.body)
    }
}

impl AdminClient {
    /// `GET /info`: everything the core says about itself.
    pub async fn full_info(&self) -> Result<FullCoreInfo, CoreError> {
        self.get("info").await
    }
}

/// `POST /user/login` with a username and password: a sign-in session (the dashboard's
/// sign-in page). A 429 says how long to wait.
pub async fn login(
    client: &CoreClient,
    username: &str,
    password: &Secret,
) -> Result<models::LoginReply, CoreError> {
    client
        .json(
            Method::POST,
            "user/login",
            Auth::Basic(username, password),
            Some(b"{}".to_vec()),
        )
        .await
}

/// `value` as one path segment: percent-encoded, so a name with `/` or `?` stays one.
pub fn segment(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes())
        .collect::<String>()
        .replace('+', "%20")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segments_are_encoded() {
        assert_eq!(segment("plain"), "plain");
        assert_eq!(segment("a b/c?d"), "a%20b%2Fc%3Fd");
    }
}
