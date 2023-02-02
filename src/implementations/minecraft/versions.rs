use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use crate::traits::{Error, ErrorInner};

#[derive(Serialize, Deserialize, Debug, TS)]
#[ts(export)]
pub struct MinecraftVersions {
    old_alpha: Vec<String>,
    snapshot: Vec<String>,
    release: Vec<String>,
}

pub async fn get_vanilla_versions() -> Result<MinecraftVersions, Error> {
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
            .map_err(|_| api_changed_error.clone())?
            .as_str(),
    )
    .map_err(|_| api_changed_error.clone())?;

    let versions = response["versions"]
        .as_array()
        .ok_or_else(|| api_changed_error.clone())?;

    #[derive(Serialize, Deserialize, Debug)]
    struct Version {
        id: String,
        r#type: String,
    }

    let mut ret = MinecraftVersions {
        old_alpha: Vec::new(),
        snapshot: Vec::new(),
        release: Vec::new(),
    };

    for version in versions.iter() {
        let version: Version =
            serde_json::from_value(version.to_owned()).map_err(|_| api_changed_error.clone())?;
        match version.r#type.as_str() {
            "old_alpha" => ret.old_alpha.push(version.id),
            "snapshot" => ret.snapshot.push(version.id),
            "release" => ret.release.push(version.id),
            _ => {}
        }
    }
    Ok(ret)
}

pub async fn get_fabric_versions() -> Result<MinecraftVersions, Error> {
    let http = reqwest::Client::new();
    let api_changed_error = Error {
        inner: ErrorInner::APIChanged,
        detail: "Fabric API changed. Please report this bug".to_string(),
    };
    let response: Value = serde_json::from_str(
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
    .map_err(|_| api_changed_error.clone())?;

    let vanilla_versions = get_vanilla_versions().await?;
    let mut ret = MinecraftVersions {
        release: Vec::new(),
        snapshot: Vec::new(),
        old_alpha: Vec::new(),
    };
    for item in response["game"]
        .as_array()
        .ok_or_else(|| api_changed_error.clone())?
    {
        let version_str = item["version"]
            .as_str()
            .ok_or_else(|| api_changed_error.clone())?;
        if vanilla_versions.release.contains(&version_str.to_string()) {
            ret.release.push(version_str.to_string());
        }
        if vanilla_versions.snapshot.contains(&version_str.to_string()) {
            ret.snapshot.push(version_str.to_string());
        }
        if vanilla_versions
            .old_alpha
            .contains(&version_str.to_string())
        {
            ret.old_alpha.push(version_str.to_string());
        }
    }

    Ok(ret)
}

pub async fn get_paper_versions() -> Result<MinecraftVersions, Error> {
    let http = reqwest::Client::new();
    let api_changed_error = Error {
        inner: ErrorInner::APIChanged,
        detail: "Paper API changed. Please report this bug".to_string(),
    };
    let response: Value = serde_json::from_str(
        http.get("https://api.papermc.io/v2/projects/paper")
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
    .map_err(|_| api_changed_error.clone())?;

    let vanilla_versions = get_vanilla_versions().await?;
    let mut ret = MinecraftVersions {
        release: Vec::new(),
        snapshot: Vec::new(),
        old_alpha: Vec::new(),
    };
    for item in response["versions"]
        .as_array()
        .ok_or_else(|| api_changed_error.clone())?
    {
        let version_str = item
            .as_str()
            .ok_or_else(|| api_changed_error.clone())?;
        if vanilla_versions.release.contains(&version_str.to_string()) {
            ret.release.push(version_str.to_string());
        }
        if vanilla_versions.snapshot.contains(&version_str.to_string()) {
            ret.snapshot.push(version_str.to_string());
        }
        if vanilla_versions
            .old_alpha
            .contains(&version_str.to_string())
        {
            ret.old_alpha.push(version_str.to_string());
        }
    }

    Ok(ret)
}
