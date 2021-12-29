use super::json_struct::response_from_mojang::VersionManifest;
use rocket::response::content;
use rocket::response::status;
use serde_json::Value;
use std::error::Error;

#[get("/api/jar/flavours")]
pub async fn flavours() -> content::Json<String> {
    content::Json("[\"vanilla\"]".to_string())
    //Hard coded json bad
}

#[get("/api/jar/vanilla/options")]
pub async fn vanilla_options() -> content::Json<String> {
    content::Json(
        "{
            \"type\": [\"release\", \"snapshot\", \"old_alpha\", \"old_beta\"],
            \"latest\": [true, false]
    }"
        .to_string(),
    )
    //Hard coded json bad
}

#[get("/api/jar/vanilla/versions?<type>&<latest>")]
pub async fn vanilla_versions(
    r#type: Option<String>,
    latest: Option<bool>,
) -> content::Json<String> {
    let response: VersionManifest = serde_json::from_str(
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
        if (r#type == "release" || r#type.is_empty()) {
            r.push(response.latest.release);
        }
        if (r#type == "snapshot" || r#type.is_empty()) {
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
    let response: VersionManifest = serde_json::from_str(
        minreq::get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for version in response.versions {
        if version.id == requested_version {
            let response: Value =
                serde_json::from_str(minreq::get(version.url).send().unwrap().as_str().unwrap())
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
