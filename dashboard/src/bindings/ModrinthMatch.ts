// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.
import type { ModrinthUpdate } from './ModrinthUpdate';

export type ModrinthMatch = {
  project_id: string;
  version_id: string;
  slug: string;
  title: string;
  icon_url: string | null;
  version_number: string;
  update: ModrinthUpdate | null;
};
