use std::path::PathBuf;

use axum::{
    body::Bytes,
    extract::Path,
    routing::{delete, get, post},
    Extension, Json, Router,
};
use axum_auth::AuthBearer;
use safe_path::scoped_join;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use ts_rs::TS;

use crate::{
    auth::user::UserAction,
    traits::{Error, ErrorInner},
    util::list_dir,
    AppState,
};

// list of protected file extension that cannot be modified
static PROTECTED_EXTENSIONS: [&str; 8] = ["jar", "lua", "sh", "exe", "bat", "cmd", "msi", ".lodestone_config"];

fn is_file_protected(path: &std::path::Path) -> bool {
    if let Some(ext) = path.extension() {
        PROTECTED_EXTENSIONS.contains(&ext.to_str().unwrap())
    } else {
        false
    }
}

use super::{
    global_fs::{File, FileType},
    util::try_auth,
};

async fn list_instance_files(
    Extension(state): Extension<AppState>,
    Path((uuid, relative_path)): Path<(String, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<File>>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ReadInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    drop(users);
    let instances = state.instances.lock().await;
    let instance = instances
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await;
    let root = instance.path().await;
    drop(instance);
    drop(instances);
    let path = scoped_join(&root, relative_path).map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Invalid path".to_string(),
    })?;
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
            .map({
                let root = root.clone();
                move |p| {
                // remove the root path from the file path
                let path = p.strip_prefix(&root).unwrap();
                let r: File = path.into();
                r
            }})
            .collect(),
    ))
}

async fn read_instance_file(
    Extension(state): Extension<AppState>,
    Path((uuid, relative_path)): Path<(String, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<String, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::ReadInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    drop(users);
    let instances = state.instances.lock().await;
    let instance = instances
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await;
    let root = instance.path().await;
    drop(instance);
    drop(instances);
    let path = scoped_join(root, relative_path).map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Invalid path".to_string(),
    })?;
    if !path.exists() || !path.is_file() {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Path is not a file".to_string(),
        });
    }
    tokio::fs::read_to_string(path).await.map_err(|_| Error {
        inner: ErrorInner::MalformedFile,
        detail: "File is not UFT8 encoded".to_string(),
    })
}

async fn write_instance_file(
    Extension(state): Extension<AppState>,
    Path((uuid, relative_path)): Path<(String, String)>,
    body: Bytes,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::WriteInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    drop(users);
    let instances = state.instances.lock().await;
    let instance = instances
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await;
    let root = instance.path().await;
    drop(instance);
    drop(instances);
    let path = scoped_join(root, relative_path).map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Invalid path".to_string(),
    })?;
    // if target has a protected extension, or no extension, deny
    if is_file_protected(&path) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Cannot modify protected file".to_string(),
        });
    }
    // create the file if it doesn't exist
    tokio::fs::write(path, body).await.map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Failed to write file".to_string(),
    })?;
    Ok(Json(()))
}

async fn make_instance_directory(
    Extension(state): Extension<AppState>,
    Path((uuid, relative_path)): Path<(String, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::WriteInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    drop(users);
    let instances = state.instances.lock().await;
    let instance = instances
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await;
    let root = instance.path().await;
    drop(instance);
    drop(instances);
    let path = scoped_join(root, relative_path).map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Invalid path".to_string(),
    })?;
    // create the file if it doesn't exist
    tokio::fs::create_dir_all(path).await.map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Failed to create directory".to_string(),
    })?;
    Ok(Json(()))
}

async fn remove_instance_file(
    Extension(state): Extension<AppState>,
    Path((uuid, relative_path)): Path<(String, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let users = state.users.lock().await;
    let requester = try_auth(&token, users.get_ref()).ok_or(Error {
        inner: ErrorInner::Unauthorized,
        detail: "Token error".to_string(),
    })?;
    if !requester.can_perform_action(&UserAction::WriteInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    drop(users);
    let instances = state.instances.lock().await;
    let instance = instances
        .get(&uuid)
        .ok_or(Error {
            inner: ErrorInner::InstanceNotFound,
            detail: "".to_string(),
        })?
        .lock()
        .await;
    let root = instance.path().await;
    drop(instance);
    drop(instances);
    let path = scoped_join(root, relative_path).map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Invalid path".to_string(),
    })?;
    // if target has a protected extension, or no extension, deny
    if is_file_protected(&path) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Cannot modify protected file".to_string(),
        });
    }
    if !path.exists() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "Path does not exist".to_string(),
        });
    }
    if path.is_dir() {
        tokio::fs::remove_dir_all(path).await.map_err(|_| Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Failed to remove directory".to_string(),
        })?;
    } else {
        tokio::fs::remove_file(path).await.map_err(|_| Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Failed to remove file".to_string(),
        })?;
    }
    Ok(Json(()))
}

pub fn get_instance_fs_routes() -> Router {
    Router::new()
        .route(
            "/instance/:uuid/fs/ls/*relative_path",
            get(list_instance_files),
        )
        .route(
            "/instance/:uuid/fs/read/*relative_path",
            get(read_instance_file),
        )
        .route(
            "/instance/:uuid/fs/write/*relative_path}",
            post(write_instance_file),
        )
        .route(
            "/instance/:uuid/fs/mkdir/*relative_path}",
            post(make_instance_directory),
        )
        .route(
            "/instance/:uuid/fs/remove/*relative_path}",
            delete(remove_instance_file),
        )
}
