// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.
import type { InstalledMod } from './InstalledMod';
import type { ModKind } from './ModKind';
import type { ModLoader } from './ModLoader';

export type InstalledModList = {
  kind: ModKind;
  loader: ModLoader;
  game_version: string;
  directory: string;
  restart_required: boolean;
  mods: Array<InstalledMod>;
};
