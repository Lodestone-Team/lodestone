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
mod properties_manager;
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
        Ok(uuid) => (Status::Created, uuid),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[get("/api/instance/<uuid>/download-progress")]
async fn download_status(uuid: String, state: &State<MyManagedState>) -> (Status, String) {
    if !state.download_status.contains_key(&uuid) {
        return (Status::NotFound, "does not exists".to_string());
    }

    (
        Status::Ok,
        format!(
            "{}/{}",
            state.download_status.get(&uuid).unwrap().0,
            state.download_status.get(&uuid).unwrap().1
        ),
    )
}

#[get("/api/instance/<uuid>/start")]
async fn start(state: &State<MyManagedState>, uuid: String) -> (Status, String) {
    match state.instance_manager.lock().await.start_instance(uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[get("/api/instance/<uuid>/stop")]
async fn stop(state: &State<MyManagedState>, uuid: String) -> (Status, String) {
    match state.instance_manager.lock().await.stop_instance(uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[get("/api/instance/<uuid>/send/<command>")]
async fn send(uuid: String, command: String, state: &State<MyManagedState>) -> (Status, String) {
    match state
        .instance_manager
        .lock()
        .await
        .send_command(uuid, command)
    {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::InternalServerError, reason),
    }
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
            ).unwrap())),
            download_status: CHashMap::new(),
            mongodb_client: client,
        })
}
