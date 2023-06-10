use lazy_static::lazy_static;
use std::path::PathBuf;

use once_cell::sync::OnceCell;
use semver::{BuildMetadata, Prerelease};

static LODESTONE_PATH: OnceCell<PathBuf> = OnceCell::new();

pub fn lodestone_path() -> &'static PathBuf {
    LODESTONE_PATH.get().unwrap()
}

static PATH_TO_INSTANCES: OnceCell<PathBuf> = OnceCell::new();

pub fn path_to_instances() -> &'static PathBuf {
    PATH_TO_INSTANCES.get().unwrap()
}

static PATH_TO_BINARIES: OnceCell<PathBuf> = OnceCell::new();

pub fn path_to_binaries() -> &'static PathBuf {
    PATH_TO_BINARIES.get().unwrap()
}

static PATH_TO_STORES: OnceCell<PathBuf> = OnceCell::new();

pub fn path_to_stores() -> &'static PathBuf {
    PATH_TO_STORES.get().unwrap()
}

static PATH_TO_GLOBAL_SETTINGS: OnceCell<PathBuf> = OnceCell::new();

pub fn path_to_global_settings() -> &'static PathBuf {
    PATH_TO_GLOBAL_SETTINGS.get().unwrap()
}

static PATH_TO_USERS: OnceCell<PathBuf> = OnceCell::new();

pub fn path_to_users() -> &'static PathBuf {
    PATH_TO_USERS.get().unwrap()
}

static PATH_TO_TMP: OnceCell<PathBuf> = OnceCell::new();

pub fn path_to_tmp() -> &'static PathBuf {
    PATH_TO_TMP.get().unwrap()
}

static APP_STATE: OnceCell<AppState> = OnceCell::new();

pub fn init_app_state(app_state: AppState) {
    let _ = APP_STATE.set(app_state);
}

pub fn app_state() -> &'static AppState {
    APP_STATE.get().unwrap()
}

/// Initialize the paths for the lodestone instance.
/// This function should only be called once.
///
/// Also creates the directories if they don't exist.
pub fn init_paths(lodestone_path: PathBuf) {
    let path_to_instances = lodestone_path.join("instances");
    let path_to_binaries = lodestone_path.join("bin");
    let path_to_stores = lodestone_path.join("stores");
    let path_to_global_settings = lodestone_path.join("global_settings.json");
    let path_to_users = lodestone_path.join("stores").join("users.json");
    let path_to_tmp = lodestone_path.join("tmp");

    std::fs::create_dir_all(&path_to_instances).unwrap();
    std::fs::create_dir_all(&path_to_binaries).unwrap();
    std::fs::create_dir_all(&path_to_stores).unwrap();
    std::fs::create_dir_all(&path_to_tmp).unwrap();
    // std::fs::File::create(&path_to_global_settings).unwrap();
    // std::fs::File::create(&path_to_users).unwrap();
    // std::fs::File::create(&path_to_tmp).unwrap();

    let _ = LODESTONE_PATH.set(lodestone_path);
    let _ = PATH_TO_INSTANCES.set(path_to_instances);
    let _ = PATH_TO_BINARIES.set(path_to_binaries);
    let _ = PATH_TO_STORES.set(path_to_stores);
    let _ = PATH_TO_GLOBAL_SETTINGS.set(path_to_global_settings);
    let _ = PATH_TO_USERS.set(path_to_users);
    let _ = PATH_TO_TMP.set(path_to_tmp);
}

thread_local! {
    pub static VERSION: semver::Version = semver::Version {
        major: 0,
        minor: 4,
        patch: 4,
        pre: Prerelease::new("").unwrap(),
        build: BuildMetadata::EMPTY,
    };

    pub static LODESTONE_EPOCH_SEC: i64 = 1667530800;
    pub static LODESTONE_EPOCH_MIL: i64 = 1667530800000;
}

lazy_static! {
    pub static ref SNOWFLAKE_GENERATOR: std::sync::Mutex<snowflake::SnowflakeIdGenerator> =
        std::sync::Mutex::new(snowflake::SnowflakeIdGenerator::with_epoch(
            1,
            1,
            std::time::UNIX_EPOCH + std::time::Duration::from_millis(1667530800000)
        ));
}

use crate::generic::GenericInstance;
use crate::minecraft::MinecraftInstance;
use crate::AppState;
#[enum_dispatch::enum_dispatch(
    TInstance,
    TConfigurable,
    TMacro,
    TPlayerManagement,
    TResourceManagement,
    TServer,
    TManifest
)]
#[derive(Clone)]
pub enum GameInstance {
    MinecraftInstance,
    GenericInstance,
}
