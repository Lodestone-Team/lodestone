use std::{env, sync::atomic::Ordering};

use crate::{AppState, VERSION};
use axum::{Extension, Json};
use raw_cpuid::CpuId;
use serde::{Deserialize, Serialize};
use sysinfo::{DiskExt, System, SystemExt};

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
    up_since : i64,
}

pub async fn get_client_info(Extension(state): Extension<AppState>) -> Json<ClientInfo> {
    let cpu = CpuId::new();
    let sys = System::new_all();
    Json(ClientInfo {
        version: VERSION.with(|v| v.clone()),
        is_setup: state.is_setup.load(Ordering::Relaxed),
        os: env::consts::OS.to_string(),
        arch: env::consts::ARCH.to_string(),
        cpu: cpu
            .get_processor_brand_string()
            .map_or_else(|| "Unknown".to_string(), |v| v.as_str().to_string()),
        cpu_count: cpu
            .get_feature_info()
            .map(|v| v.max_logical_processor_ids())
            .unwrap_or(0)
            .into(),
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
