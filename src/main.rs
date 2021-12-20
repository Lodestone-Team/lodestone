#[macro_use] extern crate rocket;
use rocket::response::content;
use serde::{Serialize, Deserialize};
use serde_json::{Result, Value};
use rocket::tokio::time::{sleep, Duration};

#[get("/versions/<rtype>")]
async fn versions(rtype: String) -> content::Json<String> {
    delay();
    let response: Response = serde_json::from_str(minreq::get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
    .send().unwrap().as_str().unwrap()).unwrap();
    let mut r = Vec::new();
    for version in response.versions {
        if version.r#type == rtype {
            r.push(version.id);
        }
    }
    content::Json(serde_json::to_string(&r).unwrap())
}

#[get("/server/<version>")]
async fn server(version: String) -> content::Json<String> {
    let response: Response = serde_json::from_str(minreq::get("https://launchermeta.mojang.com/mc/game/version_manifest.json")
    .send().unwrap().as_str().unwrap()).unwrap();
    for version_indiv in response.versions {
        if version_indiv.id == version {
           let response : Value = serde_json::from_str(minreq::get(version_indiv.url).send().unwrap().as_str().unwrap()).unwrap();
           return content::Json(response["downloads"]["server"]["url"].to_string());
        }
    }
    content::Json("error".to_string())
    
}

fn delay() {
    let mut n : u128 = 1;
    for i in 1u128..100000000 {
        n = n + 1;
    }
}

#[derive(Deserialize, Serialize)]
#[allow(non_snake_case)]
struct Version {
    id: String,
    r#type: String, // bruh
    url: String,
    time: String,
    releaseTime: String,
}

#[derive(Deserialize, Serialize)]
struct Response {
    versions: Vec<Version>,
}

#[launch]
fn rocket() -> _ {

    // let response = minreq::get("http://launchermeta.mojang.com/mc/game/version_manifest.json").send().unwrap();
    // println!("{}", response.as_str().unwrap());
    rocket::build().mount("/", routes![versions, server])
}