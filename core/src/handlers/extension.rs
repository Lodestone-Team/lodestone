use std::num::NonZeroU16;

use axum::{
    extract::Path,
    routing::{get, put},
    Json, Router,
};
use axum_auth::AuthBearer;

use color_eyre::eyre::{eyre, Context};
use serde_json::Value;
use tracing::error;

use crate::{
    error::{Error, ErrorKind},
    extension::{self, FetchExtensionManifestError},
    prelude::lodestone_path,
    AppState, auth::user::UserAction,
};

async fn is_git_installed() -> Json<bool> {
    Json(which::which("git").is_ok())
}

#[derive(serde::Deserialize)]
struct FetchExtensionManifestBody {
    username: String,
    repo: String,
}

impl axum::response::IntoResponse for FetchExtensionManifestError {
    fn into_response(self) -> axum::response::Response {
        match self {
            FetchExtensionManifestError::NotFound => (
                axum::http::StatusCode::NOT_FOUND,
                "GitHub API returned 404. Does the user and repo exist?".to_string(),
            )
                .into_response(),
            FetchExtensionManifestError::Other(status_code, e) => {
                (axum::http::StatusCode::from_u16(status_code).unwrap(), e).into_response()
            }
            FetchExtensionManifestError::Http(e) => {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
            }
            FetchExtensionManifestError::BadResponse(e) => {
                (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e).into_response()
            }
            FetchExtensionManifestError::BadManifest(e) => {
                (axum::http::StatusCode::UNPROCESSABLE_ENTITY, e).into_response()
            }
        }
    }
}

async fn fetch_extension_manifest(
    Json(body): Json<FetchExtensionManifestBody>,
) -> Result<Json<extension::Manifest>, FetchExtensionManifestError> {
    let manifest = extension::get_manifest(&body.username, &body.repo).await?;
    Ok(Json(manifest))
}

async fn install_extension(
    Path((username, repo)): Path<(String, String)>,
    axum::extract::State(state): axum::extract::State<AppState>,
    // AuthBearer(token): AuthBearer,
) -> Result<(), Error> {
    // let requester = state
    //     .users_manager
    //     .read()
    //     .await
    //     .try_auth(&token)
    //     .ok_or_else(|| Error {
    //         kind: ErrorKind::Unauthorized,
    //         source: eyre!("Token error"),
    //     })?;
    // requester.try_action(&UserAction::InstallExtension)?;
    let path = lodestone_path().join("extensions");
    tokio::fs::create_dir_all(&path).await.context("Failed to create extensions directory")?;
    let manager = extension::ExtensionManager::new(path);
    manager.install_extension(&username, &repo).await?;
    Ok(())
}

pub fn get_extension_routes(state: AppState) -> Router {
    Router::new()
        .route("/extension/gitstatus", get(is_git_installed))
        .route("/extension/fetchmanifest", get(fetch_extension_manifest))
        .route("/extension/install/:username/:repo", put(install_extension))
        .with_state(state)
}
