// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.

export type ModpackInfo = {
  name: string;
  version_number: string | null;
  summary: string | null;
  game_version: string;
  loader: string;
  loader_version: string | null;
  server_file_count: number;
  server_download_size: number;
  unsupported_reason: string | null;
  upload_id: string | null;
};
