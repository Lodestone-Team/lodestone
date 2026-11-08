// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.

export type ModrinthVersionFile = {
  filename: string;
  size: number;
  primary: boolean;
  sha512: string;
  url: string;
};
