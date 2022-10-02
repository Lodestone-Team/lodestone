export type ErrorInner = "FailedToReadFileOrDir" | "FailedToWriteFileOrDir" | "FailedToCreateFileOrDir" | "FailedToRemoveFileOrDir" | "FileOrDirNotFound" | "FiledOrDirAlreadyExists" | "FailedToWriteStdin" | "FailedToReadStdout" | "StdinNotOpen" | "StdoutNotOpen" | "FailedToAcquireLock" | "FailedToUpload" | "FailedToDownload" | "InstanceStarted" | "InstanceStopped" | "InstanceStarting" | "InstanceStopping" | "InstanceErrored" | "InstanceNotFound" | "MalformedFile" | "FieldNotFound" | "ValueNotFound" | "TypeMismatch" | "MalformedVersionString" | "VersionNotFound" | "FailedToRun" | "FailedToExecute" | "FailedToAcquireStdin" | "FailedToAcquireStdout" | "APIChanged" | "UnsupportedOperation" | "MalformedRequest" | "UserNotFound" | "UserAlreadyExists" | "InvalidPassword" | "PermissionDenied";

export interface ClientError {
  inner: ErrorInner;
  detail: string;
}
