// How names and values are written out: the game and loader as one phrase,
// middle truncation for file names, paths with the home folder as "~".
// No React and no i18n here, so the tests (node --test) can import it.
import type { Game } from '../bindings/Game';

/** What an unknown number or value shows as. */
export const DASH = '–';

/** The product name of a loader or server software id (Modrinth's ids). */
export const LOADER_NAMES: Record<string, string> = {
  fabric: 'Fabric',
  quilt: 'Quilt',
  forge: 'Forge',
  neoforge: 'NeoForge',
  paper: 'Paper',
  spigot: 'Spigot',
  bukkit: 'Bukkit',
  purpur: 'Purpur',
  folia: 'Folia',
  velocity: 'Velocity',
  bungeecord: 'BungeeCord',
  waterfall: 'Waterfall',
  sponge: 'Sponge',
  vanilla: 'Minecraft',
};

/**
 * A loader's product name: "Fabric" for `fabric`. An id we don't know is
 * shown with its first letter capitalised, so a new loader still reads as a
 * name.
 */
export function loaderName(id: string): string {
  const trimmed = id.trim();
  const known = LOADER_NAMES[trimmed.toLowerCase()];
  if (known) return known;
  return trimmed.charAt(0).toUpperCase() + trimmed.slice(1);
}

/**
 * A product and its version as one plain phrase: "Fabric 0.16.9", or just
 * "Fabric" without a version. The game and its loaders are always written
 * this way, never with a version tag (only mod, plugin and modpack versions
 * get one).
 */
export function productPhrase(
  product: string,
  version?: string | null
): string {
  const v = version?.trim();
  return v ? `${product.trim()} ${v}` : product.trim();
}

/** "Minecraft 1.21.1" */
export const gamePhrase = (version?: string | null) =>
  productPhrase('Minecraft', version);

/** "Fabric 0.16.9" for the loader id `fabric` */
export const loaderPhrase = (id: string, version?: string | null) =>
  productPhrase(loaderName(id), version);

/**
 * The game phrase of an instance: "Minecraft 1.21.4" for Java,
 * "Minecraft Bedrock 1.21" and the game's own name for other games.
 */
export function instanceGamePhrase(game: Game, version: string | null): string {
  switch (game.type) {
    case 'MinecraftJava':
      return gamePhrase(version);
    case 'MinecraftBedrock':
      return productPhrase('Minecraft Bedrock', version);
    default:
      return productPhrase(game.game_display_name || game.game_name, version);
  }
}

/**
 * The loader phrase of an instance ("Fabric 0.16.9", "Paper 1.21.4"), or null
 * when it has none to show: vanilla Minecraft, Bedrock and other games.
 */
export function instanceLoaderPhrase(
  game: Game,
  loaderVersion: string | null
): string | null {
  if (game.type !== 'MinecraftJava') return null;
  switch (game.variant.type) {
    case 'Vanilla':
      return null;
    case 'Other':
      return productPhrase(game.variant.name, loaderVersion);
    default:
      return productPhrase(game.variant.type, loaderVersion);
  }
}

/**
 * Text cut in the middle to at most `max` characters, so the end stays
 * visible: "fabric-api-0.116…+1.21.1.jar". It keeps the extension, or at
 * least a third of the length, at the end. Characters are code points, so an
 * emoji is never split. Text that fits comes back unchanged.
 */
export function middleTruncate(
  text: string,
  max: number,
  opts: { keepTail?: number } = {}
): string {
  const chars = Array.from(text);
  if (chars.length <= max) return text;
  const room = Math.max(0, Math.floor(max) - 1); // one for the "…"
  if (max < 5) return '…' + chars.slice(chars.length - room).join('');
  const dot = text.lastIndexOf('.');
  const ext =
    dot > 0 && text.length - dot <= 8 ? Array.from(text.slice(dot)).length : 0;
  const tail = Math.min(
    room - 1,
    Math.max(opts.keepTail ?? 0, ext, Math.ceil(room / 3))
  );
  const head = room - tail;
  return (
    chars.slice(0, head).join('') +
    '…' +
    chars.slice(chars.length - tail).join('')
  );
}

/**
 * The home folder at the start of a path, recognised by its shape:
 * /Users/x, /home/x, /root or C:\Users\x. The core may run on another
 * machine than the browser, so the path's own shape is all there is to go by.
 */
export function homeOf(path: string): string | null {
  const unix = /^(\/Users\/[^/]+|\/home\/[^/]+|\/root)(?=\/|$)/.exec(path);
  if (unix) return unix[1];
  const windows = /^([A-Za-z]:[\\/]Users[\\/][^\\/]+)(?=[\\/]|$)/i.exec(path);
  return windows ? windows[1] : null;
}

/**
 * A path to show: the home folder as "~", and when it is still longer than
 * `max`, the first part and the last `keepParts` parts with "…" between:
 * "~/…/instances/old-pack". The separator stays the path's own.
 */
export function shortenPath(
  path: string,
  opts: { home?: string | null; keepParts?: number; max?: number } = {}
): string {
  const { keepParts = 2, max = 48 } = opts;
  const home = opts.home === undefined ? homeOf(path) : opts.home;
  const sep = path.includes('\\') && !path.includes('/') ? '\\' : '/';
  let shown = path;
  if (home && (path === home || path.startsWith(home))) {
    const rest = path.slice(home.length);
    if (rest === '' || rest[0] === '/' || rest[0] === '\\') shown = '~' + rest;
  }
  if (Array.from(shown).length <= max) return shown;
  const trailing = shown.endsWith(sep) ? sep : '';
  const parts = shown
    .split(/[\\/]/)
    .filter((part, i) => part !== '' || i === 0);
  // parts[0] is "~", a drive ("C:") or "" for a path from the root
  if (parts.length <= keepParts + 1) return middleTruncate(shown, max);
  const first = parts[0] === '' ? '' : parts[0];
  const last = parts.slice(parts.length - keepParts).join(sep);
  const result = `${first}${sep}…${sep}${last}${trailing}`;
  return Array.from(result).length <= max
    ? result
    : middleTruncate(result, max);
}

/** The first `max` items to show and how many are left for "Show N more". */
export function splitShown<T>(
  items: T[],
  max: number
): { shown: T[]; rest: number } {
  const n = Math.max(0, max);
  return { shown: items.slice(0, n), rest: Math.max(0, items.length - n) };
}
