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

#[derive(Debug, Serialize, Clone, TS)]
#[ts(export)]
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
    MacroNotFound,

    // Process errors:
    FailedToExecute,
    FailedToAcquireStdin,
    FailedToAcquireStdout,
    FailedToAcquireStderr,

    // API changed
    APIChanged,

    // Unsupported Op
    UnsupportedOperation,

    // Malformed request
    MalformedRequest,

    // User errors:
    UserNotFound,
    UserAlreadyExists,
    Unauthorized,
    PermissionDenied,
}
#[derive(Debug, Serialize, Clone, TS)]
#[serde(rename = "ClientError")]
#[ts(export)]
pub struct Error {
    pub inner: ErrorInner,
    pub detail: String,
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self.inner {
            ErrorInner::MalformedRequest => (StatusCode::BAD_REQUEST, json!(self).to_string()),
            ErrorInner::PermissionDenied => (StatusCode::FORBIDDEN, json!(self).to_string()),
            ErrorInner::Unauthorized => (StatusCode::UNAUTHORIZED, json!(self).to_string()),
            ErrorInner::FileOrDirNotFound => (StatusCode::NOT_FOUND, json!(self).to_string()),
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
    pub cmd_args: Vec<String>,
    pub description: String,
    pub port: u32,
    pub min_ram: Option<u32>,
    pub max_ram: Option<u32>,
    pub creation_time: i64,
    pub path: String,
    pub auto_start: bool,
    pub restart_on_crash: bool,
    pub timeout_last_left: Option<u32>,
    pub timeout_no_activity: Option<u32>,
    pub start_on_connection: bool,
    pub backup_period: Option<u32>,
    pub state: State,
    pub player_count: Option<u32>,
    pub max_player_count: Option<u32>,
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
            cmd_args: self.cmd_args().await,
            description: self.description().await,
            port: self.port().await,
            min_ram: self.min_ram().await,
            max_ram: self.max_ram().await,
            creation_time: self.creation_time().await,
            path: self.path().await.display().to_string(),
            auto_start: self.auto_start().await,
            restart_on_crash: self.restart_on_crash().await,
            timeout_last_left: self.timeout_last_left().await.unwrap_or(None),
            timeout_no_activity: self.timeout_no_activity().await.unwrap_or(None),
            start_on_connection: self.start_on_connection().await,
            backup_period: self.backup_period().await.unwrap_or(None),
            state: self.state().await,
            player_count: self.get_player_count().await,
            max_player_count: self.get_max_player_count().await,
        }
    }
}
