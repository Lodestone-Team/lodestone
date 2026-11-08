import type { PluralForms } from './core.ts';
import type { en } from './en.ts';

/** The English catalog, the source of the keys and of their placeholders */
export type SourceCatalog = typeof en;
export type MessageKey = keyof SourceCatalog;

/** A catalog with the keys of `Source`: plain messages stay plain */
export type CatalogOf<Source> = {
  [K in keyof Source]: Source[K] extends string ? string : PluralForms;
};

/** A translation of the whole catalog: a missing or extra key fails tsc */
export type Messages = CatalogOf<SourceCatalog>;

type Placeholders<S> = S extends `${string}{${infer Name}}${infer Rest}`
  ? Name | Placeholders<Rest>
  : never;

// every form of a plural message, as English writes them
type SourceText<K extends MessageKey> = SourceCatalog[K] extends string
  ? SourceCatalog[K]
  : SourceCatalog[K][keyof SourceCatalog[K]];

type ParamNames<K extends MessageKey> = Placeholders<SourceText<K>>;
type IsPlural<K extends MessageKey> = SourceCatalog[K] extends string
  ? false
  : true;

/** The values of a message's placeholders; plural messages need `count` */
export type MessageParams<K extends MessageKey, V> = {
  [P in ParamNames<K>]: P extends 'count' ? number : V;
} & (IsPlural<K> extends true ? { count: number } : unknown);

/** The arguments after the key: none for a message without placeholders */
export type MessageArgs<K extends MessageKey, V> = [ParamNames<K>] extends [
  never,
]
  ? IsPlural<K> extends true
    ? [params: { count: number }]
    : [params?: undefined]
  : [params: MessageParams<K, V>];

export type TFunction = <K extends MessageKey>(
  key: K,
  ...args: MessageArgs<K, string | number>
) => string;
