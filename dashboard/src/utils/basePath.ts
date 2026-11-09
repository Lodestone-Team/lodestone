/**
 * The path the dashboard is served under, e.g. "/core" on lodestone.cc, or ""
 * when it is served from the root (Tauri, `next dev`, self-hosted copies).
 *
 * next.config.js sets basePath and NEXT_PUBLIC_BASE_PATH from the
 * DASHBOARD_BASE_PATH environment variable at build time. Next prefixes its own
 * /_next files with basePath, but not plain strings that point into public/,
 * so those go through asset().
 */
export const BASE_PATH: string = process.env.NEXT_PUBLIC_BASE_PATH ?? '';

/** A path into public/ (starting with "/"), prefixed with the base path. */
export function asset(path: string): string {
  return BASE_PATH + path;
}

/** window.location.pathname without the base path, as react-router sees it. */
export function stripBasePath(pathname: string): string {
  if (!BASE_PATH) return pathname;
  if (pathname === BASE_PATH) return '/';
  if (pathname.startsWith(BASE_PATH + '/'))
    return pathname.slice(BASE_PATH.length);
  return pathname;
}
