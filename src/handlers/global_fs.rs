use std::path::PathBuf;

use axum::{
    body::Bytes,
    extract::Path,
    routing::{delete, get, post},
    Extension, Json, Router,
};
use axum_auth::AuthBearer;

use serde::{Deserialize, Serialize};

use ts_rs::TS;

use crate::{
    auth::user::UserAction,
    traits::{Error, ErrorInner},
    util::list_dir,
    AppState,
};

use super::util::try_auth;

#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum FileType {
    File,
    Directory,
    Unknown,
}
#[derive(Debug, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct File {
    pub name: String,
    pub path: String,
    pub creation_time: Option<u64>,
    pub modification_time: Option<u64>,
    pub file_type: FileType,
}

impl From<&std::path::Path> for File {
    fn from(path: &std::path::Path) -> Self {
        let file_type = if path.is_dir() {
            FileType::Directory
        } else if path.is_file() {
            FileType::File
        } else {
            FileType::Unknown
        };
        Self {
            name: path.file_name().unwrap().to_str().unwrap().to_string(),
            path: path.to_str().unwrap().to_string(),
            // unix timestamp
            // if we cant get the time, return none
            creation_time: path
                .metadata()
                .ok()
                .and_then(|m| m.created().ok())
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()),
            modification_time: path
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()),

            file_type,
        }
    }
}

async fn list_files(
    Extension(state): Extension<AppState>,
    Path(absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<File>>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ReadGlobalFile) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access global files".to_string(),
        });
    }
    drop(users);

    let path = PathBuf::from(absolute_path);
    if !path.exists() || !path.is_dir() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "Path is not a directory".to_string(),
        });
    }
    Ok(Json(
        list_dir(&path, None)
            .await?
            .iter()
            .map(|p| {
                let r: File = p.as_path().into();
                r
            })
            .collect(),
    ))
}

async fn read_file(
    Extension(state): Extension<AppState>,
    Path(absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<String>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ReadGlobalFile) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access global files".to_string(),
        });
    }
    drop(users);

    let path = PathBuf::from(absolute_path);
    if !path.exists() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "File not found".to_string(),
        });
    }
    Ok(Json(tokio::fs::read_to_string(&path).await.map_err(
        |_| Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "File must be UTF8 encoded".to_string(),
        },
    )?))
}

async fn write_file(
    Extension(state): Extension<AppState>,
    Path(absolute_path): Path<String>,
    body: Bytes,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::WriteGlobalFile) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access global files".to_string(),
        });
    }
    drop(users);

    let path = PathBuf::from(absolute_path);
    if !path.exists() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "File not found".to_string(),
        });
    }
    tokio::fs::write(path, body).await.map_err(|e| Error {
        inner: ErrorInner::MalformedRequest,
        detail: format!("Error writing file: {}", e),
    })?;
    Ok(Json(()))
}

async fn make_directory(
    Extension(state): Extension<AppState>,
    Path(absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::WriteGlobalFile) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access global files".to_string(),
        });
    }
    drop(users);

    let path = PathBuf::from(absolute_path);
    if path.exists() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "File or directory already exists".to_string(),
        });
    }
    tokio::fs::create_dir(path).await.map_err(|e| Error {
        inner: ErrorInner::MalformedRequest,
        detail: format!("Failed to create directory: {}", e),
    })?;
    Ok(Json(()))
}

async fn remove_file(
    Extension(state): Extension<AppState>,
    Path(absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::WriteGlobalFile) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access global files".to_string(),
        });
    }
    drop(users);

    let path = PathBuf::from(absolute_path);
    if !path.exists() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "File or directory not found".to_string(),
        });
    }
    tokio::fs::remove_file(path).await.map_err(|e| Error {
        inner: ErrorInner::MalformedRequest,
        detail: format!("Failed to remove file: {}", e),
    })?;
    Ok(Json(()))
}

pub fn get_global_fs_routes() -> Router {
    Router::new()
        .route("/fs/ls/*absolute_path", get(list_files))
        .route("/fs/read/*absolute_path", get(read_file))
        .route("/fs/write/*absolute_path", post(write_file))
        .route("/fs/mkdir/*absolute_path", post(make_directory))
        .route("/fs/rm/*absolute_path", delete(remove_file))
}
