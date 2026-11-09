//! Users, permissions, the profile, API keys and Minecraft links: work package D (see `launcher/docs/host-parity.md`).
//!
//! Add the routes as `impl AdminClient` methods here, with this area's own types.
//! To do:
//!
//! - `GET /user/info, GET /user/list, POST /user, DELETE /user/{uid}`
//! - `PUT /user/{uid}/password, PUT /user/{uid}/rename, PUT /user/{uid}/update_perm`
//! - `GET/POST /user/api_keys, DELETE /user/api_keys/{id}, GET /user/{uid}/api_keys, DELETE /user/{uid}/api_keys/{id}`
//! - `GET /user/minecraft_links, DELETE /user/minecraft_links/{mc}, GET /minecraft_links, DELETE /minecraft_links/{mc}`

use super::{AdminClient, models::PublicUser};
use crate::cores::CoreError;

impl AdminClient {
    /// `GET /user/info`: the signed-in user.
    pub async fn me(&self) -> Result<PublicUser, CoreError> {
        self.get("user/info").await
    }
}
