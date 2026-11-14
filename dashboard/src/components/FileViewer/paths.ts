// Path helpers for the file viewer. Paths are relative to the instance
// directory and use the separator of the OS the core runs on.

/** Guess the core's directory separator from the instance's absolute path. */
export const getDirectorySeparator = (instancePath: string) =>
  // assume only linux paths contain /
  instancePath.includes('/') ? '/' : '\\';

/**
 * Whether the core's file system ignores case in names: Windows does, and so
 * does macOS by default. `os` is the core's `/info` os (Rust's
 * `std::env::consts::OS`); when it isn't known yet, a Windows-style separator
 * is the best guess.
 */
export const coreIgnoresCase = (os: string | undefined, separator: string) =>
  os ? os === 'windows' || os === 'macos' : separator === '\\';

/**
 * Whether a folder already has an entry called `name`. New file and New folder
 * refuse such a name: creating a file over an existing one empties it.
 */
export const nameTaken = (
  entries: readonly { name: string }[],
  name: string,
  ignoreCase: boolean
) => {
  const fold = (s: string) => (ignoreCase ? s.toLowerCase() : s);
  return entries.some((entry) => fold(entry.name) === fold(name));
};

/**
 * Whether `path` is `target` itself or, when `target` is a folder, inside it.
 * A plain prefix test would also match a sibling: `backup.sh` starts with
 * `backup`.
 */
export const isSameOrInside = (
  path: string,
  target: string,
  separator: string
) => path === target || path.startsWith(target + separator);
