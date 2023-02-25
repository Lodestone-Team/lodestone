use axum::routing::get;
use axum::Json;
use axum::Router;

use crate::traits::t_configurable::InstanceGameType;


pub async fn get_available_games() -> Json<Vec<InstanceGameType>> {
    Json(vec![
        InstanceGameType::MinecraftVanilla,
        InstanceGameType::MinecraftFabric,
        InstanceGameType::MinecraftPaper,
        InstanceGameType::MinecraftForge,
    ])
}

pub fn get_instance_setup_config_routes() -> Router {
    Router::new()
        .route("/games", get(get_available_games))
        .with_state(())
}
