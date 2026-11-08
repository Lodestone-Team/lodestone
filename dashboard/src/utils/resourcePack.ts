/** Checks for the server resource pack form, the same rules as the core's. */
import type { PlainMessageKey } from '../i18n/types.ts';

/**
 * Why `url` can't be a resource pack URL, as a message key to show with
 * `t()`, or `null` when it can (or is empty). A key rather than the text, so
 * this file stays free of the i18n runtime (it runs under `node --test`).
 */
export function packUrlErrorKey(url: string): PlainMessageKey | null {
  const trimmed = url.trim();
  if (!trimmed) return null;
  if (/\s/.test(trimmed)) return 'common.resourcePack.urlSpaces';
  let parsed: URL;
  try {
    parsed = new URL(trimmed);
  } catch {
    return 'common.resourcePack.notAUrl';
  }
  if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:')
    return 'common.resourcePack.urlScheme';
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
