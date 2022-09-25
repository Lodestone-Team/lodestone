use std::thread;

use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sys_info::{
    cpu_num, cpu_speed, disk_info, loadavg, mem_info, os_release, os_type, DiskInfo, MemInfo,
};
use systemstat::{CPULoad, Duration, Platform, System};
use tokio::time::sleep;
extern crate systemstat;

// Since MemInfo is not serializable, we need to create a new struct that is serializable.
#[derive(Serialize, Deserialize)]
pub struct MemInfoDef {
    total: u64,
    free: u64,
    avail: u64,
    buffers: u64,
    cached: u64,
    swap_total: u64,
    swap_free: u64,
}

// implement from for MemInfoDef
impl From<MemInfo> for MemInfoDef {
    fn from(mem_info: MemInfo) -> Self {
        MemInfoDef {
            total: mem_info.total,
            free: mem_info.free,
            avail: mem_info.avail,
            buffers: mem_info.buffers,
            cached: mem_info.cached,
            swap_total: mem_info.swap_total,
            swap_free: mem_info.swap_free,
        }
    }
}

pub async fn get_ram() -> Json<MemInfoDef> {
    Json(mem_info().expect("Failed to get memory info").into())
}

// Since DiskInfo is not serializable, we need to create a new struct that is serializable.
#[derive(Serialize, Deserialize)]
pub struct DiskInfoDef {
    total: u64,
    free: u64,
}

// implement from for DiskInfoDef
impl From<DiskInfo> for DiskInfoDef {
    fn from(disk_info: DiskInfo) -> Self {
        DiskInfoDef {
            total: disk_info.total,
            free: disk_info.free,
        }
    }
}

pub async fn get_disk() -> Json<DiskInfoDef> {
    Json(disk_info().expect("Failed to get disk info").into())
}

#[derive(Serialize, Deserialize)]
pub struct CPUInfo {
    pub cpu_speed: u64,
    pub cpu_load: f32,
}

pub async fn get_cpu() -> Json<CPUInfo> {
    let sys = System::new();
    let cpu_aggregate = sys.cpu_load_aggregate().expect("Failed to get cpu load");
    sleep(Duration::from_secs(1)).await;
    let cpu_load = cpu_aggregate.done().expect("Failed to get cpu load");

    Json(CPUInfo {
        cpu_speed: cpu_speed().expect("Failed to get cpu speed"),
        cpu_load: cpu_load.user,
    })
}

#[derive(Serialize, Deserialize)]
pub struct OsInfo {
    pub os_release: String,
    pub os_type: String,
}

pub async fn get_os_info() -> Json<OsInfo> {
    Json(OsInfo {
        os_release: os_release().expect("Failed to get os release"),
        os_type: os_type().expect("Failed to get os type"),
    })
}

#[derive(Serialize, Deserialize)]
pub struct Uptime {
    pub uptime: u64,
}
// implements from Duration to Uptime
impl From<Duration> for Uptime {
    fn from(duration: Duration) -> Self {
        Uptime {
            uptime: duration.as_secs(),
        }
    }
}

pub async fn get_uptime() -> Json<Uptime> {
    Json(System::new().uptime().expect("Failed to get uptime").into())
}
