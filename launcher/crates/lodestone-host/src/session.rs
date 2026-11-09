//! Signing in to the Host's own core as its owner.
//!
//! The Tauri Host ran the core in its own process and asked it for an owner token
//! directly (`tauri_export::get_owner_jwt`). This Host runs the core as a child, so it
//! uses the core's local claim instead (`core/src/local_claim.rs`): it proves it can
//! write the core's data folder, which only this OS user can, and the core writes an
//! owner sign-in token into that folder ([`local_claim::claim_session`]). The token is
//! tied to the core's HTTPS certificate and only ever sent to it. Nothing is saved:
//! the Host claims again each time it starts.
//!
//! - A core without an owner yet (first run): the user picks the owner's username and
//!   password, as on the dashboard's setup page, and the claim makes the account. The
//!   password is what the web dashboard and other devices sign in with.
//! - A core whose owner turned off "Let apps on this computer claim owner access": the
//!   claim is refused and the user signs in with a username and password, as on the
//!   dashboard's sign-in page ([`sign_in_with_password`]).

use std::{path::Path, sync::Arc};

use http_client::HttpClient;
use launcher_core::{
    Secret,
    cores::{
        CoreClient, CoreError,
        admin::{AdminClient, FullCoreInfo, PublicUser},
        local_claim::{self, ClaimError, OwnerCredentials},
    },
    game::download::Sleep,
};

/// The name the core logs the claim under.
pub const CLAIM_NAME: &str = "Lodestone Host";

/// Signed in to a core.
#[derive(Clone, Debug)]
pub struct Session {
    pub admin: AdminClient,
    pub info: FullCoreInfo,
    pub me: PublicUser,
}

/// Why signing in didn't work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SignInError {
    /// The core has no owner yet: ask for one ([`claim_owner`] with the account).
    NeedsOwner,
    /// The core refused the claim (its owner turned it off): sign in with a password.
    Refused,
    /// Anything else, for people.
    Failed(String),
}

impl From<ClaimError> for SignInError {
    fn from(err: ClaimError) -> Self {
        match err {
            ClaimError::Refused => SignInError::Refused,
            err => SignInError::Failed(err.to_string()),
        }
    }
}

impl From<CoreError> for SignInError {
    fn from(err: CoreError) -> Self {
        SignInError::Failed(err.to_string())
    }
}

/// Signs in to the core at `local_url` (its plain-HTTP address on this computer),
/// whose data folder is `data_dir`, as its owner. `owner` makes the owner account on a
/// core that has none; without it such a core gives [`SignInError::NeedsOwner`].
pub async fn claim_owner(
    http: Arc<dyn HttpClient>,
    local_url: url::Url,
    data_dir: &Path,
    owner: Option<OwnerCredentials>,
    sleep: Sleep,
) -> Result<Session, SignInError> {
    let local = CoreClient::new(http, local_url);
    let info = local.info().await?;
    if !info.is_setup && owner.is_none() {
        return Err(SignInError::NeedsOwner);
    }
    let session =
        local_claim::claim_session(&local, data_dir, owner.as_ref(), CLAIM_NAME, sleep).await?;
    let admin = AdminClient::new(session.client()?, session.token.clone());
    finish(admin).await
}

/// Signs in with a username and password (`POST /user/login`), over the core's
/// plain-HTTP address on this computer.
pub async fn sign_in_with_password(
    http: Arc<dyn HttpClient>,
    local_url: url::Url,
    username: &str,
    password: &Secret,
) -> Result<Session, SignInError> {
    let local = CoreClient::new(http, local_url);
    let reply = launcher_core::cores::admin::login(&local, username, password).await?;
    finish(AdminClient::new(local, Secret::new(reply.token))).await
}

/// Signs in to another core (`client`, with the certificate the user trusted) with a
/// username and password (`POST /user/login`), for Settings' other cores.
pub async fn sign_in_to(
    client: CoreClient,
    username: &str,
    password: &Secret,
) -> Result<Session, SignInError> {
    let reply = launcher_core::cores::admin::login(&client, username, password).await?;
    finish(AdminClient::new(client, Secret::new(reply.token))).await
}

/// What the shell needs once signed in: the core's details and who is signed in.
async fn finish(admin: AdminClient) -> Result<Session, SignInError> {
    let info = admin.full_info().await?;
    let me = admin.me().await?;
    Ok(Session { admin, info, me })
}

/// Checks the owner account the user typed on the first run, as the dashboard's setup
/// form does. `None` when it is fine; the core checks again.
pub fn owner_problem(username: &str, password: &str, confirm: &str) -> Option<OwnerProblem> {
    if username.trim().is_empty() {
        Some(OwnerProblem::NoUsername)
    } else if password.is_empty() {
        Some(OwnerProblem::NoPassword)
    } else if password != confirm {
        Some(OwnerProblem::Mismatch)
    } else {
        None
    }
}

/// What is wrong with the owner account typed on the first run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerProblem {
    NoUsername,
    NoPassword,
    Mismatch,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_owner_form_is_checked() {
        assert_eq!(
            owner_problem(" ", "pw", "pw"),
            Some(OwnerProblem::NoUsername)
        );
        assert_eq!(owner_problem("a", "", ""), Some(OwnerProblem::NoPassword));
        assert_eq!(owner_problem("a", "x", "y"), Some(OwnerProblem::Mismatch));
        assert_eq!(owner_problem("a", "x", "x"), None);
    }
}
