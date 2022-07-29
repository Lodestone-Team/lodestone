pub mod t_server;
pub mod t_configurable;
pub mod t_player;
pub mod t_resource;
pub mod t_macro;

pub enum MaybeUnsupported<T> {
    Supported(T),
    Unsupported,
}
#[derive(Debug)]
pub enum ErrorInner {
    // IO errors:
    FailedToReadFile,
    FailedToWriteFile,
    FailedToCreateFileOrDir,
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

    // version string errors:
    MalformedVersionString,
    VersionNotFound,

    // Macro errors:
    FailedToRun,

    // Process errors:
    FailedToExecute
}
#[derive(Debug)]
pub struct Error {
    pub inner : ErrorInner,
    pub detail : String
}