/// The errors of this crate. The messages are meant for users.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// A value from a user or a file is not valid, such as a link, a search filter or a
    /// modpack. Lodestone Core answers these with 400 Bad Request.
    #[error("{0}")]
    Invalid(String),
    /// What was asked for does not exist, such as a Minecraft version.
    #[error("{0}")]
    NotFound(String),
    /// Something failed that is not the caller's fault.
    #[error("{0}")]
    Internal(String),
    /// A file operation failed. `context` says which; the cause is `source`.
    #[error("{context}")]
    Io {
        context: String,
        source: std::io::Error,
    },
    /// Writing JSON failed. `context` says what; the cause is `source`.
    #[error("{context}")]
    Json {
        context: String,
        source: serde_json::Error,
    },
}
