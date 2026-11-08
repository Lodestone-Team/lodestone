use axum::{
    extract::{Query, Request, State},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, put},
    Json, Router,
};

use color_eyre::eyre::Context;

use crate::{
    auth::user::UserAction,
    error::Error,
    extension::{self, FetchExtensionManifestError},
    prelude::lodestone_path,
    AppState,
};

use super::util::{require_code_execution, AuthedUser};

/// Extensions are code that runs on the core, so every extension route is owner only.
async fn require_owner(
    requester: AuthedUser,
    request: Request,
    next: Next,
) -> Result<Response, Error> {
    requester.owner_only("manage extensions")?;
    Ok(next.run(request).await)
}

async fn is_git_installed(_requester: AuthedUser) -> Json<bool> {
    Json(which::which("git").is_ok())
}

#[derive(serde::Deserialize)]
struct ExtensionRequestBody {
    url: String,
}

impl axum::response::IntoResponse for FetchExtensionManifestError {
    fn into_response(self) -> axum::response::Response {
        match self {
            FetchExtensionManifestError::NotFound => (
                axum::http::StatusCode::NOT_FOUND,
                "GitHub API returned 404. Does the user and repo exist?".to_string(),
            )
                .into_response(),
            FetchExtensionManifestError::InvalidUrl(e) => {
                (axum::http::StatusCode::BAD_REQUEST, e).into_response()
            }
            FetchExtensionManifestError::Other(status_code, e) => (
                axum::http::StatusCode::from_u16(status_code)
                    .unwrap_or(axum::http::StatusCode::BAD_GATEWAY),
                e,
            )
                .into_response(),
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

#[derive(serde::Serialize)]
struct FetchManifestRet {
    manifest: extension::Manifest,
    /// GitHub username
    username: String,
    is_domain_true: bool,
}

async fn fetch_extension_manifest(
    State(state): State<AppState>,
    _requester: AuthedUser,
    Query(body): Query<ExtensionRequestBody>,
) -> Response {
    if let Err(e) = require_code_execution(&state) {
        return e.into_response();
    }
    match extension::get_manifest(&body.url).await {
        Ok(manifest) => Json(FetchManifestRet {
            manifest: manifest.manifest,
            username: manifest.username,
            is_domain_true: manifest.domain == body.url,
        })
        .into_response(),
        Err(e) => e.into_response(),
    }
}

async fn install_extension(
    State(state): State<AppState>,
    requester: AuthedUser,
    Json(body): Json<ExtensionRequestBody>,
) -> Result<(), Error> {
    requester
        .require(&state, UserAction::InstallExtension)
        .await?;
    require_code_execution(&state)?;
    let path = lodestone_path().join("extensions");
    tokio::fs::create_dir_all(&path)
        .await
        .context("Failed to create extensions directory")?;
    let manager = extension::ExtensionManager::new(path);
    manager.install_extension(&body.url).await?;
    Ok(())
}

pub fn get_extension_routes(state: AppState) -> Router {
    Router::new()
        .route("/extension/gitstatus", get(is_git_installed))
        .route("/extension/fetchmanifest", get(fetch_extension_manifest))
        .route("/extension/install", put(install_extension))
        .route_layer(middleware::from_fn_with_state(state.clone(), require_owner))
        .with_state(state)
}
