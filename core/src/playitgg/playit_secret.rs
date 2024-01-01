/*
Copyright 2022 Developed Methods LLC

Redistribution and use in source and binary forms, with or without modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice, this list of conditions and the following disclaimer in the documentation and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
*/

use rand::Rng;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::time::Duration;

use playit_agent_core::api::{
    api::{AgentType, ApiError, ApiErrorNoFail, ApiResponseError, AuthError, ReqTunnelsList},
    http_client::HttpClientError,
    PlayitApi,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use super::errors::CliError;
use playit_agent_core::api::api::{
    AssignedManagedCreate, ClaimSetupResponse, PortType, ReqClaimExchange, ReqClaimSetup,
    ReqTunnelsCreate, TunnelOriginCreate, TunnelType,
};
use playit_agent_core::api::ip_resource::IpResource;
use playit_agent_core::network::address_lookup::{AddressLookup, AddressValue};
use playit_agent_core::tunnel_runner::TunnelRunner;
use playit_agent_core::utils::now_milli;

pub fn claim_generate() -> String {
    let mut buffer = [0u8; 5];
    rand::thread_rng().fill(&mut buffer);
    hex::encode(&buffer)
}

pub fn claim_url(code: &str) -> Result<String, CliError> {
    if hex::decode(code).is_err() {
        return Err(CliError::InvalidClaimCode.into());
    }

    Ok(format!("https://playit.gg/claim/{}", code,))
}

pub async fn claim_exchange(
    claim_code: &str,
    agent_type: AgentType,
    wait_sec: u32,
) -> Result<String, CliError> {
    let api = PlayitApi::create(API_BASE.to_string(), None);

    let end_at = if wait_sec == 0 {
        u64::MAX
    } else {
        now_milli() + (wait_sec as u64) * 1000
    };

    loop {
        let setup = api
            .claim_setup(ReqClaimSetup {
                code: claim_code.to_string(),
                agent_type,
                version: format!("playit-cli {}", env!("CARGO_PKG_VERSION")),
            })
            .await?;

        match setup {
            ClaimSetupResponse::WaitingForUserVisit => {
                let msg = format!("Waiting for user to visit {}", claim_url(claim_code)?);
                println!("{}", msg);
            }
            ClaimSetupResponse::WaitingForUser => {
                println!("Waiting for user to approve");
            }
            ClaimSetupResponse::UserAccepted => {
                println!("User accepted, exchanging code for secret");
                break;
            }
            ClaimSetupResponse::UserRejected => {
                println!("User rejected");
                return Err(CliError::AgentClaimRejected);
            }
        }

        tokio::time::sleep(Duration::from_secs(3)).await;
    }

    let secret_key = loop {
        match api
            .claim_exchange(ReqClaimExchange {
                code: claim_code.to_string(),
            })
            .await
        {
            Ok(res) => break res.secret_key,
            Err(ApiError::Fail(status)) => {
                let msg = format!("code \"{}\" not ready, {:?}", claim_code, status);
                println!("{}", msg);
            }
            Err(error) => return Err(error.into()),
        };

        if now_milli() > end_at {
            println!("reached time limit");
            return Err(CliError::TimedOut);
        }

        tokio::time::sleep(Duration::from_secs(2)).await;
    };

    Ok(secret_key)
}

pub struct PlayitSecret {
    secret: RwLock<Option<String>>,
    path: Option<String>,
    allow_path_read: bool,
}

pub const API_BASE: &'static str = "https://api.playit.gg";

impl PlayitSecret {
    pub async fn create_api(&self) -> Result<PlayitApi, CliError> {
        let secret: String = self.get().await?;
        Ok(PlayitApi::create(API_BASE.to_string(), Some(secret)))
    }

    pub fn with_default_path(&mut self) -> &mut Self {
        if self.path.is_none() {
            self.path.replace("playit.toml".to_string());
        }
        self
    }

    pub async fn ensure_valid(&mut self) -> Result<&mut Self, CliError> {
        let api = match self.create_api().await {
            Ok(v) => v,
            Err(_) => {
                {
                    let mut secret = self.secret.write().await;
                    let _ = secret.take();
                }
                return Ok(self);
            }
        };

        println!("checking if secret key is valid");
        tokio::time::sleep(Duration::from_secs(1)).await;

        loop {
            match api.agents_rundata().await {
                Ok(data) => {
                    println!("secret key valid, agent has {} tunnels", data.tunnels.len());
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    break;
                }
                Err(ApiErrorNoFail::ClientError(error)) => {
                    println!(
                        "Failed to load data from api\nretrying in 3 seconds {:?}",
                        error
                    );
                    tokio::time::sleep(Duration::from_secs(3)).await;
                }
                Err(ApiErrorNoFail::ApiError(ApiResponseError::Auth(
                    AuthError::InvalidAgentKey,
                ))) => {
                    if !self.path.is_some() {
                        return Err(CliError::InvalidSecret);
                    }
                }
                Err(ApiErrorNoFail::ApiError(error)) => {
                    println!("unexpected error checking if secret is valid {:?}", error);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    return Err(CliError::ApiError(error));
                }
            }
        }

        Ok(self)
    }
    pub async fn get_or_setup(&mut self) -> Result<String, CliError> {
        if let Ok(secret) = self.get().await {
            return Ok(secret);
        }

        if self.path.is_none() {
            return Err(CliError::SecretFilePathMissing);
        }

        let claim_code = claim_generate();
        let secret = claim_exchange(&claim_code, AgentType::Assignable, 0).await?;

        {
            let mut lock = self.secret.write().await;
            lock.replace(secret.clone());
        }

        self.write_secret(secret.clone()).await?;
        Ok(secret)
    }

    async fn write_secret(&mut self, secret: String) -> Result<(), CliError> {
        let path = self
            .path
            .as_ref()
            .ok_or(CliError::SecretFilePathMissing)?
            .trim();

        let content = if path.ends_with(".toml") {
            toml::to_string(&OldConfig { secret_key: secret }).unwrap()
        } else {
            secret
        };

        if let Err(error) = tokio::fs::write(path, &content).await {
            println!("failed to save secret, path: {} {}", path, &error);
            tokio::time::sleep(Duration::from_secs(5)).await;
            return Err(CliError::SecretFileWriteError(error));
        }

        self.allow_path_read = true;
        Ok(())
    }

    pub async fn get(&self) -> Result<String, CliError> {
        {
            let lock = self.secret.read().await;
            if let Some(value) = &*lock {
                let trimmed = value.trim();
                if hex::decode(trimmed).is_err() {
                    return Err(CliError::MalformedSecret);
                }
                return Ok(trimmed.to_string());
            }
        }

        if !self.allow_path_read {
            return Err(CliError::MissingSecret);
        }

        let file_path = self.path.as_ref().ok_or(CliError::MissingSecret)?;

        let mut lock = self.secret.write().await;

        let content = tokio::fs::read_to_string(file_path)
            .await
            .map_err(|_| CliError::SecretFileLoadError)?;
        let trimmed = content.trim();

        if hex::decode(trimmed).is_err() {
            let config =
                toml::from_str::<OldConfig>(&content).map_err(|_| CliError::MalformedSecret)?;
            let trimmed = config.secret_key.trim();

            hex::decode(trimmed).map_err(|_| CliError::MalformedSecret)?;

            lock.replace(trimmed.to_string());
            Ok(trimmed.to_string())
        } else {
            lock.replace(trimmed.to_string());
            Ok(trimmed.to_string())
        }
    }

    pub async fn from_args(secret: Option<String>, path: Option<String>) -> Self {
        let mut secret = secret;
        let mut path = path;
        if secret.is_none() && path.is_none() {
            if let Some(secret_env) = option_env!("PLAYIT_SECRET") {
                secret.replace(secret_env.to_string());
            }
        }

        if path.is_none() {
            if let Some(path_env) = option_env!("PLAYIT_SECRET_PATH") {
                path.replace(path_env.to_string());
            }
        }

        PlayitSecret {
            secret: RwLock::new(secret),
            path,
            allow_path_read: true,
        }
    }
}

#[derive(Deserialize, Serialize)]
struct OldConfig {
    secret_key: String,
}
