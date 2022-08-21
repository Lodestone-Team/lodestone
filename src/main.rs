use crate::{
    handlers::instance::{list_instance, start_instance},
    handlers::{
        instance::{
            create_instance, get_instance_state, kill_instance, remove_instance, send_command,
            stop_instance,
        },
        users::{login, new_user, delete_user, get_user_info, update_permissions},
        ws::ws_handler,
    },
};
use axum::{
    routing::{get, post},
    Extension, Router,
};
use db::user::User;
use events::Event;
use implementations::minecraft;
use log::{debug, info};
use reqwest::{Method, header};
use serde_json::Value;
use stateful::Stateful;
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::{
    fs::create_dir_all,
    sync::{
        broadcast::{self, Receiver, Sender},
        Mutex,
    },
};
use tower_http::cors::{Any, CorsLayer};
use traits::{t_configurable::TConfigurable, TInstance};
use util::list_dir;
mod db;
mod events;
mod handlers;
mod implementations;
mod stateful;
mod traits;
mod util;

#[derive(Clone)]
pub struct AppState {
    instances: Arc<Mutex<HashMap<String, Arc<Mutex<dyn TInstance>>>>>,
    users: Arc<Mutex<Stateful<HashMap<String, User>>>>,
    event_broadcaster: Sender<Event>,
}

fn restore_instances(
    lodestone_path: &Path,
    event_broadcaster: &Sender<Event>,
) -> HashMap<String, Arc<Mutex<dyn TInstance>>> {
    let mut ret: HashMap<String, Arc<Mutex<dyn TInstance>>> = HashMap::new();

    list_dir(&lodestone_path.join("instances"), Some(true))
        .unwrap()
        .iter()
        .filter(|path| {
            debug!("{}", path.display());
            path.join(".lodestone_config").is_file()
        })
        .map(|path| {
            // read config as json
            let config: Value = serde_json::from_reader(
                std::fs::File::open(path.join(".lodestone_config")).unwrap(),
            )
            .unwrap();
            config
        })
        .map(|config| {
            match config["type"]
                .as_str()
                .unwrap()
                .to_ascii_lowercase()
                .as_str()
            {
                "minecraft" => {
                    debug!(
                        "Restoring Minecraft instance {}",
                        config["name"].as_str().unwrap()
                    );
                    minecraft::Instance::restore(
                        serde_json::from_value(config).unwrap(),
                        event_broadcaster.clone(),
                    )
                }
                _ => unimplemented!(),
            }
        })
        .for_each(|instance| {
            ret.insert(instance.uuid(), Arc::new(Mutex::new(instance)));
        });
    ret
}

fn restore_users(path_to_user_json: &Path) -> HashMap<String, User> {
    if std::fs::File::open(path_to_user_json)
        .unwrap()
        .metadata()
        .unwrap()
        .len()
        == 0
    {
        return HashMap::new();
    }
    let users: HashMap<String, User> =
        serde_json::from_reader(std::fs::File::open(path_to_user_json).unwrap()).unwrap();
    users
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Debug)
        .format_module_path(false)
        // .format_timestamp(None)
        .format_target(false)
        .init();
    let lodestone_path = PathBuf::from(
        std::env::var("LODESTONE_PATH")
            .unwrap_or_else(|_| std::env::current_dir().unwrap().display().to_string()),
    );
    std::env::set_current_dir(&lodestone_path).expect("Failed to set current dir");

    let web_path = lodestone_path.join("web");
    let dot_lodestone_path = lodestone_path.join(".lodestone");
    let path_to_intances = lodestone_path.join("instances");
    create_dir_all(&dot_lodestone_path).await.unwrap();
    create_dir_all(&web_path).await.unwrap();
    create_dir_all(&path_to_intances).await.unwrap();
    info!("Lodestone path: {}", lodestone_path.display());

    let (tx, _rx): (Sender<Event>, Receiver<Event>) = broadcast::channel(128);

    let shared_state = AppState {
        instances: Arc::new(Mutex::new(restore_instances(&lodestone_path, &tx))),
        users: Arc::new(Mutex::new(Stateful::new(
            restore_users(&dot_lodestone_path.join("users")),
            {
                let dot_lodestone_path = dot_lodestone_path.clone();
                Box::new(move |users, _| {
                    serde_json::to_writer(
                        std::fs::File::create(&dot_lodestone_path.join("users")).unwrap(),
                        users,
                    )
                    .unwrap();
                    Ok(())
                })
            },
            {
                let dot_lodestone_path = dot_lodestone_path.clone();
                Box::new(move |users, _| {
                    serde_json::to_writer(
                        std::fs::File::create(&dot_lodestone_path.join("users")).unwrap(),
                        users,
                    )
                    .unwrap();
                    Ok(())
                })
            },
        ))),
        event_broadcaster: tx.clone(),
    };

    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::OPTIONS,
            Method::PATCH,
            Method::DELETE,
        ])
        .allow_headers([header::ORIGIN, header::CONTENT_TYPE]) // Note I can't find X-Auth-Token but it was in the original rocket version, hope it's fine
        .allow_origin(Any);

    let app = Router::new()
        .route("/ws", get(ws_handler))
        .route("/list", get(list_instance))
        .route("/new", post(create_instance))
        .route("/start/:uuid", post(start_instance))
        .route("/stop/:uuid", post(stop_instance))
        .route("/remove/:uuid", post(remove_instance))
        .route("/kill/:uuid", post(kill_instance))
        .route("/send/:uuid/:cmd", post(send_command))
        .route("/state/:uuid", get(get_instance_state))
        .route("/users/create", post(new_user))
        .route("/users/delete/:uuid", post(delete_user))
        .route("/users/info/:uuid", get(get_user_info))
        .route("/users/update_perm", post(update_permissions))
        .route("/users/login", get(login))
        .layer(Extension(shared_state))
        .layer(cors);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
