use async_trait::async_trait;
use axum::response::IntoResponse;

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use serde_json::json;
use ts_rs::TS;

use self::t_manifest::TManifest;
use self::t_server::State;
use self::{
    t_configurable::TConfigurable, t_macro::TMacro, t_player::TPlayerManagement,
    t_resource::TResourceManagement, t_server::TServer,
};

pub mod t_configurable;
pub mod t_macro;
pub mod t_manifest;
pub mod t_player;
pub mod t_resource;
pub mod t_server;

pub type MaybeUnsupported<T> = Option<T>;
pub use core::option::Option::None as Unsupported;
pub use core::option::Option::Some as Supported;

#[derive(Debug, Serialize, Clone)]
pub enum ErrorInner {
    // IO errors:
    FailedToReadFileOrDir,
    FailedToWriteFileOrDir,
    FailedToCreateFileOrDir,
    FailedToRemoveFileOrDir,
    FileOrDirNotFound,
    FiledOrDirAlreadyExists,

    // Stdin/stdout errors:
    FailedToWriteStdin,
    FailedToReadStdout,
    StdinNotOpen,
    StdoutNotOpen,
    FailedToAcquireLock,

    // Network errors:
    FailedToUpload,
    FailedToDownload,

    // Instance operation errors
    InstanceStarted,
    InstanceStopped,
    InstanceStarting,
    InstanceStopping,
    InstanceErrored,
    InstanceNotFound,
    PortInUse,

    // Config file errors:
    MalformedFile,
    FieldNotFound,
    ValueNotFound,
    TypeMismatch,

    // version string errors:
    MalformedVersionString,
    VersionNotFound,

    // Macro errors:
    FailedToRun,

    // Process errors:
    FailedToExecute,
    FailedToAcquireStdin,
    FailedToAcquireStdout,

    // API changed
    APIChanged,

    // Unsupported Op
    UnsupportedOperation,

    // Malformed request
    MalformedRequest,

    // User errors:
    UserNotFound,
    UserAlreadyExists,
    InvalidPassword,
    PermissionDenied,
}
#[derive(Debug, Serialize, Clone)]
pub struct Error {
    pub inner: ErrorInner,
    pub detail: String,
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self.inner {
            ErrorInner::MalformedRequest => (StatusCode::BAD_REQUEST, json!(self).to_string()),
            ErrorInner::PermissionDenied => (StatusCode::FORBIDDEN, json!(self).to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, json!(self).to_string()),
        };
        (status, error_message).into_response()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, TS)]
#[ts(export)]
pub struct InstanceInfo {
    pub uuid: String,
    pub name: String,
    pub flavour: String,
    pub game_type: String,
    pub description: String,
    pub port: u32,
    pub state: State,
    pub player_count: Option<u32>,
    pub max_player_count: Option<u32>,
    pub creation_time: i64,
    pub path: String,
}

#[async_trait]
pub trait TInstance:
    TConfigurable + TMacro + TPlayerManagement + TResourceManagement + TServer + TManifest + Sync + Send
{
    async fn get_instance_info(&self) -> InstanceInfo {
        InstanceInfo {
            uuid: self.uuid().await,
            name: self.name().await,
            flavour: self.flavour().await,
            game_type: self.game_type().await,
            description: self.description().await,
            port: self.port().await,
            state: self.state().await,
            player_count: self.get_player_count().await,
            max_player_count: self.get_max_player_count().await,
            creation_time: self.creation_time().await,
            path: self.path().await.display().to_string(),
        }
    }
}
