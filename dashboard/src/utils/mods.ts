import { Game } from 'bindings/Game';
import { InstalledMod } from 'bindings/InstalledMod';
import { ModKind } from 'bindings/ModKind';
import { ModLoader } from 'bindings/ModLoader';
import { ModrinthVersion } from 'bindings/ModrinthVersion';

/** What kind of add-ons an instance supports, and its Modrinth loader. */
export type ModSupport = { kind: ModKind; loader: ModLoader };

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
    case 'Paper':
      return { kind: 'plugin', loader: 'paper' };
    case 'Spigot':
      return { kind: 'plugin', loader: 'spigot' };
    default:
      return null;
  }
};

export const modTabTitle = (kind: ModKind) =>
  kind === 'plugin' ? 'Plugins' : 'Mods';

/** The singular noun for the kind, e.g. "mod". */
export const modNoun = (kind: ModKind) =>
  kind === 'plugin' ? 'plugin' : 'mod';

/**
 * The Modrinth loaders whose versions the core installs on an instance with
 * this loader without `force` (MODS_SPEC 2, install step 1).
 */
export const compatibleLoaders = (loader: ModLoader): string[] =>
  loader === 'paper' ? ['paper', 'bukkit', 'spigot'] : [loader];

/** Whether the core would install this version without `force`. */
export const isVersionCompatible = (
  version: ModrinthVersion,
  loader: ModLoader,
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

/** A count such as 1234567 as "1.2M". */
export const formatCount = (count: number) =>
  new Intl.NumberFormat('en-US', {
    notation: 'compact',
    maximumFractionDigits: 1,
  }).format(count);

/** An ISO date as e.g. "Mar 4, 2025", or the input if it doesn't parse. */
export const formatDate = (iso: string) => {
  const date = new Date(iso);
  if (isNaN(date.getTime())) return iso;
  return date.toLocaleDateString(undefined, {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
};
