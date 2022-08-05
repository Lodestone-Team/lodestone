use axum::{routing::get, Extension, Router};
use implementations::minecraft;
use log::{debug, info, warn};
use serde_json::Value;
use std::{
    path::{Path, PathBuf},
    sync::Arc, net::SocketAddr, thread,
};
use tokio::{
    fs::create_dir_all,
    sync::broadcast::{self, Sender},
};
use traits::TInstance;
use util::list_dir;

use crate::handlers::ws::ws_handler;
mod handlers;
mod implementations;
mod traits;
mod util;

#[derive(Clone)]
pub struct AppState {
    list_of_instances: Vec<Arc<dyn TInstance>>,
    event_broadcaster: Sender<String>,
}

fn restore_instances(lodestone_path: &Path) -> Vec<Arc<dyn TInstance>> {
    let mut ret: Vec<Arc<dyn TInstance>> = vec![];

    list_dir(&lodestone_path.join("instances"), Some(true))
        .unwrap()
        .iter()
        .filter(|path| path.join(".lodestone_config").is_file())
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
                    minecraft::Instance::restore(serde_json::from_value(config).unwrap())
                }
                _ => unimplemented!(),
            }
        })
        .map(|instance| Arc::new(instance))
        .for_each(|instance| {
            ret.push(instance);
        });
    ret
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .format_module_path(false)
        .format_timestamp(None)
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

    let (tx, _rx) = broadcast::channel(16);

    let shared_state = AppState {
        list_of_instances: restore_instances(&lodestone_path),
        event_broadcaster: tx.clone(),
    };
    let app = Router::new()
        .route("/ws", get(ws_handler))
        .layer(Extension(shared_state));

    thread::spawn( {
        let tx = tx.clone();
        move || {
        loop {
            let mut event = String::new();
            std::io::stdin().read_line(&mut event).unwrap();
            tx.send(event).unwrap();
        }
        }
    });

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    axum::Server::bind(&addr)
    .serve(app.into_make_service())
    .await
    .unwrap();

}
