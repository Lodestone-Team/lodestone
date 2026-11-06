/**
 * The dashboard version, e.g. "0.5.1".
 *
 * Set NEXT_PUBLIC_DASHBOARD_VERSION at build time (next.config.js `env`) so
 * that package.json is not bundled: Next inlines the value and the
 * package.json fallback below is never evaluated.
 */
export const DASHBOARD_VERSION: string =
  process.env.NEXT_PUBLIC_DASHBOARD_VERSION ||
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  require('../../package.json').version;
