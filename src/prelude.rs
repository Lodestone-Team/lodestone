use lazy_static::lazy_static;
use std::path::PathBuf;

use semver::{BuildMetadata, Prerelease};
thread_local! {
    pub static VERSION: semver::Version = semver::Version {
        major: 0,
        minor: 3,
        patch: 0,
        pre: Prerelease::new("").unwrap(),
        build: BuildMetadata::EMPTY,
    };
    pub static LODESTONE_PATH : PathBuf = PathBuf::from(
        match std::env::var("LODESTONE_PATH") {
    Ok(v) => v,
    Err(_) => home::home_dir().unwrap_or_else(|| std::env::current_dir().expect("what kinda os are you running lodestone on???")).join(".lodestone").to_str().unwrap().to_string(),
}
    );
    pub static PATH_TO_INSTANCES : PathBuf = LODESTONE_PATH.with(|p| p.join("instances"));
    pub static PATH_TO_BINARIES : PathBuf = LODESTONE_PATH.with(|p| p.join("bin"));
    pub static PATH_TO_STORES : PathBuf = LODESTONE_PATH.with(|p| p.join("stores"));
    pub static PATH_TO_USERS : PathBuf = PATH_TO_STORES.with(|p| p.join("users.json"));
}

lazy_static! {
    pub static ref SNOWFLAKE_GENERATOR: std::sync::Mutex<snowflake::SnowflakeIdGenerator> =
        std::sync::Mutex::new(snowflake::SnowflakeIdGenerator::with_epoch(
            1,
            1,
            std::time::UNIX_EPOCH + std::time::Duration::from_millis(1667530800000)
        ));
}

pub fn get_snowflake() -> i64 {
    SNOWFLAKE_GENERATOR.lock().unwrap().real_time_generate()
}

use crate::minecraft::MinecraftInstance;
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
#[derive(enum_kinds::EnumKind)]
#[enum_kind(GameInstanceKind, derive(Hash))]
pub enum GameInstance {
    MinecraftInstance,
}


impl<'de> serde::Deserialize<'de> for GameInstanceKind {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "minecraft" => Ok(GameInstanceKind::MinecraftInstance),
            _ => Err(serde::de::Error::custom(format!(
                "Unknown game type: {}",
                s
            ))),
        }
    }
}
impl serde::Serialize for GameInstanceKind {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            GameInstanceKind::MinecraftInstance => serializer.serialize_str("minecraft"),
        }
    }
}

impl ToString for GameInstanceKind {
    fn to_string(&self) -> String {
        match self {
            GameInstanceKind::MinecraftInstance => "minecraft".to_string(),
        }
    }
}
