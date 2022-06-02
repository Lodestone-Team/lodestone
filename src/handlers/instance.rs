use std::fs;

use crate::managers::server_instance::InstanceConfig;
use crate::managers::types::ResourceType;
use crate::services::file_service;
use crate::MyManagedState;
use rocket::data::{Capped, Data, ToByteUnit};
use rocket::form::{Form, FromForm};
use rocket::fs::{TempFile, NamedFile};
use rocket::http::{ContentType, Status};
use rocket::response::content;
use rocket::serde::json::{json, Json, Value};
use rocket::tokio::fs::File;
use rocket::tokio::time::Duration;
use rocket::State;
#[get("/instances")]
pub async fn get_list(state: &State<MyManagedState>) -> Value {
    json!(&state.instance_manager.lock().await.list_instances())
}

#[post("/instance/<uuid>", data = "<config>")]
pub async fn setup(
    uuid: String,
    config: Json<InstanceConfig>,
    state: &State<MyManagedState>,
) -> (Status, String) {
    let mut manager = state.instance_manager.lock().await;
    let mut config = config.into_inner();
    config.uuid = Some(uuid);
    match manager.create_instance(config, state).await {
        Ok(uuid) => (Status::Created, uuid),
        Err(reason) => (Status::BadRequest, reason),
    }
}

#[delete("/instance/<uuid>")]
pub async fn delete(uuid: String, state: &State<MyManagedState>) -> (Status, String) {
    match state.instance_manager.lock().await.delete_instance(&uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::BadRequest, reason),
    }
}

#[get("/instance/<uuid>/download-progress")]
pub async fn download_status(uuid: String, state: &State<MyManagedState>) -> (Status, String) {
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

#[post("/instance/<uuid>/start")]
pub async fn start(state: &State<MyManagedState>, uuid: String) -> (Status, String) {
    state.instance_manager.lock().await;
    match state.instance_manager.lock().await.start_instance(&uuid) {
        Ok(()) => {
            return (Status::Ok, "Ok".to_string());
        }
        Err(reason) => {
            return (Status::BadRequest, reason);
        }
    }
}

#[post("/instance/<uuid>/stop")]
pub async fn stop(state: &State<MyManagedState>, uuid: String) -> (Status, String) {
    match state.instance_manager.lock().await.stop_instance(&uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::BadRequest, reason),
    }
}

#[get("/instance/<uuid>/status")]
pub async fn status(state: &State<MyManagedState>, uuid: String) -> (Status, String) {
    match state.instance_manager.lock().await.get_status(&uuid) {
        //return status in lowercase
        Ok(status) => (Status::Ok, status.to_lowercase()),
        Err(reason) => (Status::BadRequest, reason),
    }
}

#[post("/instance/<uuid>/send/<command>")]
pub async fn send(
    uuid: String,
    command: String,
    state: &State<MyManagedState>,
) -> (Status, String) {
    match state
        .instance_manager
        .lock()
        .await
        .send_command(&uuid, command)
    {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::BadRequest, reason),
    }
}

#[get("/instance/<uuid>/playercount")]
pub async fn player_count(uuid: String, state: &State<MyManagedState>) -> (Status, String) {
    match state.instance_manager.lock().await.player_num(&uuid) {
        Ok(size) => (Status::Ok, size.to_string()),
        Err(reason) => (Status::BadRequest, reason),
    }
}

#[get("/instance/<uuid>/playerlist")]
pub async fn player_list(uuid: String, state: &State<MyManagedState>) -> (Status, Value) {
    match state.instance_manager.lock().await.player_list(&uuid) {
        Ok(vec) => (Status::Ok, json!(vec)),
        Err(reason) => (Status::BadRequest, json!(reason)),
    }
}

#[get("/instance/<uuid>/resources/<resource_type>/list")]
pub async fn list_resource(
    uuid: String,
    resource_type: ResourceType,
    state: &State<MyManagedState>,
) -> (Status, content::Json<String>) {
    match state
        .instance_manager
        .lock()
        .await
        .list_resource(&uuid, resource_type)
    {
        Ok(list) => (
            Status::Ok,
            content::Json(
                json!({
                    "loaded" : list.0,
                    "unloaded" : list.1,
                })
                .to_string(),
            ),
        ),
        Err(reason) => {
            return (Status::BadRequest, content::Json(reason));
        }
    }
}

#[get("/instance/<uuid>/resources/<resource_type>/load/<resource_name>")]
pub async fn load_resource(
    uuid: String,
    resource_type: ResourceType,
    resource_name: String,
    state: &State<MyManagedState>,
) -> (Status, content::Json<String>) {
    match state
        .instance_manager
        .lock()
        .await
        .load(&uuid, resource_type, &resource_name)
    {
        Ok(_) => (Status::Ok, content::Json("Ok".to_string())),
        Err(reason) => {
            return (Status::BadRequest, content::Json(reason));
        }
    }
}

#[get("/instance/<uuid>/resources/<resource_type>/unload/<resource_name>")]
pub async fn unload_resource(
    uuid: String,
    resource_type: ResourceType,
    resource_name: String,
    state: &State<MyManagedState>,
) -> (Status, content::Json<String>) {
    match state
        .instance_manager
        .lock()
        .await
        .unload(&uuid, resource_type, &resource_name)
    {
        Ok(_) => (Status::Ok, content::Json("Ok".to_string())),
        Err(reason) => {
            return (Status::BadRequest, content::Json(reason));
        }
    }
}

#[derive(FromForm)]
pub struct Upload<'r> {
    // TODO figure our how to check if valid jar file
    resource_type: ResourceType,
    file: Capped<TempFile<'r>>,
}

#[post("/instance/<uuid>/files/upload", data = "<upload>")]
pub async fn upload(
    uuid: String,
    upload: Form<Upload<'_>>,
    state: &State<MyManagedState>,
) -> (Status, String) {
    let upload_inner = upload.into_inner();
    if !upload_inner.file.is_complete() {
        return (Status::PayloadTooLarge, "File too large".to_string());
    }
    let is_correct_type = match upload_inner.resource_type {
        ResourceType::Mod => upload_inner.file.content_type().unwrap().sub() == "java-archive",
        ResourceType::World => upload_inner.file.content_type().unwrap().is_zip(),
    };
    if !is_correct_type {
        return (
            Status::UnsupportedMediaType,
            "Non-matching resource type".to_string(),
        );
    }
    match state
        .instance_manager
        .lock()
        .await
        .upload(
            &uuid,
            upload_inner.file.into_inner(),
            upload_inner.resource_type,
        )
        .await
    {
        Ok(_) => (Status::Ok, "Saved".to_string()),
        Err(_) => (Status::InternalServerError, "Failed to save".to_string()),
    }
}

#[get("/instance/<uuid>/files/download/mod/<name>")]
pub async fn download_mod(
    uuid: String,
    name: String,
    state: &State<MyManagedState>,
) -> (Status, Result<NamedFile, std::io::Error>) {
    let file_result = state.instance_manager.lock().await.get_mod(&uuid, &name).await;
    match file_result {
        Ok(_) => (Status::Ok, Ok(file_result.unwrap())),
        Err(_) => (Status::NotFound, file_result),
    }
}

#[get("/instance/<uuid>/files/download/world/<name>")]
pub async fn download_world(
    uuid: String,
    name: String,
    state: &State<MyManagedState>,
) -> (Status, Result<NamedFile, std::io::Error>) {
    let file_result = state.instance_manager.lock().await.get_world(&uuid, &name).await;
    match file_result {
        Ok(_) => (Status::Ok, Ok(file_result.unwrap())),
        Err(_) => (Status::NotFound, file_result),
    }
}
