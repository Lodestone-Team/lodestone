// The language-independent part of the dashboard's translations: the
// supported languages, matching a browser language to one of them, and
// filling a message's {placeholders}. No React and no catalogs here, so the
// tests (node --test) can import it.

export const LANGUAGES = ['en', 'fr', 'zh-Hans', 'zh-Hant'] as const;
export type Language = (typeof LANGUAGES)[number];

/** Each language's name in that language, for the language picker */
export const LANGUAGE_NAMES: Record<Language, string> = {
  en: 'English',
  fr: 'Français',
  'zh-Hans': '简体中文',
  'zh-Hant': '繁體中文',
};

export const isLanguage = (value: unknown): value is Language =>
  typeof value === 'string' && (LANGUAGES as readonly string[]).includes(value);

/**
 * The supported language for a BCP 47 tag (e.g. "fr-CA", "zh-TW"), or null.
 * Chinese without a script is Simplified, except for Taiwan, Hong Kong and
 * Macau.
 */
export function matchLanguage(tag: string): Language | null {
  const [primary, ...subtags] = tag.toLowerCase().split(/[-_]/);
  switch (primary) {
    case 'en':
      return 'en';
    case 'fr':
      return 'fr';
    case 'zh':
      if (subtags.includes('hant')) return 'zh-Hant';
      if (subtags.includes('hans')) return 'zh-Hans';
      if (subtags.some((s) => s === 'tw' || s === 'hk' || s === 'mo'))
        return 'zh-Hant';
      return 'zh-Hans';
    default:
      return null;
  }
}

/** The first supported language of the browser's list, else English */
export function detectLanguage(tags: readonly string[]): Language {
  for (const tag of tags) {
    const language = matchLanguage(tag);
    if (language) return language;
  }
  return 'en';
}

/** A message with plural forms, picked by its `count` (Intl.PluralRules) */
export type PluralForms = { other: string } & Partial<
  Record<Exclude<Intl.LDMLPluralRule, 'other'>, string>
>;
export type MessageValue = string | PluralForms;

/** The names in a message's {placeholders}, in order (repeats included) */
export function placeholders(template: string): string[] {
  return Array.from(template.matchAll(/\{(\w+)\}/g), (m) => m[1]);
}

const pluralRules = new Map<string, Intl.PluralRules>();
const countFormats = new Map<string, Intl.NumberFormat>();

/** The form of a plural message for `count` */
export function selectPlural(
  locale: string,
  forms: PluralForms,
  count: number
): string {
  let rules = pluralRules.get(locale);
  if (!rules) {
    rules = new Intl.PluralRules(locale);
    pluralRules.set(locale, rules);
  }
  // e.g. French has a `many` form for millions, which can fall back to `other`
  return forms[rules.select(count)] ?? forms.other;
}

/** `count` with the locale's digit grouping, e.g. 1,234 or 1 234 */
export function formatCount(locale: string, count: number): string {
  let format = countFormats.get(locale);
  if (!format) {
    format = new Intl.NumberFormat(locale);
    countFormats.set(locale, format);
  }
  return format.format(count);
}

/**
 * Fills the {placeholders} of a message. `count` is formatted as a number of
 * the locale; other numbers are written as is (ports, years, versions).
 * Unknown placeholders are left in place.
 */
export function interpolate(
  locale: string,
  template: string,
  params?: Record<string, string | number>
): string {
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (match, name: string) => {
    if (!(name in params)) return match;
    const value = params[name];
    if (name === 'count' && typeof value === 'number')
      return formatCount(locale, value);
    return String(value);
  });
}

/** The text of a message: its plural form for `params.count`, filled in */
export function formatMessage(
  locale: string,
  message: MessageValue,
  params?: Record<string, string | number>
): string {
  const template =
    typeof message === 'string'
      ? message
      : selectPlural(locale, message, Number(params?.count ?? 0));
  return interpolate(locale, template, params);
}
