// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.
import type { ModrinthGalleryImage } from './ModrinthGalleryImage';

export type ModrinthProject = {
  id: string;
  slug: string;
  project_type: string;
  title: string;
  description: string;
  body: string;
  icon_url: string | null;
  categories: Array<string>;
  loaders: Array<string>;
  game_versions: Array<string>;
  downloads: number;
  followers: number;
  client_side: string;
  server_side: string;
  license_id: string | null;
  source_url: string | null;
  issues_url: string | null;
  wiki_url: string | null;
  discord_url: string | null;
  gallery: Array<ModrinthGalleryImage>;
  published: string;
  updated: string;
};
