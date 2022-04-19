use mongodb::{bson::doc};
use rocket::http::Status;
use rocket::response::content;
use rocket::{State};
use rocket::serde::json::{json, Json, Value};
use crate::MyManagedState;
use crate::managers::server_instance::InstanceConfig;
use crate::util::db_util::mongo_schema::*;
use crate::instance_manager::resource_management::ResourceType;


#[get("/instances")]
pub async fn get_list(state: &State<MyManagedState>) -> Value {
    json!(&state.instance_manager.lock().await.list_instances())
}

#[post("/instance/<uuid>", data = "<config>")]
pub async fn setup(uuid : String, config: Json<InstanceConfig>, state: &State<MyManagedState>) -> (Status, String) {
    let mut manager = state.instance_manager.lock().await;
    let mut config = config.into_inner();
    config.uuid = Some(uuid);
    match manager.create_instance(config, state).await {
        Ok(uuid) => (Status::Created, uuid),
        Err(reason) => (Status::BadRequest, reason),
    }
}

#[delete("/instance/<uuid>")]
pub async fn delete(uuid : String, state: &State<MyManagedState>) -> (Status, String) {
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
    match state.instance_manager.lock().await.start_instance(&uuid) {
        Ok(()) => (Status::Ok, "Ok".to_string()),
        Err(reason) => (Status::BadRequest, reason),
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
pub async fn send(uuid: String, command: String, state: &State<MyManagedState>) -> (Status, String) {
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
    match state
        .instance_manager
        .lock()
        .await
        .player_num(&uuid)
    {
        Ok(size) => (Status::Ok, size.to_string()),
        Err(reason) => (Status::BadRequest, reason),
    }
}

#[get("/instance/<uuid>/playerlist")]
pub async fn player_list(uuid: String, state: &State<MyManagedState>) -> (Status, Value) {
    match state
        .instance_manager
        .lock()
        .await
        .player_list(&uuid)
    {
        Ok(vec) => (Status::Ok, json!(vec)),
        Err(reason) => (Status::BadRequest, json!(reason)),
    }
}

#[get("/instance/<uuid>/log?<start>&<end>")]
pub async fn get_logs(uuid: String, start: String, end: String, state: &State<MyManagedState>) -> (Status, Value) {
    let mut result = Vec::new();
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
                return (Status::BadRequest, json!(err.to_string()));
            },
            Ok(logs) => {
                for log in logs {
                    result.push(log.unwrap());
                }
            },
}

    (Status::Ok, json!(result))
}

#[get("/instance/<uuid>/resources/<resource_type>/list")]
pub async fn list_resource(uuid: String, resource_type: ResourceType, state: &State<MyManagedState>) -> (Status, content::Json<String>) {
    match state.instance_manager.lock().await.list_resource(&uuid, resource_type) {
        Ok(list) => {
            (Status::Ok, content::Json(json!({
                "loaded" : list.0,
                "unloaded" : list.1,
            }).to_string()))
        },
        Err(reason) => {
            return (Status::BadRequest, content::Json(reason));
        },
    }

}

#[get("/instance/<uuid>/resources/<resource_type>/load/<resource_name>")]
pub async fn load_resource(uuid: String, resource_type: ResourceType, resource_name : String, state: &State<MyManagedState>) -> (Status, content::Json<String>) {
    match state.instance_manager.lock().await.load(&uuid, resource_type, &resource_name) {
        Ok(_) => {
            (Status::Ok, content::Json("Ok".to_string()))
        },
        Err(reason) => {
            return (Status::BadRequest, content::Json(reason));
        },
    }
}

#[get("/instance/<uuid>/resources/<resource_type>/unload/<resource_name>")]
pub async fn unload_resource(uuid: String, resource_type: ResourceType, resource_name : String, state: &State<MyManagedState>) -> (Status, content::Json<String>) {
    match state.instance_manager.lock().await.unload(&uuid, resource_type, &resource_name) {
        Ok(_) => {
            (Status::Ok, content::Json("Ok".to_string()))
        },
        Err(reason) => {
            return (Status::BadRequest, content::Json(reason));
        },
    }

}
