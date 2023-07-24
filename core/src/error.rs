use std::fmt::{Display, Formatter};

use axum::http::StatusCode;
use axum::response::IntoResponse;
use color_eyre::Report;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;
use ts_rs::TS;

use crate::error;

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
pub enum ErrorKind {
    NotFound,
    UnsupportedOperation,
    BadRequest,
    PermissionDenied,
    Unauthorized,
    Internal,
}

#[derive(Error, Debug)]
#[error("An error occurred ({kind}): {source}")]
pub struct Error {
    pub kind: ErrorKind,
    pub source: color_eyre::Report,
}


impl Error {
    pub fn log(self) -> Self {
        error!("An error occurred ({kind}): {source}", kind = self.kind, source = self.source);
        self
    }
}


impl Display for ErrorKind {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ErrorKind::NotFound => write!(f, "Not Found"),
            ErrorKind::UnsupportedOperation => write!(f, "Unsupported Operation"),
            ErrorKind::BadRequest => write!(f, "Bad Request"),
            ErrorKind::PermissionDenied => write!(f, "Permission Denied"),
            ErrorKind::Unauthorized => write!(f, "Unauthorized"),
            ErrorKind::Internal => write!(f, "Internal Error"),
        }
    }
}

impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Error", 2)?;
        state.serialize_field("kind", &self.kind)?;
        let vec: Vec<String> = self.source.chain().map(|cause| cause.to_string()).collect();
        state.serialize_field("causes", &vec)?;
        state.end()
    }
}

#[test]
fn test_error_serialization() {
    let error = Error {
        kind: ErrorKind::NotFound,
        source: Report::msg("Test"),
    };
    let json = serde_json::to_string(&error).unwrap();
    assert_eq!(json, r#"{"kind":"NotFound","causes":["Test"]}"#);
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status = match self.kind {
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::UnsupportedOperation => StatusCode::NOT_IMPLEMENTED,
            ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
            ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, json!(self).to_string()).into_response()
    }
}

impl From<Report> for Error {
    fn from(source: Report) -> Self {
        // try downcasting to a known error
        let kind = if let Some(io_error) = source.downcast_ref::<std::io::Error>() {
            // check if the error is a not found error
            if io_error.kind() == std::io::ErrorKind::NotFound {
                ErrorKind::NotFound
            } else {
                ErrorKind::Internal
            }
        } else {
            ErrorKind::Internal
        };

        Self { kind, source }
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self {
            kind,
            source: Report::msg("No source"),
        }
    }
}
