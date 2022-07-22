pub mod t_server;
pub mod t_configurable;
pub mod t_player;
pub mod t_resource;
pub mod t_macro;
pub mod t_events;

pub enum MaybeUnsupported<T> {
    Supported(T),
    Unsupported,
}

pub enum ErrorInner {
    // IO errors:
    FailedToReadFile,
    FailedToWriteFile,
    FileOrDirNotFound,
    FiledOrDirAlreadyExists,

    // Stdin/stdout errors:
    FailedToWriteStdin,
    FailedToReadStdout,
    StdinNotOpen,
    StdoutNotOpen,
    FailedToAquireLock,

    // Network errors:
    FailedToUpload,
    FailedToDownload,

    // Instance operation errors
    InstanceAlreadyStarted,
    InstanceAlreadyStopped,
    InstanceAlreadyStarting,
    InstanceAlreadyStopping,

    // Config file errors:
    MalformedFile,
    FieldNotFound,
    ValueNotFound,
    TypeMismatch,

    // Macro errors:
    FailedToRun
}

pub struct Error {
    inner : ErrorInner,
    detail : String
}