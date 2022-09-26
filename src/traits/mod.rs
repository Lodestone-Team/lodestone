use axum::response::IntoResponse;

use axum::http::StatusCode;
use serde::Serialize;
use serde_json::json;

use self::{t_configurable::TConfigurable, t_macro::TMacro, t_player::TPlayerManagement, t_resource::TResourceManagement, t_server::TServer};

pub mod t_server;
pub mod t_configurable;
pub mod t_player;
pub mod t_resource;
pub mod t_macro;

pub type MaybeUnsupported<T> = Option<T>;
pub use core::option::Option::None as Unsupported;
pub use core::option::Option::Some as Supported;


#[derive(Debug, Serialize)]
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
#[derive(Debug, Serialize)]
pub struct Error {
    pub inner : ErrorInner,
    pub detail : String
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self.inner {
            ErrorInner::MalformedRequest => (StatusCode::BAD_REQUEST, json!(self).to_string()),
            ErrorInner::PermissionDenied => (StatusCode::FORBIDDEN, json!(self).to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, json!(self).to_string())
        };
        (status, error_message).into_response()
    }
}

pub trait TInstance : TConfigurable + TMacro + TPlayerManagement + TResourceManagement + TServer + Sync + Send {

}
