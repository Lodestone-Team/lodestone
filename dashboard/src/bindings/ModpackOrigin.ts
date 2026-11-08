// Hand-written to match CLIENT_MODS_SPEC 3.1 until the core generates this file with ts-rs.

/**
 * Where a modpack instance came from. For an uploaded .mrpack, project_id
 * and version_id are empty and icon_url is null.
 */
export type ModpackOrigin = {
  project_id: string;
  version_id: string;
  name: string;
  version_number: string | null;
  icon_url: string | null;
};
