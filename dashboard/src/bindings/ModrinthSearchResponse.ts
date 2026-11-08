// Hand-written to match MODS_SPEC.md until the core generates this file with ts-rs.
import type { ModrinthSearchHit } from './ModrinthSearchHit';

export type ModrinthSearchResponse = {
  hits: Array<ModrinthSearchHit>;
  offset: number;
  limit: number;
  total_hits: number;
};
