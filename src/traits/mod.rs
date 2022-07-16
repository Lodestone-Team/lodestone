pub mod t_server;
pub mod t_configurable;
pub mod t_player;
pub mod t_resource;
pub mod t_macro;

pub enum MaybeUnsupported<T> {
    Supported(T),
    Unsupported,
}

pub enum ErrorInner {
    // IO errors:
    FailedToReadFile,
    FailedToWriteFile,
    FileOrDirNotFound,

    // Stdin/stdout errors:
    FailedToWriteStdin,
    FailedToReadStdout,
    StdinNotOpen,
    StdoutNotOpen,
    FailedToAquireLock,

    // Instance operation errors
    InstanceAlreadyStarted,
    InstanceAlreadyStopped,
    InstanceAlreadyStarting,
    InstanceAlreadyStopping,

    // File errors:
    MalformedFile,

    // Macro errors:
    FailedToRun
}

pub struct Error {
    inner : ErrorInner,
    detail : String
}