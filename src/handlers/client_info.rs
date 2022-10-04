use std::{env, sync::atomic::Ordering};

use crate::{AppState, prelude::VERSION};
use axum::{Extension, Json, Router, routing::get};
use serde::{Deserialize, Serialize};
use sysinfo::{CpuExt, DiskExt, System, SystemExt};

#[derive(Serialize, Deserialize)]
pub struct ClientInfo {
    version: semver::Version,
    is_setup: bool,
    os: String,
    arch: String,
    cpu: String,
    cpu_count: u32,
    total_ram: u64,
    total_disk: u64,
    host_name: String,
    uuid: String,
    client_name: String,
    up_since: i64,
}

pub async fn get_client_info(Extension(state): Extension<AppState>) -> Json<ClientInfo> {
    let sys = System::new_all();
    Json(ClientInfo {
        version: VERSION.with(|v| v.clone()),
        is_setup: state.is_setup.load(Ordering::Relaxed),
        os: env::consts::OS.to_string(),
        arch: env::consts::ARCH.to_string(),
        cpu: {
            let cpu_str = sys
                .cpus()
                .first()
                .map_or_else(|| "Unknown CPU", |v| v.brand());
            if cpu_str.is_empty() {
                "Unknown CPU".to_string()
            } else {
                cpu_str.to_string()
            }
        },
        cpu_count: sys.cpus().len() as u32,
        host_name: sys
            .host_name()
            .unwrap_or_else(|| "Unknown Hostname".to_string()),
        total_ram: sys.total_memory(),
        total_disk: sys.disks().iter().fold(0, |acc, v| acc + v.total_space()),
        client_name: state.client_name.lock().await.clone(),
        uuid: state.uuid.clone(),
        up_since: state.up_since,
    })
}

pub fn get_client_info_routes() -> Router {
    Router::new().route("/info", get(get_client_info))
}
