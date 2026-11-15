import { Game } from 'bindings/Game';
import { InstalledMod } from 'bindings/InstalledMod';
import { InstalledModKind } from 'bindings/InstalledModKind';
import { InstalledModLoader } from 'bindings/InstalledModLoader';
import { ModrinthVersion } from 'bindings/ModrinthVersion';
import { formatDate as formatLocaleDate, formatNumber, t } from 'i18n';
import { safeImageUrl } from 'utils/markdown';

/** What kind of add-ons an instance supports, and its Modrinth loader. */
export type ModSupport = { kind: InstalledModKind; loader: InstalledModLoader };

/**
 * The mod or plugin support of an instance, or null if it has none
 * (MODS_SPEC section 0). The core decides; this only chooses whether the
 * tab is shown.
 */
export const getModSupport = (game: Game): ModSupport | null => {
  if (game.type !== 'MinecraftJava') return null;
  switch (game.variant.type) {
    case 'Fabric':
      return { kind: 'mod', loader: 'fabric' };
    case 'Forge':
      return { kind: 'mod', loader: 'forge' };
    case 'NeoForge':
      return { kind: 'mod', loader: 'neoforge' };
    case 'Paper':
      return { kind: 'plugin', loader: 'paper' };
    case 'Spigot':
      return { kind: 'plugin', loader: 'spigot' };
    default:
      return null;
  }
};

/** The tab's name, "Mods" or "Plugins", in the current language. */
export const modTabTitle = (kind: InstalledModKind) =>
  kind === 'plugin' ? t('mods.tab.plugins') : t('mods.tab.mods');

/**
 * The Modrinth loaders whose versions the core installs on an instance with
 * this loader without `force` (MODS_SPEC 2, install step 1).
 */
export const compatibleLoaders = (loader: InstalledModLoader): string[] =>
  loader === 'paper' ? ['paper', 'bukkit', 'spigot'] : [loader];

/** Whether the core would install this version without `force`. */
export const isVersionCompatible = (
  version: ModrinthVersion,
  loader: InstalledModLoader,
  gameVersion: string
) =>
  version.game_versions.includes(gameVersion) &&
  version.loaders.some((l) => compatibleLoaders(loader).includes(l));

/** Whether search box text should be resolved as a Modrinth link. */
export const looksLikeModrinthLink = (text: string) =>
  text.toLowerCase().includes('modrinth.com/');

/** The project's page on modrinth.com. */
export const modrinthProjectUrl = (projectType: string, slug: string) =>
  `https://modrinth.com/${encodeURIComponent(projectType)}/${encodeURIComponent(slug)}`;

/** The name to show for an installed mod. */
export const installedModName = (mod: InstalledMod) =>
  mod.modrinth?.title || mod.metadata?.name || mod.file_name;

/** The version to show for an installed mod, if any. */
export const installedModVersion = (mod: InstalledMod) =>
  mod.modrinth?.version_number || mod.metadata?.version || null;

/** A count such as 1234567 as "1.2M" in the current language. */
export const formatCount = (count: number) =>
  formatNumber(count, { notation: 'compact', maximumFractionDigits: 1 });

/**
 * An ISO date as e.g. "Mar 4, 2025" in the current language, or the input
 * if it doesn't parse.
 */
export const formatDate = (iso: string) => {
  const date = new Date(iso);
  if (isNaN(date.getTime())) return iso;
  return formatLocaleDate(date, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
};

/** The file the core downloads for a version: the primary one, or the first. */
export const primaryFile = (version: ModrinthVersion) =>
  version.files.find((file) => file.primary) ?? version.files[0] ?? null;

/**
 * A modpack icon URL to show for an instance, or null: only https URLs on
 * Modrinth's CDN, through the same filter as the Mods tab's images.
 */
export const modpackIconUrl = (url: string | null | undefined) => {
  const safe = url ? safeImageUrl(url) : null;
  return safe && safe.startsWith('https://cdn.modrinth.com/') ? safe : null;
};
