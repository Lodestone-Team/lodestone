use std::fmt::{Display, Formatter};

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use color_eyre::Report;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{error, warn};
use ts_rs::TS;

#[derive(Debug, Clone, Deserialize, Serialize, TS)]
#[ts(export)]
pub enum ErrorKind {
    NotFound,
    UnsupportedOperation,
    BadRequest,
    PermissionDenied,
    Unauthorized,
    // too many attempts in a short time, such as failed logins (HTTP 429)
    TooManyRequests,
    // the request conflicts with the current state, such as a file that already exists (HTTP 409)
    Conflict,
    External,
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
        error!(
            "An error occurred ({kind}): {source}",
            kind = self.kind,
            source = self.source
        );
        self
    }
}

impl Error {
    pub fn ts_syntax_error(context: &str) -> Error {
        Error {
            kind: ErrorKind::Internal,
            source: Report::msg(format!("Syntax error parsing ts ({context})")),
        }
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
            ErrorKind::TooManyRequests => write!(f, "Too Many Requests"),
            ErrorKind::Conflict => write!(f, "Conflict"),
            ErrorKind::Internal => write!(f, "Internal Error"),
            ErrorKind::External => write!(f, "External Error"),
        }
    }
}

/// The body sent to clients: `{"kind": ..., "causes": [<top-level message>]}`.
///
/// Only the top-level message is included. The causes below it can hold absolute paths and
/// I/O or database details, so they are only logged (see `IntoResponse`).
impl Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut state = serializer.serialize_struct("Error", 2)?;
        state.serialize_field("kind", &self.kind)?;
        state.serialize_field("causes", &[self.source.to_string()])?;
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

#[test]
fn test_error_serialization_hides_inner_causes() {
    use color_eyre::eyre::WrapErr;
    let source = Err::<(), _>(std::io::Error::other("/home/someone/secret/path"))
        .wrap_err("Failed to read file")
        .unwrap_err();
    let error = Error {
        kind: ErrorKind::Internal,
        source,
    };
    let json = serde_json::to_string(&error).unwrap();
    assert_eq!(
        json,
        r#"{"kind":"Internal","causes":["Failed to read file"]}"#
    );
}

impl IntoResponse for Error {
    fn into_response(self) -> axum::response::Response {
        let status = match self.kind {
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::UnsupportedOperation => StatusCode::NOT_IMPLEMENTED,
            ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ErrorKind::PermissionDenied => StatusCode::FORBIDDEN,
            ErrorKind::Unauthorized => StatusCode::UNAUTHORIZED,
            ErrorKind::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            ErrorKind::Conflict => StatusCode::CONFLICT,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
            ErrorKind::External => StatusCode::BAD_GATEWAY,
        };
        // the client only gets the top-level message, so log the whole chain here
        if status.is_server_error() {
            error!("Request failed ({}): {:#}", self.kind, self.source);
        } else {
            warn!("Request failed ({}): {:#}", self.kind, self.source);
        }
        (status, Json(self)).into_response()
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
