use axum::{
    body::Bytes,
    extract::{Multipart, Path},
    routing::{delete, get, put},
    Extension, Json, Router,
};
use axum_auth::AuthBearer;
use headers::HeaderMap;
use log::debug;
use reqwest::header::CONTENT_LENGTH;
use tokio::io::AsyncWriteExt;
use walkdir::WalkDir;

use crate::{
    auth::user::UserAction,
    events::{
        new_fs_event, CausedBy, Event, EventInner, FSOperation, FSTarget, ProgressionEvent,
        ProgressionEventInner,
    },
    traits::{t_configurable::TConfigurable, Error, ErrorInner},
    types::{InstanceUuid, Snowflake},
    util::{list_dir, rand_alphanumeric, scoped_join_win_safe},
    AppState,
};

// list of protected file extension that cannot be modified
static PROTECTED_EXTENSIONS: [&str; 10] = [
    "jar",
    "lua",
    "sh",
    "exe",
    "bat",
    "cmd",
    "msi",
    "lodestone_config",
    "out",
    "inf",
];

fn is_file_protected(path: impl AsRef<std::path::Path>) -> bool {
    let path = path.as_ref();
    if let Some(ext) = path.extension() {
        PROTECTED_EXTENSIONS.contains(&ext.to_str().unwrap())
    } else {
        true
    }
}

use super::{global_fs::File, util::decode_base64};

async fn list_instance_files(
    Extension(state): Extension<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<Vec<File>>, Error> {
    let relative_path = decode_base64(&base64_relative_path).ok_or(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Relative path is not valid urlsafe base64".to_string(),
    })?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::ReadInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    let root = instance.path().await;
    drop(instances);
    let path = scoped_join_win_safe(&root, relative_path)?;
    if !path.exists() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "File not found".to_string(),
        });
    }
    if !path.is_dir() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "Path is not a directory".to_string(),
        });
    }
    let ret: Vec<File> = list_dir(&path, None)
        .await?
        .iter()
        .map(move |p| {
            // remove the root path from the file path
            let mut r: File = p.as_path().into();
            r.path = p.strip_prefix(&root).unwrap().to_str().unwrap().to_string();
            r
        })
        .collect();
    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username,
    };
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Read,
        FSTarget::Directory(path),
        caused_by,
    ));
    Ok(Json(ret))
}

async fn read_instance_file(
    Extension(state): Extension<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<String, Error> {
    let relative_path = decode_base64(&base64_relative_path).ok_or(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Relative path is not valid urlsafe base64".to_string(),
    })?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::ReadInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    let root = instance.path().await;
    drop(instances);
    let path = scoped_join_win_safe(root, relative_path)?;
    if !path.exists() || !path.is_file() {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Path is not a file".to_string(),
        });
    }
    let ret = tokio::fs::read_to_string(&path).await.map_err(|_| Error {
        inner: ErrorInner::MalformedFile,
        detail: "Only text file encoded in UTF-8 is supported.".to_string(),
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

async fn write_instance_file(
    Extension(state): Extension<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    body: Bytes,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path).ok_or(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Relative path is not valid urlsafe base64".to_string(),
    })?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::WriteInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    let root = instance.path().await;
    drop(instances);
    let path = scoped_join_win_safe(root, relative_path)?;
    // if target has a protected extension, or no extension, deny
    if !requester.can_perform_action(&UserAction::WriteGlobalFile) && is_file_protected(&path) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: format!(
                "File extension {} is protected",
                path.extension()
                    .map(|s| s.to_str().unwrap())
                    .unwrap_or("none")
            ),
        });
    }
    // create the file if it doesn't exist
    tokio::fs::write(&path, body).await.map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Failed to write file".to_string(),
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

async fn make_instance_directory(
    Extension(state): Extension<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path).ok_or(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Relative path is not valid urlsafe base64".to_string(),
    })?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::WriteInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    let root = instance.path().await;
    drop(instances);
    let path = scoped_join_win_safe(root, relative_path)?;
    // create the file if it doesn't exist
    tokio::fs::create_dir_all(&path).await.map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Failed to create directory".to_string(),
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

async fn remove_instance_file(
    Extension(state): Extension<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path).ok_or(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Relative path is not valid urlsafe base64".to_string(),
    })?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::WriteInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    let root = instance.path().await;
    drop(instances);
    let path = scoped_join_win_safe(root, relative_path)?;
    // if target has a protected extension, or no extension, deny
    if !requester.can_perform_action(&UserAction::WriteGlobalFile) && is_file_protected(&path) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: format!(
                "File extension {} is protected",
                path.extension()
                    .map(|s| s.to_str().unwrap())
                    .unwrap_or("none")
            ),
        });
    }
    if !path.exists() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "Path does not exist".to_string(),
        });
    }
    if path.is_file() {
        tokio::fs::remove_file(&path).await.map_err(|_| Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Failed to remove file".to_string(),
        })?;
    } else {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Path is not a file".to_string(),
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

async fn remove_instance_dir(
    Extension(state): Extension<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path).ok_or(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Relative path is not valid urlsafe base64".to_string(),
    })?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::WriteInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    let root = instance.path().await;
    drop(instances);
    let path = scoped_join_win_safe(&root, relative_path)?;
    if path == root {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Cannot delete instance root".to_string(),
        });
    }
    // if target has a protected extension, or no extension, deny
    if !requester.can_perform_action(&UserAction::WriteGlobalFile) && is_file_protected(&path) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: format!(
                "File extension {} is protected",
                path.extension()
                    .map(|s| s.to_str().unwrap())
                    .unwrap_or("none")
            ),
        });
    }
    if !path.exists() {
        return Err(Error {
            inner: ErrorInner::FileOrDirNotFound,
            detail: "Path does not exist".to_string(),
        });
    }
    if path.is_dir() {
        if requester.can_perform_action(&UserAction::WriteGlobalFile) {
            tokio::fs::remove_dir_all(&path).await.map_err(|_| Error {
                inner: ErrorInner::MalformedRequest,
                detail: "Failed to remove directory".to_string(),
            })?;
        } else {
            // recursively access all files in the directory and check if they are protected
            for entry in WalkDir::new(path.clone()) {
                let entry = entry.map_err(|_| Error {
                    inner: ErrorInner::MalformedRequest,
                    detail: "Failed to read directory while scanning for protected files"
                        .to_string(),
                })?;
                if entry.file_type().is_file() && is_file_protected(entry.path()) {
                    return Err(Error {
                        inner: ErrorInner::PermissionDenied,
                        detail: format!(
                            "File extension {} is protected",
                            entry
                                .path()
                                .extension()
                                .map(|s| s.to_str().unwrap())
                                .unwrap_or("none")
                        ),
                    });
                }
            }
            tokio::fs::remove_dir_all(&path).await.map_err(|_| Error {
                inner: ErrorInner::MalformedRequest,
                detail: "Failed to remove directory".to_string(),
            })?;
        }
    } else {
        return Err(Error {
            inner: ErrorInner::MalformedRequest,
            detail: "Path is not a directory".to_string(),
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

async fn new_instance_file(
    Extension(state): Extension<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path).ok_or(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Relative path is not valid urlsafe base64".to_string(),
    })?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::WriteInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    let root = instance.path().await;
    drop(instances);
    let path = scoped_join_win_safe(root, relative_path)?;
    // if target has a protected extension, or no extension, deny
    if !requester.can_perform_action(&UserAction::WriteGlobalFile) && is_file_protected(&path) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: format!(
                "File extension {} is protected",
                path.extension()
                    .map(|s| s.to_str().unwrap())
                    .unwrap_or("none")
            ),
        });
    }
    if path.exists() {
        return Err(Error {
            inner: ErrorInner::FiledOrDirAlreadyExists,
            detail: "Path already exists".to_string(),
        });
    }
    tokio::fs::File::create(&path).await.map_err(|_| Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Failed to create file".to_string(),
    })?;

    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username,
    };
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Create,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(Json(()))
}

async fn download_instance_file(
    Extension(state): Extension<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    AuthBearer(token): AuthBearer,
) -> Result<String, Error> {
    let relative_path = decode_base64(&base64_relative_path).ok_or(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Relative path is not valid urlsafe base64".to_string(),
    })?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::ReadInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to access instance files".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    let root = instance.path().await;
    drop(instances);
    let path = scoped_join_win_safe(&root, relative_path)?;
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

    state.download_urls.lock().await.get(&key).unwrap();

    let caused_by = CausedBy::User {
        user_id: requester.uid,
        user_name: requester.username,
    };
    let _ = state.event_broadcaster.send(new_fs_event(
        FSOperation::Download,
        FSTarget::File(path),
        caused_by,
    ));
    Ok(key)
}

async fn upload_instance_file(
    Extension(state): Extension<AppState>,
    Path((uuid, base64_relative_path)): Path<(InstanceUuid, String)>,
    headers: HeaderMap,
    AuthBearer(token): AuthBearer,
    mut multipart: Multipart,
) -> Result<Json<()>, Error> {
    let relative_path = decode_base64(&base64_relative_path).ok_or(Error {
        inner: ErrorInner::MalformedRequest,
        detail: "Relative path is not valid urlsafe base64".to_string(),
    })?;
    let requester = state
        .users_manager
        .read()
        .await
        .try_auth(&token)
        .ok_or(Error {
            inner: ErrorInner::Unauthorized,
            detail: "Token error".to_string(),
        })?;
    if !requester.can_perform_action(&UserAction::WriteInstanceFile(uuid.clone())) {
        return Err(Error {
            inner: ErrorInner::PermissionDenied,
            detail: "Not authorized to write instance files".to_string(),
        });
    }
    let instances = state.instances.lock().await;
    let instance = instances.get(&uuid).ok_or(Error {
        inner: ErrorInner::InstanceNotFound,
        detail: "".to_string(),
    })?;
    let root = instance.path().await;
    drop(instances);
    let path_to_dir = scoped_join_win_safe(&root, relative_path)?;
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

    let event_id = Snowflake::default();
    let total = headers
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<f64>().ok());
    let _ = state.event_broadcaster.send(Event {
        event_inner: EventInner::ProgressionEvent(ProgressionEvent {
            event_id,
            progression_event_inner: ProgressionEventInner::ProgressionStart {
                progression_name: "Uploading files".to_string(),
                producer_id: None,
                total,
                inner: None,
            },
        }),
        details: "".to_string(),
        snowflake: Snowflake::default(),
        caused_by: CausedBy::User {
            user_id: requester.uid.clone(),
            user_name: requester.username.clone(),
        },
    });
    while let Ok(Some(mut field)) = multipart.next_field().await {
        let name = field.file_name().ok_or_else(|| Error {
            inner: ErrorInner::MalformedRequest,
            detail: "No file name".to_string(),
        })?;
        let name = sanitize_filename::sanitize(name);
        let path = scoped_join_win_safe(&path_to_dir, &name)?;
        // if the file has a protected extension, or no extension, deny
        if !requester.can_perform_action(&UserAction::WriteGlobalFile) && is_file_protected(&path) {
            return Err(Error {
                inner: ErrorInner::PermissionDenied,
                detail: format!(
                    "File extension {} is protected",
                    path.extension()
                        .map(|s| s.to_str().unwrap())
                        .unwrap_or("none")
                ),
            });
        }
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

        while let Some(chunk) = field.chunk().await.map_err(|e| {
            std::fs::remove_file(&path).ok();
            let _ = state.event_broadcaster.send(Event {
                event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                    event_id,
                    progression_event_inner: ProgressionEventInner::ProgressionEnd {
                        success: false,
                        message: Some(e.to_string()),
                        inner: None,
                    },
                }),
                details: "".to_string(),
                snowflake: Snowflake::default(),
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
                    event_id,
                    progression_event_inner: ProgressionEventInner::ProgressionUpdate {
                        progress_message: format!("Uploading {}", name),
                        progress: chunk.len() as f64,
                    },
                }),
                details: "".to_string(),
                snowflake: Snowflake::default(),
                caused_by: CausedBy::User {
                    user_id: requester.uid.clone(),
                    user_name: requester.username.clone(),
                },
            });
            debug!("Received chunk of size {}", chunk.len());
            file.write_all(&chunk).await.map_err(|e| {
                std::fs::remove_file(&path).ok();
                let _ = state.event_broadcaster.send(Event {
                    event_inner: EventInner::ProgressionEvent(ProgressionEvent {
                        event_id,
                        progression_event_inner: ProgressionEventInner::ProgressionEnd {
                            success: false,
                            message: Some(e.to_string()),
                            inner: None,
                        },
                    }),
                    details: "".to_string(),
                    snowflake: Snowflake::default(),
                    caused_by: CausedBy::User {
                        user_id: requester.uid.clone(),
                        user_name: requester.username.clone(),
                    },
                });
                Error {
                    inner: ErrorInner::FailedToCreateFileOrDir,
                    detail: "Failed to write to file".to_string(),
                }
            })?;
        }

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
    let _ = state.event_broadcaster.send(Event {
        event_inner: EventInner::ProgressionEvent(ProgressionEvent {
            event_id,
            progression_event_inner: ProgressionEventInner::ProgressionEnd {
                success: true,
                message: Some("Upload complete".to_string()),
                inner: None,
            },
        }),
        details: "".to_string(),
        snowflake: Snowflake::default(),
        caused_by: CausedBy::User {
            user_id: requester.uid.clone(),
            user_name: requester.username.clone(),
        },
    });
    Ok(Json(()))
}

pub fn get_instance_fs_routes() -> Router {
    Router::new()
        .route(
            "/instance/:uuid/fs/:base64_relative_path/ls",
            get(list_instance_files),
        )
        .route(
            "/instance/:uuid/fs/:base64_relative_path/read",
            get(read_instance_file),
        )
        .route(
            "/instance/:uuid/fs/:base64_relative_path/write",
            put(write_instance_file),
        )
        .route(
            "/instance/:uuid/fs/:base64_relative_path/mkdir",
            put(make_instance_directory),
        )
        .route(
            "/instance/:uuid/fs/:base64_relative_path/rm",
            delete(remove_instance_file),
        )
        .route(
            "/instance/:uuid/fs/:base64_relative_path/rmdir",
            delete(remove_instance_dir),
        )
        .route(
            "/instance/:uuid/fs/:base64_relative_path/new",
            put(new_instance_file),
        )
        .route(
            "/instance/:uuid/fs/:base64_relative_path/download",
            get(download_instance_file),
        )
        .route(
            "/instance/:uuid/fs/:base64_relative_path/upload",
            put(upload_instance_file),
        )
}
