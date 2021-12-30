#[macro_use]
extern crate rocket;
extern crate sanitize_filename;

use chashmap::CHashMap;
use futures_util::lock::Mutex;
use instance::InstanceConfig;
use instance_manager::InstanceManager;
use rocket::http::Status;
use rocket::response::{content, status};
use rocket::serde::json::{json, Json, Value};
use rocket::State;
use serde::{Deserialize, Serialize};
use std::env;
use std::sync::Arc;
mod handlers;
mod instance;
mod instance_manager;
mod util;
use handlers::jar;
use mongodb::{bson::doc, options::ClientOptions, sync::Client};

pub struct MyManagedState {
    instance_manager: Arc<Mutex<InstanceManager>>,
    download_status: CHashMap<String, (u64, u64)>,
    mongodb_client: Client,
}

#[post("/api/instance", data = "<config>")]
async fn setup(config: Json<InstanceConfig>, state: &State<MyManagedState>) -> (Status, String) {
    let mut manager = state.instance_manager.lock().await;
    let config = config.into_inner();
    match manager.create_instance(config, state).await {
        Ok(uuid) => {(Status::Created, uuid)},
        Err(reason) => (Status::InternalServerError, reason)
    }
}

#[get("/api/status/<instance_name>")]
async fn download_status(instance_name: String, state: &State<MyManagedState>) -> String {
    if !state.download_status.contains_key(&instance_name) {
        return "does not exists".to_string();
    }
    return format!(
        "{}/{}",
        state.download_status.get(&instance_name).unwrap().0,
        state.download_status.get(&instance_name).unwrap().1
    );
}

// #[get("/count")]
// async fn test(hit_count: &State<HitCount>) -> String {
//     let current_count = hit_count.count.load(Ordering::Relaxed);
//     hit_count.count.store(current_count + 1, Ordering::Relaxed);
//     format!("Number of visits: {}", current_count)
// }

#[get("/api/start/<uuid>")]
async fn start(state: &State<MyManagedState>, uuid: String) -> String {
    state
        .instance_manager
        .lock()
        .await
        .start_instance(uuid)
        .unwrap();
    "Ok".to_string()
}

#[get("/api/stop/<uuid>")]
async fn stop(state: &State<MyManagedState>, uuid: String) -> String {
    state
        .instance_manager
        .lock()
        .await
        .stop_instance(uuid)
        .unwrap();
    "Ok".to_string()
}

#[get("/api/send/<uuid>/<command>")]
async fn send(uuid: String, command: String, state: &State<MyManagedState>) -> String {
    state
        .instance_manager
        .lock()
        .await
        .send_command(uuid, command)
        .unwrap();
    "Ok".to_string()
}

#[launch]
async fn rocket() -> _ {
    let mut client_options = ClientOptions::parse("mongodb://localhost:27017").unwrap();
    client_options.app_name = Some("MongoDB Client".to_string());

    let client = Client::with_options(client_options).unwrap();

    rocket::build()
        .mount(
            "/",
            routes![
                start,
                stop,
                send,
                setup,
                download_status,
                jar::vanilla_versions,
                jar::vanilla_jar,
                jar::vanilla_options,
                jar::flavours
            ],
        )
        .manage(MyManagedState {
            instance_manager: Arc::new(Mutex::new(InstanceManager::new(
                format!("{}/InstanceTest/", env::current_dir().unwrap().display()),
                client.clone(),
            ))),
            download_status: CHashMap::new(),
            mongodb_client: client,
        })
}
