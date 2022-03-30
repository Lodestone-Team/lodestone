use rocket::http::Status;
use rocket::serde::json::{json, Json, Value, from_str};

#[get("/jar/flavours")]
pub async fn flavours() -> Value {
    json!(["vanilla", "fabric", "paper"])
}

// Beginning of vanilla

mod vanilla_structs{
    use serde::{Deserialize, Serialize};
    #[derive(Deserialize, Serialize)]
    #[allow(non_snake_case)]
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

#[get("/jar/vanilla/filters")]
pub async fn vanilla_filters() -> Value {
    json!({
        "type": ["release", "snapshot", "old_alpha", "old_beta"],
        "latest": [true, false]
    })
}

#[get("/jar/vanilla/versions?<type>&<latest>")]
pub async fn vanilla_versions(
    r#type: Option<String>,
    latest: Option<bool>,
) -> Result<Value, (Status, Value)> {
    let response: vanilla_structs::VersionManifest = from_str(
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
    Ok(json!(r))
}

#[get("/jar/vanilla/<requested_version>")]
pub async fn vanilla_jar(
    requested_version: String,
) -> Result<Value, (Status, Value)> {
    let response: vanilla_structs::VersionManifest = from_str(
        minreq::get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for version in response.versions {
        if version.id == requested_version {
            let response: Value = from_str(
                minreq::get(version.url).send().unwrap().as_str().unwrap(),
            )
            .unwrap();
            if response["downloads"]["server"]["url"] == Value::Null{
                return Err((Status::NotFound, json!({"error": "No server jar found for this version"})));
            }
            return Ok(json!({"url": response["downloads"]["server"]["url"].to_string().replace("\"", "")}));
        }
    }

    Err((Status::NotFound, json!({"error": "Version not found"})))
}

// End of vanilla

// Beginning of fabric
mod fabric_structs{
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

#[get("/jar/fabric/filters")]
pub async fn fabric_filters() -> Value {
    json!({
        "stable": [true, false]
    })
    //Hard coded json bad
}

#[get("/jar/fabric/versions?<stable>")]
pub async fn fabric_versions(stable: Option<bool>) -> Result<Value, (Status, Value)> {
    let response: Vec<fabric_structs::ServerVersion> = from_str(
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
    
    Ok(json!(r))
}

#[get("/jar/fabric/<requested_version>")]
pub async fn fabric_jar(
    requested_version: String,
) -> Result<Value, (Status, Value)> {
    let response: Vec<fabric_structs::LoaderVersion> = from_str(
        minreq::get(format!("https://meta.fabricmc.net/v2/versions/loader/{}", requested_version))
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let loader_version = &response[0].loader.version;

    let response: Vec<fabric_structs::InstallerVersion> = from_str(
        minreq::get("https://meta.fabricmc.net/v2/versions/installer")
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let installer_version = &response[0].version;

    Ok(json!({
        "loader": loader_version,
        "installer": installer_version,
        "url": format!(
            "https://meta.fabricmc.net/v2/versions/loader/{}/{}/{}/server/jar",
            requested_version, loader_version, installer_version
        )
    }))
}

// End of fabric

// Beginning of paper
mod paper_structs{
    use serde::{Deserialize, Serialize};
    #[derive(Deserialize, Serialize)]
    pub struct ProjectInfo {
        // pub project_id: String,
        // pub project_name: String,
        // pub version_groups: Vec<String>,
        pub versions: Vec<String>,
    }

    #[derive(Deserialize, Serialize)]
    pub struct VersionInfo {
        // pub project_id: String,
        // pub project_name: String,
        // pub version: String,
        pub builds: Vec<i32>,
    }
}

#[get("/jar/paper/filters")]
pub async fn paper_filters() -> Value {
    json!({})
}
#[get("/jar/paper/versions")]
pub async fn paper_versions() -> Result<Value, (Status, Value)> {
    // fetch from https://papermc.io/api/v2/projects/paper/
    let response: paper_structs::ProjectInfo = from_str(
        minreq::get("https://papermc.io/api/v2/projects/paper/")
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    ).unwrap();


    //return inverted versions list cuz paper sorts them backwards
    Ok(json!(response.versions.iter().rev().collect::<Vec<&String>>()))
}

#[get("/jar/paper/<requested_version>")]
pub async fn paper_jar(
    requested_version: String,
) -> Result<Value, (Status, Value)> {
    // fetch from https://papermc.io/api/v2/projects/paper/versions/<version>
    let response: paper_structs::VersionInfo = from_str(
        minreq::get(format!("https://papermc.io/api/v2/projects/paper/versions/{}", requested_version))
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    ).unwrap();

    if response.builds.len() == 0 {
        return Err((Status::NotFound, json!({"error": "Version not found"})));
    }

    //get the largest build number from response.builds
    let largest_build = match response.builds.iter().max() {
        Some(build) => build,
        None => return Err((Status::InternalServerError, json!({"error": "Failed to get largest build number"}))),
    };

    // example: https://papermc.io/api/v2/projects/paper/versions/1.17.1/builds/409/downloads/paper-1.17.1-409.jar
    Ok(json!({
        "url": format!(
            "https://papermc.io/api/v2/projects/paper/versions/{}/builds/{}/downloads/paper-{}-{}.jar",
            requested_version, largest_build, requested_version, largest_build
        )
    }))
}

// End of paper
