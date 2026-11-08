// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.

export type ModInstallSkipped = {
  project_id: string;
  reason: string;
};
