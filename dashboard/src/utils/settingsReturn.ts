/*
 * Where the settings modal goes back to when it closes: the page it was
 * opened from. Kept in this tab's sessionStorage, so it survives a reload; a
 * settings link opened in a new tab has none, and closes to the home page.
 */

const KEY = 'settingsReturnTo';

/** Remember the page settings is opened from */
export function rememberSettingsReturn(pathname: string) {
  if (pathname.startsWith('/settings')) return;
  try {
    sessionStorage.setItem(KEY, pathname);
  } catch {
    // no storage: closing goes to the home page
  }
}

/** The page settings was opened from, or null */
export function settingsReturnPath(): string | null {
  try {
    const path = sessionStorage.getItem(KEY);
    return path && path.startsWith('/') && !path.startsWith('/settings')
      ? path
      : null;
  } catch {
    return null;
  }
}

export function forgetSettingsReturn() {
  try {
    sessionStorage.removeItem(KEY);
  } catch {
    // nothing to forget
  }
}
