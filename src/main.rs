
#![allow(clippy::comparison_chain, clippy::type_complexity)]

use crate::{
    handlers::instance::{list_instance, start_instance},
    handlers::{
        instance::{
            create_instance, get_instance_state, kill_instance, remove_instance, send_command,
            stop_instance,
        },
        users::{change_password, delete_user, get_user_info, login, new_user, update_permissions},
        ws::ws_handler,
    },
    traits::Error,
    util::rand_alphanumeric,
};
use argon2::{password_hash::SaltString, Argon2, PasswordHasher};
use axum::{
    routing::{delete, get, post},
    Extension, Router,
};
use events::Event;
use implementations::minecraft;
use json_store::user::User;
use log::{debug, info};
use rand_core::OsRng;
use reqwest::{header, Method};
use serde_json::Value;
use stateful::Stateful;
use std::{
    collections::{HashMap},
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
mod events;
mod handlers;
mod implementations;
mod json_store;
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
                        None,
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

    let mut stateful_users = Stateful::new(
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
    );

    if !stateful_users
        .get_ref()
        .iter().any(|(_, user)| user.is_owner)
    {
        let owner_psw: String = rand_alphanumeric(8);
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hashed_psw = argon2
            .hash_password(owner_psw.as_bytes(), &salt)
            .unwrap()
            .to_string();
        let uid = uuid::Uuid::new_v4().to_string();
        let owner = User {
            username: "owner".to_string(),
            is_owner: true,
            permissions: HashMap::new(),
            uid: uid.clone(),
            hashed_psw,
            is_admin: false,
            secret: rand_alphanumeric(32),
        };
        stateful_users
            .transform(Box::new(move |users| -> Result<(), Error> {
                users.insert(uid.clone(), owner.clone());
                Ok(())
            }))
            .unwrap();
        info!("Created owner account since none was present");
        info!("Username: owner");
        info!("Password: {}", owner_psw);
    }

    let shared_state = AppState {
        instances: Arc::new(Mutex::new(restore_instances(&lodestone_path, &tx))),
        users: Arc::new(Mutex::new(stateful_users)),
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

    let api_routes = Router::new()
        .route("/ws", get(ws_handler))
        .route("/instances/list", get(list_instance))
        .route("/instances/new", post(create_instance))
        .route("/instances/start/:uuid", post(start_instance))
        .route("/instances/stop/:uuid", post(stop_instance))
        .route("/instances/remove/:uuid", post(remove_instance))
        .route("/instances/kill/:uuid", post(kill_instance))
        .route("/instances/send/:uuid/:cmd", post(send_command))
        .route("/instances/state/:uuid", get(get_instance_state))
        .route("/users/create", post(new_user))
        .route("/users/delete/:uid", delete(delete_user))
        .route("/users/info/:uid", get(get_user_info))
        .route("/users/update_perm", post(update_permissions))
        .route("/users/login", get(login))
        .route("/users/passwd", post(change_password))
        .layer(Extension(shared_state))
        .layer(cors);
    let app = Router::new().nest("/api/v1", api_routes);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
