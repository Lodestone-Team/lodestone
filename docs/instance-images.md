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
Numbers in such fields must be integers that fit 64 bits (signed or unsigned); any other
number makes the manifest invalid, since a reader couldn't write it back as it was.
Anywhere else (`game`, `producer`, `base`, a `source`, an `env`, a world, a withheld
file) an unknown field makes the manifest invalid with an error naming it, so a new field
there needs a new schema: dropping it would change the id, and the image would fail its
check with a misleading mismatch instead. Informational values it doesn't know (a withheld reason, an
`env_from` input, a loader, a flavour) are kept as text. An unknown `source`, `kind`,
`base` or `env` value makes the manifest invalid, since nothing can act on it safely. A
newer schema is refused with its own error, and an older one goes to the v1 readers.
Settings are the exception: fields a reader doesn't know are dropped, so local settings can
never get into an image; a new setting needs a new schema.

Unknown fields are kept within a budget, not without limit: per image at most 4,096 JSON
values and 64 KiB of keys and strings, per entry 16 values and 1 KiB, and all of an
image's entries together 1,048,576 values and 16 MiB (an entry's first unknown field
counts 16 values more, for the map that holds it), nested at most 16 deep. That is room
for new fields, not for a payload; a manifest past it is invalid.
Reading never costs much more memory than a real image of the manifest's size: before
parsing, a manifest with more than 32,000,000 JSON values (objects, arrays and the commas
between items; a real entry has about 20) or nesting deeper than 32 is refused, and every
list is refused as soon as it passes its bound while being read (1,000,000 entries,
removed paths and withheld files, 10,000 worlds, 1,000 excludes, 16 `env_from` inputs,
100,000 items in each setting's list or map).
A manifest from elsewhere comes in a compressed export, so without these a small file
could exhaust memory.

A `content` image has no worlds (and so no world files); `data` and `full` images may.

`parent`, `created`, `producer`, `label`, `projected_from` and entries' `modified` are
informational. `entries[].modified` is optional, `mode` defaults to 420 and `env` to
unknown on both sides. `null` is the same as a missing field everywhere: a field with a
default (`settings`, `entries`, `mode`, `env`, `producer` and its `app` and `version`, a
setting's list…) reads `null` as that default. Only the required fields can't be `null`:
`schema`, `kind`, `game`; an entry's `path`, `size`, `sha256` and `source`; and in
settings an op's or whitelisted player's `uuid` and `name` and an op's `level`.
`producer` is informational but takes no unknown fields, like `game`.

`parent` is never the image itself: images with the same contents have the same id, so an
image recorded with nothing changed takes the parent of the one it equals. Following
parents can still come back to an id (a restore to an earlier state has that state's id
again), so walkers stop at an id they have seen.

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
  "mode": 420,                      // unix bits, at most 0o777; Windows keeps read-only only
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
`sha512`. `mode` is at most 0o777: setuid, setgid and sticky are never recorded (placing
applies only the permission bits anyway), so two images can't differ only by them. `world` is set exactly when the path is `saves/<world>/…`, and names a world in
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
run vanilla, and the flavour is a server setting. A side with no settings at all is the same
as no settings for that side (it is left out, so it gives the same id). `icon` and
`background` are store keys: their `sha256` must be a SHA-256 and their `size` at most
256 MiB.

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
singletons and composition exclusions) are refused, the rest is trusted to be NFC. Known
gaps, trusted rather than checked: Arabic hamza and madda marks after an alef
(U+0653–U+0655), Indic nuktas and two-part vowel signs (Devanagari U+0928 + U+093C, which
NFC writes U+0929; Bengali, Oriya, Tamil, Malayalam, Sinhala) and other scripts'
compositions. No part may have Unicode's default-ignorable characters (U+00AD, U+034F,
U+061C, U+115F–U+1160, U+17B4–U+17B5, U+180B–U+180F, U+200B–U+200F, U+202A–U+202E,
U+2060–U+206F, U+3164, U+FE00–U+FE0F, U+FEFF, U+FFA0, U+FFF0–U+FFF8, U+1BCA0–U+1BCA3,
U+1D173–U+1D17A, U+E0000–U+E0FFF): HFS+ ignores some of them when it compares names, so
`\u200C.lodestone_config` would be `.lodestone_config` there (git's CVE-2014-9390), and
the direction overrides make a name display as another. No part may have the shape of a Windows short (8.3) name (at most 8 bytes
ending in `~` and digits, with at most one extension of at most 3 bytes: `LODEST~1`,
`SESSIO~1.LOC`): NTFS opens a long-named file through its short alias, so `LODEST~1` can be
`.lodestone_config`. The first part can't differ only in letter case from `saves` (it
would land among a client's worlds without being a world file). A file whose name breaks
these rules is withheld as `bad_name`. Other names that differ only in letter case are
valid; the shared crate reports them so a case-insensitive disk can refuse them.

The names below that guard something (never recorded, Lodestone's own `.lodestone*`
files, server-only and client-only files, configs, a server's world folders) match
whatever their letter case, since Windows and macOS's default disks ignore it:
`.LODESTONE_CONFIG` is `.lodestone_config` and `LOGS/` is `logs/`.

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
vanilla `DIM-1`/`DIM1` left inside a split level (or those folders in another letter case),
a server folder named `saves`, a non-world file inside a world's folder, a top-level file
named like a world (`world` the file can't sit beside `world/` the level), a world named
like other server content (`mods`, `config`, `plugins`, `libraries`…). World names are
compared ignoring letter case for this (`World/level.dat` is in the level `world` on most
disks), and a server's worlds must be named as on disk, no two differing only in case.
Materializing maps every path before placing anything, so such an image fails with a
layout error and an empty staging folder.

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
- The client view never carries Lodestone's own files (top-level `.lodestone*`: a server's
  modpack record and history are its bookkeeping, and images from a server can't write
  Lodestone's files) or a server's own state that a client never uses: `plugins/`,
  `bukkit.yml`, `spigot.yml`, `commands.yml`, `help.yml`, `permissions.yml`, `paper.yml`,
  `purpur.yml`, `pufferfish.yml`, Paper's `.console_history` (commands the admin typed)
  and `version_history.json`, `usercache.json`, `usernamecache.json`, Forge's and
  NeoForge's `run.sh`, `run.bat` and `user_jvm_args.txt` (top-level, any letter case). Unlike server-only files these may be
  on a client (a client writes its own `usercache.json`) and stay in a Launcher's images;
  they only never go from a server to players, so a change to them leaves the client
  view's id as it was.
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
does (only `"`, `\` and U+0000 to U+001F, as `\n`, `\t`… or `\u00XX`; U+007F and the C1
controls are written as they are, in UTF-8) and integers in decimal.

## Deltas

```jsonc
{ "from": "<image id>", "to": "<image id>",
  "added": [ ENTRY, ... ], "removed": [ "<path>", ... ], "replaced": [ ENTRY, ... ],
  "changes": [ { "game": GAME }, { "settings": SETTINGS }, ... ] }
```

Between any two images: entries by path (`replaced` when anything about one differs), and
each other field that differs, whole, with its new value. Applying refuses a delta for
another image, paths that aren't where it says, a result whose id isn't `to`, and a result
that isn't a valid image (a delta's entries are checked like any manifest's). `from` and
`to` are empty only between unsealed images; a sealed image refuses a delta that leaves
either out. A delta from elsewhere (a server's answer) is read like a manifest: its shape is
checked first, its lists have the manifest's bounds (and at most 64 `changes`), and its
entries' and `extra` change's unknown fields the same budgets.

## Store

Content-addressed, shared by every instance on one machine:

```
<root>/objects/ab/<sha256>        whole files (small) or chunk lists (large)
<root>/chunks/ab/<sha256>         chunks of large files
<root>/images/<instance>/<id>.json
<root>/images/<instance>/stat-cache.json
<root>/{objects,chunks,images/<instance>}/.incoming-<16 hex>.tmp    writes in flight
```

- Objects are named by the SHA-256 of their uncompressed bytes. Encoding belongs to the
  store, not the manifest: the core stores zstd frames (level 3, checksum and content size
  in the frame, exactly as its backups write them, so its existing objects are read in
  place), the Launcher stores raw files (APFS clones where possible, checked by hash
  before they are kept). Moving an object between stores re-encodes it.
- Files over 1 MiB are split with FastCDC (min 16 KiB, average 64 KiB, max 256 KiB,
  implemented in the shared crate, no new dependency). The object is then a chunk list.
  Region files dedup well because Minecraft rewrites a chunk in place or moves only that
  chunk. FastCDC here is the 2016 paper's, with cut-point skipping and normalized chunking
  level 2: a gear fingerprint `fp = (fp << 1) + GEAR[byte]`, no cut before 16 KiB (those
  bytes aren't hashed), 18 mask bits before 64 KiB and 14 after, a cut at 256 KiB at the
  latest. `GEAR` is 256 outputs of SplitMix64 seeded with `0x4c6f646573746f6e`
  ("Lodeston"); the masks put their bits on every second (18 bits: `0xaaaaaaaaa0000000`) or
  third (14 bits: `0x9249249249000000`) bit from bit 63 down. A cut happens where
  `fp & mask == 0`, counting the cut byte into the chunk.
- A chunk list is text with one spelling, `lodestone-chunks 1\n`, `size <file size>\n`,
  then `<chunk sha256> <chunk size>\n` per chunk in order (decimal, no leading zeros).
  Readers refuse anything else: a size of 1 MiB or less, chunks of 0 or over 256 KiB,
  chunks before the last under 16 KiB, sizes that don't add up. An object whose plain
  bytes are as long as the entry's `size` is the file itself (every file up to 1 MiB, and
  large files the core stored whole before chunking); any other length must be a chunk
  list for that size. A list is under 1% of its file, so the two can't be confused.
  Reading a list reassembles the file and checks each chunk's and the whole file's
  SHA-256. A manifest from elsewhere can name an object with any size, so the store never
  judges an object by the size it is asked about: an object that is no file of that size
  is the asker's error when its own bytes check out (a whole file hashing to its name, or
  a list whose chunks do) and damaged only when they don't.
- Writes: temp file, written out to the drive, read-only, rename; manifests last, after
  the store makes everything written since the last manifest durable (the objects'
  folders flushed, and on Apple platforms one flush of the drive's cache). On Linux and
  Windows each object is flushed on its own (`fsync`, `FlushFileBuffers`); on Apple
  platforms an object gets a plain `fsync(2)` and the batch one `F_FULLFSYNC` before the
  manifest, as git's batch mode does: `F_FULLFSYNC` per 64 KiB chunk made a first snapshot
  about ten times slower (9 MB/s, against 80 MB/s batched). Saving a manifest checks that every
  object its `object` entries and stored files name is present (and touches it like a write
  that finds it, below), and fails if one isn't: an image never names what the store lost.
  Reads verify the SHA-256. Like the core's backups, presence is checked cheaply (a raw
  object's length, a zstd frame's header, a chunk list's header for exactly that size); a
  damaged object is found when read, and deleting it lets the next write store it again
  (only an object damaged by its own bytes is deleted). An object is read whole to parse a
  chunk list only when its first bytes are a list header for exactly the size asked about,
  so a wrong size in a manifest from elsewhere never loads a large whole object into
  memory. A list is complete only when every chunk is there at its size: saving a manifest,
  importing an export and the "what's missing" check before materializing all look at
  every chunk, so a list that lost a damaged chunk (deleted on its own) counts as missing,
  and downloading or importing the file again writes the chunk back.
- A stat cache (path, size, mtime, older than scan start minus 2 s) skips rehashing
  unchanged files, as backups already do; the Launcher gains it too. It is kept per
  instance next to its manifests, keyed by the path relative to the folder walked, and is
  only an optimization: a missing or odd one hashes everything again.
- A manifest is a file named `<id>.json` in its instance's folder (instance ids are 1 to
  100 of `A-Z a-z 0-9 - _`, not a Windows device name); other files there aren't
  manifests. Loading one checks its contents hash to the id in its name. Callers must use
  instance ids that are unique ignoring letter case (the core's are lower-case UUIDs, the
  Launcher's name folders on the same disk): two ids differing only in case share one
  folder on most disks.
- GC: mark and sweep over every manifest; refuses to delete anything while a manifest
  can't be read or has a schema other than 2 (newer, or from before images: those live in
  their own folders); temp files are swept after an hour. Objects named by entries (any
  source: the store may hold a downloadable file's bytes) and by the settings' stored files
  (icon, background) are live, and so are the chunks of live chunk lists. Whether a live
  object is a list is decided from the object, never from the sizes manifests give it: it
  is read when some name could mean a list (a size over 1 MiB other than its length, or no
  size, for objects older formats name), and is a list exactly when it parses as one. A
  name with a size the object doesn't have is that manifest's error and blocks nothing.
  When a live chunk list can't be read, no chunk is deleted. Objects and chunks modified in
  the last hour are kept (with the chunks of such lists), and a write that finds an object
  already there, a snapshot that takes a file from its stat cache, and saving a manifest
  touch it (and a list's chunks) when it is older than 10 minutes, so a snapshot running
  at the same time never loses what it is about to name, and a save after a snapshot longer
  than the hour of grace fails rather than name what was collected. No lock is needed
  between GC and writers: GC renames a file it found old to a temporary name, reads its
  time again and deletes it only if it is still old, putting back one touched meanwhile;
  a claim touches, then checks the file is still there, and fails (so the manifest isn't
  saved) when GC took it. The grace is never under 20 minutes, twice the touch interval,
  or GC could take a file a claim found new enough not to touch. A save doesn't claim
  again what the same store claimed in the last 5 minutes. Objects only older
  formats use are passed in by the caller, which refuses the same way when it can't read
  those manifests.
- Snapshots walk the folder with the side's disk layout. Files never recorded, outside a
  client's game folder, or left out by the image's `excludes` are skipped without a
  withheld record (`excludes` says what they are). `excludes` match canonical paths
  (`saves/<world>/…` on both sides), case-sensitively, with the core's backup pattern
  syntax (`name` at any depth, `a/b` from the root, `*` within a part, `**` across parts,
  trailing `/` for folders). Links are withheld as `symlink`; folders that can't be
  listed, files that can't be read and things that aren't files as `unreadable`; names
  that aren't canonical, files the layout can't read back as themselves and files never
  placed on this side as `bad_name` (a folder with such a name once, not each file in
  it); files that change while read, three tries each, as `kept_changing`. A withheld
  path is canonical where the layout gives one, else as found relative to the game
  folder. Entries record `mode` as the unix bits without setuid, setgid and sticky
  (0o444 or 0o644 on Windows); placing a file applies only those bits, plus the owner's
  write bit, as the core's and the Launcher's restores always did (the game, mods and
  Lodestone must be able to change what a restore put back).
- Materializing places files in a staging folder, never over an existing file, checking
  every byte, and refuses names that differ only in letter case when the staging disk
  ignores case (comparing the disk paths themselves). The staging folder must be on the
  volume of the folder it replaces: the swap is a rename, and the case check looks at the
  staging folder's disk.
- Snapshots hash and store on a few threads (half the processors, at most 4, unless the
  caller says); a backup of a running server asks for one or two.

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

```
image.json            the image, sealed
images/<id>.json      only the image's `content` link
objects/<sha256>      plain bytes of each object entries with source `object` and the
                      settings' stored files (icon, background) name; for a file over
                      1 MiB, its chunk list (a file the store keeps whole is cut on the
                      way out, as a store cuts it)
chunks/<sha256>       the chunks those lists name, each once
```

Entries are deflated with a fixed timestamp, so the same program gives the same file for the
same image, whatever its store's encoding. Different programs may not: deflate's output
depends on its implementation, and the core and the Launcher build different ones, so
exports are compared by their image ids, never their bytes. The file is written as
`<file>.part` and renamed when whole. The reader refuses any other name (folder
entries `objects/`, `chunks/` and `images/` aside), a name twice, local headers that differ
from the central directory, encrypted entries, more than 1,000,000 entries or 64 GB
declared, an image whose files add up to more than 64 GB (entries can share an object and
a list can repeat a chunk, so a small export could otherwise name an image that fills the
disk when placed), manifests over 512 MiB, a manifest whose contents don't hash to its id, a linked
manifest the image doesn't link to, an object the images need that is missing or of the
wrong size, a chunk list that isn't one or names a missing or wrongly sized chunk, and any
object or chunk nothing needs. Import asks the caller whether the disk has room for what
it will unpack, then checks each chunk's and each whole file's SHA-256 before the store
names anything by it.

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
- Secrets stay out of typed settings, but a server's own `server.properties` is an ordinary
  `object` entry of its data images, `rcon.password` and `management-server-secret`
  included. That is fine in the local store; an export that leaves the computer (M6) must
  strip or withhold those properties.

## Migration

- Readers for core backup manifests (schema 1), `.lodesnap` (schema 1), the core's
  `record.json` and `history.json`, and the Launcher's `modpack.json`, `content.json` and
  world backup zips. Old data stays readable; new writes use schema 2.
- GC understands both until everything is converted. Conversion is lazy; old objects are
  never rewritten (core objects are already SHA-256 zstd, Launcher objects SHA-256 raw).
  The image store at the core's backup folder, zstd encoded, reads the core's objects in
  place, and an image store at the Launcher's snapshot folder, raw, the Launcher's. The
  caller passes the objects of the old manifests that remain to the image store's GC as
  also live (`core_backup::Manifest::objects`), and, for an old manifest replaced by an
  image and its rest, the objects only the rest names (`core_backup::Rest::objects`,
  `lodesnap::Rest::objects`: files the image withholds, files beside a Launcher game
  folder).
- **Downgrades never lose images.** Released cores (Lodestone Host 0.6.0-beta.2 on) and
  Launchers (0.1.0-beta.1 on) collect their shared `objects/` against their own manifests
  only, so after going back to one, its first cleanup would delete every object only
  images name (whole files and chunk lists). Before a store writes anything to its folder
  it puts a guard where each old collector looks: `servers/.images/images.json`, a manifest
  of schema 2, which makes the core's collector refuse ("made by a newer Lodestone"), and
  `instances/.images/images.json`, which isn't a snapshot manifest, so the Launcher's keeps
  everything. `.images` is no valid server id, and neither file parses as a backup or a
  snapshot, so nothing else in the old programs sees them. The guard stays for as long as
  images use the folder; while a released version runs it frees no space there, and loses
  nothing. This was chosen over a separate image root (every old object an image reuses
  would have to be copied or linked across, and an image naming an old-root object would
  break when the old collector deleted it) and over a schema-1 list of everything images
  use (rewritten with every image, and losing objects whenever it fell behind). M2's and
  M5's own collection of old manifests skips the `.images` folders. Tested against the
  released collectors' code (`tests/downgrade_guard.rs`).
- A converted core backup restores through the store: `core_backup::disk_layout` is the
  server layout its paths went through, so materializing with it puts every entry back at
  its backup path. Files the image withholds (a name no canonical path has, a vanilla
  `DIM-1` left in a Paper level, a world named only by letter case, a file named like a
  world) aren't entries: their objects are in the store and the rest lists them
  (`core_backup::Rest::unmapped_files`), and a restore that puts back everything the
  backup had places them itself at their disk paths (reading each through the store, which
  checks every byte), or uses the old restorer for backups that have them. The same holds
  for a snapshot's files beside the game folder and withheld game-folder files
  (`lodesnap::Rest`). The old backup is also the first image snapshot's stat cache
  (`core_backup::stat_cache`, its files by server path, trusted up to its scan time), so
  that snapshot reads only what changed.
- A large file the core stored whole stays whole. When a full scan reads it again
  unchanged, the store cuts it into chunks as it does any large file (it can't know the
  file's hash before reading it), finds the whole object already there and keeps it as the
  object; the chunks it wrote are unused, and GC frees them. Only files that change move
  to chunks.
  Writing chunks is cheap once flushes are batched (above), so a full scan of a large
  unchanged file costs one more write of its chunks, freed again by GC; hashing such files
  first would read every new large file twice to save that.
- Round-trip tests: every old format converts to an image and back without changing a byte.
  World backup zips are the exception: they are only read (a zip can't be rewritten byte for
  byte, and images replace them). Byte for byte holds for the fields each format had at
  commit f5ce27ad, which the readers mirror: the Launcher adds defaulted fields to its
  documents without a new version, and the core's files have none, so a field added later
  is dropped by a conversion and its write-back (keeping unknown fields would mean
  buffering them, which is what made reading costly).

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
  each program's own writer formats them). The rest types are serde, so a store can keep a
  rest beside its image once the old file is gone.
- **Old files from elsewhere** (a `.lodesnap` someone sent) are read as manifests are: the
  shape is checked before parsing, only the version field is probed, lists are bounded
  while read, an instance's settings are read within a budget (4,096 values, 64 KiB) and
  other unknown fields are skipped without being kept, and a zip with a name twice or
  local headers that disagree with its directory is refused. A path longer than an image's
  withheld list may show (4096 bytes) stays in the rest with a note instead of failing the
  conversion.
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
  other files only: data; both: full), and a content selection whose files include worlds
  (one from elsewhere) is full. A file without a mode (0, from before the Launcher
  kept modes) gets 0o644, what the Launcher's restore gave it, and the rest remembers it so
  writing back says 0 again. `instance.json` gives the game and the recommended
  memory, and `icon.png` and `background.jpg` the client settings' icon and background (one
  over 256 MiB is left out of the settings, with a note); modes keep only their permission
  bits (0o777; the Launcher's writer never wrote more, and the rest keeps any other bits a
  file from elsewhere had);
  every other instance field is local, reported and kept in the rest, never in the image.
  Files beside the game folder (`content.json`, `modpack.json`, shared options) are kept in
  the rest. `.lodesnap` files are read with the Launcher's limits.
- **The core's `record.json` and `history.json`** become one content image per history
  entry, chained by `parent`, plus one for the record (on the newest entry's base and game;
  the same id when nothing changed since). An entry's parent is the nearest earlier entry
  with another id, never itself. Entries with the same files share one id (a restore to
  entry 1 is entry 1's image again), so a store keeps one manifest for them; the rest says
  which image each entry had, so the history can be written back from a store. They list the files the pack wrote, which is all
  the record knew, with explicit sources: a CDN download `modrinth` (project and version from
  its URL), an override `base` when the pack is the base, anything else `object`. The server's
  own files join the lineage with the first content image the core records. Neither file has
  a version, and the core ignores unknown fields, so a newer file can't be refused. The
  record keeps neither the pack index's `env` nor which overrides folder a file came from,
  so every converted entry's `env` is unknown: such an image must never be served as a
  client view before its env is resolved from the base pack (M2, M3), or players would get
  server-only pack mods and `server-overrides` files.
- **The Launcher's `modpack.json` and `content.json`** become one content image: the base is
  the pack (`modrinth_pack`, `curseforge_pack`, or `loader` for an instance not made from a
  pack); a file `content.json` knows on Modrinth's CDN is `modrinth` with its env from the
  project's `client_side`/`server_side`, on CurseForge `curseforge`, an unchanged pack file
  `base`, anything else `object`. `content.json` (like the core's `record.json`) keys files
  by their enabled name; the caller says which are disabled on disk (`Known::disabled`),
  and their entries have the disabled name, as on disk, so placing the image doesn't
  enable them. Old history entries keep the names they recorded. The game is the caller's
  (neither file has it).
- **World backup zips** become data images of `saves/<world>/…`, each entry hashed straight
  from the zip, with the Launcher's limits; entries that fail a check are withheld. Zip
  times are local without a zone, so entries have no `modified`.

## Milestones

1. Shared crate `crates/lodestone-image`: manifest, identity, paths, views, store, FastCDC,
   v1 readers, property tests. (Done first: manifest, identity, paths and disk layouts, env,
   views, deltas. Then: store, FastCDC, snapshots, materializing, GC and exports; v1
   readers.)
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
