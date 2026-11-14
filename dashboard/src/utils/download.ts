// Starting a download of a link the core made. This module has no imports, so
// its tests run with plain `node --test`.

/** How long the hidden frame stays: enough for the core to answer. */
export const DOWNLOAD_FRAME_LIFETIME_MS = 60_000;

/**
 * Starts downloading `url` without opening a popup. The links come from a
 * request first, and window.open after an await runs once the click's user
 * activation is used up or gone, so the popup blocker dropped every file but
 * the first (and slow ones entirely), without a word.
 *
 * With `openUrl` (Lodestone Host, whose webview opens no new windows) the
 * system browser gets the link. Otherwise a hidden frame loads it: the core
 * sends downloads as attachments, so the browser saves the file, and an error
 * page stays out of sight instead of replacing the dashboard.
 */
export async function startDownload(
  url: string,
  doc: Pick<Document, 'createElement' | 'body'>,
  openUrl?: (url: string) => Promise<unknown>
): Promise<void> {
  if (openUrl) {
    await openUrl(url);
    return;
  }
  const frame = doc.createElement('iframe');
  frame.style.display = 'none';
  frame.src = url;
  doc.body.appendChild(frame);
  // the browser keeps a started download going once the frame is gone
  setTimeout(() => frame.remove(), DOWNLOAD_FRAME_LIFETIME_MS);
}
