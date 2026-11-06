use std::path::PathBuf;
use std::sync::{LazyLock, OnceLock};

static LODESTONE_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn lodestone_path() -> &'static PathBuf {
    LODESTONE_PATH.get().unwrap()
}

static PATH_TO_INSTANCES: OnceLock<PathBuf> = OnceLock::new();

pub fn path_to_instances() -> &'static PathBuf {
    PATH_TO_INSTANCES.get().unwrap()
}

static PATH_TO_BINARIES: OnceLock<PathBuf> = OnceLock::new();

pub fn path_to_binaries() -> &'static PathBuf {
    PATH_TO_BINARIES.get().unwrap()
}

static PATH_TO_STORES: OnceLock<PathBuf> = OnceLock::new();

pub fn path_to_stores() -> &'static PathBuf {
    PATH_TO_STORES.get().unwrap()
}

static PATH_TO_GLOBAL_SETTINGS: OnceLock<PathBuf> = OnceLock::new();

pub fn path_to_global_settings() -> &'static PathBuf {
    PATH_TO_GLOBAL_SETTINGS.get().unwrap()
}

static PATH_TO_USERS: OnceLock<PathBuf> = OnceLock::new();

pub fn path_to_users() -> &'static PathBuf {
    PATH_TO_USERS.get().unwrap()
}

static PATH_TO_TMP: OnceLock<PathBuf> = OnceLock::new();

pub fn path_to_tmp() -> &'static PathBuf {
    PATH_TO_TMP.get().unwrap()
}

static APP_STATE: OnceLock<AppState> = OnceLock::new();

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
///
/// # Panics
/// If a directory cannot be created. [`try_init_paths`] returns the error instead.
pub fn init_paths(lodestone_path: PathBuf) {
    try_init_paths(lodestone_path).expect("Failed to create the Lodestone directories");
}

/// Same as [`init_paths`], but returns an error instead of panicking when a directory
/// cannot be created.
pub fn try_init_paths(lodestone_path: PathBuf) -> std::io::Result<()> {
    let path_to_instances = lodestone_path.join("instances");
    let path_to_binaries = lodestone_path.join("bin");
    let path_to_stores = lodestone_path.join("stores");
    let path_to_global_settings = lodestone_path.join("global_settings.json");
    let path_to_users = lodestone_path.join("stores").join("users.json");
    let path_to_tmp = lodestone_path.join("tmp");

    std::fs::create_dir_all(&path_to_instances)?;
    std::fs::create_dir_all(&path_to_binaries)?;
    std::fs::create_dir_all(&path_to_stores)?;
    std::fs::create_dir_all(&path_to_tmp)?;

    let _ = LODESTONE_PATH.set(lodestone_path);
    let _ = PATH_TO_INSTANCES.set(path_to_instances);
    let _ = PATH_TO_BINARIES.set(path_to_binaries);
    let _ = PATH_TO_STORES.set(path_to_stores);
    let _ = PATH_TO_GLOBAL_SETTINGS.set(path_to_global_settings);
    let _ = PATH_TO_USERS.set(path_to_users);
    let _ = PATH_TO_TMP.set(path_to_tmp);
    Ok(())
}

/// A process-wide constant that is computed on first use.
///
/// `with` mirrors the API of the `thread_local!` statics these constants used to be, so
/// `VERSION.with(|v| ...)` keeps working. New code can dereference it instead.
pub struct LazyConst<T>(LazyLock<T>);

impl<T> LazyConst<T> {
    pub fn with<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        f(&self.0)
    }
}

impl<T> std::ops::Deref for LazyConst<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

/// The version of this build, parsed from Cargo.toml.
pub static VERSION: LazyConst<semver::Version> = LazyConst(LazyLock::new(|| {
    semver::Version::parse(env!("CARGO_PKG_VERSION")).expect("CARGO_PKG_VERSION is valid semver")
}));

pub const LODESTONE_EPOCH_SEC: i64 = 1667530800;
pub const LODESTONE_EPOCH_MIL: i64 = LODESTONE_EPOCH_SEC * 1000;

pub static SNOWFLAKE_GENERATOR: LazyLock<std::sync::Mutex<snowflake::SnowflakeIdGenerator>> =
    LazyLock::new(|| {
        std::sync::Mutex::new(snowflake::SnowflakeIdGenerator::with_epoch(
            1,
            1,
            std::time::UNIX_EPOCH + std::time::Duration::from_millis(LODESTONE_EPOCH_MIL as u64),
        ))
    });

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
