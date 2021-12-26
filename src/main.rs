

#[macro_use] extern crate rocket;

use std::io::BufRead;
use std::sync::{Mutex, Arc};
use std::time::Duration;

use instance::ServerInstance;
use rocket::{response::content, request::FromRequest, request::Request};
use rocket::{State, request};
use serde::{Serialize, Deserialize};
use serde_json::{Result, Value};
use std::sync::atomic::{AtomicUsize, Ordering};
mod instance;


struct HitCount {
    count: AtomicUsize
}

#[get("/versions/<rtype>")]
async fn versions(rtype: String) -> content::Json<String> {
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

#[get("/count")]
async fn test(hit_count: &State<HitCount>) -> String {
    let current_count = hit_count.count.load(Ordering::Relaxed);
    hit_count.count.store(current_count + 1, Ordering::Relaxed);
    format!("Number of visits: {}", current_count)
}

#[get("/start")]
fn start() {
    let mut server_test = ServerInstance::new(None);
    server_test.start().unwrap();
    server_test.stdout.unwrap().lines()
    .filter_map(|line| line.ok())
    .for_each(|line| println!("process thread returned {}", line));

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
    rocket::build().mount("/", routes![versions, server, test, start]).manage(HitCount { count: AtomicUsize::new(0) })

}