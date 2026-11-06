// Path helpers for the file viewer. Paths are relative to the instance
// directory and use the separator of the OS the core runs on.

/** Guess the core's directory separator from the instance's absolute path. */
export const getDirectorySeparator = (instancePath: string) =>
  // assume only linux paths contain /
  instancePath.includes('/') ? '/' : '\\';
