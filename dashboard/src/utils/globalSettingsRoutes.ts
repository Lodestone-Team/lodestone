/** the pages of the dashboard's Global Settings, under /dashboard/ */
export const GLOBAL_SETTINGS_PATHS = ['core-settings', 'version'] as const;

export type GlobalSettingsPath = (typeof GLOBAL_SETTINGS_PATHS)[number];

/** where the Core Settings used to be, and where its links go now */
export const LEGACY_CORE_SETTINGS_PATH = '/settings/general';
export const CORE_SETTINGS_PATH = '/dashboard/core-settings';

/** the Global Settings page `pathname` shows, or null if it is another page */
export function globalSettingsPage(
  pathname: string
): GlobalSettingsPath | null {
  const match = /^\/dashboard\/([^/]+)\/?$/.exec(pathname);
  return GLOBAL_SETTINGS_PATHS.find((path) => path === match?.[1]) ?? null;
}

export const isGlobalSettingsPath = (pathname: string) =>
  globalSettingsPage(pathname) !== null;
