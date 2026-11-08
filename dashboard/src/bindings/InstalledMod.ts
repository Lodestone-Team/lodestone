// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.
import type { ModFileMetadata } from './ModFileMetadata';
import type { ModrinthMatch } from './ModrinthMatch';

export type InstalledMod = {
  file_name: string;
  enabled: boolean;
  size: number;
  sha512: string;
  metadata: ModFileMetadata | null;
  modrinth: ModrinthMatch | null;
};
