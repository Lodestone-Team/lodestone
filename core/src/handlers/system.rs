use axum::{extract::State, routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use sysinfo::{CpuRefreshKind, Disks};

use tokio::time::sleep;

use crate::AppState;

use super::util::AuthedUser;

// Since MemInfo is not serializable, we need to create a new struct that is serializable.
#[derive(Serialize, Deserialize)]
pub struct MemInfo {
    total: u64,
    free: u64,
}

pub async fn get_ram(State(state): State<AppState>, _requester: AuthedUser) -> Json<MemInfo> {
    let mut sys = state.system.lock().await;
    sys.refresh_memory();
    Json(MemInfo {
        total: sys.total_memory(),
        free: sys.available_memory(),
    })
}

// Since DiskInfo is not serializable, we need to create a new struct that is serializable.
#[derive(Serialize, Deserialize)]
pub struct DiskInfo {
    total: u64,
    free: u64,
}

pub async fn get_disk(_requester: AuthedUser) -> Json<DiskInfo> {
    let disks = Disks::new_with_refreshed_list();
    Json(DiskInfo {
        total: disks.iter().fold(0, |acc, v| acc + v.total_space()),
        free: disks.iter().fold(0, |acc, v| acc + v.available_space()),
    })
}

#[derive(Serialize, Deserialize)]
pub struct CPUInfo {
    pub cpu_speed: u64,
    pub cpu_load: f32,
}

pub async fn get_cpu_info(State(state): State<AppState>, _requester: AuthedUser) -> Json<CPUInfo> {
    state
        .system
        .lock()
        .await
        .refresh_cpu_specifics(CpuRefreshKind::everything());
    // CPU usage is measured between two refreshes; wait without holding the lock
    sleep(tokio::time::Duration::from_millis(100)).await;
    let mut sys = state.system.lock().await;
    sys.refresh_cpu_usage();
    let cpu_count = sys.cpus().len().max(1);
    Json(CPUInfo {
        cpu_speed: sys.cpus().iter().fold(0, |acc, v| acc + v.frequency()) / cpu_count as u64,
        cpu_load: sys.cpus().iter().fold(0.0, |acc, v| acc + v.cpu_usage()) / cpu_count as f32,
    })
}

pub fn get_system_routes(state: AppState) -> Router {
    Router::new()
        .route("/system/ram", get(get_ram))
        .route("/system/disk", get(get_disk))
        .route("/system/cpu", get(get_cpu_info))
        .with_state(state)
}
