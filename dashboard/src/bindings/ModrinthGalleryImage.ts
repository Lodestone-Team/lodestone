// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.

export type ModrinthGalleryImage = {
  url: string;
  title: string | null;
  description: string | null;
  featured: boolean;
};
