//! playit.gg: work package C (see `launcher/docs/host-parity.md`). Every route is the
//! owner's only; the runner and its tunnels are the core's, not an instance's.
//!
//! - `GET /playitgg/generate_signup_link`, `POST /playitgg/verify_key`
//! - `POST /playitgg/start_cli`, `POST /playitgg/stop_cli`, `GET /playitgg/cli_is_running`
//! - `GET /playitgg/get_tunnels -> PlayitTunnelInfo[]`

use http_client::Method;
use serde::{Deserialize, Serialize};

use super::AdminClient;
use crate::cores::CoreError;

/// Where to sign up for playit.gg (`PlayitSignupData`). The core waits for the claim.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayitSignupData {
    pub url: String,
    #[serde(default)]
    pub claim_code: String,
}

/// A playit.gg tunnel (`PlayitTunnelInfo`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayitTunnelInfo {
    #[serde(default)]
    pub local_ip: String,
    #[serde(default)]
    pub local_port: u16,
    #[serde(default)]
    pub tunnel_id: String,
    #[serde(default)]
    pub name: String,
    /// What players type to join.
    #[serde(default)]
    pub server_address: String,
    #[serde(default)]
    pub active: bool,
}

impl AdminClient {
    /// `GET /playitgg/generate_signup_link`: a link to make a playit.gg account. The
    /// core gives the same link again for a few minutes.
    pub async fn playit_signup_link(&self) -> Result<PlayitSignupData, CoreError> {
        self.get("playitgg/generate_signup_link").await
    }

    /// `POST /playitgg/verify_key`: whether the core has a working playit.gg key.
    pub async fn playit_verify(&self) -> Result<bool, CoreError> {
        self.json(Method::POST, "playitgg/verify_key", None::<()>)
            .await
    }

    /// `POST /playitgg/start_cli`: starts the runner. Its state follows as
    /// `PlayitggRunnerEvent`s.
    pub async fn playit_start(&self) -> Result<(), CoreError> {
        self.call(Method::POST, "playitgg/start_cli", None::<()>)
            .await
    }

    /// `POST /playitgg/stop_cli`.
    pub async fn playit_stop(&self) -> Result<(), CoreError> {
        self.call(Method::POST, "playitgg/stop_cli", None::<()>)
            .await
    }

    /// `GET /playitgg/cli_is_running`.
    pub async fn playit_running(&self) -> Result<bool, CoreError> {
        self.get("playitgg/cli_is_running").await
    }

    /// `GET /playitgg/get_tunnels`.
    pub async fn playit_tunnels(&self) -> Result<Vec<PlayitTunnelInfo>, CoreError> {
        self.get("playitgg/get_tunnels").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_tunnel() {
        let tunnel: PlayitTunnelInfo = serde_json::from_str(
            r#"{"local_ip":"127.0.0.1","local_port":25565,"tunnel_id":"t1",
                "name":"SMP","server_address":"x.joinmc.link","active":true}"#,
        )
        .unwrap();
        assert_eq!(tunnel.local_port, 25565);
        assert!(tunnel.active);
    }
}
