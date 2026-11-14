# Instance images

One format for every copy of a Minecraft instance that Lodestone keeps or moves: server
backups, Launcher snapshots, modpack history, what a server sends players so they can
join, and sharing between Hosts, Launchers and friends. It replaces five formats that
each do part of the job:

| Today | Becomes |
| --- | --- |
| Core backups (`core/src/backups`) | Data images |
| Launcher snapshots, `.lodesnap` (`launcher-core/src/snapshots.rs`) | Images; `.lodesnap` export becomes an image export |
| Core modpack record and history (`.lodestone_modpack/`) | The base and the lineage of content images |
| Launcher `modpack.json` and its update plan | The base and the lineage of content images |
| `LDS2:` share codes (`launcher-core/src/content/share.rs`) | A text encoding of an image whose files can all be downloaded |
| The server's client pack (`core/src/mods/client.rs`) | The client view of the server's content image (the `.mrpack` client pack stays for other launchers) |

Decisions this builds on:

- An instance's **game version and loader type are fixed** when it is made. Only the
  loader version can change in place, and changes a modpack update or a linked server
  brings in. "Copy with another version or loader" makes a new instance.
- On ARM, Java is chosen natively, never emulated silently (see the Launcher's
  `game/runtime.rs`); images never carry Java paths.

## Ideas

1. **One canonical image; server and client are views of it.** Views are computed, never
   edited, and the same image and rules always give the same view, byte for byte.
2. **Stored as full listings, sent as deltas.** Every stored image lists every file, so
   no image depends on another and deleting one never breaks the rest (what backups
   already do). A delta between any two images is computed when needed; only bytes the
   receiver can't download elsewhere travel.
3. **Every file says where it comes from**: the base pack, a Modrinth or CurseForge
   version, or bytes in the store. Exports and transfers send references, not jars that
   can be downloaded again.
4. **Two tiers, one format.** *Content* (mods, configs, versions, settings) is recorded
   at every change; *data* (worlds and everything else) is recorded by backups and
   snapshots.

## The image

```jsonc
{
  "schema": 2,
  "id": "<sha256 of the canonical manifest>",      // content-addressed, see "Identity"
  "kind": "content" | "data" | "full",
  "parent": "<image id>" | null,                    // lineage, informational
  "created": 1794645624,                            // seconds; not part of the id
  "producer": { "app": "core" | "launcher", "version": "0.6.0" },
  "label": "Added JEI",                             // what made it
  "base": BASE | null,
  "game": { "minecraft": "1.21.1", "loader": "fabric", "loader_version": "0.16.10" },
  "settings": SETTINGS,
  "entries": [ ENTRY, ... ],                        // sorted by path
  "removed": [ "mods/some-pack-mod.jar" ],          // base files deliberately taken out
  "worlds": [ WORLD, ... ],                         // data and full images
  "content": "<content image id>",                  // data images: the content they ran with
  "excludes": [ "dynmap/web/tiles/" ],              // what this image left out on purpose
  "withheld": [ WITHHELD, ... ],                    // known files not carried, and why
  "rules": 1,                                       // version of the view and env rules
  "side": "client" | "server",                      // views only; part of the id
  "projected_from": "<image id>"                    // views only; not part of the id
}
```

Reading is forward compatible within a schema. Fields a reader doesn't know are kept (on
the image and on each entry) and written back as they were, so the id still verifies.
Informational values it doesn't know (a withheld reason, an `env_from` input, a loader, a
flavour) are kept as text. An unknown `source`, `kind`, `base` or `env` value makes the
manifest invalid, since nothing can act on it safely. A newer schema is refused with its
own error, and an older one goes to the v1 readers. Settings are the exception: fields a
reader doesn't know are dropped, so local settings can never get into an image; a new
setting needs a new schema.

`parent`, `created`, `producer`, `label`, `projected_from` and entries' `modified` are
informational. `entries[].modified` is optional, `mode` defaults to 420 and `env` to
unknown on both sides.

### Base

```jsonc
{ "modrinth_pack": { "project": "…", "version": "…", "sha512": "<of the .mrpack>" } }
{ "curseforge_pack": { "project": 0, "file": 0 } }
{ "loader": {} }      // nothing but "game": a plain instance
```

A base is a reference to a whole pack, both sides of it. Each view reads its own share
(`overrides/` plus `server-overrides/` or `client-overrides/`, and index files by their
`env`). The pack's files are not listed in `entries` unless the instance differs from
the pack there. Images converted from the old modpack records list them anyway, with
explicit sources, since those records are the only list of what the pack wrote (see "v1
readers").

### Entry

```jsonc
{
  "path": "mods/jei-19.21.jar",     // canonical path, see "Paths"
  "size": 1234,
  "sha256": "…",                    // store key
  "sha1": "…", "sha512": "…",       // when a source publishes them
  "mode": 420,                      // unix bits; Windows keeps read-only only
  "modified": 1794645624000000000,  // ns, informational; not part of the id
  "source": SOURCE,
  "env": { "client": "required", "server": "optional" },
  "env_from": [ "modrinth_environment", "owner_mark" ],
  "world": "world"                  // set for world files
}
```

`source` is one of:

| `source` | Meaning | Carried in an export as |
| --- | --- | --- |
| `{"base": {}}` | The base pack's copy, changed nowhere | Nothing |
| `{"modrinth": {"project", "version", "url"}}` | A Modrinth file (CDN URL, checked against `sha512`) | A reference |
| `{"curseforge": {"project", "file", "downloadable"}}` | A CurseForge file | A reference; listed for the user when not downloadable |
| `{"object": {}}` | Bytes only this store has | The object |

`env` values are `required`, `optional`, `unsupported`, `unknown`. `env_from` records why,
so views can be recomputed when the rules change and the UI can say why a file did or
didn't go to players. Inputs, in order of precedence, with their `env_from` names: the
owner's "For players" mark (`owner_mark`), the base pack's `env` (`base_pack`), Modrinth's
version `environment` (`modrinth_environment`), Modrinth's project
`client_side`/`server_side` (`modrinth_project`), the jar's own `environment`
(`jar_environment`). Each side is decided on its own: the first input that says something
other than `unknown` for that side wins, and `env_from` lists the inputs that decided a
side, sorted. The owner's mark only decides the client side: "For players" is `required`,
"Not for players" is `unsupported`. Modrinth's version `environment` maps as:
`client_and_server` required/required, `client_only` and `singleplayer_only`
required/unsupported, `client_only_server_optional` required/optional, `server_only` and
`dedicated_server_only` unsupported/required, `server_only_client_optional`
optional/required, `client_or_server` and `client_or_server_prefers_both`
optional/optional (client/server); `unknown` and values not listed are no input.

A `modrinth` source must have a `url` on `https://cdn.modrinth.com/` and the entry a
`sha512`. `world` is set exactly when the path is `saves/<world>/…`, and names a world in
`worlds`.

### Settings

Typed fields, each with a scope. Local fields are never written into an image.

| Scope | Fields |
| --- | --- |
| shared | (in `game`) |
| server | port, `server.properties` values, ops, whitelist, flavour |
| client | recommended memory, resource pack order, shader in use, icon, background |
| local (never in an image) | Java path, Rosetta, JVM and game arguments, window, play time |

```jsonc
{ "server": { "port": 25565, "properties": { "motd": "…" },
              "ops": [ { "uuid": "…", "name": "…", "level": 4, "bypasses_player_limit": false } ],
              "whitelist": [ { "uuid": "…", "name": "…" } ], "flavour": "paper" },
  "client": { "recommended_memory_mb": 6144, "resource_packs": [ "vanilla", "file/x.zip" ],
              "shader": "x.zip", "icon": { "sha256": "…", "size": 1234 }, "background": null } }
```

Secret properties (`rcon.password`, `management-server-secret`) are never carried; a
manifest with them is invalid. A Paper or Spigot server's `game.loader` is `vanilla`: players
run vanilla, and the flavour is a server setting.

### World

```jsonc
{ "name": "world", "level_name": "world", "data_version": 3953,
  "game_version": "1.21.1", "size": 123456, "last_played": 1794645624,
  "quiesced": true }
```

`name` is the folder (`saves/<name>/`); the other fields are optional.

### Withheld

```jsonc
{ "path": "mods/blocked.jar", "reason": "curseforge_not_downloadable" | "symlink" | "unreadable" | "bad_name" | "excluded" | "kept_changing" }
```

`path` is the file's path as found, which may not be a valid canonical path (that may be
why it was withheld): shown, never placed. For an image converted from an old format it is
the path that format had (relative to the server folder, the instance folder or the zip).
`bad_name` also covers a path the side's disk layout can't place or read back as itself;
`excluded` also covers a path an image never records that an old listing had; `unreadable`
also covers a file an old format knew only by SHA-1 whose bytes no one could hash.

## Paths

Paths are relative to the game folder and the same on both sides, `/`-separated, NFC,
no `..`, no absolute paths, no NUL, `\` or `:`. So that every path can be created on
Windows, macOS and Linux alike, they also have no empty or `.` parts, no control
characters, none of `<>"|?*`, no Windows device names (`CON`, `PRN`, `AUX`, `NUL`,
`COM0`–`COM9`, `LPT0`–`LPT9`, `COM¹²³`, `LPT¹²³`, `CONIN$`, `CONOUT$`, in any case and
with any extension), no part ending with a space or a dot, at most 4096 bytes and 255 bytes
per part. NFC without Unicode's tables (no shared dependency has them): names with what NFC
changes in practice (combining accents, conjoining jamo, combining kana marks, NFC's
singletons and composition exclusions) are refused, the rest is trusted to be NFC. A file
whose name breaks these rules is withheld as `bad_name`. Names that differ only in letter
case are valid; the shared crate reports them so a case-insensitive disk can refuse them.

| Canonical | Server disk | Client disk |
| --- | --- | --- |
| `mods/…`, `config/…`, `defaultconfigs/…`, `kubejs/…` | `<server>/…` | `<instance>/minecraft/…` |
| `server.jar`, `libraries/…`, `server.properties`, ops and whitelist files | as is | never placed |
| `options.txt`, `resourcepacks/…`, `shaderpacks/…`, `screenshots/…` | never placed | `minecraft/…` |
| `saves/<world>/…` (vanilla layout: `DIM-1`, `DIM1` inside) | `<level>/…`; Paper/Bukkit split `<level>_nether/DIM-1`, `<level>_the_end/DIM1` | `minecraft/saves/<world>/…` |

Server-only also covers `banned-players.json`, `banned-ips.json` and `eula.txt`;
client-only also `optionsof.txt`, `optionsshaders.txt`, `servers.dat`, `servers.dat_old`,
`realms_persistence.json`, and loose files in `saves/` (not in a world). On a server, a
world is any top-level folder the layout knows as one (the level and folders with a
`level.dat`); its files are `saves/<world>/…`. Only the level is split. The split folders'
other files (their own `level.dat`, `uid.dat`, Paper's settings) are kept in the world as
`saves/<level>/.bukkit_nether/…` and `saves/<level>/.bukkit_the_end/…`, which vanilla
ignores. What the mapping can't read back as itself is an error and the file is withheld: a
vanilla `DIM-1`/`DIM1` left inside a split level, a server folder named `saves`, a non-world
file inside a world's folder, a world named like other server content (`mods`, `config`,
`plugins`, `libraries`…).

The mapping is a two-way function per side and per server flavour, tested both ways
(including Windows names and names that differ only in letter case).

Always left out: logs, crash reports, caches, `natives`, `session.lock`, Lodestone's own
working folders (`.lodestone-restore*`, staging), and `.lodestone_config` (a server's
identity). Precisely: the top-level folders `logs`, `crash-reports`, `cache`, `.cache`,
`natives`, `.fabric`, `.mixin.out`, `debug` and `.lodestone-snapshot`; `session.lock` and
`.lodestone-restore*` at any depth; `.lodestone_modpack/{objects,staging,snapshot}/`; the
top-level file `.lodestone_config`. A manifest with an entry there is invalid.

## Views

```
view(image, side, rules) = entries with env[side] ≠ unsupported that the side can hold
                           (configs only when marked shared with players)
                         + the side's share of the base
                         + the side's settings
                         + { side, projected_from: image id, rules }
```

- Configs are files in `config/`, `defaultconfigs/` and `kubejs/`. "Marked shared" is an
  `owner_mark` in `env_from` with `env.client` not `unsupported`. The server view carries
  every config.
- A view keeps the image's base, game, `removed`, worlds, content link, excludes, withheld
  files and unknown fields, and none of its informational fields (`parent`, `created`,
  `producer`, `label`, entries' `modified`). World files go to both views, so the server
  view of a Launcher snapshot is what "Host this" makes; the server-to-player flow uses
  the content image, which has no worlds.

- Client view rule: everything that is not client-unsupported goes to players, so
  `optional` mods (JEI) are included and players can always join. The owner's mark
  forces a mod in; Modrinth's `server_only` and `client: unsupported` keep it out.
- A view is canonical (sorted, no timestamps), so its id is its hash and "is the player in
  step?" is a hash comparison. `projected_from` is not part of the id, so a change only the
  server sees leaves the client view's id as it was. The core recomputes views; it doesn't
  store them.
- Losslessness: `view(server) ∪ view(client) ∪ local = image`, tested for every fixture
  pack, where local is the entries neither view carries (a mod unsupported on both sides)
  and the local settings, which are never in an image anyway. Compared by id.

## Identity

An image's `id` is the SHA-256 of its canonical manifest: sorted keys and entries, without
`id`, `created`, `modified`, `label`, `producer`, `parent` and `projected_from`. Two images
with the same files, sources, base, game and settings have the same id, wherever they were
made and whatever they were recorded after.

Canonical means: lists that are sets sorted (entries by path, each `env_from`, `removed`,
worlds by name, `excludes`, `withheld`, ops, whitelist; the resource pack order is kept),
compact JSON with every object's keys sorted by their UTF-8 bytes, `null` values left out
(a missing optional field and `null` are the same image), strings escaped as serde_json
does (only `"`, `\` and control characters) and integers in decimal.

## Deltas

```jsonc
{ "from": "<image id>", "to": "<image id>",
  "added": [ ENTRY, ... ], "removed": [ "<path>", ... ], "replaced": [ ENTRY, ... ],
  "changes": [ { "game": GAME }, { "settings": SETTINGS }, ... ] }
```

Between any two images: entries by path (`replaced` when anything about one differs), and
each other field that differs, whole, with its new value. Applying refuses a delta for
another image, paths that aren't where it says, and a result whose id isn't `to`.

## Store

Content-addressed, shared by every instance on one machine:

```
<root>/objects/ab/<sha256>        whole files (small) or chunk lists (large)
<root>/chunks/ab/<sha256>         chunks of large files
<root>/images/<instance>/<id>.json
```

- Objects are named by the SHA-256 of their uncompressed bytes. Encoding belongs to the
  store, not the manifest: the core stores zstd frames, the Launcher stores raw files
  (APFS clones where possible). Moving an object between stores re-encodes it.
- Files over 1 MiB are split with FastCDC (min 16 KiB, average 64 KiB, max 256 KiB,
  implemented in the shared crate, no new dependency). The object is then a chunk list.
  Region files dedup well because Minecraft rewrites a chunk in place or moves only that
  chunk.
- Writes: temp file, fsync, read-only, rename; manifests last. Reads verify the SHA-256.
- A stat cache (path, size, mtime, older than scan start minus 2 s) skips rehashing
  unchanged files, as backups already do; the Launcher gains it too.
- GC: mark and sweep over every manifest; refuses to delete anything while a manifest
  can't be read or has a newer schema; temp files are swept after an hour.

## When images are recorded

| Where | Trigger | Image |
| --- | --- | --- |
| Core | A Lodestone action: Mods tab install, remove, enable, disable, update; modpack update or restore; upload, edit or delete in content folders; a "For players" mark; a loader version change | Content image per operation, labelled |
| Core | Server about to start, if a quick check finds content changed outside Lodestone | Content image "Changed outside Lodestone" |
| Core | Backup (scheduled or manual) | Data image, linked to the current content image |
| Launcher | A Launcher action: install or remove content, apply a server update, update the pack, loader version change | Content image per operation |
| Launcher | Game about to launch, if a quick check finds content changed by hand | Content image |
| Launcher | Snapshot on demand; before risky operations (content updates, modpack updates, server updates, loader version changes); after a play session only if the user turns it on | Data or full image |

One image per operation, not per file: a job (Update all) is one image, and changes
within a few seconds of it are merged in. An image is written only after the operation
succeeded. Files the game or a running server writes (`options.txt`, logs, configs a mod
rewrites at startup) don't make content images; snapshots and backups capture them.

## Retention

- Content images: the lineage is kept like modpack history (newest 30, plus every image a
  data image links to). Manifests only; objects are shared.
- Data images: the core's backup rules (keep last N, hourly, daily, weekly buckets;
  manual and pinned never deleted; safety images before a restore kept 30 days). The
  Launcher offers the same, per instance.
- A player whose image was pruned sends a summary of what it has (paths and hashes), and
  the core computes the delta from that.

## Server to player

```
GET  /launcher/instances/{uuid}/image?side=client&have=<id>,<id>
  → 200 { view manifest, or a delta against the newest `have` it knows }
  → 204 when the newest `have` is current
GET  /launcher/instances/{uuid}/objects/{sha256}     bytes only the server has
```

Same scopes as today's client pack (public listing with the client pack scope, or
`ViewInstance`), over Connect and friend tunnels too. The Launcher:

1. shows what it will install (base pack, mods the server added, files only the server has,
   game and loader), with the existing trust advisory; launch settings never come from a
   server;
2. installs the game and loader, gets the base (the `.mrpack`, cached and hash-checked),
   applies the entries (Modrinth by CDN and SHA-512, objects from the server, CurseForge
   when allowed, listed otherwise), checks every SHA-256, places files atomically;
3. records "I am C<n>" plus the server link;
4. next Play: `have=C<n>`; current means join at once, otherwise apply the delta with the
   three-way merge modpack updates already use (the player's own mods and changed configs
   stay).

Works for Fabric, Quilt, Forge and NeoForge servers.

## Restore

| Choice | Puts back |
| --- | --- |
| World only | The world's folders, after a safety image |
| World and its mods | The world plus the content image it ran with, in one swap |
| Everything | The whole image (the core's current Everything scope) |
| As new | A new server or instance beside the original |

When the current content differs from a world's linked content, the restore says what
differs and offers to restore it too. Restore uses the core's journaled swap (staging,
rename aside, roll back on error, replay at startup); the Launcher adopts the same journal.

## Sharing

An export is a zip: `image.json`, the referenced manifests (content image of a data image),
and `objects/` holding only objects whose `source` is `object`. Import checks every path and
hash with the `.lodesnap` reader's rules (entry and size limits, no duplicates, free space)
and always creates a new instance or server first; launch settings are reset unless chosen,
as today.

- Host to Host: restore as new from an image.
- Launcher to Host: "Host this" sends the image; the core makes a server from its server view.
- Host to Launcher: "Download this world" sends a data image; the Launcher builds the linked
  content image.
- Launcher to Launcher: share an image file, or an `LDS2:`-style code when every file can be
  downloaded.

## Security

- Every byte is checked against its SHA-256 (and SHA-512 for downloads) before use.
- Downloads only from Modrinth's CDN, CurseForge, the base pack's allowed hosts, or the
  server that sent the image.
- Launch settings and Java never come from another computer unless the user chooses them.
- `.lodestone*` paths stay protected for users without WriteGlobalFile; images from a server
  can't write outside the game folder or to Lodestone's own files.
- Files that aren't on Modrinth are flagged in the install summary.
- CurseForge files whose authors block distribution travel only as references.

## Migration

- Readers for core backup manifests (schema 1), `.lodesnap` (schema 1), the core's
  `record.json` and `history.json`, and the Launcher's `modpack.json`, `content.json` and
  world backup zips. Old data stays readable; new writes use schema 2.
- GC understands both until everything is converted. Conversion is lazy; old objects are
  never rewritten (core objects are already SHA-256 zstd, Launcher objects SHA-256 raw).
- Round-trip tests: every old format converts to an image and back without changing a byte.
  World backup zips are the exception: they are only read (a zip can't be rewritten byte for
  byte, and images replace them).

### v1 readers

`lodestone_image::legacy` reads each old format without depending on the core or the
Launcher (it carries copies of their serde types). A conversion gives the image (valid,
sealed), the *rest*, and notes:

- **The rest** is what the old file has and an image doesn't model: a backup's id, server,
  kind, pinned, live, statistics and stat-cache time; a snapshot's selection, note and
  whole `instance.json`; a history entry's time in milliseconds, origin, changes, kept files
  and changelog, and its index of how to get files back; the titles, slugs, dates and
  lookups of `content.json`; files the image withholds. It belongs beside the image (the
  backup store keeps a backup's bookkeeping), never in it, so ids depend only on contents.
  Writing back takes the image's values for what it carries and the rest for the others;
  for an unchanged image the result is the original file, byte for byte (core backups,
  snapshot manifests, `record.json`, `history.json`, `modpack.json`, `content.json`, as
  each program's own writer formats them).
- **Notes** say what didn't go into the image: withheld files, local settings dropped, a
  pack that couldn't be the base, a level that isn't a world folder, unrecorded excludes.
- **Hashes the old formats lack.** The modpack records list files by SHA-1 only, and an
  entry needs a SHA-256 and size; a `modrinth_pack` base needs the `.mrpack`'s SHA-512. The
  caller supplies them (hashing files on disk or in the old store, asking Modrinth); a file
  no one could hash is withheld as `unreadable`, and a pack without its hash is no base.

Per format:

- **Core backups (schema 1)** become data images. Paths go through the server layout: the
  level and every other top-level folder with a `level.dat` are worlds, and a Paper or
  Spigot level is folded back into one world. `name` is the label, `level_name` the
  `level-name` property, `quiesced` each world's, `skipped` the withheld files, the execute
  bit mode 755 (else 644). Schema 1 recorded no excludes: the image's `excludes` are empty
  without meaning nothing was left out. A `level_name` that isn't a single folder
  (`worlds/survival`) is no world, and its files keep their server paths.
- **Snapshots and `.lodesnap` (schema 1)** become images through the client layout; the
  kind follows the selection (content categories only: content; worlds, screenshots and
  other files only: data; both: full). `instance.json` gives the game and the recommended
  memory, and `icon.png` and `background.jpg` the client settings' icon and background;
  every other instance field is local, reported and kept in the rest, never in the image.
  Files beside the game folder (`content.json`, `modpack.json`, shared options) are kept in
  the rest. `.lodesnap` files are read with the Launcher's limits.
- **The core's `record.json` and `history.json`** become one content image per history
  entry, chained by `parent`, plus one for the record (on the newest entry's base and game;
  the same id when nothing changed since). They list the files the pack wrote, which is all
  the record knew, with explicit sources: a CDN download `modrinth` (project and version from
  its URL), an override `base` when the pack is the base, anything else `object`. The server's
  own files join the lineage with the first content image the core records. Neither file has
  a version, and the core ignores unknown fields, so a newer file can't be refused.
- **The Launcher's `modpack.json` and `content.json`** become one content image: the base is
  the pack (`modrinth_pack`, `curseforge_pack`, or `loader` for an instance not made from a
  pack); a file `content.json` knows on Modrinth's CDN is `modrinth` with its env from the
  project's `client_side`/`server_side`, on CurseForge `curseforge`, an unchanged pack file
  `base`, anything else `object`. `content.json` keys files by their enabled name, so a
  disabled mod's entry has that path. The game is the caller's (neither file has it).
- **World backup zips** become data images of `saves/<world>/…`, each entry hashed straight
  from the zip, with the Launcher's limits; entries that fail a check are withheld. Zip
  times are local without a zone, so entries have no `modified`.

## Milestones

1. Shared crate `crates/lodestone-image`: manifest, identity, paths, views, store, FastCDC,
   v1 readers, property tests. (Done first: manifest, identity, paths and disk layouts, env,
   views, deltas. Then: store, FastCDC, v1 readers.)
2. Core: backups write data images with their content link and excludes; content images on
   Lodestone actions and before start; modpack history as lineage; Paper world layout.
3. Core: client view and delta endpoints; objects endpoint. The `.mrpack` client pack stays.
4. Launcher: build and update instances from a server image, replacing the client pack sync;
   content images on actions and before launch.
5. Launcher: snapshots as images (exports drop downloadable jars), `LDS2:` as an encoding,
   `.lodesnap` v1 import; world snapshots on demand and before risky operations, after
   sessions optional; restore with linked content.
6. Moving images between computers (Host to Host, Launcher to Host, Host to Launcher,
   Launcher to Launcher) and a full-image option in the UI.
