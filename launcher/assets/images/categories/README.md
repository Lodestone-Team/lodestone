# Category icons

The icons of the Content tab's Modrinth filters (`instances/content/filters.rs`).

- The category, feature and performance icons are Modrinth's own, as its API serves
  them (`GET https://api.modrinth.com/v2/tag/category`), with an `xmlns` added so they
  parse. Modrinth's site is AGPL-3.0, like Lodestone. `equipment-pack.svg` and
  `realistic-shader.svg` are the resource pack and shader versions of icons whose
  names the mods (or resource packs) use too.
- `any.svg`, `client.svg`, `server.svg` and `open-source.svg` are Lucide's `globe`,
  `monitor`, `server` and `code` (ISC license), in the same style.

Resolutions have no icons on Modrinth; their labels ("16x") say enough.
