use std::{env, sync::OnceLock};

use crate::{
    capabilities::CoreCapabilities,
    prelude::VERSION,
    product::{CoreProduct, LaunchedBy},
    tls::TlsInfo,
    AppState,
};
use axum::{routing::get, Json, Router};
use serde::{Deserialize, Serialize};
use sysinfo::{Disks, System};

#[derive(Serialize, Deserialize)]
pub struct CoreInfo {
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
    core_name: String,
    up_since: i64,
    tls: TlsInfo,
    /// The optional features this core has. Missing on older cores, which have them all.
    capabilities: CoreCapabilities,
    /// Lodestone Host or the standalone core. Missing on older cores: treat as `core`.
    product: CoreProduct,
    /// What started the core. Missing on older cores: unknown.
    launched_by: LaunchedBy,
}

/// Hardware details that do not change while the core runs.
struct HardwareInfo {
    cpu: String,
    cpu_count: u32,
    total_ram: u64,
    total_disk: u64,
    host_name: String,
}

/// `/info` needs no login (the dashboard calls it before sign-in), so read the hardware once
/// instead of building a new `sysinfo::System` on every request.
fn hardware_info() -> &'static HardwareInfo {
    static HARDWARE_INFO: OnceLock<HardwareInfo> = OnceLock::new();
    HARDWARE_INFO.get_or_init(|| {
        let sys = crate::util::cpu_and_memory_system();
        let cpu = sys
            .cpus()
            .first()
            .map(|cpu| cpu.brand())
            .filter(|brand| !brand.is_empty())
            .unwrap_or("Unknown CPU")
            .to_string();
        HardwareInfo {
            cpu,
            cpu_count: sys.cpus().len() as u32,
            total_ram: sys.total_memory(),
            total_disk: Disks::new_with_refreshed_list()
                .iter()
                .fold(0, |acc, v| acc + v.total_space()),
            host_name: System::host_name().unwrap_or_else(|| "Unknown Hostname".to_string()),
        }
    })
}

pub async fn get_core_info(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> Json<CoreInfo> {
    let hardware = hardware_info();
    Json(CoreInfo {
        version: VERSION.with(|v| v.clone()),
        is_setup: state.first_time_setup_key.lock().await.is_none(),
        os: env::consts::OS.to_string(),
        arch: env::consts::ARCH.to_string(),
        cpu: hardware.cpu.clone(),
        cpu_count: hardware.cpu_count,
        host_name: hardware.host_name.clone(),
        total_ram: hardware.total_ram,
        total_disk: hardware.total_disk,
        core_name: state.global_settings.lock().await.core_name(),
        uuid: state.uuid.clone(),
        up_since: state.up_since,
        tls: (*state.tls_info).clone(),
        capabilities: CoreCapabilities::new(state.docker_bridge.is_available()),
        product: state.product,
        launched_by: state.launched_by,
    })
}

pub fn get_core_info_routes(state: AppState) -> Router {
    Router::new()
        .route("/info", get(get_core_info))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use crate::{
        global_settings::GlobalSettingsData,
        product::{CoreProduct, LaunchedBy},
        test_util::{serve_api, test_app_state},
    };

    async fn info_of(product: CoreProduct, launched_by: LaunchedBy) -> serde_json::Value {
        let dir = tempfile::tempdir().unwrap();
        let mut state = test_app_state(dir.path(), GlobalSettingsData::default()).await;
        state.product = product;
        state.launched_by = launched_by;
        let base = serve_api(state).await;
        reqwest::get(format!("{base}/info"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn info_says_what_this_core_is_and_what_started_it() {
        let info = info_of(CoreProduct::Core, LaunchedBy::LodestoneCli).await;
        assert_eq!(info["product"], "core");
        assert_eq!(info["launched_by"], "lodestone_cli");
        let info = info_of(CoreProduct::Host, LaunchedBy::Host).await;
        assert_eq!(info["product"], "host");
        assert_eq!(info["launched_by"], "host");
        let info = info_of(CoreProduct::Core, LaunchedBy::Docker).await;
        assert_eq!(info["launched_by"], "docker");
    }
}
