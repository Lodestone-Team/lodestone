// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.
import type { ModrinthDependency } from './ModrinthDependency';
import type { ModrinthVersionFile } from './ModrinthVersionFile';
import type { ModrinthVersionType } from './ModrinthVersionType';

export type ModrinthVersion = {
  id: string;
  project_id: string;
  name: string;
  version_number: string;
  version_type: ModrinthVersionType;
  game_versions: Array<string>;
  loaders: Array<string>;
  date_published: string;
  downloads: number;
  files: Array<ModrinthVersionFile>;
  dependencies: Array<ModrinthDependency>;
};
