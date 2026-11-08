// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.

export type ModrinthUrlTarget = {
  project_id: string;
  slug: string;
  project_type: string;
  version_id: string | null;
};
