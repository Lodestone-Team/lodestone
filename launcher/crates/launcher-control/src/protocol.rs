//! The messages, one JSON object per line in each direction.
//!
//! A request:
//!
//! ```json
//! {"v":1,"id":1,"method":"instances.list","params":{}}
//! ```
//!
//! Its response has the same `id`, and either a `result` or an `error`:
//!
//! ```json
//! {"v":1,"id":1,"result":{"instances":[]}}
//! {"v":1,"id":1,"error":{"code":"not_found","message":"No instance is called \"x\"."}}
//! ```
//!
//! While a long request runs (with `"wait": true`), progress lines with its `id` may come
//! before the response:
//!
//! ```json
//! {"v":1,"id":7,"progress":{"job":3,"label":"Downloading","files_done":4,"files_total":9}}
//! ```
//!
//! After `events.subscribe`, events come with the subscription's number until the
//! connection closes or `events.unsubscribe`:
//!
//! ```json
//! {"v":1,"subscription":2,"event":"log","data":{"instance":"…","text":"…"}}
//! ```

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

/// The protocol version. A request with another `v` is refused with
/// [`codes::PROTOCOL_VERSION`].
pub const PROTOCOL_VERSION: u32 = 1;

/// Lines longer than this are refused, so a client can't make the launcher buffer
/// without end.
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

/// A request from a client.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Request {
    pub v: u32,
    /// Chosen by the client and returned unchanged: a number or a string.
    pub id: Value,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl Request {
    pub fn new(id: impl Into<Value>, method: impl Into<String>, params: Value) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            id: id.into(),
            method: method.into(),
            params,
        }
    }

    pub fn to_line(&self) -> String {
        let mut line = serde_json::to_string(self).expect("a request always serializes");
        line.push('\n');
        line
    }

    /// Reads a request line. A line that isn't a request is answered with the error,
    /// and the `id` when it could be read (else `null`).
    // The error is the answer to send, once per bad line; its size doesn't matter.
    #[allow(clippy::result_large_err)]
    pub fn parse(line: &str) -> Result<Request, (Value, ErrorObject)> {
        let value: Value = serde_json::from_str(line).map_err(|err| {
            (
                Value::Null,
                ErrorObject::new(codes::PARSE_ERROR, format!("The line isn't JSON: {err}")),
            )
        })?;
        let Value::Object(object) = value else {
            return Err((
                Value::Null,
                ErrorObject::new(codes::INVALID_REQUEST, "A request is a JSON object."),
            ));
        };
        let id = object.get("id").cloned().unwrap_or(Value::Null);
        if !matches!(id, Value::Number(_) | Value::String(_)) {
            return Err((
                Value::Null,
                ErrorObject::new(
                    codes::INVALID_REQUEST,
                    "A request needs an \"id\", a number or a string.",
                ),
            ));
        }
        match object.get("v").and_then(Value::as_u64) {
            Some(v) if v == u64::from(PROTOCOL_VERSION) => {}
            Some(v) => {
                return Err((
                    id,
                    ErrorObject::new(
                        codes::PROTOCOL_VERSION,
                        format!(
                            "This launcher speaks protocol version {PROTOCOL_VERSION}, not {v}. Use a lodestone-launcher-cli from the same release as the launcher."
                        ),
                    )
                    .with_data(json!({ "supported": PROTOCOL_VERSION })),
                ));
            }
            None => {
                return Err((
                    id,
                    ErrorObject::new(
                        codes::PROTOCOL_VERSION,
                        format!("A request needs \"v\": {PROTOCOL_VERSION}."),
                    ),
                ));
            }
        }
        let Some(method) = object.get("method").and_then(Value::as_str) else {
            return Err((
                id,
                ErrorObject::new(codes::INVALID_REQUEST, "A request needs a \"method\"."),
            ));
        };
        let params = object.get("params").cloned().unwrap_or(Value::Null);
        if !matches!(params, Value::Object(_) | Value::Null) {
            return Err((
                id,
                ErrorObject::new(codes::INVALID_PARAMS, "\"params\" is a JSON object."),
            ));
        }
        Ok(Request {
            v: PROTOCOL_VERSION,
            id,
            method: method.to_string(),
            params: if params.is_null() {
                Value::Object(Map::new())
            } else {
                params
            },
        })
    }
}

/// An error in a response: a stable `code` (see [`codes`]), a message for people, and
/// sometimes `data` with details for programs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ErrorObject {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl ErrorObject {
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }

    /// The CLI's exit code for this error.
    pub fn exit_code(&self) -> i32 {
        codes::exit_code(&self.code)
    }
}

impl std::fmt::Display for ErrorObject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ErrorObject {}

/// A line from the launcher.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Response {
        id: Value,
        result: Result<Value, ErrorObject>,
    },
    Progress {
        id: Value,
        progress: Value,
    },
    Event {
        subscription: u64,
        event: String,
        data: Value,
    },
}

impl Message {
    pub fn result(id: Value, result: Result<Value, ErrorObject>) -> Self {
        Message::Response { id, result }
    }

    pub fn to_value(&self) -> Value {
        match self {
            Message::Response {
                id,
                result: Ok(result),
            } => json!({ "v": PROTOCOL_VERSION, "id": id, "result": result }),
            Message::Response {
                id,
                result: Err(error),
            } => json!({ "v": PROTOCOL_VERSION, "id": id, "error": error }),
            Message::Progress { id, progress } => {
                json!({ "v": PROTOCOL_VERSION, "id": id, "progress": progress })
            }
            Message::Event {
                subscription,
                event,
                data,
            } => json!({
                "v": PROTOCOL_VERSION,
                "subscription": subscription,
                "event": event,
                "data": data,
            }),
        }
    }

    pub fn to_line(&self) -> String {
        let mut line = self.to_value().to_string();
        line.push('\n');
        line
    }

    /// Reads a line from the launcher.
    pub fn parse(line: &str) -> Result<Message, String> {
        let value: Value = serde_json::from_str(line)
            .map_err(|err| format!("The launcher sent bad JSON: {err}"))?;
        let Value::Object(mut object) = value else {
            return Err("The launcher sent something that isn't a JSON object.".into());
        };
        match object.get("v").and_then(Value::as_u64) {
            Some(v) if v == u64::from(PROTOCOL_VERSION) => {}
            v => {
                return Err(format!(
                    "The launcher speaks protocol version {}, and this CLI version {PROTOCOL_VERSION}. Use a lodestone-launcher-cli from the same release as the launcher.",
                    v.map_or("unknown".to_string(), |v| v.to_string())
                ));
            }
        }
        if let Some(subscription) = object.get("subscription").and_then(Value::as_u64) {
            let event = object
                .get("event")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let data = object.remove("data").unwrap_or(Value::Null);
            return Ok(Message::Event {
                subscription,
                event,
                data,
            });
        }
        let id = object.remove("id").unwrap_or(Value::Null);
        if let Some(progress) = object.remove("progress") {
            return Ok(Message::Progress { id, progress });
        }
        if let Some(error) = object.remove("error") {
            let error: ErrorObject = serde_json::from_value(error)
                .map_err(|err| format!("The launcher sent a bad error: {err}"))?;
            return Ok(Message::Response {
                id,
                result: Err(error),
            });
        }
        match object.remove("result") {
            Some(result) => Ok(Message::Response {
                id,
                result: Ok(result),
            }),
            None => Err("The launcher sent a line without a result, error or event.".into()),
        }
    }
}

/// The error codes, and the CLI's exit code for each.
///
/// Codes are stable: programs may match on them. Messages are for people and may
/// change.
pub mod codes {
    /// The line isn't JSON.
    pub const PARSE_ERROR: &str = "parse_error";
    /// The JSON isn't a request.
    pub const INVALID_REQUEST: &str = "invalid_request";
    /// The client speaks another protocol version.
    pub const PROTOCOL_VERSION: &str = "protocol_version";
    pub const METHOD_NOT_FOUND: &str = "method_not_found";
    /// A parameter is missing or wrong.
    pub const INVALID_PARAMS: &str = "invalid_params";
    /// No instance, account, world, core, server or job by that name.
    pub const NOT_FOUND: &str = "not_found";
    /// The name matches more than one thing; `data.matches` lists them.
    pub const AMBIGUOUS: &str = "ambiguous";
    /// A destructive method was called without `"confirm": true`.
    pub const CONFIRMATION_REQUIRED: &str = "confirmation_required";
    /// The core isn't trusted for mods: installing its mods needs the user's decision.
    /// `data.advisory` lists what would be installed.
    pub const UNTRUSTED_CORE: &str = "untrusted_core";
    /// The core's HTTPS certificate isn't trusted; `data.fingerprint` is what it
    /// presents, for the user to compare.
    pub const UNTRUSTED_CERTIFICATE: &str = "untrusted_certificate";
    /// The server's mods changed since the instance was made: update or not is the
    /// user's decision. `data.changes` lists them.
    pub const MODS_CHANGED: &str = "mods_changed";
    /// Hosting a server accepts Minecraft's EULA, which only the user can do.
    pub const EULA_REQUIRED: &str = "eula_required";
    /// The instance (or the launcher) is busy: the game runs, or a job does.
    pub const BUSY: &str = "busy";
    /// The change conflicts with what is there, such as a name in use.
    pub const CONFLICT: &str = "conflict";
    /// The feature is off or not available here (offline accounts, no Modrinth).
    pub const UNAVAILABLE: &str = "unavailable";
    /// Sign in first (to Minecraft, or to a core).
    pub const NOT_SIGNED_IN: &str = "not_signed_in";
    /// The connection is from another user.
    pub const PERMISSION_DENIED: &str = "permission_denied";
    /// The operation ran and failed; the message says why.
    pub const FAILED: &str = "failed";
    pub const CANCELLED: &str = "cancelled";
    pub const TIMEOUT: &str = "timeout";
    pub const INTERNAL: &str = "internal";
    /// The CLI couldn't reach a running launcher.
    pub const NOT_RUNNING: &str = "not_running";
    /// The command line is wrong.
    pub const USAGE: &str = "usage";
    /// The system has no Unix domain sockets (Windows before 10 1803).
    pub const UNSUPPORTED_PLATFORM: &str = "unsupported_platform";

    /// Every code with its exit code and meaning, for `lodestone-launcher-cli schema`.
    pub const ALL: &[(&str, i32, &str)] = &[
        (FAILED, 1, "The operation ran and failed."),
        (INTERNAL, 1, "A bug in the launcher."),
        (USAGE, 2, "The command line is wrong."),
        (PARSE_ERROR, 2, "The request line isn't JSON."),
        (INVALID_REQUEST, 2, "The JSON isn't a request."),
        (METHOD_NOT_FOUND, 2, "No such method."),
        (INVALID_PARAMS, 2, "A parameter is missing or wrong."),
        (
            NOT_RUNNING,
            3,
            "Lodestone Launcher isn't running (or the socket is elsewhere).",
        ),
        (
            UNSUPPORTED_PLATFORM,
            3,
            "This system has no Unix domain sockets for the control socket (Windows before 10 1803).",
        ),
        (
            NOT_FOUND,
            4,
            "No such instance, account, world, core, server or job.",
        ),
        (
            AMBIGUOUS,
            4,
            "The name matches more than one thing; use the id.",
        ),
        (
            CONFIRMATION_REQUIRED,
            5,
            "Destructive: ask the user, then pass --yes.",
        ),
        (
            UNTRUSTED_CORE,
            6,
            "The core isn't trusted for mods: the user decides.",
        ),
        (
            UNTRUSTED_CERTIFICATE,
            6,
            "The core's certificate isn't trusted: the user compares the fingerprint.",
        ),
        (
            MODS_CHANGED,
            6,
            "The server's mods changed: the user decides whether to update.",
        ),
        (
            EULA_REQUIRED,
            6,
            "Minecraft's EULA must be accepted by the user.",
        ),
        (BUSY, 7, "The game or another job is running."),
        (CONFLICT, 7, "Conflicts with what is there."),
        (
            TIMEOUT,
            8,
            "Waiting took longer than --timeout; the job goes on.",
        ),
        (UNAVAILABLE, 9, "Not available here (feature off, offline)."),
        (NOT_SIGNED_IN, 9, "Sign in first."),
        (
            PERMISSION_DENIED,
            9,
            "Refused: another user, or not allowed.",
        ),
        (
            PROTOCOL_VERSION,
            9,
            "The CLI and the launcher are from different releases.",
        ),
        (CANCELLED, 10, "Cancelled."),
    ];

    /// The CLI's exit code for an error code; 1 for codes it doesn't know.
    pub fn exit_code(code: &str) -> i32 {
        ALL.iter()
            .find(|(c, _, _)| *c == code)
            .map_or(1, |(_, exit, _)| *exit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_round_trip() {
        let request = Request::new(7, "instances.list", json!({ "all": true }));
        let line = request.to_line();
        assert!(line.ends_with('\n'));
        assert_eq!(Request::parse(line.trim_end()).unwrap(), request);
        // params may be left out
        let parsed = Request::parse(r#"{"v":1,"id":"a","method":"meta.ping"}"#).unwrap();
        assert_eq!(parsed.params, json!({}));
        assert_eq!(parsed.id, json!("a"));
    }

    #[test]
    fn bad_requests_get_codes() {
        let code = |line: &str| Request::parse(line).unwrap_err().1.code;
        assert_eq!(code("not json"), codes::PARSE_ERROR);
        assert_eq!(code("[1]"), codes::INVALID_REQUEST);
        assert_eq!(code(r#"{"v":1,"method":"x"}"#), codes::INVALID_REQUEST);
        assert_eq!(
            code(r#"{"v":1,"id":{},"method":"x"}"#),
            codes::INVALID_REQUEST
        );
        assert_eq!(code(r#"{"v":1,"id":1}"#), codes::INVALID_REQUEST);
        assert_eq!(
            code(r#"{"v":2,"id":1,"method":"x"}"#),
            codes::PROTOCOL_VERSION
        );
        assert_eq!(code(r#"{"id":1,"method":"x"}"#), codes::PROTOCOL_VERSION);
        assert_eq!(
            code(r#"{"v":1,"id":1,"method":"x","params":[1]}"#),
            codes::INVALID_PARAMS
        );
        // the id comes back when it could be read
        let (id, _) = Request::parse(r#"{"v":9,"id":42,"method":"x"}"#).unwrap_err();
        assert_eq!(id, json!(42));
    }

    #[test]
    fn messages_round_trip() {
        let messages = [
            Message::result(json!(1), Ok(json!({ "ok": true }))),
            Message::result(
                json!("x"),
                Err(ErrorObject::new(codes::NOT_FOUND, "gone").with_data(json!({ "id": "a" }))),
            ),
            Message::Progress {
                id: json!(3),
                progress: json!({ "label": "Downloading" }),
            },
            Message::Event {
                subscription: 2,
                event: "log".into(),
                data: json!({ "text": "hi" }),
            },
        ];
        for message in messages {
            let line = message.to_line();
            assert_eq!(line.matches('\n').count(), 1);
            assert_eq!(Message::parse(&line).unwrap(), message);
        }
        assert!(Message::parse(r#"{"v":2,"id":1,"result":1}"#).is_err());
        assert!(Message::parse(r#"{"v":1,"id":1}"#).is_err());
    }

    #[test]
    fn exit_codes_are_stable() {
        assert_eq!(codes::exit_code(codes::FAILED), 1);
        assert_eq!(codes::exit_code(codes::INVALID_PARAMS), 2);
        assert_eq!(codes::exit_code(codes::NOT_RUNNING), 3);
        assert_eq!(codes::exit_code(codes::NOT_FOUND), 4);
        assert_eq!(codes::exit_code(codes::CONFIRMATION_REQUIRED), 5);
        assert_eq!(codes::exit_code(codes::UNTRUSTED_CORE), 6);
        assert_eq!(codes::exit_code(codes::BUSY), 7);
        assert_eq!(codes::exit_code(codes::TIMEOUT), 8);
        assert_eq!(codes::exit_code(codes::NOT_SIGNED_IN), 9);
        assert_eq!(codes::exit_code(codes::CANCELLED), 10);
        assert_eq!(codes::exit_code("something_new"), 1);
        // every code is listed once
        let mut seen = std::collections::HashSet::new();
        for (code, _, _) in codes::ALL {
            assert!(seen.insert(*code), "{code} is listed twice");
        }
    }
}
