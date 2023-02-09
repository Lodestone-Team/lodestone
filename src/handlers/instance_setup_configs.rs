use std::collections::HashSet;

use axum::routing::get;
use axum::Router;
use axum::{extract::Path, Json};

use crate::error::Error;
use crate::implementations::minecraft::versions::MinecraftVersions;
use crate::prelude::GameInstanceKind;

use crate::implementations::minecraft;

pub async fn get_available_games() -> Json<HashSet<GameInstanceKind>> {
    Json(HashSet::from([GameInstanceKind::MinecraftInstance]))
}

pub async fn get_available_flavours(
    Path(game_type): Path<GameInstanceKind>,
) -> Json<HashSet<String>> {
    match game_type {
        GameInstanceKind::MinecraftInstance => Json(HashSet::from([
            minecraft::Flavour::Vanilla.to_string(),
            minecraft::Flavour::Fabric { loader_version: None, installer_version: None }.to_string(),
            minecraft::Flavour::Paper { build_version: None }.to_string(),
            minecraft::Flavour::Forge { build_version: None }.to_string(),
        ])),
    }
}

pub async fn get_minecraft_versions(
    Path(flavour): Path<String>,
) -> Result<Json<MinecraftVersions>, Error> {
    Ok(Json(match flavour.as_str() {
        "vanilla" => minecraft::versions::get_vanilla_versions().await?,
        "fabric" => minecraft::versions::get_fabric_versions().await?,
        "paper" => minecraft::versions::get_paper_versions().await?,
        "forge" => minecraft::versions::get_forge_versions().await?,
        _ => unimplemented!(),
    }))
}

pub fn get_instance_setup_config_routes() -> Router {
    Router::new()
        .route("/games", get(get_available_games))
        .route("/games/:game_type/flavours", get(get_available_flavours))
        .route(
            "/games/minecraft/flavours/:flavour/versions",
            get(get_minecraft_versions),
        )
        .with_state(())
}
