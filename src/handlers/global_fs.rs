use std::path::PathBuf;

use axum::{
    body::{Bytes, StreamBody},
    extract::{Multipart, Path},
    http,
    routing::{delete, get, put},
    Extension, Json, Router,
};
use axum_auth::AuthBearer;

use futures::{Future, Stream};
use headers::{HeaderMap, HeaderName};
use log::debug;
use reqwest::header::CONTENT_LENGTH;
use serde::{Deserialize, Serialize};

use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
use ts_rs::TS;

use crate::{
    auth::user::UserAction,
    events::{
        new_fs_event, CausedBy, Event, EventInner, FSOperation, FSTarget, ProgressionEvent,
        ProgressionEventInner, new_progression_event_id,
    },
    prelude::get_snowflake,
    traits::{Error, ErrorInner},
    util::{list_dir, rand_alphanumeric},
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
#[serde(rename = "ClientFile")]
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
    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username,
    };
    let ret: Vec<File> = list_dir(&path, None)
        .await?
        .iter()
        .map(|p| {
            let r: File = p.as_path().into();
            r
        })
        .collect();
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Read,
        FSTarget::Directory(path),
        caused_by,
    ));
    Ok(Json(ret))
}

async fn read_file(
    Extension(state): Extension<AppState>,
    Path(absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<String, Error> {
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
    let ret = tokio::fs::read_to_string(&path).await.map_err(|_| Error {
        inner: ErrorInner::FileOrDirNotFound,
        detail: "You may only view text files encoded in UTF-8.".to_string(),
    })?;
    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username,
    };
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Read,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(ret)
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
    tokio::fs::write(&path, body).await.map_err(|e| Error {
        inner: ErrorInner::MalformedRequest,
        detail: format!("Error writing file: {}", e),
    })?;

    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username,
    };
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Write,
        FSTarget::File(path),
        caused_by,
    ));
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
    tokio::fs::create_dir(&path).await.map_err(|e| Error {
        inner: ErrorInner::MalformedRequest,
        detail: format!("Failed to create directory: {}", e),
    })?;

    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username,
    };
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Create,
        FSTarget::Directory(path),
        caused_by,
    ));
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
    if path.is_file() {
        tokio::fs::remove_file(&path).await.map_err(|e| Error {
            inner: ErrorInner::MalformedRequest,
            detail: format!("Failed to remove file: {}", e),
        })?;
    } else {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Path is not a file.".to_string(),
        });
    }
    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username,
    };
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Delete,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(Json(()))
}

async fn remove_dir(
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
    if path.is_dir() {
        tokio::fs::remove_file(&path).await.map_err(|e| Error {
            inner: ErrorInner::MalformedRequest,
            detail: format!("Failed to remove dir: {}", e),
        })?;
    } else {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Path is not a directory.".to_string(),
        });
    }

    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username,
    };
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Delete,
        FSTarget::Directory(path),
        caused_by,
    ));

    Ok(Json(()))
}

async fn new_file(
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
            inner: ErrorInner::FiledOrDirAlreadyExists,
            detail: "File already exists.".to_string(),
        });
    }

    tokio::fs::File::create(&path).await.map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Failed to create file".to_string(),
    })?;

    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username.clone(),
    };
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Create,
        FSTarget::File(path),
        caused_by,
    ));

    Ok(Json(()))
}

async fn download_file(
    Extension(state): Extension<AppState>,
    Path(absolute_path): Path<String>,
    AuthBearer(token): AuthBearer,
) -> Result<String, Error> {
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
    if !path.is_file() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "Path is not a file".to_string(),
        });
    }
    let key = rand_alphanumeric(32);
    state
        .download_urls
        .lock()
        .await
        .insert(key.clone(), path.clone());
    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username.clone(),
    };
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Download,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(key)
}

async fn upload_file(
    Extension(state): Extension<AppState>,
    Path(absolute_path_to_dir): Path<String>,
    headers: HeaderMap,
    AuthBearer(token): AuthBearer,
    mut multipart: Multipart,
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

    let path_to_dir = PathBuf::from(absolute_path_to_dir);
    if path_to_dir.exists() && !path_to_dir.is_dir() {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Path is not a directory".to_string(),
        });
    }
    if !path_to_dir.exists() {
        tokio::fs::create_dir_all(&path_to_dir)
            .await
            .map_err(|_| Error {
                inner: ErrorInner::FailedToCreateFileOrDir,
                detail: "Failed to create directory".to_string(),
            })?;
    }

    let total = headers
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());

    while let Ok(Some(mut field)) = multipart.next_field().await {
        let name = field.file_name().ok_or_else(|| Error {
            inner: ErrorInner::MalformedRequest,
            detail: "No file name".to_string(),
        })?.to_owned();
        let path = path_to_dir.join(&name);
        let path = if path.exists() {
            // add a postfix to the file name
            let mut postfix = 1;
            // get the file name without the extension
            let file_name = path.file_stem().unwrap().to_str().unwrap().to_string();
            loop {
                let new_path = path.with_file_name(format!(
                    "{}_{}.{}",
                    file_name,
                    postfix,
                    path.extension().unwrap().to_str().unwrap()
                ));
                if !new_path.exists() {
                    break new_path;
                }
                postfix += 1;
            }
        } else {
            path
        };
        let mut file = tokio::fs::File::create(&path).await.map_err(|_| Error {
            inner: ErrorInner::FailedToCreateFileOrDir,
            detail: "Failed to create file".to_string(),
        })?;
        let event_id = new_progression_event_id();
        let _ = state.event_broadcaster.send(Event {
            event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                event_id: event_id.clone(),
                progression_event_inner: ProgressionEventInner::ProgressionStart {
                    progression_name: format!("Uploading {}", name),
                    producer_id: "".to_string(),
                    total,
                    inner: None,
                },
            }),
            details: "".to_string(),
            snowflake: get_snowflake(),
            caused_by: CausedBy::User {
                user_id: requester.uid.clone(),
                user_name: requester.username.clone(),
            },
        });
        while let Some(chunk) = field.chunk().await.map_err(|e| {
            std::fs::remove_file(&path).ok();
            let _ = state.event_broadcaster.send(Event {
                event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                    event_id: event_id.clone(),
                    progression_event_inner: ProgressionEventInner::ProgressionEnd {
                        success: false,
                        message: Some(e.to_string()),
                        inner: None,
                    },
                }),
                details: "".to_string(),
                snowflake: get_snowflake(),
                caused_by: CausedBy::User {
                    user_id: requester.uid.clone(),
                    user_name: requester.username.clone(),
                },
            });
            Error {
                inner: ErrorInner::MalformedRequest,
                detail: "Failed to read chunk".to_string(),
            }
        })? {
            let _ = state.event_broadcaster.send(Event {
                event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                    event_id: event_id.clone(),
                    progression_event_inner: ProgressionEventInner::ProgressionUpdate {
                        progress_message: format!("Uploading {}", name),
                        progress: chunk.len() as f64,
                    },
                }),
                details: "".to_string(),
                snowflake: get_snowflake(),
                caused_by: CausedBy::User {
                    user_id: requester.uid.clone(),
                    user_name: requester.username.clone(),
                },
            });
            debug!("Received chunk of size {}", chunk.len());
            file.write_all(&chunk).await.map_err(|_| {
                std::fs::remove_file(&path).ok();
                Error {
                    inner: ErrorInner::FailedToCreateFileOrDir,
                    detail: "Failed to write to file".to_string(),
                }
            })?;
        }
        let _ = state.event_broadcaster.send(Event {
            event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                event_id: event_id.clone(),
                progression_event_inner: ProgressionEventInner::ProgressionEnd {
                    success: true,
                    message: Some("Upload complete".to_string()),
                    inner: None,
                },
            }),
            details: "".to_string(),
            snowflake: get_snowflake(),
            caused_by: CausedBy::User {
                user_id: requester.uid.clone(),
                user_name: requester.username.clone(),
            },
        });
        let caused_by = CausedBy::User {
            user_id: requester.uid.clone(),
            user_name: requester.username.clone(),
        };
        let _ = state.event_broadcaster.send(new_fs_event(
            FSOperation::Upload,
            FSTarget::File(path),
            caused_by,
        ));
    }

    Ok(Json(()))
}

async fn download(
    Extension(state): Extension<AppState>,
    Path(key): Path<String>,
) -> Result<
    (
        [(HeaderName, String); 3],
        StreamBody<ReaderStream<tokio::fs::File>>,
    ),
    Error,
> {
    if let Some(path) = state.download_urls.lock().await.get(&key) {
        let file = tokio::fs::File::open(&path).await.map_err(|_| Error {
            inner: ErrorInner::IOError,
            detail: "Failed to open file".to_string(),
        })?;

        let headers = [
            (
                http::header::CONTENT_DISPOSITION,
                "application/octet-stream".to_string(),
            ),
            (
                http::header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"{}\"",
                    path.file_name()
                        .map(|s| s.to_str().map(|s| s.to_string()))
                        .flatten()
                        .unwrap_or("unknown".to_string())
                ),
            ),
            if let Some(metadata) = file.metadata().await.ok() {
                (http::header::CONTENT_LENGTH, metadata.len().to_string())
            } else {
                // if we can't get the file size, we just don't set the header
                // but the rust compiler enforces array length to be known at compile time
                // so we just set a dummy header
                (http::header::ACCEPT_LANGUAGE, "*".to_string())
            },
        ];
        let stream = ReaderStream::new(file);
        let body = StreamBody::new(stream);
        Ok((headers, body))
    } else {
        Err(Error {
            inner: ErrorInner::NotFound,
            detail: "File not found with the given key".to_string(),
        })
    }
}

pub fn get_global_fs_routes() -> Router {
    Router::new()
        .route("/fs/ls/*absolute_path", get(list_files))
        .route("/fs/read/*absolute_path", get(read_file))
        .route("/fs/write/*absolute_path", put(write_file))
        .route("/fs/mkdir/*absolute_path", put(make_directory))
        .route("/fs/rm/*absolute_path", delete(remove_file))
        .route("/fs/rmdir/*absolute_path", delete(remove_dir))
        .route("/fs/new/*absolute_path", put(new_file))
        .route("/fs/download/*absolute_path", get(download_file))
        .route("/fs/upload/*absolute_path", put(upload_file))
        .route("/file/:key", get(download))
}
