use rocket::http::Status;
use rocket::State;
use rocket::serde::json::Json;
use crate::MyManagedState;
use crate::managers::server_instance::InstanceConfig;


#[post("/api/instance", data = "<config>")]
pub async fn setup(config: Json<InstanceConfig>, state: &State<MyManagedState>) -> (Status, String) {
    let mut manager = state.instance_manager.lock().await;
    let config = config.into_inner();
    match manager.create_instance(config, state).await {
        Ok(uuid) => (Status::Created, uuid),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[delete("/api/instance/<uuid>")]
pub async fn delete(uuid : String, state: &State<MyManagedState>) -> (Status, String) {
    match state.instance_manager.lock().await.delete_instance(uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[get("/api/instance/<uuid>/download-progress")]
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

#[get("/api/instance/<uuid>/start")]
pub async fn start(state: &State<MyManagedState>, uuid: String) -> (Status, String) {
    match state.instance_manager.lock().await.start_instance(uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[get("/api/instance/<uuid>/stop")]
pub async fn stop(state: &State<MyManagedState>, uuid: String) -> (Status, String) {
    match state.instance_manager.lock().await.stop_instance(uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[get("/api/instance/<uuid>/status")]
pub async fn status(state: &State<MyManagedState>, uuid: String) -> (Status, String) {
    match state.instance_manager.lock().await.get_status(uuid) {
        //return status in lowercase
        Ok(status) => (Status::Ok, status.to_lowercase()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}


#[get("/api/instance/<uuid>/send/<command>")]
pub async fn send(uuid: String, command: String, state: &State<MyManagedState>) -> (Status, String) {
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