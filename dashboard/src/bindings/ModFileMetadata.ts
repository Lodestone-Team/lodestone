// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.

export type ModFileMetadata = {
  id: string | null;
  name: string | null;
  version: string | null;
  description: string | null;
  authors: Array<string>;
};
