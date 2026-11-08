// Dates, numbers and sizes formatted through Intl for a language. Use the
// bound versions from useI18n() in components; these take the language.
import type { Language } from './core.ts';

const cache = new Map<string, Intl.NumberFormat | Intl.DateTimeFormat>();

function cached<F extends Intl.NumberFormat | Intl.DateTimeFormat>(
  key: string,
  make: () => F
): F {
  let format = cache.get(key) as F | undefined;
  if (!format) {
    format = make();
    cache.set(key, format);
  }
  return format;
}

const toDate = (value: Date | number) =>
  typeof value === 'number' ? new Date(value) : value;

export function formatNumber(
  language: Language,
  value: number,
  options?: Intl.NumberFormatOptions
): string {
  return cached(
    `n:${language}:${JSON.stringify(options ?? {})}`,
    () => new Intl.NumberFormat(language, options)
  ).format(value);
}

/** A date (and time, with `timeStyle`); a date is "Nov 8, 2026" by default */
export function formatDate(
  language: Language,
  value: Date | number,
  options: Intl.DateTimeFormatOptions = { dateStyle: 'medium' }
): string {
  return cached(
    `d:${language}:${JSON.stringify(options)}`,
    () => new Intl.DateTimeFormat(language, options)
  ).format(toDate(value));
}

export const formatTime = (language: Language, value: Date | number) =>
  formatDate(language, value, { timeStyle: 'medium' });

export const formatDateTime = (language: Language, value: Date | number) =>
  formatDate(language, value, { dateStyle: 'medium', timeStyle: 'short' });

const relativeFormats = new Map<string, Intl.RelativeTimeFormat>();

const RELATIVE_UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ['year', 365 * 24 * 3600],
  ['month', 30 * 24 * 3600],
  ['week', 7 * 24 * 3600],
  ['day', 24 * 3600],
  ['hour', 3600],
  ['minute', 60],
  ['second', 1],
];

/**
 * "3 minutes ago", "in 2 days", "yesterday" for a time in milliseconds,
 * relative to `now`. Under 10 seconds is "now".
 */
export function formatRelativeTime(
  language: Language,
  time: Date | number,
  now: number = Date.now()
): string {
  let format = relativeFormats.get(language);
  if (!format) {
    format = new Intl.RelativeTimeFormat(language, { numeric: 'auto' });
    relativeFormats.set(language, format);
  }
  const seconds = (toDate(time).getTime() - now) / 1000;
  if (Math.abs(seconds) < 10) return format.format(0, 'second');
  for (const [unit, size] of RELATIVE_UNITS) {
    if (Math.abs(seconds) >= size || unit === 'second')
      return format.format(Math.trunc(seconds / size), unit);
  }
  return format.format(0, 'second');
}

const BYTE_UNITS = [
  'byte',
  'kilobyte',
  'megabyte',
  'gigabyte',
  'terabyte',
  'petabyte',
] as const;

/**
 * A size in bytes with a unit of the language (1024 per step), e.g. "1.5 MB",
 * "1,5 Mo", "1.5MB". The unit is the one that fits `unitOf` (the total of a
 * download, so its progress uses the same unit).
 */
export function formatBytes(
  language: Language,
  bytes: number,
  decimals = 2,
  unitOf: number = bytes
): string {
  const i =
    unitOf <= 0
      ? 0
      : Math.min(
          BYTE_UNITS.length - 1,
          Math.max(0, Math.floor(Math.log(unitOf) / Math.log(1024)))
        );
  return formatNumber(language, bytes / Math.pow(1024, i), {
    style: 'unit',
    unit: BYTE_UNITS[i],
    unitDisplay: 'short',
    maximumFractionDigits: Math.max(0, decimals),
  });
}

/** "a, b and c" in the language */
export function formatList(
  language: Language,
  items: string[],
  type: 'conjunction' | 'disjunction' = 'conjunction'
): string {
  return new Intl.ListFormat(language, { style: 'long', type }).format(items);
}
