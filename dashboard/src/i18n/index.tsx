// The dashboard's translations.
//
// - Components: `const t = useT();` then `t('common.save')`, or
//   `const { t, rich, formatDate, ... } = useI18n();`. They re-render when
//   the language changes.
// - Code outside React (toasts from data hooks, yup messages, errors):
//   the global `t()`, which reads the language when it is called. Don't call
//   it at module level: the text would be fixed at import time.
//
// Messages are in en.ts (the source) and fr.ts, zh-Hans.ts, zh-Hant.ts.
// `{name}` placeholders are filled from the params; a message with plural
// forms is picked by `count` (Intl.PluralRules), e.g.
// t('files.selected', { count: 3 }).
import {
  createContext,
  Fragment,
  ReactNode,
  useContext,
  useEffect,
  useMemo,
} from 'react';
import { formatMessage, Language, MessageValue, selectPlural } from './core.ts';
import { en } from './en.ts';
import { fr } from './fr.ts';
import { zhHans } from './zh-Hans.ts';
import { zhHant } from './zh-Hant.ts';
import * as format from './format.ts';
import { getLanguage, initLanguage, useLanguage } from './language.ts';
import type { MessageArgs, MessageKey, Messages, TFunction } from './types.ts';

export type { MessageKey, PlainMessageKey, TFunction } from './types.ts';
export type { Language } from './core.ts';
export { LANGUAGES, LANGUAGE_NAMES } from './core.ts';
export { useLanguagePreference, type LanguagePreference } from './language.ts';

const catalogs: Record<Language, Messages> = {
  en,
  fr,
  'zh-Hans': zhHans,
  'zh-Hant': zhHant,
};

const lookup = (language: Language, key: MessageKey): MessageValue =>
  catalogs[language][key] ?? en[key] ?? key;

const translate = (
  language: Language,
  key: MessageKey,
  params?: Record<string, string | number>
) => formatMessage(language, lookup(language, key), params);

/** A message in the current language, for code outside React components */
export const t: TFunction = (key, ...[params]) =>
  translate(getLanguage(), key, params as Record<string, string | number>);

export type RichFunction = <K extends MessageKey>(
  key: K,
  ...args: MessageArgs<K, ReactNode>
) => ReactNode;

/** A message whose placeholders can be elements, e.g. a link or <b> */
function richMessage(
  language: Language,
  key: MessageKey,
  params?: Record<string, ReactNode>
): ReactNode {
  const message = lookup(language, key);
  const template =
    typeof message === 'string'
      ? message
      : selectPlural(language, message, Number(params?.count ?? 0));
  const parts = template.split(/\{(\w+)\}/g);
  // odd indexes are placeholder names
  return parts.map((part, i) =>
    i % 2 === 0 ? (
      part
    ) : (
      <Fragment key={i}>
        {params && part in params
          ? typeof params[part] === 'number' && part === 'count'
            ? format.formatNumber(language, params[part] as number)
            : params[part]
          : `{${part}}`}
      </Fragment>
    )
  );
}

function makeI18n(language: Language) {
  return {
    language,
    t: ((key, ...[params]) =>
      translate(
        language,
        key,
        params as Record<string, string | number>
      )) as TFunction,
    rich: ((key, ...[params]) =>
      richMessage(
        language,
        key,
        params as Record<string, ReactNode>
      )) as RichFunction,
    formatNumber: (value: number, options?: Intl.NumberFormatOptions) =>
      format.formatNumber(language, value, options),
    formatDate: (value: Date | number, options?: Intl.DateTimeFormatOptions) =>
      format.formatDate(language, value, options),
    formatTime: (value: Date | number) => format.formatTime(language, value),
    formatDateTime: (value: Date | number) =>
      format.formatDateTime(language, value),
    formatRelativeTime: (time: Date | number, now?: number) =>
      format.formatRelativeTime(language, time, now),
    formatBytes: (bytes: number, decimals?: number, unitOf?: number) =>
      format.formatBytes(language, bytes, decimals, unitOf),
    formatList: (items: string[], type?: 'conjunction' | 'disjunction') =>
      format.formatList(language, items, type),
  };
}

export type I18n = ReturnType<typeof makeI18n>;

// English outside the provider (e.g. Storybook stories without it)
const I18nContext = createContext<I18n>(makeI18n('en'));

/** Provides the current language to useI18n() and useT(); keeps <html lang> */
export function I18nProvider({ children }: { children: ReactNode }) {
  const language = useLanguage();
  useEffect(initLanguage, []);
  const value = useMemo(() => makeI18n(language), [language]);
  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

/** The language, t(), rich() and Intl formatters for it */
export const useI18n = (): I18n => useContext(I18nContext);

/** t() for the current language; a new function when the language changes */
export const useT = (): TFunction => useContext(I18nContext).t;

// Formatters for code outside React, in the current language
export const formatNumber = (
  value: number,
  options?: Intl.NumberFormatOptions
) => format.formatNumber(getLanguage(), value, options);
export const formatDate = (
  value: Date | number,
  options?: Intl.DateTimeFormatOptions
) => format.formatDate(getLanguage(), value, options);
export const formatDateTime = (value: Date | number) =>
  format.formatDateTime(getLanguage(), value);
export const formatRelativeTime = (time: Date | number, now?: number) =>
  format.formatRelativeTime(getLanguage(), time, now);
export const formatBytes = (
  bytes: number,
  decimals?: number,
  unitOf?: number
) => format.formatBytes(getLanguage(), bytes, decimals, unitOf);
