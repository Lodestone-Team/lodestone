use super::json_struct::response_from_mojang::VersionManifest;
use rocket::response::content;
use serde_json::Value;
use std::error::Error;

#[get("/api/jar/vanilla/<rtype>")]
pub async fn get_vanilla_versions(rtype: String) -> content::Json<String> {
    let response: VersionManifest = serde_json::from_str(
        minreq::get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();

    let mut r = Vec::new();
    for version in response.versions {
        if version.r#type == rtype || rtype.is_empty() {
            r.push(version.id);
        }
    }
    content::Json(serde_json::to_string(&r).unwrap())
}

#[get("/api/jar/vanilla/url/<requested_version>")]
pub fn get_vanilla_jar(requested_version: String) -> content::Json<String> {
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
            return content::Json(
                response["downloads"]["server"]["url"]
                    .to_string()
                    .replace("\"", ""),
            );
        }
    }

    content::Json("jar not found".to_string())
}
