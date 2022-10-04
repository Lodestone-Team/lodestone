use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::traits::{Error, ErrorInner};


#[derive(Serialize, Deserialize, Debug)]
pub struct MinecraftVersions {
    old_alpha: HashSet<String>,
    snapshot: HashSet<String>,
    release: HashSet<String>,
}

pub async fn get_vanilla_versions() -> Result<Vec<String>, Error> {
    let http = reqwest::Client::new();
    let api_changed_error = Error {
        inner: ErrorInner::APIChanged,
        detail: "Mojang API changed. Please report this bug".to_string(),
    };
    let response: Value = serde_json::from_str(
        http.get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
            .send()
            .await
            .map_err(|_| Error {
                inner: ErrorInner::FailedToUpload,
                detail: "".to_string(),
            })?
            .text()
            .await
            .map_err(|_| api_changed_error)?
            .as_str(),
    )
    .map_err(|_| Error {
        inner: ErrorInner::APIChanged,
        detail: "Mojang API changed. Please report this bug".to_string(),
    })?;

    let versions = response["versions"].as_array().ok_or(Error {
        inner: ErrorInner::APIChanged,
        detail: "Mojang API changed. Please report this bug".to_string(),
    })?;

    Ok(versions
        .iter()
        .map(|v| v["id"].as_str().unwrap().to_string())
        .collect())
}

pub async fn get_fabric_versions() -> Result<Vec<String>, Error> {
    let http = reqwest::Client::new();
    let api_changed_error = Error {
        inner: ErrorInner::APIChanged,
        detail: "Fabric API changed. Please report this bug".to_string(),
    };
    let reponse: Value = serde_json::from_str(
        http.get("https://meta.fabricmc.net/v2/versions")
            .send()
            .await
            .map_err(|_| Error {
                inner: ErrorInner::FailedToUpload,
                detail: "".to_string(),
            })?
            .text()
            .await
            .map_err(|_| api_changed_error.clone())?
            .as_str(),
    )
    .map_err(|_| Error {
        inner: ErrorInner::APIChanged,
        detail: "Fabric API changed. Please report this bug".to_string(),
    })?;

    Ok(reponse["game"]
        .as_array()
        .ok_or(api_changed_error)?
        .iter()
        .map(|v| v["version"].as_str().unwrap().to_string())
        .collect())
}
