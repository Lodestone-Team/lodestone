

#[macro_use] extern crate rocket;

use std::sync::{Mutex, Arc};
use instance::ServerInstance;
use rocket::{response::content};
use rocket::{State};
use serde_json::{Value};
use std::sync::atomic::{AtomicUsize};
use std::path::Path;
use chashmap::CHashMap;
mod instance;
mod util;
mod handlers;
mod instance_manager;
use handlers::jar;
use mongodb::{bson::doc, options::ClientOptions, sync::Client};

struct HitCount {
    count: AtomicUsize
}

pub struct MyManagedState {
    server : Arc<Mutex<ServerInstance>>,
    download_status: CHashMap<String, (u64, u64)>,
    mongoDBClient: Client
}

#[get("/setup/<instance_name>/<url>")]
async fn setup(instance_name : String, url : String, state: &State<MyManagedState>) -> String {
    let path = format!("/home/peter/Lodestone/backend/InstanceTest/{}", instance_name); // TODO: Add a global path string
    if Path::new(path.as_str()).exists() {
        return "instance already exists".to_string()
    }

    std::fs::create_dir(path.as_str()).unwrap();
    println!("{}",url);
    util::download_file(url.as_str(), format!("{}/server.jar", path).as_str(), state, instance_name.as_str()).await.unwrap();

    format!("downloaded to {}", path)    
}

#[get("/status/<instance_name>")]
async fn download_status(instance_name : String, state: &State<MyManagedState>) -> String {
    if !state.download_status.contains_key(&instance_name) {
        return "does not exists".to_string();
    }
    return format!("{}/{}", state.download_status.get(&instance_name).unwrap().0, state.download_status.get(&instance_name).unwrap().1)
}


// #[get("/count")]
// async fn test(hit_count: &State<HitCount>) -> String {
//     let current_count = hit_count.count.load(Ordering::Relaxed);
//     hit_count.count.store(current_count + 1, Ordering::Relaxed);
//     format!("Number of visits: {}", current_count)
// }

#[get("/start")]
async fn start(state: &State<MyManagedState>) -> String {
    let server = state.server.clone();
    if server.lock().unwrap().is_running() {
       return "already running".to_string();
    }
    let mut instance = server.lock().unwrap();

    let client_ref = state.mongoDBClient.clone();

    instance.start(client_ref).unwrap();
    "server starting".to_string()
    // let server_test_mutex = ServerInstance::new(None);
    // let mut server = server_test_mutex.lock().unwrap();
    // server.start().unwrap();
    // server.stdout.as_ref().unwrap().lock().unwrap();
    // for rec in  {
    //     println!("Server said: {}", rec);
    // }
}

#[get("/stop")]
fn stop(state: &State<MyManagedState>) -> String {
    let server = state.server.clone();
    if !server.lock().unwrap().is_running() {
        return "already stopped".to_string();
    }
    let mut instance = server.lock().unwrap();
    instance.stop().unwrap();
    "server stopped".to_string()
    
}

#[get("/send/<command>")]
fn send(command: String, state: &State<MyManagedState>) -> String {
    let server = state.server.clone();
    if !server.lock().unwrap().is_running() {
        return "sever not started".to_string();
    }
    let instance = server.lock().unwrap();
    instance.stdin.clone().unwrap().send(format!("{}\n", command.clone())).unwrap();
    format!("sent command: {}", command)
}

#[launch]
async fn rocket() -> _ {

    let mut client_options = ClientOptions::parse("mongodb://localhost:27017").unwrap();
    client_options.app_name = Some("MongoDB Client".to_string());

    let client = Client::with_options(client_options).unwrap();

    rocket::build()
    .mount("/", routes![start, stop, send, setup, download_status, jar::vanilla_versions, jar::vanilla_jar, jar::vanilla_options , jar::flavours])
    .manage(MyManagedState{
        server : Arc::new(Mutex::new(ServerInstance::new(None, "/home/peter/Lodestone/backend/InstanceTest".to_string(), "test".to_string()))),
        download_status: CHashMap::new(),
        mongoDBClient: client
    })
}