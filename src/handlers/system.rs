use axum::{Json, Router, routing::get};
use serde::{Deserialize, Serialize};
use sysinfo::{CpuExt, CpuRefreshKind, DiskExt, System, SystemExt};

use tokio::time::sleep;

// Since MemInfo is not serializable, we need to create a new struct that is serializable.
#[derive(Serialize, Deserialize)]
pub struct MemInfo {
    total: u64,
    free: u64,
}

pub async fn get_ram() -> Json<MemInfo> {
    let sys = System::new_all();
    Json(MemInfo {
        total: sys.total_memory(),
        free: sys.free_memory(),
    })
}

// Since DiskInfo is not serializable, we need to create a new struct that is serializable.
#[derive(Serialize, Deserialize)]
pub struct DiskInfo {
    total: u64,
    free: u64,
}

pub async fn get_disk() -> Json<DiskInfo> {
    let sys = System::new_all();
    let disks = sys.disks();
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

pub async fn get_cpu_info() -> Json<CPUInfo> {
    let mut sys = System::new();
    sys.refresh_cpu_specifics(CpuRefreshKind::everything());
    sleep(tokio::time::Duration::from_millis(100)).await;
    sys.refresh_cpu();
    Json(CPUInfo {
        cpu_speed: {
            sys.cpus().iter().fold(0, |acc, v| acc + v.frequency()) / sys.cpus().len() as u64
        },
        cpu_load: sys.cpus().iter().fold(0.0, |acc, v| acc + v.cpu_usage())
            / sys.cpus().len() as f32,
    })
}

pub fn get_system_routes() -> Router {
    Router::new()
        .route("/system/ram", get(get_ram))
        .route("/system/disk", get(get_disk))
        .route("/system/cpu", get(get_cpu_info))
}
