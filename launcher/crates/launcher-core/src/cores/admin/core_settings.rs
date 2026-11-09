//! The core's settings, what it lists publicly, and UPnP: work package D (see
//! `launcher/docs/host-parity.md`). After the dashboard's `pages/settings/CoreSettings.tsx`
//! and `components/Instance/PublicListingSetting.tsx`.
//!
//! The public listing's types and calls here are shared with the instance Settings tab
//! (package C), which shows one instance's row.

use http_client::Method;
use serde::{Deserialize, Serialize};

use super::{AdminClient, models::GlobalSettingsData, segment};
use crate::cores::CoreError;

/// A core setting that is a switch (`PUT /global_settings/<route>` with a JSON bool).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreSwitch {
    SafeMode,
    CodeExecution,
    AllowLanHttp,
    AllowLocalClaim,
    ListInstancesPublicly,
}

impl CoreSwitch {
    fn route(self) -> &'static str {
        match self {
            CoreSwitch::SafeMode => "safe_mode",
            CoreSwitch::CodeExecution => "code_execution",
            CoreSwitch::AllowLanHttp => "allow_lan_http",
            CoreSwitch::AllowLocalClaim => "allow_local_claim",
            CoreSwitch::ListInstancesPublicly => "list_instances_publicly",
        }
    }

    /// Its value in `settings`.
    pub fn get(self, settings: &GlobalSettingsData) -> bool {
        match self {
            CoreSwitch::SafeMode => settings.safe_mode,
            CoreSwitch::CodeExecution => settings.code_execution_enabled,
            CoreSwitch::AllowLanHttp => settings.allow_lan_http,
            CoreSwitch::AllowLocalClaim => settings.allow_local_claim,
            CoreSwitch::ListInstancesPublicly => settings.list_instances_publicly,
        }
    }

    /// Sets it in `settings`, as the core now has it.
    pub fn set(self, settings: &mut GlobalSettingsData, value: bool) {
        let field = match self {
            CoreSwitch::SafeMode => &mut settings.safe_mode,
            CoreSwitch::CodeExecution => &mut settings.code_execution_enabled,
            CoreSwitch::AllowLanHttp => &mut settings.allow_lan_http,
            CoreSwitch::AllowLocalClaim => &mut settings.allow_local_claim,
            CoreSwitch::ListInstancesPublicly => &mut settings.list_instances_publicly,
        };
        *field = value;
    }
}

impl AdminClient {
    /// `GET /global_settings`.
    pub async fn global_settings(&self) -> Result<GlobalSettingsData, CoreError> {
        self.get("global_settings").await
    }

    /// `GET /global_settings` as the core sent it, to tell which settings an older core
    /// lacks (the dashboard hides those).
    pub async fn global_settings_raw(&self) -> Result<serde_json::Value, CoreError> {
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

    /// `PUT /global_settings/<switch>`.
    pub async fn set_core_switch(&self, switch: CoreSwitch, value: bool) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!("global_settings/{}", switch.route()),
            Some(value),
        )
        .await
    }

    /// `PUT /gateway/open_port/{port}`: asks the router to forward `port` (UPnP).
    pub async fn open_port(&self, port: u16) -> Result<(), CoreError> {
        self.call(
            Method::PUT,
            &format!("gateway/open_port/{port}"),
            None::<()>,
        )
        .await
    }

    /// `GET /public_listing`: what every Minecraft Java instance shows publicly (the
    /// owner's).
    pub async fn public_listing_overview(&self) -> Result<PublicListingOverview, CoreError> {
        self.get("public_listing").await
    }

    /// `GET /instance/{uuid}/public_listing`.
    pub async fn instance_public_listing(
        &self,
        uuid: &str,
    ) -> Result<InstancePublicListing, CoreError> {
        self.get(&format!("instance/{}/public_listing", segment(uuid)))
            .await
    }

    /// `PUT /instance/{uuid}/public_listing`: changes the flags `change` names (only the
    /// owner may). The reply is the instance's new scope.
    pub async fn set_public_scope(
        &self,
        uuid: &str,
        change: &PublicScopeChange,
    ) -> Result<PublicScope, CoreError> {
        self.json(
            Method::PUT,
            &format!("instance/{}/public_listing", segment(uuid)),
            Some(change),
        )
        .await
    }
}

/// What the public routes show of an instance (`PublicScope`). Without `listed`
/// nothing is shown.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct PublicScope {
    pub listed: bool,
    pub status: bool,
    pub address: bool,
    pub client_pack: bool,
}

impl PublicScope {
    /// Applies a change the core accepted.
    pub fn apply(&mut self, change: &PublicScopeChange) {
        let flags = [
            (&mut self.listed, change.listed),
            (&mut self.status, change.status),
            (&mut self.address, change.address),
            (&mut self.client_pack, change.client_pack),
        ];
        for (field, value) in flags {
            if let Some(value) = value {
                *field = value;
            }
        }
    }
}

/// A change to a [`PublicScope`]: the flags given are set, the others kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct PublicScopeChange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub listed: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_pack: Option<bool>,
}

/// One flag of a [`PublicScope`] besides `listed`, as the row's checkboxes show them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeFlag {
    Status,
    Address,
    ClientPack,
}

impl ScopeFlag {
    pub const ALL: [ScopeFlag; 3] = [ScopeFlag::Status, ScopeFlag::Address, ScopeFlag::ClientPack];

    pub fn get(self, scope: &PublicScope) -> bool {
        match self {
            ScopeFlag::Status => scope.status,
            ScopeFlag::Address => scope.address,
            ScopeFlag::ClientPack => scope.client_pack,
        }
    }

    /// The change that sets this flag to `value`.
    pub fn change(self, value: bool) -> PublicScopeChange {
        let mut change = PublicScopeChange::default();
        match self {
            ScopeFlag::Status => change.status = Some(value),
            ScopeFlag::Address => change.address = Some(value),
            ScopeFlag::ClientPack => change.client_pack = Some(value),
        }
        change
    }
}

/// `GET /instance/{uuid}/public_listing` (`InstancePublicListing`). A core from before
/// the scopes sends only `listed`: [`Self::has_scopes`] is false, and the dashboard
/// hides the setting then.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct InstancePublicListing {
    /// The core's "List servers publicly" switch: while it is off, a listed instance
    /// isn't shown either.
    #[serde(default)]
    pub list_instances_publicly: bool,
    #[serde(default)]
    pub listed: bool,
    #[serde(default)]
    pub status: Option<bool>,
    #[serde(default)]
    pub address: Option<bool>,
    #[serde(default)]
    pub client_pack: Option<bool>,
}

impl InstancePublicListing {
    /// Whether the core knows the scope flags (not only `listed`).
    pub fn has_scopes(&self) -> bool {
        self.status.is_some()
    }

    pub fn scope(&self) -> PublicScope {
        PublicScope {
            listed: self.listed,
            status: self.status.unwrap_or(false),
            address: self.address.unwrap_or(false),
            client_pack: self.client_pack.unwrap_or(false),
        }
    }
}

/// One instance in `GET /public_listing` (`InstancePublicScope`).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct InstancePublicScope {
    pub uuid: String,
    pub name: String,
    #[serde(flatten)]
    pub scope: PublicScope,
}

/// `GET /public_listing` (`PublicListingOverview`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct PublicListingOverview {
    pub list_instances_publicly: bool,
    pub instances: Vec<InstancePublicScope>,
}

/// The core name's longest length, as the dashboard checks it.
pub const CORE_NAME_MAX: usize = 32;
/// The public domain's longest length.
pub const DOMAIN_MAX: usize = 253;

/// What is wrong with a core name, as the dashboard checks it; the core checks again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoreNameProblem {
    Empty,
    TooLong,
}

pub fn core_name_problem(name: &str) -> Option<CoreNameProblem> {
    if name.is_empty() {
        Some(CoreNameProblem::Empty)
    } else if name.chars().count() > CORE_NAME_MAX {
        Some(CoreNameProblem::TooLong)
    } else {
        None
    }
}

/// Whether a domain is too long (it may be empty).
pub fn domain_too_long(domain: &str) -> bool {
    domain.chars().count() > DOMAIN_MAX
}

/// The port typed in the "Open port" dialog, as the dashboard checks it.
pub fn parse_port(text: &str) -> Result<u16, PortProblem> {
    let text = text.trim();
    if text.is_empty() {
        return Err(PortProblem::Required);
    }
    let number: i64 = text.parse().map_err(|_| PortProblem::NotNumber)?;
    if (1..=65535).contains(&number) {
        Ok(number as u16)
    } else {
        Err(PortProblem::Range)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PortProblem {
    Required,
    NotNumber,
    Range,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_public_listing() {
        let overview: PublicListingOverview = serde_json::from_value(serde_json::json!({
            "list_instances_publicly": true,
            "instances": [{ "uuid": "a", "name": "SMP", "listed": true, "status": true,
                            "address": false, "client_pack": false }],
        }))
        .unwrap();
        assert!(overview.instances[0].scope.listed && overview.instances[0].scope.status);
        let old: InstancePublicListing = serde_json::from_value(
            serde_json::json!({ "list_instances_publicly": false, "listed": true }),
        )
        .unwrap();
        assert!(!old.has_scopes());
        let new: InstancePublicListing = serde_json::from_value(serde_json::json!({
            "list_instances_publicly": false, "listed": true, "status": false,
            "address": true, "client_pack": false,
        }))
        .unwrap();
        assert!(new.has_scopes() && new.scope().address);
    }

    #[test]
    fn a_change_sends_only_its_flags() {
        assert_eq!(
            serde_json::to_value(ScopeFlag::Address.change(true)).unwrap(),
            serde_json::json!({ "address": true })
        );
        let mut scope = PublicScope::default();
        scope.apply(&PublicScopeChange {
            listed: Some(true),
            ..Default::default()
        });
        assert!(scope.listed && !scope.status);
    }

    #[test]
    fn switches_read_and_write_their_setting() {
        let mut settings = GlobalSettingsData::default();
        CoreSwitch::AllowLocalClaim.set(&mut settings, true);
        assert!(settings.allow_local_claim);
        assert!(CoreSwitch::AllowLocalClaim.get(&settings));
        assert!(!CoreSwitch::SafeMode.get(&settings));
    }

    #[test]
    fn checks_names_and_ports_as_the_dashboard() {
        assert_eq!(core_name_problem(""), Some(CoreNameProblem::Empty));
        assert_eq!(
            core_name_problem(&"x".repeat(33)),
            Some(CoreNameProblem::TooLong)
        );
        assert_eq!(core_name_problem("Home"), None);
        assert!(domain_too_long(&"a".repeat(254)));
        assert!(!domain_too_long(""));
        assert_eq!(parse_port(" 25565 "), Ok(25565));
        assert_eq!(parse_port(""), Err(PortProblem::Required));
        assert_eq!(parse_port("abc"), Err(PortProblem::NotNumber));
        assert_eq!(parse_port("0"), Err(PortProblem::Range));
        assert_eq!(parse_port("65536"), Err(PortProblem::Range));
    }
}
