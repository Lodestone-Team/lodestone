# Instance images

One format for every copy of a Minecraft instance that Lodestone keeps or moves: server
backups, Launcher snapshots, modpack history, what a server sends players so they can
join, and sharing between Hosts, Launchers and friends. It replaces five formats that
each do part of the job:

| Today | Becomes |
| --- | --- |
| Core backups (`core/src/backups`) | Data images |
| Launcher snapshots, `.lodesnap`, world backup zips (were `launcher-core/src/snapshots.rs`, `worlds.rs`) | Data and full images in the Launcher's image store; the export is an image export, `.lodeimg` (done in M5) |
| Core modpack record and history (`.lodestone_modpack/`) | The base and the lineage of content images |
| Launcher `modpack.json` and its update plan | `modpack.json` stays as the updater's current state and names the base; the history is the lineage of content images |
| `LDS2:` share codes (`launcher-core/src/content/share.rs`) | A text encoding of an image whose files can all be downloaded (the format stays: see "M5 decisions") |
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
decisions"), and list in `removed` the pack's files the server no longer has ("M3
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
(`jar_environment`: `fabric.mod.json`'s `environment`, `quilt.mod.json`'s
`minecraft.environment`, or a Forge or NeoForge `mods.toml`'s top-level
`clientSideOnly = true`, client only, or `displayTest = "IGNORE_SERVER_VERSION"` on every
`[[mods]]` entry, server only). Each side is decided on its own: the first input that says something
other than `unknown` for that side wins, and `env_from` lists the inputs that decided a
side, sorted. The owner's mark only decides the client side: "For players" is `required`,
"Not for players" is `unsupported` (the core passes it only where the other inputs don't
make the client side `required`: "M3 decisions"). Modrinth's version `environment` maps as:
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
                         + the side's share of the base, less the side's share of
                           `removed` (on the client, only paths it could carry: no configs)
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
  files and unknown fields (the client view only part of `removed`: below), and none of its informational fields (`parent`, `created`,
  `producer`, `label`, entries' `modified`). World files go to both views, so the server
  view of a Launcher snapshot is what "Host this" makes; the server-to-player flow uses
  the content image, which has no worlds.

- The client view's `removed` keeps only the paths the client view could carry a file at:
  none the client can't hold, none of Lodestone's own files, a server's state or disabled
  files (as above), and no configs, since a removed path has no entry an owner could have
  marked shared (`lodestone_image::view::in_client_removed`). Players get a pack's configs
  from the pack, not the server, so a pack config the server deleted stays on players'
  machines as the pack shipped it, just as an edit to it never reaches them; a change to
  `removed` that only touches such paths leaves the client view's id as it was. The server
  view keeps all of `removed`, so a restore keeps the deletion, and losslessness holds.
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
<root>/images/<instance>/restore-journal.json       the core and the Launcher: a restore's swap in progress
<root>/images/<instance>/snapshots.json             the Launcher: the instance's snapshots, each naming its image
<root>/images/_deleted_<16 hex>/                    the Launcher: a deleted instance's snapshots, with retired.json
<root>/images/<instance>/save-off.marker            the core: a live backup has the server's saving off
<root>/images/collection.lock                       held by GC (see below)
<root>/{objects,lists,chunks,images/<instance>}/.incoming-<16 hex>.tmp    writes in flight
```

The core's root is its backup folder (`<lodestone>/backups`); the Launcher's is
`<data>/image-store/` (see "Launcher store decisions" and "M5 decisions").

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
| Launcher | Snapshot on demand; before risky operations (content updates, modpack updates, loader version changes, and server updates from milestone 4's second half); after a play session only if the user turns it on (off by default); the safety snapshot before a restore | Data or full image (see "M5 decisions") |

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
outside Lodestone and the backup excludes didn't change), and the Launcher's before every
snapshot, whose content link must be what was on disk ("Changed while playing" when the
game's writes are the only change; see "M5 decisions").

## Retention

- Content images are the lineage, which is also the modpack history: the newest 30, the
  current one and its parent, and every image a data image links to are kept. Manifests only; objects are
  shared.
- Data images: the core's backup rules (keep last N, hourly, daily, weekly buckets;
  manual and pinned never deleted; safety images before a restore kept 30 days), never
  the content lineage's. The Launcher offers the same, per instance: its own snapshots
  (before a change, after playing) keep the newest 5, one a day for 7 days and one a week
  for 4 weeks by default; the user's own, imported and pinned ones are never deleted.
- A player whose views were all pruned gets the whole view for a `have` the core no longer
  knows, and diffs it against the view it applied itself (`lodestone_image::diff`), which
  gives the same delta the server would ("Server to player").

## Server to player

A server serves its players the client view of its current content image, deltas to it,
and the bytes of the files only it has. The core does this (`core/src/images/serve.rs`,
`objects.rs`, `handlers/player_image.rs`); the Launcher consumes it in M4.

**Routes.** Every route has three forms, with the same query and answers; each checks
access exactly as the client pack's route beside it does, and answers 404 alike for a
server that doesn't exist, isn't listed, isn't shared or is a Docker container:

| Prefix | Who | Access |
| --- | --- | --- |
| `/api/v1/launcher/instances/{uuid}/` | a signed-in user or launcher key | `ViewInstance` on the server (no bearer token is 400, a bad one 401, no permission 403) |
| `/api/v1/public/instances/{uuid}/` | anyone the core serves (while the core serves only this computer, another device gets 403 `{"kind": "LocalOnly"}`, as for the client pack) | the server listed publicly with the client-pack scope, while listing is on |
| `/api/v1/public/instances/{uuid}/` on a Connect invite's info port or a friend tunnel | a friend | the shared server only |

```
GET  …/image?side=client[&have=<view id>,<view id>…][&rules=<n>]   (HEAD: the same, without the body)
GET  …/objects/{sha256}                                             (and HEAD)
```

On the main port (both prefixes) the core's remote-access check comes first: while the
core serves only this computer, another device gets 403 `{"kind": "LocalOnly"}` on every
route. No route takes a request body; any other method is 405, except that on the main
port `OPTIONS` is the CORS preflight, which the core's CORS layer answers (200); through an
invite's info port or a friend tunnel, which have no CORS layer, `OPTIONS` is 405 like any
other method. Conditional requests aren't honoured (an
`If-None-Match` naming the current view still gets 200): a player says what it has with
`have`. Unknown query parameters are ignored, so a client may send `rules` to any core.

Errors: a query that can't be read at all (a parameter given twice, `have=a&have=b` or
`side` twice) is refused before anything but the remote-access check, even before the
sign-in on `/launcher/…` and the 404 for a server that isn't listed, with 400 and a
plain-text body. On `/launcher/…` a request without a bearer token is 400 with a plain-text
body too. Every other error has the JSON error body `{"kind": "<kind>", "causes":
["<message>"]}`: bad values of `side`, `have` or `rules` (400 `BadRequest`, checked after
access, so an unlisted server is 404 first), and an error that isn't the player's (an
image or file the store can't read for now: 500 `Internal`, whose message never names the
store's folders). A client reads the JSON body only when `Content-Type` is
`application/json`.

Every 503 has `Retry-After` (seconds) and the JSON error body, whose `kind` says why:
`Conflict` (the first content image, or a file the store lost, is being recorded: ask
again after the delay), `Internal` (that record failed or ran a moment ago: none starts
again before the delay) or `TooManyRequests` (too many downloads at once, or the store's
garbage collection is running; objects only). Each is described below.

Answers can be large: a 300-mod pack's view is about 160 KiB of JSON and one of 450 mods
passes 256 KiB, and an object is a whole file. Read every route as a stream (through a
tunnel, the forwarded port with an ordinary HTTP client, not `lodestone_connect::info::get`,
which stops at 256 KiB). A client stops reading a view or delta past
`lodestone_image::store::MAX_MANIFEST_BYTES` (512 MiB, the limit exports use too) and
treats it as an error: it must hold the body to check it, and a hostile server or peer
could send one that never ends.

**Finding out.** The image routes answer wherever the client pack's access holds: on the
public list, for a server whose listing has `has_client_pack` at all (present, true or
false: the client-pack scope is on); for a friend's shared server and with `ViewInstance`,
always. `has_client_pack` and a shared server's `modpack` only say whether a `.mrpack`
can be built (Fabric, with an enabled jar); images serve Fabric, Quilt, Forge and NeoForge
servers alike, so a client must not take them as "no image". A core older than milestone 3
answers 404 on `…/image` even where it serves the `.mrpack`: a 404 on `…/image` for a
server whose listing says `has_client_pack: true` (or whose `.mrpack` route answers 200)
means an older core, and the client falls back to the `.mrpack`.

**`GET …/image`.** `side` must be `client` (anything else is 400: a server view carries
the server's configs and secrets). `have` lists client view ids the player has, newest
first, comma-separated, at most 16, each 64 lowercase hex digits (else 400). `rules`, which
a client should send, is the newest view rules it knows (`lodestone_image::RULES`: ASCII
digits only, a whole number from 1, else 400; a number too large for 32 bits counts as
newer than any core's): a core whose rules are newer answers 400 `BadRequest` ("update
Lodestone"), since it makes views the client can't check; a core whose rules are older or
the same ignores it (views are made under the core's rules, which every later Lodestone
reads). Every answer of `…/image` past the access checks (200, 204, 400, 500 and 503 alike)
carries `X-Lodestone-Rules: <the core's RULES>`, so a client tells the update refusal from
other 400s without reading its message: a client whose own `RULES` is lower than that
header's tells the player to update Lodestone, whatever the status, and neither retries nor
installs anything. Answers refused before the access checks (a query that can't be read, a
missing sign-in, a 404 for a server that isn't served, `LocalOnly`) don't carry it. The first id the core knows decides (it first looks among the views it has
already learnt, and reads the server's other manifests for ids it doesn't know only when
none of those decides), so it is not always the first id:

- it is the current view: **204**, no body. The current view is the one
  `X-Lodestone-View` names, one of the `have` ids; a player whose applied view isn't that
  one switches to it, placing it as it would a received view (diffing its applied view
  against it);
- it is another view of one of the server's content images: **200** with a delta from it
  to the current view (`X-Lodestone-Answer: delta`), or the whole view when the delta
  would be no smaller;
- none is known (pruned, another server's, a content id rather than a view id, `have`
  empty): **200** with the whole view (`X-Lodestone-Answer: view`). A view whose images
  were all pruned may still get a delta while the core has one from it cached.

A view is the client view manifest JSON (an `Image` with `"side": "client"`), a delta the
`Delta` JSON of "Deltas" whose `from` and `to` are view ids: `from` is the `have` id that
decided, so it names which of the player's views to apply the delta to. Every 200 and 204 carries
`X-Lodestone-View: <current view id>`, `ETag: "<current view id>"` and
`Cache-Control: no-cache`; a 200 has `Content-Type: application/json`. The player checks
what it gets before using it:

- a view: `Image::from_json`, then `verify` (its contents hash to its id), then its own
  view's id equals its id (`view(v, Client, v.rules).id == v.id`, the check for a view
  received from elsewhere) and equals `X-Lodestone-View`;
- a delta: `Delta::from_json`, then `apply` to the view its `from` names, which checks
  `from` and that the result's id is `to`; then the result gets the same check as a whole
  view (its side is client, `view(r, Client, r.rules).id == r.id`, and `r.id` equals
  `X-Lodestone-View`). `apply` alone doesn't check that the result is a client view: a
  hostile server or peer could send a delta that adds `server.properties`, Lodestone's own
  `.lodestone*` files or an unshared config, or changes `side` or `rules`, and it would
  apply cleanly.

`lodestone_image::view::check_received(image, Side::Client)` does the view check for both
(side, known rules, `verify`, and the view of the view giving back its id); the client then
compares the id with `X-Lodestone-View`. A view or delta whose `schema` or `rules` the client
doesn't know (the parse fails with an unsupported schema, or the view check with
`UnsupportedRules`) means the server runs a newer Lodestone: the client tells the player to
update Lodestone, and neither retries nor installs anything.

While the server has no content image yet (or its pointer names a manifest that went
missing or can't be read), the answer is **503** with `Retry-After: 5` and the error body
`{"kind": "Conflict", "causes": [...]}`, and the core records the server's first content
image in the background (once, however many ask, on one thread, for a running server
too). The player asks again after the delay. When that record failed, no other starts for
a minute, doubling with each failure in a row up to 30 minutes; meanwhile the answer is
**503** with `{"kind": "Internal", …}` and `Retry-After` the seconds left (at least 5). A
player that gets `Internal` stops asking, tells the player the server couldn't prepare its
files, and offers to try again (which asks again, and may get the same answer until the
delay has passed).

**A player whose views were all pruned** (retention keeps the newest 30 content images)
names views the core no longer knows, so it gets the whole view (200,
`X-Lodestone-Answer: view`). It diffs that against the view it last applied
(`lodestone_image::diff(applied, received)`), which gives exactly the delta the server
would have sent, and goes on as with a delta. So a player keeps the view it last applied,
and every view it names in `have` (a delta may come from any of them).

**What a view places.** A view's files are its base's share for the side, less `removed`,
with the view's entries over them by path:

1. the base's share: for a `modrinth_pack` base, the pack's index files whose `env.client`
   isn't `unsupported`, and its `overrides/` with `client-overrides/` over them (as any
   Modrinth launcher installs a pack for a client); a `loader` base or none has no files.
   The core never sends a `curseforge_pack` base today; a client that gets one refuses the
   view and tells the player it can't install it;
2. less every path in the view's `removed` (pack files the server deleted, disabled or
   replaced under another name; a client view's never names a config, so the pack's
   configs stay: "Views");
3. then every entry of the view, placed over whatever the base put at its path.

A core content image lists every content file the server has (pack files too, with their
sources), so on a server built on a pack the base only adds what a server never installs
(the pack's client-only files and `client-overrides/`) and the pack's own copies of files
the view leaves out (its configs, unless marked shared with players: the player gets the
pack's version, not the server's). Two limits follow from installing the pack: the owner's
"Not for players" mark on one of the pack's own files doesn't keep it from players (the
pack's index decides those, as on Modrinth; the `.mrpack` client pack, made from the
server's mods folder, does honour it; see "M3 decisions"), and a pack file the server
changed in place reaches players as the server has it only when the view carries it. A
pack file outside the client share that the view carries anyway (a `server-overrides/` mod
the owner marked for players) has source `object`: only the server can give it.

**From one view to the next on disk.** A delta (or a player's own diff of a whole view)
only gives the new view; it is not a list of what to change on disk, because a view's files
aren't only its entries. What changes on disk is the difference between the files the old
view places and the files the new view places, each worked out in full as above:

- when `base` changes (the server's pack was updated), the client gets the new base and
  works out its client share again: the new pack's client-only files and
  `client-overrides/` change too, though they are never entries and never in the delta;
- when `removed` changes, the base files it names go, or come back when a path leaves it;
- entries with source `base` whose bytes didn't change aren't in the delta, but follow the
  base.

The three-way merge (Launcher step 4) compares those two placed file sets with what is on
the player's disk, so the player's own mods and changed configs stay.

Files a view can't give: its `withheld` paths (a mod jar that is a symbolic link, a file
that kept changing or couldn't be read when the image was recorded; names only) are files
the server has that the image holds no copy of, so a player can't get them from the
server. A player must say so before it plays, for those a client view would carry as files
(in `mods/`, say; not configs, the server's own files or disabled ones), listing them with
the reason, and not install silently without them. As code: a withheld path that parses as
a `CanonicalPath` is needed when `view::in_view` holds, for the client, for an `Entry` at
that path with unknown sides (`Entry::new(path, 0, <any hash>, Source::Object {})`); one
that doesn't parse is needed when its first part (before the first `/`) is `mods`. A view's `excludes` never reach `mods/` ("M3 decisions"), so they
only name files players don't need.

**`GET …/objects/{sha256}`.** The bytes of a file the current client view names with
source `object`, or of its client settings' icon or background. Nothing else in the store
is ever served, whatever its hash: not `modrinth`, `base` or `curseforge` entries (the
player downloads those from where they say), not server-only files, configs the view
leaves out, worlds, backups or another server's files. 400 when `sha256` isn't 64
lowercase hex digits; 404 for anything not served, including an object of a view that is
no longer current (the player asks for the image again and fetches what the new view
names). While the server has no content image it can read (none yet, or its manifest was
damaged), the answer is the image route's 503 (`Conflict`, then `Internal` while a failed
record waits) and a record starts, as for `…/image`. A large file (over 1 MiB, a chunk list in the store) is sent whole, put together
from its chunks: chunk lists never reach players.

- 200: `Content-Type: application/octet-stream`, `Content-Length`, `ETag: "<sha256>"`,
  `Accept-Ranges: bytes`, `Cache-Control: private, max-age=31536000, immutable`.
- One byte range of a file over 1 MiB (`Range: bytes=a-b`, `bytes=a-`, `bytes=-n`): 206 with
  `Content-Range: bytes a-b/<size>`; a range starting past the end, or the empty suffix
  `bytes=-0`: 416 with `Content-Range: bytes */<size>`. A file of at most 1 MiB is always sent whole (200, the
  `Range` ignored: the core reads and checks it whole to send any of it), and so are
  several ranges, another unit or a malformed header, as HTTP allows.
- HEAD: the same status and headers, no body; it checks the file is in the store (reading
  at most a large file's chunk list, so a large file that lost a chunk is only found by a
  GET that reaches it), as GET does before it answers, and counts as a download while it
  looks.
- 503 with `Retry-After: 1` and `{"kind": "TooManyRequests", …}` while 64 downloads run
  over every server and route, 16 of this server's, or 8 of this player's (by address, an
  IPv6 address's /64, or by friend; a Connect invite's info port can't tell its players
  apart and has only the first two limits). A player should run at most 4 at once.
- 503 with `Retry-After: 5` and `{"kind": "TooManyRequests", …}` while the store's garbage
  collection runs (seconds, or minutes on a large store with many backups): a request
  never waits for one. A download already running pauses until the collection ends, so a
  client gives a download a generous read timeout (minutes, not seconds) and treats a
  stall as a passing failure: it fetches again, resuming with `Range` if it likes.
- 503 with `Retry-After: 5` and `{"kind": "Conflict", …}` when the store lost a file the
  view names (found damaged and deleted, or a large file that lost a chunk: the download
  that found it ended short and took the file's chunk list away): the core records the
  server's content in the background, which stores the file again from the server's disk,
  and the player fetches again after the delay. After such a record, none starts again for
  a minute (whether it helped or not, doubling while they fail); meanwhile the answer is
  503 with `{"kind": "Internal", …}` and `Retry-After` the seconds left. A player keeps
  asking through `Conflict` answers (the record may wait for an owner's long operation, a
  modpack update, say) for up to 2 minutes in all, and gives up on a file after that or
  after 3 `Internal` answers in a row, and says the server couldn't send it.
- A download whose player takes nothing for 60 seconds ends, cut short (a player must
  read at least about 4 KiB/s: the core hands the connection 256 KiB at a time).
- Every byte is checked against the store before it is sent, but a check that fails can
  only be found after some bytes went (a damaged chunk mid-file, or the whole file's
  SHA-256 at the end): the response is then cut short. So the player checks the SHA-256
  of what it got, and fetches again (with `Range` to resume a large file, if it likes) when
  the body was short or doesn't hash to the name, at most 3 times in a row per file. A file
  the store can't read for now (the core out of file descriptors, an antivirus holding it)
  is answered the same way, a 500 before the body or a body cut short, and the next try
  usually works.

**Freshness.** The view is the server's current content image: what Lodestone's actions
change shows within the merge window (3 s after the last change, at most 30 s), and
changes made outside Lodestone show at the next record (at the latest, the check before
the server's next start). It is not the image a running server started with, so mods
added or removed while a server runs reach players before the server loads them, as the
`.mrpack` client pack does.

The Launcher (M4):

1. shows what it will install (base pack, mods the server added, files only the server has,
   game and loader), with the existing trust advisory; launch settings never come from a
   server;
2. installs the game and loader, gets the base (the `.mrpack`, cached and hash-checked),
   places the view as "What a view places" says (Modrinth by CDN and SHA-512, objects from
   the server, CurseForge when allowed, listed otherwise), checks every SHA-256, places
   files atomically;
3. records the view it applied plus the server link, and keeps that view (and any it will
   name in `have`) for deltas and for diffing a whole view;
4. next Play: `have=<that view id>` (and any other views it keeps), `rules=<its RULES>`;
   a 204 whose `X-Lodestone-View` is the applied view means join at once (one naming
   another of its views means switch to that one first), otherwise apply the delta to the
   view its `from` names (or take the whole view), check the result, and place the
   difference between what the applied view places and what the new one places ("From one
   view to the next on disk") with the three-way merge modpack updates already use (the
   player's own mods and changed configs stay).

Works for Fabric, Quilt, Forge and NeoForge servers.

## Restore

| Choice | Puts back |
| --- | --- |
| World only | The world's folders, after a safety image |
| World and its mods | The world plus the content it ran with, in one swap (the core's `world_and_mods`: the content files of the data image itself, which needs its content link) |
| Everything | The whole image (the core's current Everything scope) |
| As new | A new server or instance beside the original |

When the current content differs from a world's linked content, the restore says what
differs and offers to restore it too (the Launcher's restore, M5; the core's world restore
doesn't yet). Restore uses the core's journaled swap (staging,
rename aside, roll back on error, replay at startup); the Launcher has the same journal
(`restore-journal.json`, recovered when it starts, before anything can be played).

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
- Launcher to Launcher: share an image file (`.lodeimg`, an exported snapshot, M5), or an
  `LDS2:` code when every file can be downloaded.

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
  in the local store. The objects endpoint (M3) serves only objects the server's current
  client view names with source `object` (and its client settings' icon and background),
  never any other object of the store by its hash: not a `modrinth`, `base` or
  `curseforge` entry, a world, a backup's file or another server's; an export of a server
  that leaves the computer (M6) must strip or withhold those files.
- The core's history API returns labels and paths as they are; a client that shows them
  writes their control and direction characters visibly (as `display_text` does).
- A Launcher content image takes the whole `config/` folder and every file a pack lists.
  Some client mods keep account tokens there (an account switcher's accounts file), and
  the game rewrites them into the next image, so up to 30 images may keep a token after
  the mod dropped it. That is fine in the local store, which is readable by its owner
  only on Unix (mode 0700) and in the default data folder on Windows (its user profile's
  permissions); a data folder moved elsewhere (another drive, an exFAT disk) has whatever
  protection its new place gives, like the rest of the data folder (`accounts.json`
  included). Any image that leaves the computer must strip or withhold such files: the
  Launcher's snapshot export (M5) takes out `command_history.txt`, `hotbar.nbt` and the
  account files of known account switchers and names them in `excludes` ("M5 decisions",
  best effort); milestone 4's second half and milestone 6 must do the same. The JVM's
  crash and dump files count as secrets too: it writes them into its working folder, the
  game folder, and a crash log (`hs_err_pid*`) holds the whole command line, the access
  token included. No Launcher snapshot takes them (`hs_err_pid*`, `replay_pid*`,
  `java_pid*.hprof`, `*.mdmp`, `core`, `core.<digits>` at the top of the game folder), and
  the export strips them too. Pack files
  never include the files the game writes, `command_history.txt` (commands typed, a
  server's `/login` password among them) and `hotbar.nbt` included, and an image that
  leaves the computer must never carry those either.

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
   old snapshot store or world backup zips: they are removed. (Done: see "M5 decisions".
   It also has the full-image option.)
6. Moving images between computers (Host to Host, Launcher to Host, Host to Launcher,
   Launcher to Launcher).

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
  players" marks become each jar's `owner_mark`) and world files, including those of a
  level that `level-name` puts inside a content folder (since M3: `level-name=mods/survival`
  keeps all of `mods/survival/` out; `level-name=mods` keeps out what a level holds directly
  in its folder, `level.dat`, `region/`, `playerdata/` and the like, `scope::ContentFilter`),
  which the disk layout can't take for a world. Names match ignoring ASCII
  case. Not `eula.txt`: accepting the EULA is server state, not a content choice, and
  accepting it through the API is no operation. The rules have a version (`SCOPE_RULES`,
  2 since M3); a baseline taken under other rules is taken again rather than compared.
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
  the content scope with the policy's translated excludes that can reach content (since M3
  none that could reach `mods/`, see "M3 decisions"; written into the image's `excludes`; a pattern anchored under a path where nothing is content, a
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
  Since M3 its `removed` lists the pack's files the server no longer has ("M3 decisions").
- **`env` without the network.** The inputs are the owner's mark (marked for players:
  client `required`; left out of the client pack: client `unsupported`, since M3 only where
  the other inputs don't make it `required`), the pack index's
  `env` from `pack.json` for files that still have the pack's bytes, what Modrinth said
  about the file's version when Lodestone fetched it anyway (kept with the content state by
  SHA-512, so ids don't change across restarts), and the jar's own metadata
  (`fabric.mod.json`, `quilt.mod.json`, and since M3 a Forge or NeoForge `mods.toml`'s
  `clientSideOnly` and `displayTest`). Nothing is fetched while recording; an unknown input
  stays unknown. A jar's SHA-512 and own sides are
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
  unsupported on clients and required on servers; a file that `client-overrides/` replaces
  for clients (an `overrides/` file or an index download at the same path) unsupported on
  clients (they get the client-overrides file, placed after the downloads).
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

- **One store.** The Launcher's image store is `<data>/image-store/`
  (`DataDir::image_store_dir`), one for every instance on the machine and for its content
  images and snapshots alike (milestone 5 removed the old snapshot store), plain encoding
  (zstd isn't in the Launcher's lockfile; APFS clones files into it where it can). Nothing is
  created until the first write, which makes the store's folder readable by its owner only
  (mode 0700 on Unix): the Launcher's own files are, and an object taken from a 0600 mod
  config would otherwise be readable by anyone who can reach the data folder. On Windows the
  folder gets the data folder's permissions (a user profile's, by default), and a data
  folder moved to another drive or an exFAT disk gets what that place gives. Settings ›
  Storage counts it with the instances, as its Snapshots row ("M5 decisions").
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
  and they go into data images (snapshots). Known limit, for milestone 4b: a file outside
  the content folders that no pack lists (a script added by hand) is not
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
  leave 1 GiB), and an instance that didn't fit is tried again at the next start. Settings ›
  Storage shows the store's size.
- **Free space.** Before a record copies anything it reserves what it will read, process
  wide (`Images::reserve_space`), and is refused (no image; tried again later, as any
  failed record) when the free space can't hold that, what the records under way have
  reserved, and 1 GiB to spare: most records are work nobody
  asked for, and the game's world saves need the room more. It is conservative: a record
  under way counts in full though part of it is already written, and an object APFS
  clones (a whole file of 1 MiB or less) counts whole though it costs almost nothing.
  Snapshots, restores and imports reserve through the same count.
- **Records before a change.** `Images::record_before_change` runs before a modpack update
  ("Before a modpack update") and before a server's mods are brought in step ("Before a
  server update"): what is queued, else "Changed by hand" (or "Changed while the game ran
  or by hand") when the quick check finds a change, else the change's own label, so every
  file it replaces is in an image (nothing new is written when the content is the current
  image). A failure is logged and the change goes on. Neither update starts while a "Find
  it for me" search is saved (the app and the CLI say to finish it first, as they say to
  stop the game): the search has mods turned off, the record before the update would take
  them as changed by hand, and the search's end would restore files the update replaced. Milestone 4's second half keeps this hook when server images replace the client pack
  sync (`servers/mod.rs`, `apply_sync`), as `Images::before_risky_change(RiskyChange::
  ServerUpdate)`, which records this and takes a snapshot ("M5 decisions").
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
  keep every data and full image and its `content` link (snapshots: their own retention
  deletes them), the images held with `Images::keep_while` (a restore's), the
  current image and its parent, and the newest 30 content images by `newest_content_first`;
  delete the other content manifests. While a manifest can't be read, nothing is deleted;
  the Snapshots list shows how many can't be read, with a button to delete them.
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
  Windows device name such as `con`) record nothing. Each of these retires `images/<id>/`
  (milestone 5): a folder with snapshots is set aside as `images/_deleted_<16 hex>/` with
  its snapshots and the content they link to, until the user deletes them; one without is
  deleted, as before ("M5 decisions", Retiring).
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
  (`backups/modpack-update/<time>/`) is its rollback when it fails part way. Its world zips,
  the snapshot store and `.lodesnap` were removed by milestone 5: the snapshot before the
  update holds the worlds. The Launcher had no content history of its own, so nothing was
  removed then.
- **The CLI.** `lodestone-launcher-cli content history <instance>` (method
  `content.history`, read-only) lists the content images newest first: `{instance, current,
  images: [{id, created, label, label_shown, parent, current, minecraft, loader,
  loader_version, game_shown, base, files, withheld}]}`, where `label_shown` and
  `game_shown` went through `display_text`; `--human` prints one line each from them (the
  CLI itself has no `lodestone_image`). An unknown
  instance is `not_found`; one without images has an empty list. It waits at most 3 s for
  a record of the instance under way (the first image of a big pack can take minutes),
  then lists without it: the newest image may be missing then. The window shows no
  content history: milestone 5 added snapshots and their restore, not a history view.

### M3 decisions

Decisions milestone 3 made where this document left serving players open ("Server to
player" has the protocol itself).

- **Three route forms, the client pack's access.** `/launcher/…` (`ViewInstance`),
  `/public/…` (listed with the client-pack scope) and `/api/v1/public/…` on the Connect
  info port and friend tunnels (the shared server only). Each router checks access first,
  exactly as its client-pack route does, then calls the same handlers
  (`handlers/player_image.rs`). The `.mrpack` client pack and its routes stay as they are,
  for other launchers.
- **`have` names client views, not content images.** A player only ever has a view, and a
  change only the server sees (`server.properties`, a server-only mod) leaves the view's id
  as it was, so the player stays in step. Newest first, at most 16; the first id the core
  knows decides, looked up first among the views it has learnt, so a player that keeps
  pruned ids after a known one costs no scan of the server's manifests. The core may send
  the whole view instead of a delta whenever the delta's JSON would be no smaller.
- **A view and a delta are told apart by headers**: `X-Lodestone-Answer: view|delta`, with
  `X-Lodestone-View` naming the current view on every 200 and 204 (see "Server to player"
  for how a player checks each). A delta's result gets the whole view's check
  (`lodestone_image::view::check_received`, which the shared crate gained), since `apply`
  checks only its id: the test client does both.
- **A pruned player diffs the whole view itself**, rather than sending a summary of what it
  has (which milestone 3's plan suggested, for example in a bounded POST). A summary that
  lets the core rebuild the exact view (each entry's source, sides and mode, not only paths
  and hashes) is about as large as the view, so uploading it to get a delta back would move
  more bytes than downloading the view, and would give the public routes a request body to
  buffer and parse. `lodestone_image::diff(applied, received)` gives the player the same
  delta the server would, since the player keeps the view it applied. No route takes a
  body.
- **Pack files the server took out are the image's `removed`.** A player installs the base
  pack's client share and places the view's entries over it ("What a view places"), so a
  core content image must say which of the pack's files the server no longer has, or a
  pack mod the owner deleted, disabled or updated to a new file name would come back on
  players' machines (two Fabric APIs, and Fabric refuses to start). The recorder lists in
  `removed` every file in `pack.json` (by its canonical path) that the walk didn't find
  there and that is content the image would carry (in the content scope, not left out by
  the excludes), never a path the image lists or withholds (a pack mod the server has as a
  symbolic link is withheld, not removed: the server has it); only with a base (an uploaded pack, with
  none, removes nothing). The same rule then holds for every image, the Launcher's too:
  base share less `removed`, entries over it. A restore that puts back an image built on
  another pack version (a backup, restored in place or as a new server, or the modpack
  history when that version's `.mrpack` can't be read) writes a `pack.json` with that base
  but without the pack's file list, which only the `.mrpack` has; it keeps the restored
  image's `removed` instead (`BaseInfo::removed`), and the recorder keeps those paths in
  `removed` by the same rules while they are still gone, until an install or update
  writes the pack's files again. The image's `removed` takes in the pack's configs the
  server deleted as well (the server's own record, which a restore follows), but the
  client view leaves configs out of its `removed` ("Views"), so players keep the pack's
  copies: a server deleting `config/xaerominimap.txt`, which a pack's `overrides/` put on
  both sides, or a config the server rebuilds at start, never takes the pack's settings
  off players' machines. With the file list unknown, which of the files the
  server has are the pack's isn't known either, so `pack.json` also keeps every path the
  restored image carried (`BaseInfo::paths`), and the recorder treats them as it would
  pack files: one the owner deletes, disables or renames after the restore goes into
  `removed`. Some of them aren't the pack's (a mod the owner added), but taking out a path
  the base doesn't have changes nothing for players, so erring that way is safe, while
  erring the other way would bring the pack's old copy back.
- **Open, for the developer before M4: "Not for players" on a pack's own file.** An image
  can't both list and remove a path (`lodestone_image` refuses such a manifest), so the
  mark on one of the pack's own files (`env.client` `unsupported` from `owner_mark`) drops
  its entry from the view but can't take it out of the base's client share: a player who
  installs with images gets it anyway, while the `.mrpack` client pack, made from the
  server's mods folder, leaves it out (owners use the mark to keep a mod that crashes
  clients off players). Nothing released depends on it yet (M4 is the first client). The
  smallest fix that keeps the format is to let `removed` name a path the image also lists
  ("not from the base": the server view places the entry over it anyway, the client view
  leaves it out), with the recorder adding pack files whose entry is client-`unsupported`;
  that changes `Image::validate` in the shared crate, so it waits until the Launcher's
  store (built in parallel) has merged. Until then this limit stands.
- **Backup excludes never take mods from players.** Content images take the backup
  policy's excludes that can reach content (an M2 decision), and the client view is a
  projection of the content image, so `*.jar` or `mods/` (an owner keeping backups small)
  would leave players with no mods while the `.mrpack` still had them. So content images
  also leave out every pattern that could match `mods/` or anything in it
  (`scope::exclude_reaches_mods`: a name at any depth, or a pattern from the game folder
  whose first part is or may be `mods`). A translation may leave out less than the policy
  did, never more, so content images hold what such a pattern names; backups, which take
  the policy's patterns themselves, still leave it out. A server that had such a pattern
  records "Changed the backup excludes" once at its next check before a start. A view's
  `excludes` therefore only name files players don't need; its `withheld` files are ones
  no image could hold, and a player lists those it would need ("What a view places").
- **Only `side=client`.** A server view carries configs and secrets; `side=server` is 400.
- **Which objects are served.** Only `object` entries of the current client view, plus
  its client settings' icon and background (the core records no client settings in its
  content images yet, so today that is only `object` entries; the rule is there for images
  that carry them); anything else is 404, including an object of
  a view no longer current (the player asks for the image again). A large file is sent
  whole, put together from its chunks, so players never see chunk lists. One byte range
  (206/416) of a large file only: a small one is stored whole and must be read and checked
  whole to send any byte, so a range of it is answered with the whole file (an anonymous
  `Range: bytes=-1` would otherwise cost a megabyte of reading, decompressing and hashing
  for one byte). HEAD, immutable caching. The whole file's SHA-256 is taken with ring
  (`lodestone_image::hex::Sha256Hasher`), as every hash of the shared crate is: the `sha2`
  crate hashes in software on ARM, several times slower than reading the chunks.
- **Download limits.** At most 64 downloads run at once over every server and route
  (`MAX_OBJECT_STREAMS`), 16 of one server (`MAX_OBJECT_STREAMS_PER_SERVER`) and 8 of one
  client over every server (`MAX_OBJECT_STREAMS_PER_CLIENT`); past any, 503 with
  `Retry-After: 1`. A client is what the route can tell apart (`objects::ClientKey`): on the
  main port the peer's address (an IPv6 address's /64, since a host is given a whole /64),
  through a friend tunnel the friend's key; a Connect invite's info port can't tell its
  players apart, so it has only the other two limits. Eight leaves room for a household
  behind one address (a player runs at most 4). A download is a task that hands its bytes
  to the connection in pieces of at most 256 KiB, slices of the step they were read in (no
  copy), reading ahead at most one step of about 1 MiB, so a stalled download holds about
  2 MiB (the step, at most 1 MiB and one chunk, and what the connection buffered; about
  128 MiB over all 64); a piece the connection doesn't take within 60 seconds (`IDLE_TIMEOUT`) ends
  the download, cut short, and gives its place back. So a client that opens downloads and
  stops reading holds them a minute at most; one that reads slowly on purpose (over 4 KiB/s
  per download) keeps at most 8 places, so it takes two addresses to hold a server's 16 and
  eight to hold all 64, at the cost of their bandwidth. Behind a reverse proxy on the same
  computer every player has the proxy's address and they share 8.
- **Collections are held off per read, never per download.** A download reads at most 1 MiB
  at a time (a small file whole, or a large file's chunks), each read on a blocking thread
  of its own under its own shared hold on the store's collection lock: a slow player holds
  no thread, and a collection waits for at most one read. The objects a current view names
  are always there (the current image is never pruned, and saving its manifest checked
  them); an object of a view superseded and pruned during a download can go at a
  collection, and the download then ends short. The other way round, a read waits for a
  collection that runs (the lock is the operating system's, which a collection holds
  exclusively for its whole run): a running download stalls until it ends, so M4 gives
  downloads a generous read timeout. A new request doesn't wait: it tries the lock
  (`Store::try_hold_off_collection`) and answers 503 `TooManyRequests` with
  `Retry-After: 5` while a collection holds it, so a HEAD or GET never hangs unanswered.
  File locks give a waiting collection no priority: a collection waits on the lock (on a
  blocking thread of its own) and runs at the first instant no read holds it. Each hold
  covers one read of at most 1 MiB, so it waits only as long as reads keep overlapping,
  which on a busy server can be long.
- **Damaged and lost objects.** A whole object is checked before any byte goes; a large
  file's chunks one by one, and on a full download the whole file's SHA-256 at the end. A
  check that fails ends the response short (the player sees a truncated body and must check
  SHA-256 itself) and deletes the damaged object or chunk, but only on proof: the store's
  plain reads call any failure to open or read a file damage, and running out of file
  descriptors (or an antivirus holding a file on Windows) would then delete healthy
  objects that backups share, from an anonymous request. So the serving path reads again
  with the checked functions the shared crate gained (`Store::read_chunk_checked`,
  `remove_damaged_checked`, `remove_damaged_shallow_checked`, `remove_damaged_chunk_checked`,
  `remove_incomplete_checked`),
  which call damage only bytes that were read and are wrong, or a chunk the drive says is
  missing, and report any other failure as an I/O error that deletes nothing: the request
  is then a 500, or its body is cut short, and no record starts. The core also raises its
  soft limit on open files at startup (`open_files::raise_limit`; macOS starts programs
  with 256) to 10240, or the hard limit if lower, and never lowers it: 10240 is what macOS
  allows, and ample for 64 downloads and records. It isn't raised to the hard limit
  elsewhere, since the servers the core starts inherit it, and in containers or under
  systemd that is often a million or more, which slows programs that loop over every
  possible descriptor. A large file that lost a chunk
  (found damaged and deleted, or missing) also loses its chunk list
  (`Store::remove_incomplete_checked`): the list can't give the file whole, and while it is there the
  object looks present, so nothing would ever store the file again. Before it answers, a
  download checks the object is there (a large file's chunk list read and checked, not its
  chunks, which would cost an open per chunk on every request, under a hold on
  collections; when that read calls it damaged, a checked read of the object and the list
  only, `remove_damaged_shallow_checked`, decides whether to delete, never one of every
  chunk, which for a file of a few GiB would read all of it on a request); when it isn't, the answer is 503 rather than a 200 whose body ends at once,
  so a player can tell a passing failure from a file the server lacks, and the core records
  the server's content in the background (`ImageService::record_again`, labelled "Changed
  outside Lodestone" if the content changed meanwhile): the content stat cache can't claim
  a missing object, so the record stores the file again from the server's disk. Once
  however many ask, and none starts again for a minute after one ends, whether it helped
  or not (doubling while they fail), so a file gone from the server's disk too can't make
  players rescan it over and over. A current manifest that can't be read is saved again by
  the next record (a record that finds the same contents rewrites a manifest that doesn't
  load), and until then counts as no image.
- **No content image yet** is 503 with `Retry-After: 5`, and the first content image is
  recorded in the background ("Content when Lodestone began recording"), once however many
  players ask, on one thread. A record that fails isn't tried again for a minute, doubling
  with each failure in a row up to 30 minutes (per server, in memory, forgotten on
  success); meanwhile players get 503 with the time left as `Retry-After`. Anyone may ask
  on the public route, and each record holds the owner's operations off while it scans,
  so a server whose first record keeps failing must not be rescanned on every request. Unlike the startup sweep this also records a running server,
  and leaves `running_since` as it is, so the next stop takes the baseline again as usual.
  A pointer whose manifest went missing or is damaged counts as no image (the record saves
  it again); a read that fails otherwise (too many open files) is a 500, which a record
  wouldn't mend; a record a player's request would start checks the same way, so a state
  file that fails to read (rather than isn't there or is damaged) never starts one. An
  older view the core learnt whose manifest is gone, damaged or can't be
  read now is forgotten, and the player gets the whole view instead of a delta. The record's task settles its state however it ends: one that panics
  counts as failed, so a server is never left "being recorded" for good; a deleted server's
  state is forgotten. These 503s aren't logged per request (players poll them every few
  seconds); the record logs its failure once, and a current manifest that can't be read is
  logged once per image (again only after it read fine in between).
- **Freshness.** The view follows the current content image, not the image a running
  server started with: mods added or removed while it runs reach players before the server
  loads them, and a removed mod both sides need can stop players joining until a restart,
  as with the `.mrpack` today. Serving the image a server started with (a `started_with`
  pointer that retention keeps) is left for later, for simplicity.
- **Sides are settled when an image is recorded.** A view is a function of the image, so
  nothing is looked up while serving: no network, no jar read. The rule for what nothing
  knows is the client view's (rules 1): everything not client-`unsupported` goes to
  players, so `optional` mods (JEI) and mods with no known sides go (this fixes the client
  pack's dropping of optional mods, LS-143), and players can always join. The price is
  that a server-only jar with no metadata and no Modrinth hint reaches players; the owner's
  "Not for players" mark takes it out. That mark is the marks file's `excluded`, which in
  the marks' own model (the client pack and the Mods tab: `mods::client::base_requirement`)
  only decides a mod whose sides leave players free, so it counts only where the other
  inputs (the pack's index, Modrinth, the jar) don't make the client side `required`
  (`images::env::inputs`): unmarking a mod once marked "For players" leaves it `excluded`,
  and must not take a mod both sides need from players while the Mods tab shows it going
  to them. Unlike the client pack, the view doesn't follow mod dependencies: a mod left out
  this way that a mod players need depends on stays out of the view (the Mods tab shows it
  as needed by the other); the owner unmarks it. The Host's and the dashboard's Mods tabs
  offer the mark only on Fabric servers today (the only ones with a client pack), though the
  API takes it for any mod loader: M4, which makes the client view what players install,
  shows it for every mod loader. What Modrinth says about a jar later (a Mods-tab
  listing) takes effect at the next record, without an image of its own. `jar_environment`
  now also reads Forge's and NeoForge's `mods.toml`: top-level `clientSideOnly = true` is
  client only; `displayTest = "IGNORE_SERVER_VERSION"` on every `[[mods]]` entry (clients
  may join without it) is server only; anything else says nothing
  (`lodestone_minecraft::jar::mods_toml_environment`). That declaration is also what a
  client-optional mod (one with client features that clients may skip) declares, so a
  Forge mod Modrinth doesn't know that declares it and adds client features doesn't reach
  players. It is kept server only on purpose: most mods declaring it are server-side
  utilities, some of which crash a client that loads them, and a crashing client is worse
  than a missing optional feature; the owner's "For players" mark puts such a mod in the
  view (it beats every other input), and a mod Modrinth knows takes Modrinth's sides. Every Forge and NeoForge content
  image with such a jar gets a new id once, which nothing released depends on. The Mods
  tab's "for players" column reads the same metadata, so Forge mods that declare a side now
  show it there too.
- **Caching.** Per process and in memory: views by content id (32, at most 32 MiB of view
  JSON; a view takes a few times its JSON in memory, and the most recently used one is
  kept however large), deltas by their two view ids (64, at most 16 MiB), and which content
  images each view id comes from, per server (many share one view). All of them are
  immutable for one version of the rules (a content id is its contents), so nothing is ever
  invalidated; a server's view ids are forgotten when it is deleted, and pruned manifests
  at the next scan (a view stays known while any manifest left has it). A view id the core
  hasn't seen makes it read the manifests it hasn't read yet, each once while the core
  runs, one scan per server at a time; players asking at once for a view not cached yet
  wait for one computation. A request then costs a look at the state file's size, time and
  inode, and how many saves this process made (the pointer, `content-state.json`'s
  `current`, is read again only when any changes; every save renames a new file into place,
  and the count tells this process's own saves apart where a file system reuses inodes at
  once and keeps coarse times) and cache hits. Players asking at once for a delta not
  cached yet (all the players of the older view, after a modpack update) wait for one diff,
  as for a view. A computation (a view, a delta, a scan of the manifests) runs on a
  blocking thread that fills the cache itself and keeps its turn until it ends, so a
  request dropped meanwhile (its player hung up) neither throws the work away nor lets the
  next request start it again: a client that asks and hangs up at once costs the work
  once. A turn counts the requests holding it (the one computing and those waiting) and
  leaves its map only when the last is dropped, so a waiter that hangs up doesn't let the
  next request start a second computation either. A scan learns a manifest for good only
  when it read it or found it unusable (damaged, not a content image); one that failed to
  read in a way that may pass (too many open files) is read again by the next scan. A cached delta answers without the view it starts from being loaded (its two ids
  are its key), so cycling through many old views costs a cache hit, not a manifest read
  each. A 300-mod view takes
  about 30 ms and its delta about 60 ms the first time in a debug build, and a cached answer
  well under a millisecond.
- **What the view leaks, by the rules (rules 1).** Every file in `mods/` that isn't
  disabled or Lodestone's goes to players, including data a mod keeps there; the view
  carries the image's `withheld` paths and the excludes that can reach content (names only,
  never contents: a config withheld as `kept_changing`, an exclude naming a hidden file).
  Configs (unless marked for players), `server.properties`, plugins and worlds never go (a
  level that `level-name` puts inside `mods/` isn't content at all). Changing this needs new
  rules, since it changes view ids.
- **Rules and versions.** `…/image` takes an optional `rules=<n>`, the newest rules the
  client knows. A core whose rules are newer answers 400 ("update Lodestone") rather than
  send a view the client's `view` check refuses (`UnsupportedRules`); older or equal rules
  change nothing, since views are made under the core's rules. Every answer past the access
  checks carries `X-Lodestone-Rules`, the core's rules, which is how a client recognises
  that 400 (a header rather than a new error `kind`, which would change the core's shared
  error type): its message is for people and may change. Unknown query parameters
  are ignored, so a client sends it to every core. A client that still meets a schema or
  rules it doesn't know tells the player to update and installs nothing.
- **Finding out which servers serve images.** No new field in the listings: the image
  routes answer wherever the client pack's access holds (the client-pack scope on the
  public list, a friend's shared server, `ViewInstance`), for every mod loader, and a 404
  where the `.mrpack` is served means a core older than milestone 3 ("Server to player").
- **`base` entries need the pack.** The objects endpoint never serves them, by design: a
  player gets them from the base pack's `.mrpack`. If that pack version disappears from
  Modrinth, players can't install those files; M4 must say so clearly.
- **Denial of service.** The public routes need no sign-in, so their costs are bounded:
  views and deltas are cached (by count and bytes), unknown `have` ids cost a folder
  listing (only when no id the core knows decides), each manifest is read once per
  process, no route takes a request body, a failed first record (or any record that stores
  lost objects again) isn't followed by another for a minute or more, and downloads are
  capped (64 in all, 16 per server, 8 per client, ended after 60 seconds without progress)
  with no thread held while a player is slow. Errors a player can cause or see never carry
  the store's paths (the store's own messages are causes, which only the log gets). A first burst of unknown ids on a server with many backup-linked content images
  reads each of them once, however many of those requests hang up early (the scan keeps
  what it learnt). A running download waits while a collection runs (seconds to minutes
  on a large store); a new request is told to try again instead. The core's HTTP server has no connection timeouts of its own, so a
  stalled connection keeps what it already buffered (at most about 2 MiB of a download:
  the connection's buffers, one 256 KiB piece waiting and the step being read) until the
  client closes it. Its download's place comes back after 60 seconds but the connection
  doesn't, so the number of such connections, and their memory, isn't bounded: a client
  that opens new downloads and never reads holds about 2 MiB more for each, as long as it
  keeps the connections open (the `.mrpack` route has the same exposure). A write timeout
  on the listeners would close them; it isn't there yet. Ranges of small files are
  answered whole, so a range costs what it sends.

### M5 decisions

Decisions milestone 5 made where this document left the Launcher's snapshots open. The core's
backups ("M2 decisions", Backups) are the model; the code is
`launcher/crates/launcher-core/src/images/` (`snapshots.rs`, `retention.rs`, `retired.rs`,
`restore.rs`, `swap.rs`, `export.rs`). Nothing of the old snapshot store, `.lodesnap` or
the world backup zips is read or converted.

- **A snapshot is a data image linked to content.** It holds everything in the game folder
  that isn't content (`ContentScope::data_accepts`: worlds, `options.txt`, `servers.dat`,
  screenshots, a script no pack lists; Lodestone's own names and the JVM's crash and dump
  files never, see Security), and its `content` is the instance's current content image,
  brought up to date first. A folder is skipped only
  when all of it is content, so a folder holding some pack files is walked for the rest;
  content and data never overlap and together are everything a plain walk takes (tested).
  "Include mods and configs" takes a full image of the whole game folder instead, with its
  base and the files' Modrinth and CurseForge sources; so does every snapshot whose
  content couldn't be brought up to date (recording failed, the store is suspended for
  content, a "Find it for me" search has mods renamed), so a snapshot always holds or
  links what was on disk. The image's worlds get `level_name`, `data_version`,
  `game_version`, `last_played`, `size` (from its entries) and `quiesced: true` (the game
  never runs during a snapshot: they are refused while it does, and one a session begins
  during, Play pressed again while the snapshot after the last session reads a big world,
  stops at the next file and records nothing, `ImageError::Running`). It uses the `data` stat
  cache, and the free-space check every record uses (what it reads, the records under
  way, 1 GiB to spare).
- **The content step, every kind.** Every snapshot's content record is
  `record_before_change`, which takes in what the game wrote since the last record (a
  session's end folds those writes, configs mods rewrite, into the baseline without
  recording them, so the quick check alone never sees them, and the snapshot would link
  an older content image than what was on disk). It records what is queued, else "Changed
  by hand" when the quick check finds a change, else, when the content differs from the
  current image only by what the game wrote, a label naming the change: "Before a modpack
  update", "Before a server update", "Before going back to a snapshot" (a restore of
  content), or "Changed while playing" for the rest (manual, after playing, a content
  update, a loader change, a world-only restore's safety snapshot). Nothing new is
  recorded when the content is the current image; an instance with none gets its first.
- **Records.** `images/<id>/snapshots.json` (`version`, the instance's name for a deleted
  instance's marker, and per snapshot: `id` `YYYYMMDD-HHMMSS-xxxx`, `image`, `kind`,
  `name`, `created`, `pinned`, `full`, `worlds`, `world_details` (each world's folder,
  `level_name` and `size`, for lists that show a world by its name and its share of a
  snapshot), `content`, `files`, `withheld`, `bytes`, `new_bytes`), written whole through the store's temporary names. Several records can
  name one image; a data image's manifest is deleted only with the last record naming it.
  Read with bounds (16 MiB, 10,000 records, every id a SHA-256 or a record id's shape).
  Kinds: `manual` (the user, "Back up" in the Worlds tab, the CLI), `before_change`,
  `after_play`, `before_restore`, `imported`. A name is the user's, cut to 200 characters,
  or the label of an automatic one; labels are plain English data like content labels:
  "Before updating <pack> to <version>", "Before updating from <server>", "Before
  updating <title>" or "Before updating N mods", "Before changing <loader> to
  <version>", "After playing", "Before going back to “<name>”", "Backed up <world>",
  "Snapshot" when unnamed.
- **Reconcile, and a damaged list.** Before listing, retention, a delete or a pin, under
  the record lock: a record whose manifest is gone is dropped; a data or full manifest no
  record names is listed as a manual snapshot named by its label. Unlike the core, whose
  unreadable `backups.json` is an error, a `snapshots.json` that is there but isn't a list
  is renamed `snapshots.damaged-<unix>.json` and rebuilt that way: kinds and pins are lost,
  never a snapshot, and the user isn't stuck with a list that never opens. Retention's and
  the list's look at the store (`Images::listing`) takes a data or full image a record
  names from the record (its content link is in it) and reads only content images' and
  unnamed manifests' headers: a world's manifest can be megabytes, and the listing runs
  after every content record and each time the Snapshots list opens. So a damaged data
  manifest a record names isn't listed as unreadable (restoring it fails, saying so); its
  link is kept either way.
- **Order of writes.** Objects, the manifest (retried once when a collection took an object
  meanwhile; an image another record already names isn't saved again), the record, the
  stat cache, retention. An instance whose first image store write is a snapshot gets its
  `content-state.json` (naming the instance) first, so the id-reuse guard never takes the
  snapshot for an earlier instance's. An automatic snapshot whose image is the newest
  record's writes only the stat cache; a manual one records again, sharing the image.
- **Retention.** `retention::data_to_delete`, the core's `to_delete` with the Launcher's
  kinds: manual, imported and pinned are never deleted; `before_change` and `after_play` are
  one pool, the newest `keep_last`, then the newest per UTC day and per ISO week
  (`DEFAULT_POLICY` 5, 7 and 4; per instance in `instance.json`'s `snapshots`, each at most
  1,000); `before_restore` kept 30 days, the newest 5 older ones, and the newest when it is
  newer than every other record. Within one second, records keep the order they were
  taken (an id's random part says nothing). A snapshot a restore is reading is held
  (`keep_while`) and never deleted meanwhile. Deleting goes image, then record, then
  pruning, then a collection (at most every 15 minutes for retention, at once for the
  user's delete).
- **After a play session**, off by default: `InstanceDefaults::snapshot_after_play`
  (Settings › Instance defaults) unless the instance's `snapshots.after_play` says
  otherwise. `Images::session_ended_with` takes it on the session's own thread after the
  baseline, unless another session began, and only after a run whose game started (not a
  launch that failed or was cancelled while it was prepared); a session that begins while
  it reads stops it (see the first bullet). Known limit: with Settings › After launch set
  to quit, or when the Launcher dies while the game runs, no end is seen and none is
  taken.
- **Before risky changes**, one hook: `Images::before_risky_change(instance, RiskyChange)`
  (`ModpackUpdate`, `ServerUpdate`, `ContentUpdate`, `LoaderChange`), which records the
  content as above and takes an automatic snapshot; `before_risky_change_with` also takes
  the change's progress and Cancel (the first snapshot of a big world reads all of it: the
  update's bar shows it, and Cancel stops it and the update, recording nothing). What a
  failure does is the caller's. **A modpack update stops**: nothing changes, and the
  update prompt comes back saying why, with "Update without a snapshot" (as the world
  backups before images aborted the update); the control API's `modpacks update` fails
  with the reason. **A content update goes on** (Update all, one update and their
  CurseForge halves, once per job, and the control API's `content.update`): its message
  adds "No snapshot was taken first: …", and the API's reply `snapshot_failed`. Modpack
  updates call it in place of `record_before_change`, and no longer zip worlds
  (`update::apply` lost `backup_worlds`; its rollback copy of changed files stays).
  **Server updates: milestone 4's second half is to call it in `apply_sync`** in place of
  `record_before_change(BEFORE_SERVER_UPDATE)` (that call is still what `servers/mod.rs`
  makes), and to stop the sync when it fails, as a modpack update does; this milestone only
  removed `apply_sync`'s `backup_worlds` argument. A change of the instance's game saved from the Settings tab,
  Undo or the CLI (`Instances::save_settings`, `instances set-loader-version`;
  `game_change`: a new loader version, whatever other fields change with it) takes the
  snapshot in the background first and saves `instance.json` after, with the instance's
  files busy meanwhile so Play waits, and the list showing the new version. Settings saved
  from the tab, Undo or the CLI meanwhile wait behind it (`saves_after_snapshot`) and are
  saved once it is, compared with what it saved; only a save from elsewhere (a pack's
  update) meanwhile wins, and this change is dropped. Another Minecraft version or loader is a game
  change too (`RiskyChange::GameChange`, "Before moving to Minecraft <version> with
  <loader> <version>"): no screen of the Launcher makes it in place (a copy with another
  version or loader is a new instance, and Undo never puts back a loader version for
  another loader or game version), but a save that did would upgrade the worlds at the
  next launch, so it is snapshotted rather than trusted never to happen. A snapshot that
  fails doesn't stop the save, as for a content update; it, a dropped change and a save
  that fails are each said in a notice ("No snapshot was taken before the game changed",
  "The new loader version wasn't saved"), and `save_settings` returns the pending save
  (`PendingSave`), which `instances set-loader-version` waits for: a dropped change fails
  with `conflict`, a failed snapshot is its reply's `snapshot_failed`. A pack's or
  server's update saves with `Instances::save`: it took its own snapshot.
- **Retiring instead of wiping.** Deleting an instance, forgetting one deleted outside the
  Launcher and the id-reuse guard all retire `images/<id>/` when it holds a snapshot (a data
  or full manifest, one that can't be read, or records): `retired.json` (`former_id`,
  `name`, `instance_created`, `deleted`) is written into it, it is renamed
  `images/_deleted_<16 hex>/` (`_` never starts an id the Launcher makes), and its state,
  stat caches, journal and the content images no snapshot links to are deleted. The
  renames (retiring, and taking back below) hold collections off: a collection marks
  manifests by listing `images/`, and an entry renamed while it lists could be seen under
  neither name. One
  without snapshots is deleted as before. The marker wins over everything: a crash before
  the rename leaves a marked folder that the guard and the next forgetting retire, never
  one taken for the new instance's. Retired folders are never forgotten, are marked by
  every collection, and are listed (`Images::deleted_snapshots`, `snapshots list
  --deleted`); Settings › Storage's clean-up deletes them, which the delete dialog says.
  **An instance that comes back takes them back** (`Images::readopt_locked`): put back from
  the Trash, it has the same id and `created` time, and the first time the store looks at
  it (the guard before any record, the Snapshots list, the history) with no `images/<id>/`
  of its own yet, the retired folder whose marker names that id and time (the most
  recently deleted, if several) is renamed back, a content state naming the instance is
  written, and the marker deleted last. Until the marker goes the folder is still a
  deleted instance's: a crash part way leaves a marked folder under the id, which the
  guard retires and takes back at once. A new instance that only reuses the id has
  another `created` time and never does. Restoring a deleted instance's snapshot into a
  new instance from the list isn't done.
- **Storage.** `disk::Report.images` (`ImagesUsage`: the store folder's size, snapshot
  count, how many the clean-up removes, retired instances), read from the lists and
  markers without locks; the store is no longer "Other". "Remove" (`Clean::Snapshots`,
  `disk::clean_snapshots`) deletes the snapshots the Launcher took on its own (not pinned)
  and every retired folder, then collects at once and reports what the store shrank by;
  files from the last hour go at a later collection (the grace), which the dialog says.
  At startup, in the background, a leftover `<data>/snapshots/` with `objects/` or
  `instances/` (the old store) is deleted (`disk::remove_old_snapshot_store`, with a trace
  line). The old world backup zips (`instances/<id>/backups/<world>/*.zip`) are left where
  they are: nothing lists or restores them, but they are copies the user made, possibly
  the only ones, and they count with the instance's folder in Storage. The clean-up's
  dialog says it removes every snapshot of deleted instances, pinned ones too.
- **Restore.** Scopes `Worlds`, `WorldsAndContent`, `Everything`, plus `restore_as_new` and
  `restore_world_copy` (the Worlds tab's "Restore as copy": the world placed beside the
  others as "<world> (restored <date>)", nothing replaced, no safety snapshot). An
  in-place restore, under the record lock and inside a counted write: refused while the
  game runs; for content scopes, while a search is saved, for a snapshot of another
  Minecraft version or loader (only "as a new instance"), and for a data image whose
  content link is gone. Then the safety snapshot (refused when it fails). It owns
  `minecraft/saves/<world>` per chosen world; content scopes add every content folder and
  each file outside them that the snapshot's or the current content lists; everything owns
  each top-level name of the game folder (on disk or in the snapshot) but the
  always-left-out folders and Lodestone's names. What it replaces is what the safety
  snapshot holds under those paths, less what the target withheld or left out by its
  excludes; everything else inside them is carried back (`session.lock`, links, files no
  snapshot took, folders it changes nothing in). An owned path it neither replaces nor
  places anything in isn't owned at all (`Staged::new`): a top-level file the safety
  snapshot couldn't take (locked by another program, a name no image holds) or the target
  left out stays as it is rather than being renamed aside and deleted with the work
  folders. A file it would replace that the safety snapshot withheld as unreadable or
  changing refuses the restore, and so does any file on disk it would overwrite that the
  safety snapshot has no copy of. Names are compared exactly, so an owned path whose name
  on disk is spelled differently from the snapshot's (letter case or accents, on a disk
  that takes them for one name), or a carried path the staging also has, refuses the
  restore with a message to rename it: the copy on disk would otherwise win over the
  snapshot's without a word. The window and the control API refuse a restore while
  another job holds the instance's files busy (an install). After the swap, a content
  record that fails (the disk filled up) is logged, not the restore's failure: the files
  are back, and the next check records the change. Files are staged with
  `materialize` into `<instance>/.lodestone-restore-staging-<16>/` while collections are
  held off. The store keeps every object its manifests name, so a missing one means the
  store lost it: the restore is refused, naming it, rather than downloading (only imports
  download).
- **The swap is Launcher-local** (`images/swap.rs`): the core's journal and rollback in
  their shape (phases, owned and carried paths, nothing at a destination ever deleted),
  not shared code. The core's swap (`core/src/backups/restore.rs`) has a third folder, takes
  the server software over in its journal (`take_on_software`), splits Paper's worlds,
  pins the server's settings after it and has its own error type; sharing one would mean
  rewriting the core's restore, merged in milestone 2 and otherwise untouched here, into a
  new crate module, which isn't an additive change. Renames use the Launcher's
  `fsx::rename` (about 2 s of retries on Windows, not the core's 10 s budget), because the
  Launcher's recovery runs on the window's thread at startup. Follow-up: one
  `lodestone_image` swap module both call, once milestone 4's second half has settled.
  `Images::recover_restores` runs on the window's thread right after the images open,
  before anything can be played: a journal is undone (or, when done, only cleaned up),
  work folders no journal names are deleted, and a journal whose instance folder isn't
  there stays, logged. **One that can't be undone keeps the instance from being played**,
  like the core's locked server: while its journal is there (`Images::restore_unfinished`,
  a rollback that failed at startup or during the restore), Play first tries again
  (`retry_unfinished_restore`, renames only, on the window's thread, waiting at most
  300 ms for the record lock) and, when it still fails, refuses with "A restore of this
  instance didn't finish and couldn't be undone…" (the control API's launch ends with
  it). Every restore first undoes an unfinished one the same way, rather than write its
  journal over the one that knows what is aside, and refuses when it can't. Content
  installs don't take the record lock, so one can write into an owned folder while a
  swap or its rollback runs: a rollback puts what is in its way into the staging folder,
  and what of that the restore didn't stage (the journal lists the staged files,
  `placed`) is moved to `<instance>/restore-leftovers-<16>/` rather than deleted with the
  staging folder; after a swap that succeeded, a file in the old folder the restore didn't
  replace goes there too. The restore's result names the folder (`Restored::kept_aside`,
  a notice in the window, `kept_aside` in the control API), and a rollback's error says
  it.
- **After a restore of content**, before its content record ("Went back to snapshot
  “<name>”", with the record lock still held): `instance.json` takes the snapshot's loader
  version; `content.json` learns each restored file's Modrinth or CurseForge source
  (keeping an entry with the same SHA-512); a `modpack.json` of the same pack whose version
  differs from the snapshot's base takes the base's version (version number and date
  cleared until the next check fills them) and as its files the paths it lists that the
  snapshot also has, with their restored SHA-1. The updater never removes a file it doesn't
  list, so a pack file only the older version had is treated as the user's: approximate,
  never destructive. Added packs forget files that are gone.
- **The differing-content offer.** `preview_restore` compares the current content image with
  the snapshot's content (the link, or a full image's content files), by enabled name: one
  Modrinth project removed and added is one update, turning a file off or on is an update,
  and the loader versions are compared. The dialog says "These worlds last ran with other
  mods." and what putting their mods back does, a line each with up to five names: the
  files it puts back, removes and changes; a toggle switches to the worlds and their mods.
  The Worlds tab's "Replace world…" says that only the world goes back and points to the
  Snapshots list for its mods. Known limit: the preview compares with the current content
  image without the quick check, so a mod dropped in by hand since the last record isn't
  in the list (the restore itself records it first, and replaces it).
- **Exports** (`.lodeimg`, `export::EXTENSION`): `container::export` of the snapshot's image
  (labelled with its name, no parent) and its content image, after `PRIVATE_FILES`
  (`command_history.txt`, `hotbar.nbt`, `ias.json` at any depth: In-Game Account Switcher,
  `essential/microsoft_accounts.json` and `essential/mojang_accounts.json`: Essential; and
  at the top of the game folder the JVM's `hs_err_pid*`, `replay_pid*`, `java_pid*.hprof`,
  `*.mdmp` and `core`, for snapshots that hold them however they were made) are removed
  from both and added to their `excludes`, both re-sealed. Best effort: an unlisted
  account switcher's file still travels; the dialog lists what was left out, and the
  CurseForge files whose authors don't allow downloads elsewhere
  (`ExportReport::not_downloadable`), which whoever imports the file won't get. Written to
  `<data>/cache/snapshot-exports/<record id>/` for dragging out or Save as (Storage's cache
  clean-up removes them), or where the CLI says; leaving the dialog stops writing it.
- **Imports** make a new instance: `container::open` (refused when its image isn't the one
  the preview the user approved showed: `expect_image`), then entries with Lodestone's
  names or paths the client can't place are refused, then the caller's free-space check
  for everything (unpacked objects, the files, every possible download, 1 GiB; the same
  figure as `ImportPreview::needed_bytes`, which the dialog shows beside the free space,
  refusing to start when it is short; a short disk is `ImageError::NoRoom`, which the
  window words itself), then `import_objects`; files the store lacks are downloaded, eight
  at a time (Modrinth through its CDN URL check, SHA-256 and SHA-512 checked, the SHA-512
  read in pieces; CurseForge through the API's download URL when the author allows it,
  looked up in one `files_by_id` request) into the store; the rest are left out and
  listed. A Modrinth or CurseForge entry that wasn't downloaded (the store had its bytes:
  the export carried the same bytes under another path, or the Launcher stored them
  earlier) is checked against its SHA-512 before anything is placed, and the import is
  refused when it doesn't match, so a sender's file never passes for a project's in
  `content.json`. The preview's `mods` are every file under `mods/` at any depth (Quilt
  loads subfolders, old Forge `mods/<version>/`) and in `coremods/`, and `carried_mods`
  those whose bytes the file carries, which the dialog and the control API's confirm name
  as coming from whoever made the file. Everything the store has is placed
  into a new staging instance; `instance.json` gets the game, the recommended memory and
  default launch settings (images carry none); `content.json` the sources. It is recorded
  "Made from a snapshot", and the imported image (with its content) is kept as an
  `imported` snapshot. An import of an image whose CurseForge files couldn't be
  downloaded can't later restore "everything" from that snapshot (those objects are
  missing).
- **LDS codes stay as they are, with no decode-to-image function.** `LDS1:` and `LDS2:` already encode an image whose files can
  all be downloaded: the base (a Modrinth pack's project and version: the image's
  `modrinth_pack`), each Modrinth file as project and version (an entry with a `modrinth`
  source, its hashes coming from Modrinth when installing and every download checked
  against its SHA-512), the pack's files the instance removed (base files the image lacks:
  the Launcher records what is on disk, so they are missing entries rather than listed in
  `removed`, which only the core's server images fill), files turned off
  or on (the `.disabled` name), the resource pack order and the shader (client settings).
  A code can be made when every file is the base's or Modrinth's; others are listed as
  left out, as before. The producer keeps reading `content.json` and the listing, since it
  matches files Modrinth knows by hash, which recording never does. Codes people already
  pass around keep working; no image-shaped code was added. A code can't be turned into
  an image without Modrinth: it has no file names, sizes or hashes, nor the pack's SHA-512
  that `modrinth_pack` needs, so the image it encodes exists once it is installed. The
  claim is pinned by `tests/modpack.rs`'s `a_share_code_based_on_the_pack_recreates_the_instance`:
  the sender's and the recipient's content images, both recorded, have the same game and
  base, the same files with the same sources and SHA-256, the same pack files gone and the
  same resource pack order, and the code's items are the added Modrinth entries; they differ
  only in the files the plan lists as left out.
- **World identity is the folder name.** After a world is renamed, the Worlds tab no longer
  lists its older snapshots under it; they stay in the Snapshots list, and restoring the
  world from one puts back the old folder name. Renaming no longer moves a backups folder.
- **The UI.** Settings › Snapshots: "Take snapshot…", the after-play toggle (following the
  global default, which its hint says, until changed; then the instance's own, with "Use
  the default"), a row per snapshot (name, a badge: Automatic, After playing, Before a
  restore, Imported, Pinned; time, worlds by their own names, sizes) with Restore…,
  Export…, Pin or Unpin and Delete (a pin or delete that fails is a notice), a row for
  images that can't be read with Delete (asking a second time: they go for good), and the
  retention line. One dialog takes, restores (Only the worlds with world checkboxes,
  The worlds and their mods, Everything, As a new instance), exports (drag out or Save
  as…), deletes and imports (game, worlds, mods with the trust warning, downloads, files
  that can't be downloaded, disk space and what is free, name: the file's name, which
  exports make "<instance> - <snapshot>"). A restore opens on a choice it offers (as a new
  instance when the snapshot has no worlds and can't put content back); as a new instance
  it is named "<instance> (<snapshot>)"; work that reports no size shows no bar. The
  Worlds tab's "Back up" takes a manual snapshot named after the world (of every world:
  a snapshot holds the instance's data); its Backups panel lists the snapshots holding
  the world, each with the world's own size there and "with N other worlds", with
  "Restore as copy" and "Replace world…" (a world-only restore). `.lodeimg` files open
  the import dialog when dropped or picked with Import…; a `.lodesnap` is refused as no
  modpack.
- **Control API and CLI.** `snapshots.list` (`deleted`), `snapshots.create` (`name`,
  `everything`), `snapshots.restore` (`scope` worlds, worlds_and_mods, everything or new;
  `worlds`, `name`, `preview`; confirm in place), `snapshots.export`, `snapshots.import`
  (`preview`, `name`; confirm with mods), `snapshots.delete`, `snapshots.pin`,
  `snapshots.policy`; `worlds.backups`, `worlds.backup` and `worlds.restore` (`snapshot`,
  `mode`) work on snapshots. `snapshots.revert`, `worlds.delete_backup` and the modpack
  update's `backup_worlds` are gone; the CLI follows (`snapshots restore`, `pin`, `unpin`,
  `policy`; no `worlds delete-backup`, no `--no-backup-worlds`).
- **Not in M5:** a content history view (the CLI's `content history` stays the only one),
  restoring a deleted instance's snapshot into a new instance (they come back with the
  instance when it is put back from the Trash), restoring from images moved between computers
  other than `.lodeimg` imports (milestone 6). **The shared crate is unchanged**: no
  additive function was needed (restores and imports report progress per download, not per
  byte placed).
