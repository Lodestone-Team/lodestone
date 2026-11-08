//! The Modrinth proxy: browsing projects and versions for any logged-in user.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use super::util::AuthedUser;
use crate::error::Error;
use crate::modrinth::links::{check_api_segment, resolve_url};
use crate::modrinth::search::{compatible_loaders, is_valid_filter_value};
use crate::modrinth::{
    ModrinthProject, ModrinthProjectType, ModrinthSearchIndex, ModrinthSearchResponse,
    ModrinthUrlTarget, ModrinthVersion, SearchParams,
};
use crate::mods::bad_request;
use crate::AppState;

#[derive(Deserialize)]
struct SearchQuery {
    query: Option<String>,
    project_type: ModrinthProjectType,
    game_version: Option<String>,
    loader: Option<String>,
    index: Option<ModrinthSearchIndex>,
    offset: Option<u32>,
    limit: Option<u32>,
}

async fn search(
    State(state): State<AppState>,
    _requester: AuthedUser,
    Query(query): Query<SearchQuery>,
) -> Result<Json<ModrinthSearchResponse>, Error> {
    let params = SearchParams::new(
        query.query,
        query.project_type,
        query.game_version,
        query.loader,
        query.index,
        query.offset,
        query.limit,
    )?;
    Ok(Json(state.mods.modrinth.search(&params).await?))
}

async fn get_project(
    State(state): State<AppState>,
    _requester: AuthedUser,
    Path(id): Path<String>,
) -> Result<Json<ModrinthProject>, Error> {
    let id = check_api_segment(&id, "project id")?;
    Ok(Json(state.mods.modrinth.project(id).await?))
}

#[derive(Deserialize)]
struct VersionsQuery {
    game_version: Option<String>,
    loader: Option<String>,
}

/// A filter value from a client: empty means none.
fn filter(value: Option<String>, what: &str) -> Result<Vec<String>, Error> {
    match value
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty())
    {
        Some(v) if is_valid_filter_value(&v) => Ok(vec![v]),
        Some(_) => Err(bad_request(format!("Invalid {what}"))),
        None => Ok(Vec::new()),
    }
}

async fn get_project_versions(
    State(state): State<AppState>,
    _requester: AuthedUser,
    Path(id): Path<String>,
    Query(query): Query<VersionsQuery>,
) -> Result<Json<Vec<ModrinthVersion>>, Error> {
    let id = check_api_segment(&id, "project id")?;
    // game versions are case sensitive ("1.20.1", "24w14a"); loaders are lowercase
    let game_versions = match query.game_version.map(|v| v.trim().to_string()) {
        Some(v) if v.is_empty() => Vec::new(),
        Some(v) if is_valid_filter_value(&v) => vec![v],
        Some(_) => return Err(bad_request("Invalid game version")),
        None => Vec::new(),
    };
    // a Paper server also runs Spigot and Bukkit plugins
    let loaders: Vec<String> = filter(query.loader, "loader")?
        .iter()
        .flat_map(|l| compatible_loaders(l))
        .collect();
    Ok(Json(
        state
            .mods
            .modrinth
            .project_versions(id, &loaders, &game_versions)
            .await?,
    ))
}

async fn get_version(
    State(state): State<AppState>,
    _requester: AuthedUser,
    Path(id): Path<String>,
) -> Result<Json<ModrinthVersion>, Error> {
    let id = check_api_segment(&id, "version id")?;
    Ok(Json(state.mods.modrinth.version(id).await?))
}

#[derive(Deserialize)]
struct ResolveQuery {
    url: String,
}

/// Resolves a modrinth.com link to the project (and version) it points to.
async fn resolve(
    State(state): State<AppState>,
    _requester: AuthedUser,
    Query(query): Query<ResolveQuery>,
) -> Result<Json<ModrinthUrlTarget>, Error> {
    Ok(Json(
        resolve_url(state.mods.modrinth.as_ref(), &query.url).await?,
    ))
}

pub fn get_modrinth_routes(state: AppState) -> Router {
    Router::new()
        .route("/modrinth/search", get(search))
        .route("/modrinth/project/{id}", get(get_project))
        .route("/modrinth/project/{id}/versions", get(get_project_versions))
        .route("/modrinth/version/{id}", get(get_version))
        .route("/modrinth/resolve", get(resolve))
        .with_state(state)
}
