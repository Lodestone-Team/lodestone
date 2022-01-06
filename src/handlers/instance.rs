use mongodb::{bson::doc};
use rocket::http::Status;
use rocket::response::content;
use rocket::State;
use rocket::serde::json::Json;
use crate::MyManagedState;
use crate::managers::server_instance::InstanceConfig;
use crate::util::db_util::mongo_schema::*;



#[get("/instances")]
pub async fn get_list(state: &State<MyManagedState>) -> content::Json<String> {
    let mut r = Vec::new();
    let mongodb_client = &state.mongodb_client;
    let database_names = mongodb_client
        .list_database_names(None, None)
        .unwrap();
    for database_name in database_names.iter() {
        if database_name.contains("-") { // TODO use db filter instead
            let config = mongodb_client
                .database(&database_name)
                .collection::<InstanceConfig>("config")
                    .find_one(None, None)
                    .unwrap()
                    .unwrap();
            r.push(config);
        }
    }
    content::Json(serde_json::to_string(&r).unwrap())
}

#[post("/instance/<uuid>", data = "<config>")]
pub async fn setup(uuid : String, config: Json<InstanceConfig>, state: &State<MyManagedState>) -> (Status, String) {
    let mut manager = state.instance_manager.lock().await;
    let mut config = config.into_inner();
    config.uuid = Some(uuid);
    match manager.create_instance(config, state).await {
        Ok(uuid) => (Status::Created, uuid),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[delete("/instance/<uuid>")]
pub async fn delete(uuid : String, state: &State<MyManagedState>) -> (Status, String) {
    match state.instance_manager.lock().await.delete_instance(uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::InternalServerError, reason),
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
    match state.instance_manager.lock().await.start_instance(uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[post("/instance/<uuid>/stop")]
pub async fn stop(state: &State<MyManagedState>, uuid: String) -> (Status, String) {
    match state.instance_manager.lock().await.stop_instance(uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[get("/instance/<uuid>/status")]
pub async fn status(state: &State<MyManagedState>, uuid: String) -> (Status, String) {
    match state.instance_manager.lock().await.get_status(uuid) {
        //return status in lowercase
        Ok(status) => (Status::Ok, status.to_lowercase()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}


#[post("/instance/<uuid>/send/<command>")]
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

#[get("/instance/<uuid>/playercount")]
pub async fn player_count(uuid: String, state: &State<MyManagedState>) -> (Status, String) {
    match state
        .instance_manager
        .lock()
        .await
        .player_num(uuid)
    {
        Ok(size) => (Status::Ok, size.to_string()),
        Err(reason) => (Status::InternalServerError, reason),
    }
}

#[get("/instance/<uuid>/playerlist")]
pub async fn player_list(uuid: String, state: &State<MyManagedState>) -> (Status, content::Json<String>) {
    match state
        .instance_manager
        .lock()
        .await
        .player_list(uuid)
    {
        Ok(vec) => (Status::Ok, content::Json(serde_json::to_string(&vec).unwrap())),
        Err(reason) => (Status::InternalServerError, content::Json(reason)),
    }
}

#[get("/instance/<uuid>/log?<start>&<end>")]
pub async fn get_logs(uuid: String, start: String, end: String, state: &State<MyManagedState>) -> (Status, content::Json<String>) {
    let mut r = Vec::new();
    let mongodb_client = &state.mongodb_client;

    let start_int = start.parse::<i64>().unwrap();
    let end_int = end.parse::<i64>().unwrap();

// TODO use db filter instead
    match mongodb_client
        .database(&uuid)
        .collection::<Log>("logs")
        .find( doc! {
            "$and": [ 
                {
                    "time": {
                    
                        "$gte": start_int
                    }
                },
                {
                    "time": {
                        "$lte": end_int
                    }
                }

            ] 
        }, None)
        {
            Err(err) => {
                return (Status::InternalServerError, content::Json(err.to_string()))
            },
            Ok(logs) => {
                for log in logs {
                    r.push(log.unwrap());
                }
            },
}

    (Status::Ok, content::Json(serde_json::to_string(&r).unwrap()))
}