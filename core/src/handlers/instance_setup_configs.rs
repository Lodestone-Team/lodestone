use crate::error::Error;
use crate::error::ErrorKind;
use crate::implementations::minecraft;
use crate::minecraft::FlavourKind;
use crate::traits::t_configurable::manifest::SetupManifest;
use crate::traits::t_configurable::GameType;
use crate::AppState;
use axum::extract::Path;
use axum::routing::get;
use axum::Json;
use axum::Router;
use color_eyre::eyre::eyre;
use serde::Deserialize;
use serde::Serialize;
use ts_rs::TS;

#[allow(clippy::enum_variant_names)]
#[derive(Serialize, Deserialize, TS, Clone, Copy)]
#[ts(export)]
pub enum HandlerGameType {
    MinecraftJavaVanilla,
    MinecraftFabric,
    MinecraftForge,
    MinecraftNeoForge,
    MinecraftPaper,
    MinecraftBedrock,
}

impl From<HandlerGameType> for GameType {
    fn from(value: HandlerGameType) -> Self {
        match value {
            HandlerGameType::MinecraftJavaVanilla => Self::MinecraftJava,
            HandlerGameType::MinecraftFabric => Self::MinecraftJava,
            HandlerGameType::MinecraftForge => Self::MinecraftJava,
            HandlerGameType::MinecraftNeoForge => Self::MinecraftJava,
            HandlerGameType::MinecraftPaper => Self::MinecraftJava,
            HandlerGameType::MinecraftBedrock => Self::MinecraftBedrock,
        }
    }
}

impl TryFrom<HandlerGameType> for FlavourKind {
    type Error = Error;

    fn try_from(value: HandlerGameType) -> Result<Self, Error> {
        Ok(match value {
            HandlerGameType::MinecraftJavaVanilla => Self::Vanilla,
            HandlerGameType::MinecraftFabric => Self::Fabric,
            HandlerGameType::MinecraftForge => Self::Forge,
            HandlerGameType::MinecraftNeoForge => Self::NeoForge,
            HandlerGameType::MinecraftPaper => Self::Paper,
            HandlerGameType::MinecraftBedrock => {
                return Err(Error {
                    kind: ErrorKind::BadRequest,
                    source: eyre!("Minecraft Bedrock servers are not supported"),
                })
            }
        })
    }
}

pub async fn get_available_games() -> Json<Vec<HandlerGameType>> {
    Json(vec![
        HandlerGameType::MinecraftJavaVanilla,
        HandlerGameType::MinecraftFabric,
        HandlerGameType::MinecraftForge,
        HandlerGameType::MinecraftNeoForge,
        HandlerGameType::MinecraftPaper,
    ])
}

pub async fn get_setup_manifest(
    Path(game_type): Path<HandlerGameType>,
) -> Result<Json<SetupManifest>, Error> {
    minecraft::MinecraftInstance::setup_manifest(&game_type.try_into()?)
        .await
        .map(Json)
}

pub fn get_instance_setup_config_routes(appstate: AppState) -> Router {
    Router::new()
        .route("/games", get(get_available_games))
        .route("/setup_manifest/{game_type}", get(get_setup_manifest))
        .with_state(appstate)
}
