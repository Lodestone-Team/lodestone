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

/** A piece of a formatted number: the figures, or the unit (shown lighter). */
export type NumberPart = { text: string; unit: boolean };

/** The unit index (0 = bytes) that fits a size, 1024 per step. */
const byteUnit = (unitOf: number) =>
  unitOf <= 0
    ? 0
    : Math.min(
        BYTE_UNITS.length - 1,
        Math.max(0, Math.floor(Math.log(unitOf) / Math.log(1024)))
      );

const byteFormat = (language: Language, i: number, decimals: number) =>
  cached(
    `b:${language}:${i}:${decimals}`,
    () =>
      new Intl.NumberFormat(language, {
        style: 'unit',
        unit: BYTE_UNITS[i],
        unitDisplay: 'short',
        maximumFractionDigits: Math.max(0, decimals),
      })
  ) as Intl.NumberFormat;

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
  const i = byteUnit(unitOf);
  return byteFormat(language, i, decimals).format(bytes / Math.pow(1024, i));
}

/**
 * formatBytes() in pieces, so the unit can be styled apart from the figures.
 * The language's order and spacing are kept ("1,5 Mo", "1.5MB").
 */
export function formatBytesParts(
  language: Language,
  bytes: number,
  decimals = 2,
  unitOf: number = bytes
): NumberPart[] {
  const i = byteUnit(unitOf);
  return byteFormat(language, i, decimals)
    .formatToParts(bytes / Math.pow(1024, i))
    .map((part) => ({ text: part.value, unit: part.type === 'unit' }));
}

/**
 * "done / total" with the unit once, after the total: "1.2 / 3.4 MB", both
 * numbers in the total's unit. An unknown number is a dash.
 */
export function formatFractionParts(
  language: Language,
  done: number | null,
  total: number | null,
  opts: { bytes?: boolean } = {}
): NumberPart[] {
  const dash: NumberPart = { text: '–', unit: false };
  const slash: NumberPart = { text: ' / ', unit: true };
  if (!opts.bytes) {
    const n = (v: number | null): NumberPart =>
      v === null ? dash : { text: formatNumber(language, v), unit: false };
    return [n(done), slash, n(total)];
  }
  const unitOf = total ?? done ?? 0;
  if (done === null)
    return [
      dash,
      slash,
      ...(total === null
        ? [dash]
        : formatBytesParts(language, total, 2, unitOf)),
    ];
  if (total === null)
    // no total: the done value keeps its own unit
    return [...formatBytesParts(language, done, 2, unitOf), slash, dash];
  // only the figures of the done value: its unit is the total's
  const figures = formatBytesParts(language, done, 2, unitOf)
    .filter((part) => !part.unit)
    .map((part) => part.text)
    .join('')
    .trim();
  return [
    { text: figures, unit: false },
    slash,
    ...formatBytesParts(language, total, 2, unitOf),
  ];
}

/** A count as "1.2K", "12.3万" in the language. */
export const formatCompact = (language: Language, n: number) =>
  formatNumber(language, n, { notation: 'compact', maximumFractionDigits: 1 });

/** "a, b and c" in the language */
export function formatList(
  language: Language,
  items: string[],
  type: 'conjunction' | 'disjunction' = 'conjunction'
): string {
  return new Intl.ListFormat(language, { style: 'long', type }).format(items);
}
