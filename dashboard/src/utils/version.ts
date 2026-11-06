/**
 * The dashboard version, e.g. "0.5.1".
 *
 * next.config.js sets NEXT_PUBLIC_DASHBOARD_VERSION from package.json at build
 * time, and Next inlines it here, so package.json itself is not bundled. Do
 * not import package.json from src: that would ship the whole file.
 */
export const DASHBOARD_VERSION: string =
  process.env.NEXT_PUBLIC_DASHBOARD_VERSION ?? 'unknown';
