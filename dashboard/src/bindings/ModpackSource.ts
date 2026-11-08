// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.

export type ModpackSource =
  | { type: 'modrinth'; version_id: string }
  | { type: 'upload'; upload_id: string };
