export type ErrorInner = "FailedToReadFileOrDir" | "FailedToWriteFileOrDir" | "FailedToCreateFileOrDir" | "FailedToRemoveFileOrDir" | "FileOrDirNotFound" | "FiledOrDirAlreadyExists" | "FailedToWriteStdin" | "FailedToReadStdout" | "StdinNotOpen" | "StdoutNotOpen" | "FailedToAcquireLock" | "FailedToUpload" | "FailedToDownload" | "InstanceStarted" | "InstanceStopped" | "InstanceStarting" | "InstanceStopping" | "InstanceErrored" | "InstanceNotFound" | "MalformedFile" | "FieldNotFound" | "ValueNotFound" | "TypeMismatch" | "MalformedVersionString" | "VersionNotFound" | "FailedToRun" | "FailedToExecute" | "FailedToAcquireStdin" | "FailedToAcquireStdout" | "APIChanged" | "UnsupportedOperation" | "MalformedRequest" | "UserNotFound" | "UserAlreadyExists" | "InvalidPassword" | "PermissionDenied" | "NetworkError" | "UnknownError" | "FrontendError" ;

export class ClientError {
  readonly inner: ErrorInner;
  readonly detail: string;

  constructor(inner: ErrorInner, detail: string) {
    this.inner = inner;
    this.detail = detail;
  }

  toString() {
    return `${this.inner}: ${this.detail}`;
  }

  static fromString(s: string) {
    return new ClientError("FrontendError", s);
  }
}

export function isClientError(error: unknown): error is ClientError {
  return error instanceof ClientError;
}
