use rocket::response::content;
use serde_json::{Value};
use super::json_struct::response_from_mojang::{VersionManifest};

#[get("/versions/<rtype>")]
pub async fn versions(rtype: String) -> content::Json<String> {
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
        if version.r#type == rtype {
            r.push(version.id);
        }
    }
    content::Json(serde_json::to_string(&r).unwrap())
}

pub fn get_vanilla_url(version: String) -> Option<String> {
    let response: VersionManifest = serde_json::from_str(
        minreq::get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
            .send()
            .unwrap()
            .as_str()
            .unwrap(),
    )
    .unwrap();
    for version_indiv in response.versions {
        if version_indiv.id == version {
            let response: Value = serde_json::from_str(
                minreq::get(version_indiv.url)
                    .send()
                    .unwrap()
                    .as_str()
                    .unwrap(),
            )
            .unwrap();
            return Some(
                response["downloads"]["server"]["url"]
                    .to_string()
                    .replace("\"", ""),
            );
        }
    }
    None
}
