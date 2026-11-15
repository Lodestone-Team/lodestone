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
| Launcher `modpack.json` and its update plan | `modpack.json` stays as the updater's current state and names the base; the history is the lineage of content images |
| `LDS2:` share codes (`launcher-core/src/content/share.rs`) | A text encoding of an image whose files can all be downloaded |
| The server's client pack (`core/src/mods/client.rs`) | The client view of the server's content image (the `.mrpack` client pack stays for other launchers) |

Images replace these formats outright. Nothing reads, converts or keeps working data in
them: Lodestone was never released with backups, snapshots or modpack history in them that
must be kept, so each old format's code and data go when its milestone lands.

Decisions this builds on:

- A Launcher instance's **game version and loader type are fixed** when it is made. Only
  the loader version can change in place, and changes a modpack update or a linked server
  brings in. "Copy with another version or loader" makes a new instance. The core keeps
  its existing in-place Minecraft version change until it is retired; it records a content
  image like any other change ("Changed Minecraft version to X").
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
number makes the manifest invalid, since a reader couldn't write it back as it was. A kept
field can never have a known field's name (one built in code, or a delta's `extra` change):
that image is invalid, since its manifest would hold the name twice and couldn't be read
back, and its canonical form always uses the known field.
Anywhere else (`game`, `producer`, `base`, a `source`, an `env`, a world, a withheld
file) an unknown field makes the manifest invalid with an error naming it, so a new field
there needs a new schema: dropping it would change the id, and the image would fail its
check with a misleading mismatch instead. Informational values it doesn't know (a withheld reason, an
`env_from` input, a loader, a flavour) are kept as text. An unknown `source`, `kind`,
`base` or `env` value makes the manifest invalid, since nothing can act on it safely. A
manifest of any other schema is refused with its own error.
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
removed paths and withheld files, 10,000 worlds, 1,000 excludes of at most 256 characters
(with 2,000 parts other than `**` in all, at most 64 of them with text between two stars,
checked when the manifest is validated), 16 `env_from` inputs,
100,000 items in each setting's list or map).
A manifest from elsewhere comes in a compressed export, so without these a small file
could exhaust memory.

A `content` image has no worlds (and so no world files); `data` and `full` images may.

`parent`, `created`, `producer`, `label`, `projected_from` and entries' `modified` are
informational. `entries[].modified` is optional, `mode` defaults to 420 and `env` to
unknown on both sides. `null` is the same as a missing field everywhere: a field with a
default (`settings`, `entries`, `mode`, `env` and each of its sides, `producer` and its `app` and `version`, a
setting's list…) reads `null` as that default, and a missing `rules` reads as 1, the first
rules (not the reader's newest, which would change a manifest's meaning and id as rules
are added). Only the required fields can't be `null`: `schema`, `kind`, `game` and its
`minecraft` and `loader`; an entry's `path`, `size`, `sha256` and `source`; a world's
`name`; a withheld file's `path` and `reason`; an icon's or background's `sha256` and
`size`; every field of a `source` and of a `base`; and in settings an op's or whitelisted
player's `uuid` and `name` and an op's `level`.
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
the pack there. The core's content images list them anyway, with explicit sources (see "M2
decisions").

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
{ "path": "mods/blocked.jar", "reason": "curseforge_not_downloadable" | "symlink" | "unreadable" | "bad_name" | "kept_changing" }
```

`path` is the file's path as found, which may not be a valid canonical path (that may be
why it was withheld): shown, never placed. `bad_name` also covers a path the side's disk
layout can't place or read back as itself. Files left out by `excludes` aren't withheld
(`excludes` says what they are).

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
working files (restore folders, uploads and downloads in flight, staging, the modpack
updater's journal), and `.lodestone_config` (a server's identity). Precisely: the top-level
folders `logs`, `crash-reports`, `cache`, `.cache`, `natives`, `.fabric`, `.mixin.out` and
`debug`; `session.lock` and any name starting with `.lodestone-` at any depth (Lodestone's
working names; its lasting files are named `.lodestone_…`); `.lodestone_modpack/update.json`
and `.lodestone_modpack/{staging,snapshot}/`; the top-level file `.lodestone_config`. A
manifest with an entry there is invalid, and a restore leaves these files as they are. The
modpack updater's journal describes the files on disk when it was written: put back over
other files by a restore, it would have the updater's recovery remove the wrong ones.

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
  own bookkeeping, and images from a server can't write Lodestone's files) or a server's own state that a client never uses: `plugins/`,
  `bukkit.yml`, `spigot.yml`, `commands.yml`, `help.yml`, `permissions.yml`, `paper.yml`,
  `purpur.yml`, `pufferfish.yml`, Paper's `.console_history` (commands the admin typed)
  and `version_history.json`, `usercache.json`, `usernamecache.json`, Forge's and
  NeoForge's `run.sh`, `run.bat` and `user_jvm_args.txt`, `versions/` (where vanilla's
  bundler and Paper unpack the server jar) and every `*.jar` at the top level (old Forge's
  `forge-<version>.jar` and `minecraft_server.<version>.jar`; a client loads mods from
  `mods/`) (top-level, any letter case). Unlike server-only files these may be
  on a client (a client writes its own `usercache.json`) and stay in a Launcher's images;
  they only never go from a server to players, so a change to them leaves the client
  view's id as it was. At any depth, the client view also leaves out disabled files (names
  ending in `.disabled`) and names starting with `.lodestone` (the core's
  `mods/.lodestone_client_mods.json`), any letter case (folded like every guarding name,
  so `.lodeſtone…` counts: NTFS upper-cases `ſ` to `S`).
- A view of a view is that view filtered again by the same rules (for a view computed here,
  the same view), its informational fields dropped as any view's are: a view received from
  elsewhere is checked by comparing its id with its own view's.
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

The canonical form always writes these fields, with their defaults when unset: `schema`,
`kind`, `game` (`minecraft` and `loader`), `settings` (`{}` when neither side has any),
`entries`, `removed`, `worlds`, `excludes` and `withheld` (`[]`), and `rules`; on each entry
`path`, `size`, `sha256`, `mode` (420), `source`, `env` (both sides, `unknown` by default)
and `env_from` (`[]`); in a side's settings that is there, the server's `properties` (`{}`),
`ops` and `whitelist` (`[]`) and the client's `resource_packs` (`[]`), and each op's
`bypasses_player_limit` (`false`). Every other field is written only when it has a value
(`side` only on views). For example, a data image with one file, one world, one op and
one withheld file:

```json
{"entries":[{"env":{"client":"unknown","server":"unknown"},"env_from":[],"mode":420,"path":"mods/a.jar","sha256":"<64 hex>","size":10,"source":{"object":{}}}],"excludes":[],"game":{"loader":"fabric","loader_version":"0.16.10","minecraft":"1.21.1"},"kind":"data","removed":[],"rules":1,"schema":2,"settings":{"server":{"ops":[{"bypasses_player_limit":false,"level":4,"name":"n","uuid":"u"}],"properties":{},"whitelist":[]}},"withheld":[{"path":"x","reason":"symlink"}],"worlds":[{"name":"w"}]}
```

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
checked first, its lists have the manifest's bounds (and at most 64 `changes`, each field
at most once, so a delta holds at most about two images' worth of lists), and its
entries' and `extra` change's unknown fields the same budgets (an `extra` change naming a
known field is refused).

## Store

Content-addressed, shared by every instance on one machine:

```
<root>/objects/ab/<sha256>        whole files: only ever the file's own bytes
<root>/lists/ab/<sha256>          chunk lists of large files, by the file's SHA-256
<root>/chunks/ab/<sha256>         chunks of large files
<root>/images/<instance>/<id>.json
<root>/images/<instance>/stat-cache.json            the backups' (data) stat cache
<root>/images/<instance>/stat-cache.<name>.json     other named stat caches (the core's "content")
<root>/images/<instance>/content-state.json         the core and the Launcher: current content image and baseline
<root>/images/<instance>/pack.json                  the core: the pack a server is built on, and its files
<root>/images/<instance>/backups.json               the core: the server's backups, each naming its data image
<root>/images/<instance>/backup-policy.json         the core: the backups' schedule, retention and excludes
<root>/images/<instance>/restore-journal.json       the core: a restore's swap in progress
<root>/images/<instance>/save-off.marker            the core: a live backup has the server's saving off
<root>/images/collection.lock                       held by GC (see below)
<root>/{objects,lists,chunks,images/<instance>}/.incoming-<16 hex>.tmp    writes in flight
```

The core's root is its backup folder (`<lodestone>/backups`); the Launcher's is
`<data>/image-store/`, beside its snapshot store (see "Launcher store decisions").

- Objects are named by the SHA-256 of their uncompressed bytes. Encoding belongs to the
  store, not the manifest: the core stores zstd frames (level 3, checksum and content size
  in the frame), the Launcher stores raw files (APFS clones where possible, checked by hash
  before they are kept). Moving an object between stores re-encodes it.
- Files over 1 MiB are split with FastCDC (min 16 KiB, average 64 KiB, max 256 KiB,
  implemented in the shared crate, no new dependency). The file is then stored as a chunk
  list in `lists/`, never in `objects/`, which only ever holds a file's own bytes, so
  nothing has to tell a list from a file there. A file of up to 1 MiB is present when
  `objects/` has it, a larger one when `lists/` has its list.
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
  chunks before the last under 16 KiB, sizes that don't add up. In the store a file of up to
  1 MiB is a whole object in `objects/` and a larger one a list in `lists/`; in an export,
  which has one `objects/`, an object whose plain bytes are as long as the entry's `size` is
  the file itself and any other length must be a chunk list for that size. A list is under 1% of its file, so the two can't be
  confused. A list from elsewhere is checked as it is read, a line at a time, so one that
  names a small chunk millions of times costs no memory to check.
  Reading a list reassembles the file and checks each chunk's and the whole file's
  SHA-256. A manifest from elsewhere can name an object with any size, so the store never
  judges an object by the size it is asked about: an object that is no file of that size
  is the asker's error when its own bytes check out (a whole file hashing to its name, or
  a list whose chunks do) and damaged only when they don't.
- Writes: temp file, handed to the drive, read-only, rename; manifests last, after the
  store makes everything written since the last manifest durable with one flush of the
  drive's cache, as git's `core.fsyncMethod=batch` does. A flush of the device per 64 KiB
  chunk made a first snapshot about ten times slower on a Mac (9 MB/s, against 80 MB/s
  batched) and six to eight times slower on Linux (433 MB in 15.5 to 19.9 s on one thread,
  against 2.3 s plus 1.9 s to save with one flush). On Apple platforms each file gets a
  plain `fsync(2)` and the batch one `F_FULLFSYNC`; on Linux the batch is one `syncfs(2)` of
  the store's file system (which writes out every file and folder and flushes the drive);
  on Windows each file gets `NtFlushBuffersFileEx` without a cache flush and the batch one
  `FlushFileBuffers`, which flushes the volume (`FlushFileBuffers` per file where the first
  isn't supported). Saving a manifest flushes the drive's cache even when that store wrote
  nothing itself: the objects it names may be ones another store value or process wrote
  and hasn't flushed yet. Small files (under 64 KiB) are written from the bytes already
  read, never cloned. Materializing a staging folder that is swapped in next ends the same way:
  each file and folder handed to the drive, one flush at the end. Saving a manifest checks that every
  object its `object` entries and stored files name is present (and touches it like a write
  that finds it, below), and fails if one isn't: an image never names what the store lost.
  Reads verify the SHA-256. Presence is checked cheaply (a raw
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
  unchanged files (the core's backups use it); the Launcher gains it too. It is kept per
  instance next to its manifests, keyed by the path relative to the folder walked, and is
  only an optimization: a missing or odd one hashes everything again. Snapshots that walk
  different parts of a folder keep one each (`Store::load_stat_cache_named`: `data` is
  `stat-cache.json`, any other name of 1 to 32 of `a-z 0-9 -` is
  `stat-cache.<name>.json`), so a content snapshot never replaces the backups' cache and
  makes the next backup hash every world again.
- A manifest is a file named `<id>.json` in its instance's folder (instance ids are 1 to
  100 of `A-Z a-z 0-9 - _`, not a Windows device name); other files there aren't
  manifests. Loading one checks its contents hash to the id in its name. Listing can read
  just a manifest's header (`Store::load_header`: id, kind, parent, created, label and
  content link, the entries skipped unbuilt and nothing checked against the id), which is
  what choosing what to keep and listing a history need; whatever is placed comes from a
  manifest loaded whole. Callers must use
  instance ids that are unique ignoring letter case (the core's are lower-case UUIDs, the
  Launcher's name folders on the same disk): two ids differing only in case share one
  folder on most disks.
- GC: mark and sweep over every manifest; refuses to delete anything while a manifest
  can't be read or has a schema other than 2; temp files (only the store's
  own `.incoming-*.tmp`) are swept after an hour. The store's folder holds nothing else that
  names objects, so manifests are all it marks. Objects named by entries (any
  source: the store may hold a downloadable file's bytes) and by the settings' stored files
  (icon, background) are live, whole and as lists, and so are the chunks of live lists.
  Whether a live object has a list is decided from the store, never from the sizes
  manifests give it: its list in `lists/`, if there is one, is read whatever size names
  it. A name with a size neither the whole object nor the list has is that manifest's
  error and blocks nothing. Marking keeps each object once with the sizes it is named
  with, so its memory follows the distinct objects, not the manifests (backups share most
  of theirs). When a live chunk list can't be read, no chunk is deleted. Objects, lists
  and chunks modified within the hour before the collection started are kept (with the
  chunks of such lists): age is judged against when marking began, never when the sweep
  reaches a file, so a manifest saved during a long sweep, which wasn't marked, keeps its
  objects (each was new when marking began, or the save touched it). A write that finds an object
  already there, a snapshot that takes a file from its stat cache, and saving a manifest
  touch it (and a list's chunks) when it is older than 10 minutes, so a snapshot running
  at the same time never loses what it is about to name, and a save after a snapshot longer
  than the hour of grace fails rather than name what was collected. The store's own writers
  need no lock: GC renames a file it found old aside (`<name>.collecting-<16 hex>`, which
  keeps its name so the sweep at the next start puts back one a crash left there), reads
  its time again and deletes it only if it is still old, putting back one touched meanwhile;
  a claim touches, then checks the file is still there, and fails (so the manifest isn't
  saved) when GC took it, and so does a touch that doesn't land (a file another user owns).
  The grace is never under 20 minutes, twice the touch interval, or GC could take a file a
  claim found new enough not to touch. A save doesn't claim again what the same store
  claimed in the last 5 minutes by the wall clock (objects age by it; a monotonic clock
  stops while a laptop sleeps), and forgets those claims once the manifest is written, so a
  long-lived store holds none between snapshots. A caller that relies on objects without
  claiming them (reading an image's files while its manifest may be deleted meanwhile: a
  restore from a backup that retention could prune) holds collections off with the
  collection lock, an operating-system file lock on `images/collection.lock`: it holds it
  shared (`Store::hold_off_collection`) and GC holds it exclusively for its whole run, so
  two collections never run at once either. A thread holding the shared lock must not
  start GC (it would wait for itself).
- Snapshots walk the folder with the side's disk layout. Files never recorded, outside a
  client's game folder, or left out by the image's `excludes` are skipped without a
  withheld record (`excludes` says what they are). `excludes` match canonical paths
  (`saves/<world>/…` on both sides), case-sensitively, with the core's backup pattern
  syntax (`name` at any depth, `a/b` from the root, `*` within a part, `**` across parts,
  trailing `/` for folders); a pattern outside that syntax (a backslash, a leading `!`, an
  empty part) makes the manifest invalid, since a snapshot would ignore it and it would
  claim to have left something out. All patterns together have at most 2,000 parts other
  than `**`, and at most 64 of those have literal text between two stars (`*ab*`,
  `a*b*c`); more makes the manifest invalid, and a snapshot fed more from elsewhere ignores
  the patterns past either budget. All patterns run as one automaton over path parts,
  and the walk reuses each folder's state for what is in it. A name is matched against
  one part's glob without backtracking (the glob split at its stars; the first and last
  pieces at the name's ends, each piece between found leftmost with a linear search).
  Every glob can be live at once (`**/a*b/a*b/…` under a chain of `ab` folders), so the
  budgets are what bound reading a part: at most 2,000 globs that compare the name's ends
  (a length check and at most the glob's literal bytes) and 64 that search a name of at
  most 255 bytes, with patterns of at most 256 characters (1,024 bytes), however deep the
  folders or star-heavy the patterns. With both budgets used up that is about 40 µs per
  new 255-byte part in a release build on an Apple silicon Mac, nearly all of it the
  searches, and a walk pays it once per new part of its paths. Links are withheld as
  `symlink`; folders that can't be listed, files that can't be read and things that aren't
  files as `unreadable`; names
  that aren't canonical, files the layout can't read back as themselves and files never
  placed on this side as `bad_name` (a folder with such a name once, not each file in
  it); files that change while read, three tries each, as `kept_changing`. A withheld
  path is canonical where the layout gives one, else as found relative to the game
  folder; one longer than a withheld list may show (4096 bytes, which Windows can list) is
  left out of it and reported to the caller instead, so it can't make every snapshot of
  the folder invalid. Entries record `mode` as the unix bits without setuid, setgid and sticky
  (0o444 or 0o644 on Windows); placing a file applies only those bits, plus the owner's
  write bit, as the core's and the Launcher's restores always did (the game, mods and
  Lodestone must be able to change what a restore put back).
- Materializing places files in a staging folder, never over an existing file, checking
  every byte, and refuses names that differ only in letter case when the staging disk
  ignores case (comparing the disk paths themselves). The staging folder must be on the
  volume of the folder it replaces: the swap is a rename, and the case check looks at the
  staging folder's disk.
- Snapshots hash and store on a few threads (half the processors, at most 4, unless the
  caller says), and claim what the stat cache gives on the same threads; a backup of a
  running server asks for one or two.

## When images are recorded

| Where | Trigger | Image |
| --- | --- | --- |
| Core | A Lodestone action: Mods tab install, remove, enable, disable, update; modpack update or restore; upload, edit or delete in content folders; a "For players" mark; a loader version change | Content image per operation, labelled |
| Core | Server about to start, if a quick check finds content changed outside Lodestone | Content image "Changed outside Lodestone" |
| Core | Backup (scheduled or manual) | Data image, linked to the current content image |
| Launcher | A Launcher action: install or remove content, apply a server update, update the pack, loader version change, instance creation or import; the first start with images, for each instance from before (see "First images" in "Launcher store decisions") | Content image per operation |
| Launcher | Game about to launch, if a quick check finds content changed by hand | Content image |
| Launcher | Snapshot on demand; before risky operations (content updates, modpack updates, server updates, loader version changes); after a play session only if the user turns it on | Data or full image |

One image per operation, not per file: a job (Update all) is one image, and changes
within a few seconds of it are merged in. An image is written only after the operation
ended: once it succeeded, or, in the Launcher, once an action that changed files failed or
was cancelled part way, under a label that says so (see "Launcher store decisions"). Files
the game or a running server writes (`options.txt`, logs, configs a mod rewrites at
startup) don't make content images of their own; snapshots and backups capture them, and
they join the next content image an operation records. The images that take them in
before an operation are the core's record before a modpack update or restore and the
Launcher's before a modpack or server update, which must hold every file the change
replaces ("Before a modpack update" or "Before a modpack restore" when nothing changed
outside Lodestone and the backup excludes didn't change).

## Retention

- Content images are the lineage, which is also the modpack history: the newest 30, the
  current one and its parent, and every image a data image links to are kept. Manifests only; objects are
  shared.
- Data images: the core's backup rules (keep last N, hourly, daily, weekly buckets;
  manual and pinned never deleted; safety images before a restore kept 30 days), never
  the content lineage's. The Launcher offers the same, per instance.
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
| World and its mods | The world plus the content it ran with, in one swap (the core's `world_and_mods`: the content files of the data image itself, which needs its content link) |
| Everything | The whole image (the core's current Everything scope) |
| As new | A new server or instance beside the original |

When the current content differs from a world's linked content, the restore will say what
differs and offer to restore it too (the Launcher's restore, M5; the core's world restore
doesn't yet). Restore uses the core's journaled swap (staging,
rename aside, roll back on error, replay at startup); the Launcher adopts the same journal.

## Sharing

An export is a zip: `image.json`, the referenced manifests (content image of a data image),
and `objects/` holding only objects whose `source` is `object`. Import checks every path and
hash (entry and size limits, no duplicates, free space) and always creates a new instance or server first; launch settings are reset unless chosen,
as today.

```
image.json            the image, sealed
images/<id>.json      only the image's `content` link
objects/<sha256>      plain bytes of each object entries with source `object` and the
                      settings' stored files (icon, background) name; for a file over
                      1 MiB, its chunk list
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
- Text in an image from elsewhere that a UI shows (withheld paths, which may hold any
  character a disk allows; labels; level names; producer versions) goes through
  `display_text`, which writes control characters, line separators and invisible format
  characters such as direction overrides as `\u{…}`, so a name can't span lines or show
  as another. Validation doesn't refuse them: real names have them.
- CurseForge files whose authors block distribution travel only as references.
- Secrets stay out of typed settings, but a server's own files are ordinary `object`
  entries of its images: `server.properties` (`rcon.password`,
  `management-server-secret`) in its data and content images, and the config files in
  each plugin's folder (database passwords, bot tokens) in its content images. That is fine
  in the local store. The objects endpoint (M3) serves only objects the requested client
  view names, never any object of the store by its hash; an export that leaves the
  computer (M6) must strip or withhold those files.
- The core's history API returns labels and paths as they are; a client that shows them
  writes their control and direction characters visibly (as `display_text` does).
- A Launcher content image takes the whole `config/` folder and every file a pack lists.
  Some client mods keep account tokens there (an account switcher's accounts file), and
  the game rewrites them into the next image, so up to 30 images may keep a token after
  the mod dropped it. That is fine in the local store, which is readable by its owner
  only on Unix (mode 0700) and in the default data folder on Windows (its user profile's
  permissions); a data folder moved elsewhere (another drive, an exFAT disk) has whatever
  protection its new place gives, like the rest of the data folder (`accounts.json`
  included). Any image that leaves the computer (milestone 4's second half, and milestone
  6) must strip or withhold such files. Pack files never include the files the game
  writes, `command_history.txt` (commands typed, a server's `/login` password among them)
  and `hotbar.nbt` included, and an image that leaves the computer must never carry those
  either.

## Milestones

1. Shared crate `crates/lodestone-image`: manifest, identity, paths, views, store, FastCDC,
   property tests. (Done first: manifest, identity, paths and disk layouts, env, views,
   deltas. Then: store, FastCDC, snapshots, materializing, GC and exports.)
2. Core: backups and modpack history become images outright. Backups are data images with
   their content link and excludes; modpack history is the content-image lineage; content
   images on Lodestone actions and before start; Paper world layout. The schema-1 backup
   store and the modpack history's own storage are removed, not converted.
3. Core: client view and delta endpoints; objects endpoint. The `.mrpack` client pack stays.
4. Launcher: build and update instances from a server image, replacing the client pack sync;
   content images on actions and before launch. Done in two parts: first the Launcher's
   image store, with content images on its actions and before launch (see "Launcher store
   decisions"); then server images in place of the client pack sync.
5. Launcher: snapshots become images (exports drop downloadable jars), `LDS2:` as an
   encoding; world snapshots on demand and before risky operations, after sessions
   optional; restore with linked content. No `.lodesnap` import and no conversion of the
   old snapshot store or world backup zips: they are removed.
6. Moving images between computers (Host to Host, Launcher to Host, Host to Launcher,
   Launcher to Launcher) and a full-image option in the UI.

Nothing converts old data in any milestone.

### M2 decisions

Decisions milestone 2 made where this document left the core's behaviour open. Each part
of M2 adds its own subsection.

#### Foundation: the core's images service (`core/src/images/`)

- **One store, one collector.** The core's image store is the backup folder, zstd encoded,
  owned by `ImageService`. Its collector is the store's own GC: `ImageService::request_gc`
  runs one in the background (one at a time; a request while one runs makes it run once
  more when it ends); `ImageService::request_gc_after_pruning` does the same once pruning
  deleted images. No lock of the core's is taken around it: the store's writers claim what they rely on, and a caller
  that relies on objects without claiming them holds the store's collection lock shared
  (`Store::hold_off_collection`).
- **What content is.** Content is an allowlist (`images/scope.rs`), since data is "worlds and
  everything else": everything in `mods/`, `config/`, `defaultconfigs/`, `kubejs/`,
  `libraries/` and `versions/`; `plugins/*.jar` and `*.jar.disabled` and the config files
  (`yml`, `yaml`, `toml`, `json`, `json5`, `properties`, `conf`, `cfg`, `txt`) directly in a
  plugin's folder (deeper plugin folders hold what plugins write while the game runs); the
  top-level `*.jar`, `server.properties`, `ops.json`, `whitelist.json`, `run.sh`,
  `run.bat`, `user_jvm_args.txt`, `bukkit.yml`, `spigot.yml`, `paper.yml`, `purpur.yml`,
  `pufferfish.yml`, `commands.yml`, `help.yml` and `permissions.yml`. Never content: any name
  starting with `.lodestone` at any depth (folded, like every guarding name; the "For
  players" marks become each jar's `owner_mark`) and world files. Names match ignoring ASCII
  case. Not `eula.txt`: accepting the EULA is server state, not a content choice, and
  accepting it through the API is no operation. The rules have a version (`SCOPE_RULES`,
  1); a baseline taken under other rules is taken again rather than compared.
- **Snapshots can be narrowed** (shared crate): `SnapshotOptions::filter` decides per file or
  folder (`Candidate`: disk path, canonical path when there is one, folder or not) what the
  snapshot takes at all; what it turns down is skipped silently and is not written into
  `excludes`. A folder withheld for its name is one withheld record and isn't walked.
- **One way to find a server's worlds.** `ServerLayout::discover` (the level, then each
  top-level folder with a `level.dat` that keeps the layout valid; an unused level name when
  `level-name` can't be a world) is what the core's backups and content images use.
- **Backup excludes as image excludes.** A policy's server-disk patterns are translated to
  canonical ones (`images/excludes.rs`): non-world paths as they are, `<world>/x` to
  `saves/<world>/x`, a Paper level's `<level>_nether/DIM-1/x` to `saves/<level>/DIM-1/x` and
  its other files to `saves/<level>/.bukkit_nether/x` (`_the_end` likewise), a root glob
  that matches worlds also as `saves/<world>/…`. A translation may leave out less than the
  policy did, never more: a pattern with no safe form (one that could match `saves`, or the
  split level's own folder, whose canonical folder also holds the Nether and the End) is left
  out of the image's excludes, and matching becomes case-sensitive. Tested over every pattern
  shape against the backups' own matcher.
- **Content images in the core.** An operation (`ImageService::begin`) holds the
  instance's content gate shared while it changes files; a record holds it alone, so it
  never reads a half-written file (tokio's fair lock makes new operations wait for a waiting
  record). So no operation holds the gate across work whose length a client controls: an
  upload streams into temporary files first, and a long job (a copy, an unzip, a zip, a
  Mods-tab install, which asks Modrinth and downloads) is begun with
  `ImageService::begin_job`, which holds no gate (a record meanwhile may take its
  files half written; the record of its own label, after it succeeded, takes them whole; when
  a record of another operation took them in whole before the job succeeded, the job's record
  finds nothing changed and adds the job's label to that image, saving its manifest again
  under the same id, so "Added JEI" isn't lost; only when the entries under what the job
  wrote differ between the image current before the first new image recorded since it began
  and that image, though: otherwise the job changed no content, such as an unzip of a world,
  a copy into a folder the policy excludes or an install that rewrote the same bytes, and its
  label is dropped like any operation's that changed nothing, whatever else was recorded
  meanwhile. A job says what it wrote (`Operation::wrote`) once it is done, as it actually
  wrote it: a copy the paths it placed and a zip its archive, each at the free name it took
  when the one asked for was taken (`a.toml` copied into its own folder is `a_1.toml`), an
  unzip what it placed in the destination, an install the mods or plugins folder; over 1,024
  paths (an unzip of a flat archive) stand for the folders they are in, and when those are
  still more, for any content file, and each entry is looked up by its path and its folders,
  so the comparison, done with the content gate held, takes as long however much a job wrote;
  when that image was pruned meanwhile, the label is kept). A
  modpack update or restore begins with `ImageService::begin_exclusive`, which waits for
  every Files API operation (not the jobs, which hold no gate) and keeps new ones out, so
  none lands between its last check of the files and their placement; placing a new file
  never replaces one that appeared since (a hard link, which fails when the name is taken),
  so a job writing meanwhile makes the change fail rather than lose the job's file. A caller holding other locks (a backup) waits for the gate a bounded
  time (`flush_within`, `record_now_within`); once a record has the gate it runs to its
  end, so it never loses the labels it took. Successes are merged until 3 s pass without one, or 30 s after the first at the
  latest; a failed operation (dropped without `succeed`) records nothing. A record snapshots
  the content scope with the policy's translated excludes that can reach content (written
  into the image's `excludes`; a pattern anchored under a path where nothing is content, a
  world's `saves/<level>/…` or `/logs/`, is left out before the patterns are cut to an
  image's budgets, so editing it changes no content image, nor which content patterns fit) and the `content` stat cache, on one thread while the server runs; the image's
  game is the server's (Paper and Spigot: `vanilla`), its settings only the flavour for now,
  and its `parent` the current image. Recording the same contents again writes no manifest.
  Every file is listed as an entry, the pack's too, with explicit sources: `modrinth` for a pack download whose bytes are the pack's (or a file whose
  SHA-512 Lodestone matched to a Modrinth CDN file while installing or listing mods), `base`
  for another unchanged pack file, `object` otherwise. A pack file is matched by the SHA-256
  `pack.json` has for it; one without (as the modpack code writes them) is matched by size
  and SHA-1, read from disk once, and its SHA-256 and size are then written into `pack.json`
  under the record's gate, so later records read no pack file. A mod the Mods tab disabled
  is the pack's file under its enabled name. The image's `base` is the pack in
  `pack.json` (written by the modpack code), or none (not `loader`) when there is none, since
  the core can't tell a plain server from a pack server whose pack wasn't recorded yet.
- **`env` without the network.** The inputs are the owner's mark (marked for players:
  client `required`; left out of the client pack: client `unsupported`), the pack index's
  `env` from `pack.json` for files that still have the pack's bytes, what Modrinth said
  about the file's version when Lodestone fetched it anyway (kept with the content state by
  SHA-512, so ids don't change across restarts), and the jar's `fabric.mod.json`. Nothing is
  fetched while recording; an unknown input stays unknown. A jar's SHA-512 and own sides are
  kept in memory by its SHA-256, so a record scans only new or changed jars.
- **Order of writes.** Manifest (the store flushes what it names, then temp file and
  rename), then `content-state.json` (pointer and baseline), then the content stat cache,
  then pruning. A crash in between leaves the old pointer; the next record finds the same id
  and moves it. A pointer whose manifest went missing is saved again. A save that fails as
  missing (a collection took an object the snapshot relied on) is recorded again once. The
  small files are written through `.incoming-<16 hex>.tmp`, which the store's sweep deletes
  after a crash.
- **The check before a start.** It records what is queued, then compares each content
  file's size and modification time with the baseline (no file is read), under the canonical
  excludes the baseline was taken under (kept with it in `content-state.json`), and records
  "Changed outside Lodestone" when they differ; when they don't but the backup policy's
  excludes that can reach content changed since (which changes what an image holds, and its
  id), it records "Changed the backup excludes". It waits at most 10 s for the last stop's
  content work and then at most 10 s for an operation in progress, and never fails a
  start; but a record that already has the gate when that wait ends (a merged record that
  began just before) runs to its end first, since noting the start takes the instance's lock
  the record holds, and a record is never cut short. When a server stops, the baseline is
  taken again, under the same excludes,
  without an image, so what the server wrote while it ran joins the next image an operation
  records; a core that died while its server ran (`running_since` still set) does the same
  at its next start. A server with no content image gets "Content when Lodestone began
  recording": at startup when it is stopped (one at a time, one thread), else when it stops.
  The baseline is what the walk found, so a file that can't be read doesn't make every
  check fire.
- **Retention.** Under the instance's lock: keep every data and full image (backups: the
  backup rules delete them, never pruning) and the `content` each links to, the image a
  modpack restore in progress goes back to (`ImageService::keep_while_restoring`), the
  current pointer and its parent (the image recorded before a modpack restore, which holds
  the only copy of what the restore took off, even when a clock that went back ranks it
  among older images), and the newest 30 content images; delete the other content
  manifests.
  While any manifest can't be read, delete nothing: it may be a backup linking to anything.
  It reads manifest headers, not whole manifests (a data image lists every file of the
  server, and pruning follows every record), and a data image's link is part of its id, so
  it is read once per id and kept in memory.
  "Newest" is by `created`, and within one second by the lineage: going back from the
  current image parent by parent lists the images newest first, since each record takes
  the image current before it as its parent. Going back to earlier content saves an image
  again with its new parent, so parents can form a circle; the walk stops at an image it
  has seen, and images it doesn't reach come after, the one with more ancestors first. That
  is the order the modpack history lists them in, so the history shows what is kept.
  Pruning that deleted something asks for a collection, at most every 15 minutes: past 30
  images nearly every record prunes one, which frees a few small files, while a collection
  reads every manifest of every server; what a skipped request would have freed goes at
  the next collection.
- **Deleting a server** deletes `images/<uuid>/` (`ImageService::delete_instance`); its
  objects go at the next collection.


#### Content: recording on Lodestone's actions

- **Which actions record.** Each of these begins an operation (`ImageService::begin`) after its
  permission checks passed and right before it changes files, and succeeds only once the change
  did; a failed or refused request records nothing.
  - Mods tab (`handlers/instance_mods.rs`), inside the mods lock: install ("Added <title>", or
    "Updated <title>" when it took an older version's place), upload ("Uploaded <file>" or
    "Uploaded N mods"/"N plugins"), enable and disable ("Enabled x.jar", "Disabled x.jar"),
    remove ("Removed x.jar"), and the "For players" mark ("Marked x.jar for players", "Marked
    x.jar as not for players"). The mark changes no content file, only the jar's client `env`,
    so it makes a new image too.
  - Files API (`handlers/instance_fs.rs`): write ("Edited <path>"), mkdir and new ("Created
    <path>"), rm and rmdir ("Deleted <path>"), move ("Moved <a> to <b>", when either end is
    content), upload ("Uploaded <path>", "Uploaded N files to <dir>"), and the jobs: copy
    ("Copied <a> to <dest>", "Copied N files to <dest>"), unzip ("Unzipped <archive>") and zip
    ("Zipped to <archive>", when the archive lands among the content), each succeeding when its
    job ends with success and holding no gate while it runs (`begin_job`). An upload writes
    each file under a temporary name (`.lodestone-upload-<random>.tmp`, which no image
    records, deleted when the request ends or is dropped, and at startup when a crash left
    one over an hour old; created with the mode a plain create gives, so the placed file
    reads like any other) and begins its operation only to move them into place once the
    whole upload arrived, never replacing anything at a name (a dangling link included): a
    taken name gets the next free one (`name_1.ext`), and an upload that fails to place a
    file removes the ones it placed. A request records only when one of its paths, mapped through the server's
    layout, is content or a folder that can hold content (the server folder itself can: an
    unzip there may write anything); when the layout can't be read, everything counts, since a
    record that finds nothing changed writes no image. Reads and downloads never record.
    Docker instances never do. While a backup is being restored into the server, every Files
    API change is refused (409), and an upload that streamed while the restore began places
    nothing: the restore swaps whole folders. A job that began before the restore isn't
    stopped. The global Files API (`handlers/global_fs.rs`, for users with
    `WriteGlobalFile`) can reach a server's folder too; its changes record no content image
    and aren't refused during a restore: they show up at the next record (as "Changed
    outside Lodestone" before a start, or in the next operation's image).
  - Settings: a `server.properties` setting ("Changed <setting>") and the resource pack
    ("Changed the resource pack"). The command line, JVM and Java settings are the core's own and
    never recorded. A Minecraft version change records at once after it succeeded ("Changed
    Minecraft version to X"), outside the operation: `change_version` holds the server's version
    lock, which a start takes before the content gate.
  - A new server (created, or imported): "Created <name>" once it is set up and before it is
    listed (and before its creation's end event), so nothing starts or changes it first,
    unless its setup recorded an image already (a modpack's install). It reads every content
    file (all of an imported server's), so the creation's progression says "Recording the
    server's content" first, a step of its own in the total.
- **Merged labels stay each operation's.** What succeeds within the merge window is one image
  labelled with each operation's label in order ("Edited a; Edited b"), cut at the label limit,
  rather than a count: the history then says what happened.
- **Returning to earlier content.** An image's id is its contents, so an action that brings the
  content back to an earlier image (enabling a jar again) saves that image's manifest again under
  the new label and parent rather than keeping two copies.
- **Starts and stops** (`implementations/minecraft/server.rs`). Every start (a user's, auto start,
  a restart, the restart after a crash) runs the check before it once the version lock is held
  and before the state leaves Stopped, then notes the start (`server_started`). The check first
  waits up to 10 s for the content work of the server's last stop; it does nothing for a server
  that is running (a start refused right after); and when the last run was never taken in
  (`running_since` still set: the core stopped while the server ran and the start came before the
  startup sweep), it takes that run in as the sweep would (`after_restart`, in its own task,
  waited for up to 10 s) instead of comparing, which would record what the run wrote as "Changed
  outside Lodestone". Every change of state into Stopped or Error from another state (a stop, a
  crash, a kill, a start that never launched) counts the stop's content work as pending before
  the state's event is sent, then runs `server_stopped` in the background; the state machine never
  waits for it.
- **What Modrinth said** about installed mods is noted for `env` and sources from what the Mods tab
  fetched anyway: an install notes its version's download URLs and the project's sides, a list
  the sides of every matched mod.

#### Backups: data images (`core/src/backups/`)

- **A backup is a record naming a data image.** Images are named by their contents, so two
  backups of a server that didn't change between them (a named backup right after a scheduled
  one) are one image, while each backup keeps its own name, kind (manual, scheduled, before a
  restore), pin and time. So each server has `images/<uuid>/backups.json`: per backup its id
  (`YYYYMMDD-HHMMSS-xxxx`, as before), its image, name, kind, time, pin, `live`, `quiesced`,
  the level the image has (what "only the world" restores), and the figures the list shows
  (files, withheld files, bytes, new bytes, stored bytes, duration), so listing reads this one
  file and no manifest. A data image's manifest is deleted once no record names it, and only
  when a backup that named it is deleted. A list that can't be read is an error, never "no
  backups". Before retention or a delete picks anything, the records are brought in line
  with the images: a record whose image is gone is dropped (a crash in the middle of a
  delete), and a data image no record names (a crash between its manifest and its record,
  or a `backups.json` that went missing) is listed as a backup again: a manual backup (never
  deleted by retention) named by the image's label, at its time, with its world as the level
  when it has one, and no server name (a restore as new names the copy after the server's
  current name). The policy is `backup-policy.json`
  beside it; deleting a server deletes the folder.
- **What a backup takes.** A snapshot of the server folder with its disk layout (worlds as
  `saves/<world>/…`; a Paper or Spigot level's `<level>_nether/DIM-1` and `<level>_the_end/DIM1`
  as its `DIM-1` and `DIM1`, their other files under `.bukkit_nether/` and `.bukkit_the_end/`),
  leaving out what images never record and the policy's patterns as canonical excludes (a
  pattern with no exact canonical form is backed up anyway: it may leave out less, never more).
  Entries are all `object`; `game` and the flavour setting are the server's. Each world gets
  its `size` and `quiesced`, so a copy that may be inconsistent is another image than a
  quiesced one. What it can't carry is withheld with why: links, files that can't be read or
  kept changing (three tries), and names no canonical path holds or the layout can't place
  back (a vanilla `DIM-1` in a Paper level, a server folder named `saves`). Those last are
  **withheld, not stored**: an image has no place for a file without a canonical path, and a
  restore leaves them where they are. A `level-name` that isn't one folder (`worlds/survival`)
  isn't a world: its files are backed up as ordinary paths and only "everything" restores them.
- **The content link.** Before a backup the server's content image is brought up to date: for a
  stopped server, the check before a start (what is queued, then what changed outside
  Lodestone), or its first content image when it has none; for a running server only what is
  queued (its own writes never make content images). Each waits at most about 10 s for the
  content gate (an operation in progress), and a record that has it runs to its end. The data
  image's `content` is the current pointer, read under the instance's lock while the image is
  saved, so pruning can't take it in between; its `parent` is the previous backup's image.
- **Unchanged backups.** A scheduled backup, or an unnamed manual one right after a manual or
  pinned backup, whose image has the newest backup's id writes nothing (an unnamed manual one
  after a backup retention may delete gets a record of its own, sharing the image). Every 10th
  scheduled backup since the last full scan, and a server's first backup, read every file (no
  stat cache); the count is kept in `backups.json`.
- **Live backups.** With saving on, a backup with more than 256 MiB to read reads it first;
  that snapshot's stat cache then leaves only what changed since to read with saving off. A
  running server's snapshot uses one thread.
- **Order of writes.** Objects (the snapshot), the manifest (the store flushes what it names
  first), then the record, both under the instance's lock; then the data stat cache. A crash
  before the record leaves a data image no record names: the next retention or delete lists
  it as a backup (see above). Deleting is the mirror image: the image (when no other record
  names it), then the record, then pruning the content images nothing links to, then a
  collection, only when an image went (retention runs after every backup and usually deletes
  nothing, and a collection reads every manifest of every server). A crash between the two leaves a record whose image is gone, which the next
  retention or delete drops, so a deleted backup never comes back; a backup whose image
  couldn't be deleted keeps its record. Retention picks what to delete from the records
  read under that lock.
- **Collections.** A job holds the store's collections off (`Store::hold_off_collection`) while
  it relies on objects nothing has claimed: a backup from its snapshot to its save, a restore
  while it reads the image (its backup may be deleted meanwhile), a download while it zips. A
  collection waits for it rather than the job retrying.
- **Shutdown.** Once the core's servers have stopped, it waits up to 6 s
  (`ImageService::settle`) for each one's stop work (the baseline taken again,
  `running_since` cleared), takes the content gate alone, so a record already running (the
  merge window's, which took its labels off the queue) and an operation in progress end
  first, and records what is still queued in the merge window, since the
  process exits right after: otherwise the next start would take the server for one that
  died while running and fold a change made while Lodestone was down into the baseline,
  instead of recording "Changed outside Lodestone". A record still running at the limit is
  cut off like a crash.
- **Restore.** A restore makes the files a backup would take now match the image, and leaves
  everything else as it is: what images never record, what the current policy excludes and
  what the backup itself left out (its `excludes`: it has no copy to put back), links, files
  no image can hold, and the files the backup withheld (it has no copy of them), including
  everything now under a folder it withheld (a link to a folder, or a folder it couldn't
  list; a snapshot withholds such a folder under its canonical path). So the
  safety backup has everything the restore replaces or removes; a replaced file the safety
  backup couldn't read (withheld as unreadable or kept changing) refuses the restore. Scopes:
  everything; the world (the backup's level, needing one); and `world_and_mods`, the level plus
  the content scope (`images/scope.rs`) but the server's settings and admin lists
  (`server.properties`, `ops.json`, `whitelist.json`), as the backup's own data image has them,
  refused when the backup has no level or no content link. The data image is a whole
  snapshot, so its content files are the ones that ran with the world; the linked content
  image can be older (what a running server writes never makes a content image), and an op
  removed since the backup stays removed. Paths are placed with the server's layout (split
  when the server's flavour splits worlds). The swap replaces whole top-level names and moves
  back what it keeps; a top-level file in the way of what the restore places, that no backup
  would take (so the safety backup has no copy), refuses the restore. The permission check
  compares by disk path with the Files API's rule for a server's files (protected files and
  Lodestone's own `.lodestone*` files need WriteGlobalFile, a restore as new too), and runs
  again on the stopped server when the plan changed since the request was checked. Everything and `world_and_mods` are content operations: they hold the
  content gate from before the safety backup to the swap, and record "Restored backup “…”"
  after the server's settings are read again; a world restore doesn't touch content. Files
  API changes are refused while a restore runs (a job begun before it isn't stopped). The journal is
  `images/<uuid>/restore-journal.json`.
- **What the server is built on follows a restore of content.** `pack.json` lives with the
  images, so no restore brings it back, and the server software and modpack origin are in the
  server's settings. After everything or `world_and_mods`, before the restore's content image
  is recorded, `pack.json` gets the base of the backup's content image when it differs from
  the current one, without the pack's file list (images don't keep it), so the next update
  falls back to the files of the origin's pack version
  (`modpack::pack_info::follow_base`). Everything brings the settings back whole;
  `world_and_mods` leaves them but takes `version`, `flavour`, `jre_major_version` and
  `modpack` from the backup's copy of them (read while collections are held off, since the
  backup may be deleted during the restore), since it put back the backup's server software
  (`libraries/`, `versions/`, the top-level jars). The restore journal carries those
  settings: the swap takes them on once the journal says it is done, and a recovery that
  finds a done journal takes them on then, so the settings never name other software than
  the disk has. Without this, a Forge server restored to an
  older build would start the newer build's libraries, which are gone, and an update after
  restoring an older pack version's mods would compare them against the newer version's
  files.
- **Restore as new and download** lay the image out with the backup's own flavour (its flavour
  setting); a restore as new writes the new server's `pack.json` the same way (the base of
  the backup's content image, no files) and records its first content image ("Created
  <name>") before the server is listed, so nothing starts or changes it first.
- **Usage.** The list's stored bytes count each object the server's backup images name once, at
  its size in the store (the zstd object, or a large file's list and chunks).
- **API.** The routes and shapes stay; `RestoreScope` gains `world_and_mods`; `skipped_count` is
  the image's withheld files; `new_bytes` is what the previous backup didn't have as it is now.

#### Lineage: modpack history is the content-image lineage (`core/src/modpack/lineage.rs`)

- **No history file.** A server's history is its content images, all of them: a modpack
  install, update or restore, a Mods-tab change, an edit through the Files API, a change
  found before a start. There is no `history.json`, no `record.json` and no store of copies
  in `.lodestone_modpack/objects/`. What is kept is what the content images' retention keeps
  (the newest 30, the current one and its parent, every one a backup links to). A restore in progress keeps
  the image it goes back to (`ImageService::keep_while_restoring`), since the record before
  it could make that image the 31st.
- **The pack's files are `pack.json`.** What an update compares the server against (each
  file the pack wrote, by disk path, with its SHA-1; a download's SHA-512, size and CDN URL)
  is `images/<uuid>/pack.json`, the file the recorder reads for the base, the sources and
  `env`. A file without a SHA-512 is one of the pack's overrides, and a download's Modrinth
  project comes from its CDN URL. It lives outside the server folder, so a user who can only
  write the server's files can't edit what an update removes. Each file is listed once, by
  the path the pack wrote (a mod the Mods tab disabled is found under its enabled name), and
  gains its SHA-256 and size the first time a record matches it. With no `pack.json`, or one
  without files, an update falls back to the files of the origin's pack version on
  Modrinth, as before. An update works on it as a `PackRecord`, which is never stored.
- **`pack.json` from the pack.** The base is `modrinth_pack` with the SHA-512 of the
  `.mrpack` the operation used (hashed by the core) when the pack is on Modrinth; an
  uploaded pack is no base. Each file's `env`: an index file's as written (unknown when it
  has none); an `overrides/` file required on both sides; a `server-overrides/` file
  unsupported on clients and required on servers; an `overrides/` file that
  `client-overrides/` replaces for clients unsupported on clients (they get another file).
- **Writes.** An install records "Installed <pack> <version>" once its files and
  `pack.json` are written. An update or restore holds the server stopped and its mods lock,
  puts back an update that was interrupted, and records the content as it is
  (`ImageService::record_before_change`: what is queued, else "Changed outside Lodestone"
  when the quick check finds a change since the last record or stop, else "Changed the backup
  excludes" when only the policy's excludes that can reach content did, else "Before a
  modpack update" or "Before a modpack restore" for what the server wrote while it ran or a
  file that couldn't be read before; nothing when the content is the current image; a server
  without an image gets its first), so every file the change takes off the server is in an
  image; a restore is refused when that record fails, an update goes on (it compares the
  files themselves). A restore of a version that isn't in the history is refused before
  this record; a change refused after it (a target the server can't run, a file the user
  may not replace) leaves that image, which holds what was on disk. Then it plans, each change keeping what it found at its path (nothing
  where it adds a file; the SHA-1 of a file an update replaces or removes, the SHA-512 of
  another copy of a mod it removes, the SHA-256 the current image gives a file a restore
  replaces or removes), stages every new file (downloads too), begins its operation alone
  (`ImageService::begin_exclusive`: no Files API operation runs until it is recorded, the
  copy, unzip and zip jobs aside, which hold no gate; the mods lock keeps the Mods tab out),
  checks that every path is still as planned (refused as a conflict otherwise, so a file
  uploaded or edited while it staged is never lost), writes the journal, places the files
  (never replacing a file that appeared after the check), installs the server software
  when the Minecraft or loader version changes,
  commits the journal, writes `pack.json` and the server's modpack origin, removes the
  journal, and records "Updated <pack> to <version>" or "Restored “<label>”" (the label of
  the image it went back to). A failure before the commit puts the files back and records
  nothing.
- **The journal** (`.lodestone_modpack/update.json`) lists each change with the SHA-1 of the
  staged file it places, and whether the change committed. At the next start, an
  uncommitted change is rolled back (a file is removed only when it is the one the change
  placed); a committed one keeps its files. A crash between the commit and `pack.json`
  leaves the old `pack.json` and origin: the next update then takes the files this one
  changed for the user's and leaves them, never deleting one.
- **Restoring an entry** puts back the image's files a restore may change: content except
  the server software (`libraries/`, `versions/`, top-level jars, `run.sh`, `run.bat`,
  `user_jvm_args.txt`), `server.properties`, `ops.json` and `whitelist.json` (settings and
  admin lists Lodestone and the server's commands manage: an older image must not bring back
  an op removed since), and scripts unless the requester may add them (as for a pack). It is planned by disk path from the current image (just recorded) to the target:
  a file that differs or is missing is replaced or added from the image store (which checks
  every byte against its SHA-256), or downloaded again when the store lost it and the entry
  is a Modrinth file with its SHA-1 and SHA-512; a file only the current image has is
  removed, unless the target left it out by its `excludes` or withheld it (or a folder it is
  in): the target has no copy of it, so it says nothing about it. Files neither image lists
  (left out by excludes, withheld, not content) are never touched; a file on disk the target would replace that the current image doesn't have (it
  withheld it, or it appeared since) stops the restore, since no image could undo it, and so
  does a folder or link where a file goes. Collections are held
  off while files are copied out of the store. When the target's game differs, its loader
  version is installed (Fabric, Forge, NeoForge; other server software can't be restored).
  A target on another pack base makes that version the origin (its name from Modrinth, else
  the old name with the new ids) and its files the `pack.json`, read from its `.mrpack` when
  Lodestone can get it and its SHA-512 is the base's, else a `pack.json` with the base and
  no files. A target without a base keeps the origin. A user without `WriteGlobalFile` can
  restore a protected jar only when Modrinth knows the SHA-512 of the store's bytes, and
  can't restore a version that removes a protected file other than a mod or plugin jar (as
  the Files API refuses that delete and the Mods tab allows the jars').
- **History API.** `GET /instance/{uuid}/modpack/history` lists the content images newest
  first (from manifest headers; only the content images are loaded whole), in retention's
  order (by `created`; within one second by the lineage from the current image).
  An entry is `id` (the image id, which `UpdateSource::History.entry_id` names), `time`,
  `label`, `game_version`, `loader`, `loader_version`, `changes` (the files a restore puts
  back that changed from its parent, or the next older image when the parent is gone; a
  removed and an added file of one Modrinth project or with the same contents are one
  `updated`), `file_count`, `current` and `missing` (files a restore would write that the
  store lost and that can't be downloaded). `blame` names, for each file a restore puts back
  in the current image (a mod by its enabled name), the oldest image of the newest run that
  had it. Backups are no entries. The dashboard, the Tauri Host (which serves the
  dashboard) and the GPUI Host were changed with it.

### Launcher store decisions

Decisions the first half of milestone 4 made where this document left the Launcher's
behaviour open. The core's M2 decisions are the model; where the Launcher's case is the
same, it does the same. The code is `launcher/crates/launcher-core/src/images/`.

- **One store, apart from snapshots.** The Launcher's image store is `<data>/image-store/`
  (`DataDir::image_store_dir`), one for every instance on the machine, plain encoding (zstd
  isn't in the Launcher's lockfile; APFS clones files into it where it can). It sits beside
  the snapshot store (`<data>/snapshots/`), never inside it: each store has its own objects
  and its own collector, and neither collector reads the other's folder, so neither can
  delete what the other names (tested with both collectors over old objects). Nothing is
  created until the first write, which makes the store's folder readable by its owner only
  (mode 0700 on Unix): the Launcher's own files are, and an object taken from a 0600 mod
  config would otherwise be readable by anyone who can reach the data folder. On Windows the
  folder gets the data folder's permissions (a user profile's, by default), and a data
  folder moved to another drive or an exFAT disk gets what that place gives. Settings ›
  Storage counts it under "Other" until milestone 5 replaces the snapshot store with
  images.
- **Suspending for a data move.** A data move carries the store like everything else.
  First `Images::settle` writes what was queued, then `Images::suspend` stops every write
  to the store and waits for the running ones to end: a record (however long the first
  record of a big pack takes), a check, a session's end, deleting an instance's images, a
  collection. Each of these counts itself while it writes. While suspended, operations
  still queue their labels; records and deletes refuse (`Suspended`), the check is skipped
  (its session is still marked), the merged records' thread sleeps, and a collection
  waits. Cancel works while it waits for a write (`Images::suspend_unless`). A move that
  fails or is cancelled resumes the store, and what waited then runs. A move that succeeds
  keeps it suspended until the Launcher restarts at the new place (`Suspension::keep`, which
  is remembered: a later move, cancelled in a Launcher whose restart failed, doesn't resume
  it), since a write would remake the old folder or leave a manifest whose objects the copy
  lacks.
- **One collector.** The store's own GC, in a background thread, one at a time; a request
  while one runs makes it run once more. Pruning asks at most every 15 minutes; deleting an
  instance asks at once. It takes no lock of the Launcher's, only the store's own and the
  count of writes that suspending waits for. Before each collection it forgets instances
  deleted outside the Launcher (see "Deleting and reused ids").
- **What content is.** An allowlist, matched ignoring ASCII case: everything in the game
  folder's `mods/`, `config/`, `defaultconfigs/`, `kubejs/`, `resourcepacks/` and
  `shaderpacks/`, disabled `*.disabled` files included (turning a mod off is a content
  change). Something other than a folder at one of those names (a link named `config`) is
  offered too, so it is withheld as a link rather than silently missing. A game folder
  (`<instance>/minecraft`) that is itself a link isn't followed either: its images are
  empty, the check before a launch sees no change, and each record logs a warning (the
  layout gives the game folder no path an image could withhold). A linked instance folder
  is followed, as the walk's root. Never content:
  any name starting with `.lodestone` at any depth (folded; downloads in flight are
  `.lodestone-<sha1>.download`), `saves/`, screenshots, logs, crash reports,
  `options.txt` and the other files the game writes, and anything outside the game folder
  (`instance.json`, `content.json`, `modpack.json`, the icon). A path the layout can't map
  is judged by its parts, so a mod with a bad name is withheld as `bad_name`. Content is
  also every file that `modpack.json` or an added pack lists as one it wrote, in whatever
  folder (at most 100,000, matched ignoring ASCII case). Packs put CraftTweaker's
  `scripts/`, `global_packs/`, FancyMenu's and Patchouli's folders and the like in their
  overrides, and the updater's three-way merge changes them, so "Before a modpack update"
  has to hold them. Files the game writes are never taken this way, even when a pack ships
  them: `options.txt` and the other options files, `servers.dat`, the user caches,
  `command_history.txt`, `hotbar.nbt`, `saves/`, `screenshots/`, the always-left-out folders, `versions/`, `libraries/`,
  `assets/`, `session.lock` and Lodestone's own names. The list a scope takes is bounded by
  count and by 16 MiB of paths (a path over 4,096 bytes is skipped, not the end of the
  list), and holds each path once (a folder holding a pack file is
  found by an ordered lookup, not a set of every folder), so a pack of deep paths can't
  make each record or check build gigabytes. Data packs belong to their world (`saves/`):
  adding, enabling or removing one in the Worlds tab or the CLI makes no content image,
  and they go into data images from milestone 5. Known limit, for milestones 4b and
  5: a file outside the content folders that no pack lists (a script added by hand) is not
  content, so a restore leaves it alone and a server's client view is compared without it.
  The rules have a version (`SCOPE_RULES`, 2): a baseline taken under other rules is
  recorded again, without a label, rather than compared.
- **Game, base and settings.** The game is `instance.json`'s (no loader is `vanilla`, with
  no loader version). The base is `modpack.json`'s pack: `curseforge_pack` for a CurseForge
  pack, `modrinth_pack` for a Modrinth pack whose `.mrpack` SHA-512 is known
  (`ModpackRecord::pack_sha512`, set from Modrinth's version listing when the record is made
  and checked as 128 lowercase hex characters; an optional field, so `modpack.json` keeps
  version 1), `loader` with no `modpack.json`, and none for a pack the image can't name (a
  local `.mrpack`, a server's pack). The client settings are the recommended memory
  (`instance.json`) and the resource pack order (`options.txt`'s `resourcePacks`, at most
  1,000 entries). Not the shader: Iris and Oculus keep it in `config/*.properties`, which is
  content already. OptiFine keeps it in `optionsshaders.txt`, a file the game writes, so
  turning on a shader pack on an OptiFine instance changes no image and its label is
  dropped. Not the icon or background: they are the instance's look in the Launcher, not
  what it plays.
- **Sources and `env` from `content.json`.** Every file is listed as an entry, the pack's
  too (no `base` sources, like the core). A file directly in `mods/`, `resourcepacks/` or
  `shaderpacks/` that `content.json` describes (under its enabled name) at the same size and
  modification time gets its SHA-1 and SHA-512 (when they are lowercase hex of the right
  length), a `modrinth` source when its URL is a Modrinth CDN file and it has a SHA-512, or
  a `curseforge` source, and `env` from Modrinth's project sides (`modrinth_project`).
  Anything else is an `object` with unknown sides. Nothing is fetched while recording.
- **Operations without a gate.** An operation (`Images::begin`) holds nothing a record waits
  for, and beginning and succeeding it touch no disk, so both are safe on the window's
  thread. Launcher downloads, files added from the computer and every other file the
  Launcher writes whole (`storage::write_atomic`, through `.lodestone-tmp-<random>`) land
  under `.lodestone-*` names, which are never content, and are renamed into place (an
  import from another game folder leaves `.lodestone-*` files out too); Play
  waits while content is installed or added (from the Content tab or the CLI, a share
  code's content, or a server's mods brought in step, until the loader it brings is
  saved), and the app
  refuses content changes while the game runs, so a record never reads a file half written
  (one that changes while read is withheld as `kept_changing`), and a temporary file a
  crash leaves behind is in no image. A record during a
  long operation may take its in-between state under another operation's label; that
  operation's own record then takes the final state. Each operation notes the instance's
  image generation when it begins: when its record finds nothing changed but a new image
  was written since it began (which may hold its files), its label is added to the current
  image (saved again under the same id); otherwise it changed nothing, and its label is
  dropped. This replaces the core's job and `wrote` machinery. Known: a label is carried
  whenever a new image was written after its operation began, even when that operation
  changed nothing, so a job that failed before changing anything can add its part-way
  label ("Partly installed X") to an earlier operation's image; skipping part-way labels
  would drop the steps of a job that did change files, so it stays. An operation that says
  nothing (a job that did nothing) records nothing. The CurseForge blocked-files watcher
  installs a file found in Downloads only while the game isn't running (it waits
  otherwise). Saving `modpack.json` for a server's rebuilt pack with the same mods changes
  which files are content (the file names them), so it is an operation of its own
  (`Images::begin_rescope`) that records with no label, or only takes the baseline again
  when nothing new is in; otherwise the next launch's check would take the new list for a
  change made by hand.
- **The merge window and its thread.** Successes are merged until 3 s pass without one, or
  30 s after the first at the latest, by one thread ("lodestone-images"). It leaves an
  instance alone while its game session lasts and while a "Find it for me" search is saved
  (`mod-finder.json`: it renames mods on purpose), and records it once that ends. A record
  that fails keeps its labels and is tried again after a minute; one for an instance that
  is gone drops them. The thread sleeps until woken while nothing is queued (it polls every
  5 s only while labels wait for a saved search to end), so an idle Launcher has no timer
  of its own. It never waits on a record of an instance that is under way (the first image
  of a big pack can take minutes): it comes back to that instance after the merge window,
  so the others' labels don't wait behind it. When the Launcher quits, `Images::settle`
  waits up to 6 s for what is queued (on the window's thread, since GPUI gives a quit
  handler's future 200 ms), skipping an instance whose search is saved or whose record is
  running: those changes are found by the next launch's check.
- **Order of writes.** As the core's: snapshot (objects claimed), a free-space check (what
  it will read, what the other records under way reserved, and 1 GiB to spare; see "Free
  space" below), manifest (the store flushes what it names; temp file
  and rename; once more when a collection took an object meanwhile), then
  `content-state.json`, then the `content` stat cache, then pruning. An instance's first
  record (no state file yet) writes a state naming only the instance (`instance_created`)
  before anything else, so manifests a crash leaves are never mistaken for an earlier
  instance's. A crash in between leaves the old pointer (or none, at the first record),
  which the next record moves. When the contents are still those of the image the crash
  left and there is no pointer, the pointer moves to that image, which keeps its own label
  and time. A crash never leaves a half-written manifest. Labels queued at a crash are lost: the next check before a launch records the change as
  "Changed by hand".
- **The check before a launch.** The launch's background task calls `Images::before_launch`
  before the game's files are prepared. It first counts a session of the instance, which
  holds merged records off, whatever it then does: a count, not a flag, so Play pressed
  again as the last run's end is handled doesn't let that end take the baseline under the
  new game. It does nothing more while a "Find it for me" search is saved, and never fails
  the launch; a recorded change or a failure is noted in the run's log. It records what is
  queued (a queue of empty labels records nothing, and the check still looks); else an
  unlabelled record under new scope rules; else "Changed by hand" when a content file's
  size or modification time differs from the baseline (no file is read: a test makes a mod
  unreadable) or the game differs from the current image's (a loader version edited into
  `instance.json`). Then it notes the session in the state (`running_since`). It never
  records an instance's first image, nor waits for one being recorded, which hashes every
  file and would hold the launch for seconds on a big pack: when a record holds an
  instance that has no image yet (a new pack's, or the first-images pass at startup), Play
  goes on at once. Any other record gets at most 10 s (one that has begun runs to its
  end). Either way, a record that held the instance walked the content before the game
  started, a change made by hand included, so that session ends like any other: the
  baseline is taken again and what the game wrote makes no image of its own. Known limit:
  a first image hashes the whole pack, and the game may start while it still reads; a
  config the game rewrites at startup is then in that image as the game wrote it, or
  withheld as `kept_changing` (no copy of it in the first image). The first-images pass
  takes the most recently played instance first, the one most likely to be played right
  after the Launcher starts, so this is most likely there; the next image an operation
  records holds those configs. A record that
  runs while a session lasts notes the session (`running_since`) as the check would have.
  When the check couldn't run for another reason (the store was suspended, recording
  failed, or there is no image yet and no record under way), the session's end runs it
  instead of taking the baseline again, labelled as the launch's check would have been: a
  change made by hand before the launch is still recorded and not folded into the
  baseline, with what the game wrote in the same image, since nothing tells the two apart
  then. A failed check still notes the session as running when the instance has an image,
  so if no end is seen of it the next check's label says the game may have made the
  change; a session that began while the store was suspended can't be noted, and its
  changes are "Changed by hand". Otherwise, however the run ends (the game exits or
  crashes, the launch fails or is cancelled after the check), `Images::session_ended` takes
  the baseline again in the background without an image, so what the game wrote (configs
  rewritten at startup, Forge's `splash.properties`) joins the next image an operation
  records. The baseline keeps the current image's game, so a game changed by hand during
  the session is still found.
- **Sessions no end was seen of.** With Settings › After launch set to quit, the Launcher
  exits while the game runs, and it can die while the game runs: `running_since` is then
  still set at the next check. That check can't tell what the game wrote from a change by
  hand, so it records any change as "Changed while the game ran or by hand" (the same for
  a record before a change): a change by hand is never lost in the baseline, at the cost of
  an image of the game's own writes after each such session. Any record taken outside a
  session (an operation's, say) ends a stale `running_since`, since it took the content
  again.
- **First images.** An instance from before the Launcher recorded images gets its first
  ("Content when the Launcher began recording") from `Images::record_first_images`, which
  the app runs in the background when it starts (not the offscreen scenes or tests): one
  instance at a time, most recently played first, leaving an instance whose game runs (its
  session's end records it) or whose search is saved, and stopping while the store is
  suspended. So the first change to an existing instance has a parent holding what it
  replaced. The pass skips only an instance with a current image: one whose first record
  failed (a full disk, or the free-space check refusing a big pack) keeps the state file
  that record wrote first, and is tried again at the next start. Known gap: an operation on
  an instance in the moments before the pass reaches it records after the change; its
  image is then the first, without a parent. Cost: the pass copies every instance's
  content (mods, configs, resource and shader packs) into the store. APFS clones the files,
  so on a Mac it costs little space; on NTFS and ext4 there are no clones, so the first
  start after the update takes about as much disk space again as all instances' content
  (20 instances of 1 to 2 GB each take some 30 GB), in the background and without asking.
  The free-space check counts every image record under way, so records of several instances at
  once (this pass, imports, merged records) stop short of filling the disk together (they
  leave 1 GiB), and an instance that didn't fit is tried again at the next start. Milestone 5's storage
  view shows the store's size.
- **Free space.** Before a record copies anything it reserves what it will read, process
  wide (`Images::reserve_space`), and is refused (no image; tried again later, as any
  failed record) when the free space can't hold that, what the records under way have
  reserved, and 1 GiB to spare, as the snapshot store keeps: most records are work nobody
  asked for, and the game's world saves need the room more. It is conservative: a record
  under way counts in full though part of it is already written, and an object APFS
  clones (a whole file of 1 MiB or less) counts whole though it costs almost nothing. The
  reservation is the image store's own: the snapshot store, until milestone 5 replaces it,
  checks for its own 1 GiB spare separately, without counting records under way, so a
  snapshot taken during a big record can eat into the spare the two would otherwise keep.
- **Records before a change.** `Images::record_before_change` runs before a modpack update
  ("Before a modpack update") and before a server's mods are brought in step ("Before a
  server update"): what is queued, else "Changed by hand" (or "Changed while the game ran
  or by hand") when the quick check finds a change, else the change's own label, so every
  file it replaces is in an image (nothing new is written when the content is the current
  image). A failure is logged and the change goes on. Neither update starts while a "Find
  it for me" search is saved (the app and the CLI say to finish it first, as they say to
  stop the game): the search has mods turned off, the record before the update would take
  them as changed by hand, and the search's end would restore files the update replaced. Milestone 4's second half keeps this hook when server images replace the client pack
  sync (`servers/mod.rs`, `apply_sync`).
- **Triggers and labels.** Each begins its operation right before files change and
  succeeds once they did. An action that changes nothing when it fails (one file renamed,
  added or moved to the Trash) records nothing on failure. A removal whose file went to the
  Trash before the rest failed (taking a resource pack out of `options.txt`, or turning off
  the shader it was) still records "Removed <file>": it succeeds once the file is gone
  (`files::is_gone`). One that can fail part way
  after changing files succeeds all the same, so what it did change is its own image and
  not "Changed by hand" at the next launch: an install that placed some files adds
  "Partly installed <title>", and a pack update, a server's update, a share code, adding or
  removing an added pack, a revert and the end of "Find it for me" add " (stopped part
  way)" to their label. When nothing changed, nothing is recorded, as for any operation.
  New instances are recorded once their files are in place, in the background (Duplicate,
  an import and the like are done without waiting for it). An instance that something
  changes next is recorded first, in the task that then changes it: a pack's instance that
  a share code changes next, and a share code's empty instance before the code's content
  installs, so none of that content is in "Created". Labels: "Created <name>" (the
  New instance dialog, the CLI, a share code's empty instance, a server without a pack),
  "Copied from <name>" (Duplicate, Copy with another version or loader), "Imported from
  <launcher>", "Installed <pack> <version>" (Modrinth and CurseForge packs, and a share code
  based on a pack), "Created from <server>" (an instance made from a server's pack), "Made
  from a snapshot". Content: "Added <title>", "Updated <title>" (an install that took an
  older file's place), "Added N mods" and the like, from Modrinth or CurseForge. A job is
  one operation however long it runs: Update all (or an install of several requests) notes
  each install's label and succeeds once at its end, so it is one image ("Updated A;
  Updated B"); its CurseForge half is a job of its own, so Update all of both is at most two
  images. "Added <file>" (a file from the computer, or a CurseForge file downloaded by
  hand), "Removed <file>", "Enabled <file>", "Disabled <file>" (the Content tab, the CLI and
  the crash dialog; a file is named as it is when on, without `.disabled`), "Changed the resource packs", "Turned on the shader <file>", "Turned off
  shaders", "Added the modpack <title>", "Removed the modpack <title>", "Installed from a
  share code", "Went back to snapshot “<name>”". A pack update is "Updated <pack> to
  <version>", succeeding after the new loader version is saved (a failed save still
  records: the files changed); a server's is "Updated from <server>". Saving `instance.json`
  from Settings, Undo, a revert, an update or the CLI, with another game (Minecraft version, loader or loader version, as the
  image's `game` has it) or recommended memory, is "Changed <loader> to <version>" (a new
  version of the same loader), "Changed to <loader> <version>" or "Changed to <loader>"
  (another loader), "Changed Minecraft to <version>" (with " with <loader> <version>"
  when the loader changed too) or "Changed the recommended memory",
  merged with the update's own label when it is one. "Find it for me" records nothing while
  it searches; its end is one operation ("Find it for me turned off <file>", or nothing when
  every mod went back as it was: then `Images::search_put_back` takes the baseline again
  without recording, as a session's end, so what the game wrote during the tries joins the
  next operation's image; a put-back that failed part way is "Find it for me put the mods
  back (stopped part way)"). Its tries are launches whose checks do nothing, and each
  try's end takes the baseline again, so before the search first turns a mod off it runs
  the quick check (`Images::check_now`): a change made by hand before the search is
  "Changed by hand", not folded into a try's baseline and recorded under the search's end.
  A modpack update is refused while a search is saved, before its pack is downloaded. Known: Verify game files on a Forge instance that uses
  `minecraftArguments` creates `config/splash.properties` outside a session when it is
  missing, and the next launch's check records it as "Changed by hand". Applying a
  server's update is milestone 4's second half; its hook is `apply_sync`'s
  `record_before_change` and operation.
- **English labels.** Labels are data that travel inside images, so they are plain English
  as the core's are, never translated; `join_labels` turns control characters into spaces
  and cuts the whole to 1,000 characters, and the Launcher shows them through `display_text`.
  `KEEP_CONTENT`, `newest_content_first` and `join_labels` are the shared crate's
  (`lodestone_image::lineage`), so both programs keep and list content images alike.
- **Retention.** Under the instance's record lock after every record that wrote an image:
  keep every data and full image and its `content` link (so pruning is ready for milestone
  5's snapshots; deleting an instance's images is not, see "Deleting and reused ids"), the
  images held with `Images::keep_while` (for milestone 5's restores), the
  current image and its parent, and the newest 30 content images by `newest_content_first`;
  delete the other content manifests. While a manifest can't be read, nothing is deleted,
  and nothing shows it yet: milestone 5's history should surface an unreadable manifest so
  the user can delete it (until then each record reads every manifest's header, and the
  store keeps growing).
- **Deleting and reused ids.** Instance ids are folder names and a deleted instance's id
  goes to the next instance of the same name. Deleting an instance (after its folder went to
  the Trash) deletes `images/<id>/` and asks for a collection, so restoring it from the
  Trash starts a new history. A record, check or history that finds `content-state.json`
  naming another `created` time than `instance.json`'s, or manifests with no state at all
  (an instance's first record writes its state before any manifest, so only a deleted
  instance's leftovers look like that), takes them for an earlier instance's: a record or
  check deletes them first, the history shows none. Nothing is recorded or deleted while
  `instance.json` can't be read, nor while `content-state.json` can't be looked at or read
  (a permission, a network share timing out, a file another program holds): the record
  fails and is tried again later, so a passing error never replaces the real pointer or
  takes the images for another instance's. A state file that was read but isn't one (not
  JSON, a bad pointer) is damaged: it names no image and no instance, so nothing is deleted
  for it. An instance folder deleted outside the Launcher (in
  Finder or Explorer, by a sync tool) is forgotten once the Launcher starts and before
  every collection: `images/<id>/` goes, under the instance's record lock, when nothing is
  at `instances/<id>` or a real folder there has no `instance.json`, though not while the
  instances folder itself is missing (a drive that isn't there), and never for a link or
  junction whose target can't be reached (a pack kept on an unplugged drive). Otherwise its
  manifests would keep its objects from every collection. Ids the store can't name (a
  Windows device name such as `con`) record nothing. Each of these deletes all of
  `images/<id>/`, which today holds only content images. Milestone 5 must change them
  (`Images::wipe_locked`, `delete_instance`, `forget_deleted` and the id-reuse guard)
  before data images move into this store: a deleted instance, or a folder renamed in
  Finder or briefly removed by a sync tool, must not take its backups with it. Data and full
  images, and the content images they link to, should stay until the user deletes them, as
  the snapshot store keeps a deleted instance's snapshots until Settings › Storage cleans
  them up.
- **Cost.** A record after a Mods action re-reads only changed files, but the snapshot
  claims every cached file and stats each chunk of a big file: for a 900 MB pack (300 mods
  of 3 MiB, chunked) that is about 190 ms of planning, and a record after a mod update
  about 830 ms, in the background after the merge window. A later, additive change to the
  shared crate could skip claiming chunks it claimed minutes ago.
- **What stays, and why.** `modpack.json` is current state: the modpack updater's three-way
  merge needs each pack file's SHA-1, and it names the image's base. `content.json` is a
  cache of hashes and Modrinth and CurseForge identities for listing, updates and exports,
  which records read for sources and sides. The added-packs file is what removing an added
  pack takes off. The modpack update's copy of the files it touches
  (`backups/modpack-update/<time>/`) is its rollback when it fails part way, and its world
  zips and the snapshot store with `.lodesnap` stay until milestone 5 replaces them. The
  Launcher had no content history of its own, so nothing was removed.
- **The CLI.** `lodestone-launcher-cli content history <instance>` (method
  `content.history`, read-only) lists the content images newest first: `{instance, current,
  images: [{id, created, label, label_shown, parent, current, minecraft, loader,
  loader_version, game_shown, base, files, withheld}]}`, where `label_shown` and
  `game_shown` went through `display_text`; `--human` prints one line each from them (the
  CLI itself has no `lodestone_image`). An unknown
  instance is `not_found`; one without images has an empty list. It waits at most 3 s for
  a record of the instance under way (the first image of a big pack can take minutes),
  then lists without it: the newest image may be missing then. The window shows no
  history yet: milestone 5 adds history and restore.
