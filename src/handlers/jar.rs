use rocket::response::{content, status};
use serde_json::{json, Value};

#[get("/api/jar/flavours")]
pub async fn flavours() -> content::Json<String> {
    content::Json(json!(["vanilla", "fabric"]).to_string())
    //Hard coded json bad
}


mod VanillaStructs{
    use serde::{Deserialize, Serialize};
    #[derive(Deserialize, Serialize)]
    pub struct ServerVersion {
        pub id: String,
        pub r#type: String,
        pub url: String,
        pub time: String,
        pub releaseTime: String,
    }
    #[derive(Deserialize, Serialize)]
    pub struct LatestVersion {
        pub release: String,
        pub snapshot: String,
    }

    #[derive(Deserialize, Serialize)]
    pub struct VersionManifest {
        pub versions: Vec<ServerVersion>,
        pub latest: LatestVersion,
    }
}

mod FabricStructs{
    use serde::{Deserialize, Serialize};
    #[derive(Deserialize, Serialize)]
    pub struct ServerVersion {
        pub version: String,
        pub stable: bool,
    }


    #[derive(Deserialize, Serialize)]
    pub struct LoaderInstance{
        pub version: String,
    }

    #[derive(Deserialize, Serialize)]
    pub struct LoaderVersion {
        pub loader: LoaderInstance,
    }

    #[derive(Deserialize, Serialize)]
    pub struct InstallerVersion {
        pub url: String,
        pub maven: String,
        pub version: String,
        pub stable: bool,
    }
}

#[get("/api/jar/vanilla/filters")]
pub async fn vanilla_filters() -> content::Json<String> {
    content::Json(
        json!({
            "type": ["release", "snapshot", "old_alpha", "old_beta"],
            "latest": [true, false]
        })
        .to_string(),
    )
    //Hard coded json bad
}

#[get("/api/jar/vanilla/versions?<type>&<latest>")]
pub async fn vanilla_versions(
    r#type: Option<String>,
    latest: Option<bool>,
) -> content::Json<String> {
    let response: VanillaStructs::VersionManifest = serde_json::from_str(
        minreq::get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let mut r = Vec::new();
    let latest = match latest {
        Some(latest) => latest,
        None => false,
    };
    let r#type = match r#type {
        Some(r#type) => r#type,
        None => "".to_string(),
    };

    if latest {
        if r#type == "release" || r#type.is_empty() {
            r.push(response.latest.release);
        }
        if r#type == "snapshot" || r#type.is_empty() {
            r.push(response.latest.snapshot);
        }
    } else {
        for version in response.versions {
            if version.r#type == r#type || r#type.is_empty() {
                r.push(version.id);
            }
        }
    }

    //removes duplicate
    r.dedup_by(|a, b| a == b);
    content::Json(serde_json::to_string(&r).unwrap())
}

#[get("/api/jar/vanilla/<requested_version>")]
pub fn vanilla_jar(
    requested_version: String,
) -> Result<content::Json<String>, status::NotFound<String>> {
    let response: VanillaStructs::VersionManifest = serde_json::from_str(
        minreq::get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for version in response.versions {
        if version.id == requested_version {
            let response: Value = serde_json::from_str(
                minreq::get(version.url).send().unwrap().as_str().unwrap(),
            )
            .unwrap();
            return Ok(content::Json(
                response["downloads"]["server"]["url"]
                    .to_string()
                    .replace("\"", ""),
            ));
        }
    }

    Err(status::NotFound("Jar not found".to_string()))
}

#[get("/api/jar/fabric/filters")]
pub async fn fabric_filters() -> content::Json<String> {
    content::Json(
        json!({
            "stable": [true, false]
        })
        .to_string(),
    )
    //Hard coded json bad
}

#[get("/api/jar/fabric/versions?<stable>")]
pub async fn fabric_versions(stable: Option<bool>) -> content::Json<String> {
    let response: Vec<FabricStructs::ServerVersion> = serde_json::from_str(
        minreq::get("https://meta.fabricmc.net/v2/versions/game")
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let mut r = Vec::new();
    let stable = match stable {
        Some(stable) => stable,
        None => false,
    };

    for version in response {
        if version.stable || !stable {
            r.push(version.version);
        }
    }

    content::Json(serde_json::to_string(&r).unwrap())
}

#[get("/api/jar/fabric/<requested_version>")]
pub fn fabric_jar(
    requested_version: String,
) -> Result<content::Json<String>, status::NotFound<String>> {
    let response: Vec<FabricStructs::LoaderVersion> = serde_json::from_str(
        minreq::get(format!("https://meta.fabricmc.net/v2/versions/loader/{}", requested_version))
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let loader_version = &response[0].loader.version;

    let response: Vec<FabricStructs::InstallerVersion> = serde_json::from_str(
        minreq::get("https://meta.fabricmc.net/v2/versions/installer")
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let installer_version = &response[0].version;

    return Ok(content::Json(
        format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
            requested_version, loader_version, installer_version
        )
    ));

    // Err(status::NotFound("Jar not found".to_string()))
}