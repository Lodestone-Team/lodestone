import { useSyncExternalStore } from 'react';
import { setTauriWindowTheme } from 'utils/tauriUtil';

// The dashboard theme. The preference is stored per browser in localStorage;
// "system" follows the OS light or dark setting (prefers-color-scheme), also
// when the OS setting changes while the page is open.
//
// The resolved theme is the `data-theme` attribute on <html>, which picks the
// colour variables in globals.css and the `light:` / `dark:` Tailwind variants.

export type ThemePreference = 'system' | 'light' | 'dark';
export type Theme = 'light' | 'dark';

export const THEME_STORAGE_KEY = 'theme';

const LIGHT_QUERY = '(prefers-color-scheme: light)';

/**
 * Runs in the document head before the page renders (see pages/_document.tsx),
 * so the first paint already has the right theme. Keep it in sync with
 * resolveTheme and applyTheme below. Anything but an explicit light preference
 * or a light OS setting gets the dark theme, the dashboard's original look.
 */
export const THEME_INIT_SCRIPT = `(function () {
  var p;
  try { p = localStorage.getItem('${THEME_STORAGE_KEY}'); } catch (e) {}
  var t = p === 'light' || p === 'dark' ? p
    : window.matchMedia && window.matchMedia('${LIGHT_QUERY}').matches ? 'light' : 'dark';
  var d = document.documentElement;
  d.setAttribute('data-theme', t);
  d.style.colorScheme = t;
})();`;

const isPreference = (value: unknown): value is ThemePreference =>
  value === 'system' || value === 'light' || value === 'dark';

const readStoredPreference = (): ThemePreference => {
  try {
    const stored = window.localStorage.getItem(THEME_STORAGE_KEY);
    return isPreference(stored) ? stored : 'system';
  } catch {
    // storage blocked: follow the system
    return 'system';
  }
};

const systemTheme = (): Theme =>
  typeof window !== 'undefined' && window.matchMedia?.(LIGHT_QUERY).matches
    ? 'light'
    : 'dark';

export const resolveTheme = (preference: ThemePreference): Theme =>
  preference === 'system' ? systemTheme() : preference;

// Kept in memory as well, so a choice still applies for the session when
// localStorage is blocked
let preference: ThemePreference | null = null;
const listeners = new Set<() => void>();

const getPreference = (): ThemePreference => {
  if (preference === null) preference = readStoredPreference();
  return preference;
};

const currentTheme = (): Theme =>
  document.documentElement.getAttribute('data-theme') === 'light'
    ? 'light'
    : 'dark';

const applyTheme = () => {
  const theme = resolveTheme(getPreference());
  const root = document.documentElement;
  if (root.getAttribute('data-theme') !== theme)
    root.setAttribute('data-theme', theme);
  root.style.colorScheme = theme;
  listeners.forEach((listener) => listener());
};

export const setThemePreference = (next: ThemePreference) => {
  preference = next;
  try {
    window.localStorage.setItem(THEME_STORAGE_KEY, next);
  } catch {
    // storage blocked: the choice lasts until the page is closed
  }
  // Lodestone Host's window (title bar buttons) follows too. With "system" it is
  // given back to the OS, so prefers-color-scheme reports the OS setting again.
  setTauriWindowTheme(next === 'system' ? null : next);
  applyTheme();
};

let initialized = false;

/** Starts following OS theme changes and other tabs. Call once on the client. */
export const initTheme = () => {
  if (initialized || typeof window === 'undefined') return;
  initialized = true;
  window.matchMedia?.(LIGHT_QUERY).addEventListener('change', () => {
    if (getPreference() === 'system') applyTheme();
  });
  // the preference was changed in another tab
  window.addEventListener('storage', (event) => {
    if (event.key !== THEME_STORAGE_KEY && event.key !== null) return;
    preference = readStoredPreference();
    setTauriWindowTheme(preference === 'system' ? null : preference);
    applyTheme();
  });
  const initial = getPreference();
  if (initial !== 'system') setTauriWindowTheme(initial);
  // in case the head script didn't run (e.g. Storybook)
  applyTheme();
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

/** The stored preference and a setter for it */
export const useThemePreference = (): [
  ThemePreference,
  (preference: ThemePreference) => void,
] => {
  const value = useSyncExternalStore(
    subscribe,
    getPreference,
    (): ThemePreference => 'system'
  );
  return [value, setThemePreference];
};

/** The theme in use, 'light' or 'dark' */
export const useResolvedTheme = (): Theme =>
  useSyncExternalStore(subscribe, currentTheme, (): Theme => 'dark');
