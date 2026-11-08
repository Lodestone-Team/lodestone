// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.

export type ModrinthSearchHit = {
  project_id: string;
  slug: string;
  project_type: string;
  title: string;
  description: string;
  author: string;
  icon_url: string | null;
  downloads: number;
  follows: number;
  categories: Array<string>;
  versions: Array<string>;
  date_modified: string;
  client_side: string;
  server_side: string;
};
