use std::path::PathBuf;

use semver::{BuildMetadata, Prerelease};
thread_local! {
    pub static VERSION: semver::Version = semver::Version {
        major: 0,
        minor: 2,
        patch: 0,
        pre: Prerelease::new("alpha.1").unwrap(),
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

}

#[derive(PartialEq, Eq, Hash, Clone, Debug)]
pub enum GameType {
    Minecraft,
}

impl<'de> serde::Deserialize<'de> for GameType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "minecraft" => Ok(GameType::Minecraft),
            _ => Err(serde::de::Error::custom(format!(
                "Unknown game type: {}",
                s
            ))),
        }
    }
}
impl serde::Serialize for GameType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            GameType::Minecraft => serializer.serialize_str("minecraft"),
        }
    }
}

impl ToString for GameType {
    fn to_string(&self) -> String {
        match self {
            GameType::Minecraft => "minecraft".to_string(),
        }
    }
}
