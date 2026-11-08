// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.
import type { ModpackSource } from './ModpackSource';

export type CreateModpackInstanceRequest = {
  source: ModpackSource;
  name: string;
  port: number;
  description?: string;
  min_ram?: number;
  max_ram?: number;
  auto_start?: boolean;
  restart_on_crash?: boolean;
};
