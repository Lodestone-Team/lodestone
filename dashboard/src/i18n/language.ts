import { useSyncExternalStore } from 'react';
import { detectLanguage, isLanguage, Language } from './core.ts';

// The dashboard language. Like the theme (utils/theme.ts), the preference is
// stored per browser in localStorage; "auto" follows the browser's languages
// (navigator.languages), also when they change while the page is open.
// The language in use is the `lang` attribute of <html>.

export type LanguagePreference = 'auto' | Language;

export const LANGUAGE_STORAGE_KEY = 'language';

/**
 * Runs in the document head before the page renders (see pages/_document.tsx),
 * so <html lang> is right for the first paint (CJK fonts are picked by it).
 * Keep it in sync with matchLanguage in core.ts.
 */
export const LANGUAGE_INIT_SCRIPT = `(function () {
  var p, l = 'en';
  try { p = localStorage.getItem('${LANGUAGE_STORAGE_KEY}'); } catch (e) {}
  if (p === 'en' || p === 'fr' || p === 'zh-Hans' || p === 'zh-Hant') l = p;
  else {
    var tags = navigator.languages || [navigator.language || ''];
    for (var i = 0; i < tags.length; i++) {
      var s = String(tags[i]).toLowerCase().split(/[-_]/);
      if (s[0] === 'en' || s[0] === 'fr') { l = s[0]; break; }
      if (s[0] === 'zh') {
        l = s.indexOf('hant') > 0 ? 'zh-Hant' : s.indexOf('hans') > 0 ? 'zh-Hans'
          : s.indexOf('tw') > 0 || s.indexOf('hk') > 0 || s.indexOf('mo') > 0 ? 'zh-Hant' : 'zh-Hans';
        break;
      }
    }
  }
  document.documentElement.lang = l;
})();`;

const isPreference = (value: unknown): value is LanguagePreference =>
  value === 'auto' || isLanguage(value);

const readStoredPreference = (): LanguagePreference => {
  try {
    const stored = window.localStorage.getItem(LANGUAGE_STORAGE_KEY);
    return isPreference(stored) ? stored : 'auto';
  } catch {
    // storage blocked: follow the browser
    return 'auto';
  }
};

/** The browser's language, if the dashboard has it, else English */
export const browserLanguage = (): Language => {
  if (typeof navigator === 'undefined') return 'en';
  return detectLanguage(
    navigator.languages?.length ? navigator.languages : [navigator.language]
  );
};

// Kept in memory as well, so a choice still applies for the session when
// localStorage is blocked
let preference: LanguagePreference | null = null;
let language: Language | null = null;
const listeners = new Set<() => void>();

const getPreference = (): LanguagePreference => {
  if (preference === null)
    preference =
      typeof window === 'undefined' ? 'auto' : readStoredPreference();
  return preference;
};

const resolve = (): Language => {
  const p = getPreference();
  return p === 'auto' ? browserLanguage() : p;
};

/** The language in use. Also for code outside React (see t() in index.tsx). */
export const getLanguage = (): Language => {
  if (language === null) language = resolve();
  return language;
};

const apply = () => {
  language = resolve();
  if (typeof document !== 'undefined') document.documentElement.lang = language;
  listeners.forEach((listener) => listener());
};

export const setLanguagePreference = (next: LanguagePreference) => {
  preference = next;
  try {
    if (next === 'auto') window.localStorage.removeItem(LANGUAGE_STORAGE_KEY);
    else window.localStorage.setItem(LANGUAGE_STORAGE_KEY, next);
  } catch {
    // storage blocked: the choice lasts until the page is closed
  }
  apply();
};

let initialized = false;

/** Starts following the browser's languages and other tabs. Call once. */
export const initLanguage = () => {
  if (initialized || typeof window === 'undefined') return;
  initialized = true;
  window.addEventListener('languagechange', () => {
    if (getPreference() === 'auto') apply();
  });
  // the preference was changed in another tab
  window.addEventListener('storage', (event) => {
    if (event.key !== LANGUAGE_STORAGE_KEY && event.key !== null) return;
    preference = readStoredPreference();
    apply();
  });
  // in case the head script didn't run (e.g. Storybook)
  apply();
};

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
};

/** The stored preference and a setter for it */
export const useLanguagePreference = (): [
  LanguagePreference,
  (preference: LanguagePreference) => void,
] => {
  const value = useSyncExternalStore(
    subscribe,
    getPreference,
    (): LanguagePreference => 'auto'
  );
  return [value, setLanguagePreference];
};

/** The language in use */
export const useLanguage = (): Language =>
  useSyncExternalStore(subscribe, getLanguage, (): Language => 'en');
