/** Checks for the server resource pack form, the same rules as the core's. */

/** Why `url` can't be a resource pack URL, or `null` when it can (or is empty). */
export function packUrlError(url: string): string | null {
  const trimmed = url.trim();
  if (!trimmed) return null;
  if (/\s/.test(trimmed)) return 'The URL cannot contain spaces.';
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return 'That is not a URL.';
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:')
    return 'The URL must start with http:// or https://.';
  return null;
}

/** Whether `sha1` is empty or 40 hexadecimal characters. */
export function isSha1(sha1: string): boolean {
  const trimmed = sha1.trim();
  return trimmed === '' || /^[0-9a-fA-F]{40}$/.test(trimmed);
}

/** A size in bytes as people read it: `1.5 MB`. */
export function formatPackSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}
