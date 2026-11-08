// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.
import type { InstalledMod } from './InstalledMod';
import type { ModInstallSkipped } from './ModInstallSkipped';

export type ModInstallResult = {
  installed: Array<InstalledMod>;
  removed: Array<string>;
  skipped: Array<ModInstallSkipped>;
};
