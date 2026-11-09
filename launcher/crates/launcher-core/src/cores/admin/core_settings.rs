//! The core's settings: work package D (see `launcher/docs/host-parity.md`).
//!
//! Add the routes as `impl AdminClient` methods here, with this area's own types.
//! To do:
//!
//! - `GET /global_settings -> GlobalSettingsData; PUT /global_settings/{name|domain|safe_mode|code_execution|allow_lan_http|playit_enabled|allow_local_claim|list_instances_publicly}`
//! - `GET /public_listing -> PublicListingOverview`
//! - `PUT /gateway/open_port/{port}`

use http_client::Method;

use super::{AdminClient, models::GlobalSettingsData};
use crate::cores::CoreError;

impl AdminClient {
    /// `GET /global_settings`.
    pub async fn global_settings(&self) -> Result<GlobalSettingsData, CoreError> {
        self.get("global_settings").await
    }

    /// `PUT /global_settings/name`: the core's name (first run, and Core settings).
    pub async fn set_core_name(&self, name: &str) -> Result<(), CoreError> {
        self.call(Method::PUT, "global_settings/name", Some(name))
            .await
    }

    /// `PUT /global_settings/domain`: the public address players use (may be empty).
    pub async fn set_domain(&self, domain: &str) -> Result<(), CoreError> {
        self.call(Method::PUT, "global_settings/domain", Some(domain))
            .await
    }
}
