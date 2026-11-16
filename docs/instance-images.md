# Instance images

One format for every copy of a Minecraft instance that Lodestone keeps or moves: server
backups, Launcher snapshots, modpack history, what a server sends players so they can
join, and sharing between Hosts, Launchers and friends. It replaces five formats that
each do part of the job:

| Today | Becomes |
| --- | --- |
| Core backups (`core/src/backups`) | Data images |
| Launcher snapshots, `.lodesnap`, world backup zips (were `launcher-core/src/snapshots.rs`, `worlds.rs`) | Data and full images in the Launcher's image store; the export is an image export, `.lodeimg` (done in M5). World backups are plain zips again, outside the store, beside the snapshots: see "World backups" |
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
  view of a Launcher's image is what "Host this" makes (the core takes the server view of
  the `.lodeimg` the Launcher sends, "M6b decisions"); the server-to-player flow uses the
  content image, which has no worlds. The other way, "Download this world" places the
  client view of a server's content with its worlds (`images::from_server`).

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
  knows, and places the difference between what the view it applied placed and what the
  whole view places (the Launcher compares the two placed file sets in full, "M4b
  decisions"; a client could also diff the views, `lodestone_image::diff`, which gives
  the same delta the server would).

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
`X-Lodestone-Answer: view`). It then goes on as with a delta: what changes on disk is the
difference between the files the view it last applied places and the files the whole view
places ("From one view to the next on disk"), which needs only the view itself and what it
recorded placing (the Launcher keeps both: "M4b decisions"). A client that wants the delta
can diff the two views (`lodestone_image::diff(applied, received)`), which gives exactly
the delta the server would have sent. So a player keeps the view it last applied, and
every view it names in `have` (a delta may come from any of them).

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
| As new | A new server or instance beside the original; on the core also from a `.lodeimg` file another Host or a Launcher exported ("M6a decisions") |

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

- Host to Host: a backup exported as a `.lodeimg` (its secrets taken out) becomes a new
  server on another Host (done: "M6a decisions").
- Launcher to Host: the core makes a server from the server view of a Launcher's `.lodeimg`,
  with one picked world as its level (done in the core and the GPUI Host's "From a file":
  "M6a decisions"); the Launcher's "Host this" writes and sends that file and makes the
  server (done: "M6b decisions").
- Host to Launcher: "Download this world" gets a backup as a `.lodeimg` through the core's
  export; the Launcher builds a new instance from the client view of its linked content
  image, the client share of the modpack the server is built on (as its players get it
  through the base) and its worlds, as it does for any core's `.lodeimg` opened in it
  (done: "M6b decisions").
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
  `curseforge` entry, a world, a backup's file or another server's. An export of a server
  that leaves the computer (`POST /instance/{uuid}/backups/{id}/export`) blanks every
  `server.properties` value whose key says password, secret or token (and
  `text-filtering-config`) and drops its lines the reader can't parse, blanks Paper's
  Velocity forwarding secret (`config/paper-global.yml`, and `paper.yml` before 1.19) and
  Pufferfish's Sentry key, and withholds the plugins' folders unless the owner includes
  them, Lodestone's own files (its settings hold the Java command and arguments), the
  console's history, the JVM's crash and dump files, the programs and scripts at the top,
  known keys to the server and services' logins (proxy forwarding secrets, NeoForge's
  forwarding mods' settings and any settings at the top of `config/` that name velocity,
  forwarding or proxy, Paper's `config/legacy-backup/` copy of the old `paper.yml` with
  its Velocity secret, Floodgate's folder and Geyser's copies of its key and saved logins,
  database passwords of LuckPerms, CoreProtect, Plan, LiteBans, AuthMe, EasyAuth, Ledger
  and the map plugins, ServerTap's REST API key, AuthMe's and EasyAuth's players' password hashes, Discord bots'
  tokens (SDLink's encrypted one with its key too), BungeeGuard's and the vote listeners'
  tokens and keys, Dynmap's and BlueMap's web pages for a database; "M6a decisions" lists
  them and the ones checked that hold none), banned
  players' IP addresses and what the Launcher's export takes out. On top of those named
  lists, a generic rule withholds every settings file the export would still carry that
  holds a credential under a name that says so, whichever mod or plugin wrote it
  (`share::secrets`, "M6a decisions", Credentials found by their names): a key whose words
  name a password, secret, token or key (`botToken`, `secret-key`, `discord.bot.token`,
  `[secret] key`) with a value that isn't a placeholder, a value that is a login by its
  shape (a URL with a password, a Discord webhook or bot token), key and token files by
  their names (`.token`, `*.pem`, `*.key`, `id_rsa`, `.env`, `credentials.json`,
  `token.txt`), and settings files too big to read; "Include plugin settings" doesn't change
  it, since a credential that reaches an outside account or lets someone in never travels,
  and a server made from a file never takes such a file either. A YAML file it cleans is edited only
  when every line is in an allowlist of simple block style that SnakeYAML reads the way
  the editor does (`export::read_simple_yaml`), and withheld otherwise, so no form of YAML
  (a flow mapping over lines, an escaped key, a byte order mark before a flow mapping, a
  dotted key Bukkit's settings split into nested ones) can carry a secret past it. A server made from a file never takes those keys or programs,
  empties the same secrets, places nothing runnable, keeps functions below operator level,
  takes the operators' files (`ops.json`, Bukkit's `permissions.yml` and `commands.yml`,
  whose aliases run as the console, and permission mods' and plugins' data: LuckPerms', FTB
  Ranks', PermissionsEx's, GroupManager's, UltraPermissions', player-roles',
  ForgeEssentials', Command Aliases') only when the owner asks, sets the settings that would
  run a file, give operators' powers or move the worlds out of the server's folder to their
  defaults, deletes a settings file outside the same allowlist, writes `server.properties`
  again from what its reader parsed (online mode as the `true` or `false` Minecraft reads,
  which trims nothing from the value), and refuses a file whose `server.properties` or
  `ops.json` the summary can't read whole as the server would, so the summary shows what
  the server does and a file's settings give its sender no way in. The reader of
  `server.properties` follows Java's `Properties` line for line, comments included: a `#`
  or `!` line while the logical line is still empty (after a line holding only `\`) is a
  comment that ends with its own line, whatever backslash it ends with, so the setting
  after it is read as its own and its secret blanked.
- A server's `.lodeimg` opened in the Launcher ("Download this world", or a core export
  imported by hand) places only the client view of its content, and of that only what the
  content folders hold, and its worlds without the files only a server reads: never its
  `server.properties`, operators, plugins or their data, Paper's settings or a config not
  shared with players ("M6b decisions"); the base pack's client share comes only from
  Modrinth, checked as for players (M4b). Its objects are unpacked only for what is placed,
  each read from the file and checked: a file never gets the store's other files by
  naming their hash.
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
  Launcher's snapshot export (M5) takes out `command_history.txt`, `hotbar.nbt`, the
  official launcher's account files (`launcher_accounts*.json`,
  `launcher_msa_credentials*.bin`, `launcher_profiles.json` at the top of the game folder,
  which a `.minecraft` folder pasted into an instance brings) and the account files of
  known account switchers, and names them in `excludes` ("M5 decisions", best effort); the
  core's export does the same for a server's ("M6a decisions"; milestone 4's second half
  only receives images). The JVM's
  crash and dump files count as secrets too: it writes them into its working folder, the
  game folder, and a crash log (`hs_err_pid*`) holds the whole command line, the access
  token included. No Launcher snapshot takes them (`hs_err_pid*`, `replay_pid*`,
  `java_pid*.hprof`, `*.mdmp`, `core`, `core.<digits>` at the top of the game folder), and
  the export strips them too. Pack files
  never include the files the game writes, `command_history.txt` (commands typed, a
  server's `/login` password among them) and `hotbar.nbt` included, and an image that
  leaves the computer must never carry those either.

## World backups

Snapshots (the core's "backups", the Launcher's snapshots) are images of the whole
instance or server folder, deduplicated in the image store. A **world backup** is the other
thing people ask for: a plain, standard `.zip` of one world that opens outside Lodestone,
made only when asked and kept until the user deletes it. It is deliberately not an image:
no delta, no object store, no content link, so deleting one frees its whole size and a
player can take a world anywhere with it. The two sit side by side in the UI, each in its
own section. "World backup" always means the zip; "snapshot" always means the image.

### In the Launcher

`launcher/crates/launcher-core/src/world_backups/` (`mod.rs`, `restore.rs`), the shared
zip code in `worlds/zip.rs`, the window's `instances/world_backups.rs` (Settings ›
Snapshots' "World backups" group and its dialog) and the Worlds tab.

- **Files.** `<data>/world-backups/<instance id>/<world folder>/<id>.zip` and its
  `<id>.json`, outside the instance folder, so snapshots, exports and "Host this" never
  take them. The id has a snapshot record id's shape (`YYYYMMDD-HHMMSS-xxxx`, UTC). The
  zip's one top folder is the world's folder (`<world folder>/level.dat`, …), so it unzips
  into any `saves` folder. Files already compressed (`.mca`, `.mcc`, `.mcr`, `.dat`,
  `.dat_old`, `.nbt`, `.sqlite` with `-wal` and `-shm`, `.png`, `.jpg`, `.jpeg`, `.ogg`,
  `.gz`, `.zip`, `.jar`, `.zst`, `.lz4`, `.xz`, `.7z`) and every file of 64 MiB or more
  are stored as they are; the rest deflated. The `.json` (`WorldBackup`, `version` 1, at most
  64 KiB) has the id, the owner's `instance_created` and name, the world's folder, the
  name (the user's, cut to 200 characters, else "World backup"), the kind (`manual` or
  `before_restore`, with `restored`: the name of the backup whose restore it came before),
  `created`, what `level.dat` said (`level_name`, `game_version`, `data_version`), the
  zip's size, the files' size and count, and the names left out (at most 20, and their
  count). The zip is written as `.<id>.zip.<16 hex>.part` beside it, flushed and renamed
  (the folder flushed on Unix), and the `.json` last (`storage::write_atomic`): only a
  backup with both is listed, so none is ever half a zip.
- **What a zip holds.** Every file of the world, without following links (each file is
  opened with `O_NOFOLLOW` on Unix and must be a plain file, so one swapped for a link or
  a pipe after the world was listed refuses the backup). Left out:
  `session.lock` and Lodestone's working files (`.lodestone-…`, `paths::always_excluded`),
  and, counted and listed, links and names a canonical path can't hold
  (`CanonicalPath::new("saves/w/<path>")`: Windows device and 8.3 names, invisible
  characters, non-NFC names, …) or the zip check refuses (`check_pack_path` on
  `<world folder>/<path>`: `..` inside a name, a zip name over 512 characters), and of
  two names differing only in case (a disk that keeps case apart holds both) the later,
  with what is in it, so a zip only holds names every disk Lodestone runs on can keep and
  its own restore takes. A
  folder holding only names left out is left out too (not counted), so restoring in place
  carries it back whole rather than meeting an empty folder of that name in the zip. A
  world whose folder name the zip check or a canonical path refuses (`aux`, `COM1`, or
  `World.` outside Windows, which `is_plain_name` finds) isn't backed up at all, with a
  message saying to rename it (`worlds::zip::check_world_folder`).
- **Whose.** A backup belongs to the instance whose id names its folder and whose
  `created` time its `.json` holds. An instance put back from the Trash has both and sees
  its backups again; a new instance that only reuses the id never sees them. Backups no
  live instance owns are a deleted instance's (`WorldBackups::list_deleted`): Settings ›
  Storage counts them and its clean-up (`Clean::DeletedWorldBackups`,
  `deleted-world-backups`) deletes them for good, which its dialog says (instances still in
  the Trash count as deleted). An entry of the instances folder that isn't a readable
  instance (a broken `instance.json`, a link to a drive that isn't there) keeps whatever
  its id has, and nothing counts as deleted while the instances folder can't be read.
- **Making one** (`WorldBackups::create`, the Settings section's "Back up a world…", the
  Worlds tab's "Back up", `world_backups.create`): under the instance's record lock in a
  counted write of the image store, refused while a game session of the instance runs or
  the game has that world open (its `session.lock`, `worlds::open_in_game`), for a folder
  without `level.dat` (no restore could take it), for a world over the limits a restore's
  zip check reads (more than 500,000 entries, folders and the top folder counted, or 64
  GiB of files: `ZipPlan::within`), and without room for the world's size and 1 GiB on
  the backups' disk. Cancel, a session that begins or the game opening the
  world stops it at the next file or 8 MiB of a large one, and leaves nothing.
- **Restoring one in place** (`RestoreTo::InPlace`, "Restore…" after a confirmation,
  `world_backups.restore` mode `replace` with confirm): as a snapshot's restore, under the
  record lock, refused while a game session runs or any world of the instance is open, an
  unfinished restore undone first, only for a backup kept under a folder name a backup
  is made under (`check_world_folder`), and never when `saves/` or a folder above it is a
  link (a swap through it couldn't be undone; restore as a new world instead). The zip is checked whole before anything is
  written (`worlds::zip::check`, shape `OneWorldFolder`), then what the swap would refuse
  (from the list of what the safety backup will zip, `worlds::zip::plan`), then room: the
  zip's unpacked size plus the world's size plus 1 GiB when the instance and the backups
  share a disk (the usual case), else each with 1 GiB on its own disk; before the room, a
  world grown past the limits a backup is made within is refused (its safety backup could
  never be restored; restoring as a new world needs none). Then a **safety
  world backup** of the world as it is ("Before restoring “<name>”", the restored name cut
  so the whole fits 200 characters; none when the world isn't there any more), refused
  when it fails or is cancelled. The zip is unpacked into
  `<instance>/.lodestone-restore-staging-<16>/minecraft/saves/<world>/` and swapped in
  with M5's journaled swap (`images/swap.rs`, `restore-journal.json`,
  `Staged::with_folders`): the world's folder is owned, what the safety backup zipped is
  what it replaces, every folder the zip lists (an empty `datapacks/` too) is the zip's,
  and everything else in the world is carried back (`session.lock`, links, names no zip
  holds, folders only on disk). Just before the swap, open worlds and a game session are
  checked again (a game this Launcher didn't start may open a world while a large zip
  unpacks). A restore that ends with the world unchanged (refused, cancelled while
  unpacking, or rolled back) deletes the safety backup it made; one whose swap couldn't be
  undone keeps it. A crash is finished or undone at the next start or by Play, as for
  snapshots. Only that world changes; nothing is recorded in the content history (worlds
  aren't content). Files written into the world meanwhile are kept aside
  (`restore-leftovers-<16>/`), which the dialog says.
- **As a new world** (`RestoreTo::NewWorld`, "Restore as a new world", mode `new`): the
  zip unpacked into `saves/.lodestone-restore-staging-<instance id>-<16>/` (one disk with
  `saves`, even a linked one; what a crash leaves the startup sweep deletes), then renamed
  beside the others as "<world> (restored <date>)" (its `level.dat` name too; a folder
  name free in `saves/` and the backups' folder), replacing nothing, no safety backup.
- **Deleting one** moves its zip to the Trash, then deletes its `.json`; when the Trash
  fails nothing changes.
- **Renaming a world** in the Launcher (the Worlds tab, `WorldBackups::rename_world`)
  moves its backups with it under the record lock (only this instance's: a deleted
  instance that had the same id keeps its own under the old name), the new folder a name free in
  both `saves/` and the backups' folder, the world put back if the backups can't move. A
  world renamed outside the Launcher leaves its backups under the old folder name, listed
  as no longer in the instance.
- **Checking a zip** (`worlds::zip::check`, also used by world imports): only the end
  records and central directory are read (never a scan of the file), and the whole zip is refused for more than 500,000 entries or 64 GiB
  stated, an encrypted entry, a name `mrpack::check_pack_path` or a canonical path refuses
  (absolute, `..`, `\`, `:`, device names, …), two names one file on a disk that ignores
  case (`paths::case_collisions`, `ſ` and `s` too), a file that is also a folder or is
  listed twice, and, for a backup, an entry outside its one top folder or no `level.dat`.
  Links, `__MACOSX` and `.DS_Store`, `session.lock` and `.lodestone-…` entries are
  skipped. Unpacking (`extract_into`) creates each file new, copies at most its stated
  size plus one byte (a larger one refuses the zip), asks Cancel before each entry and
  every 8 MiB of a large one, and
  removes what it unpacked on any failure. What it unpacks is made durable as one batch
  (`lodestone_image::store::flush`: each file and folder handed to the drive, then one
  flush of the drive's cache), not with a cache flush per file.
- **The window.** Settings › Snapshots is one section (one entry in the list beside the
  page) with two groups, each under its own heading: "World backups" ("Back up a world…";
  each world's backups in a card under the world's name, a world no longer in the instance
  said so; per row Restore…, Restore as a new world, Show in Finder, Delete) and
  "Snapshots of the whole instance" (as in M5, its lead the retention). One dialog makes
  (a pick of the worlds when there are several, a name), restores (in place after a
  confirmation naming the backup and saying the world as it is is saved first; as a new
  world at once) and deletes; while it works it can't be closed, and Cancel stops the
  work. The Worlds tab's "Back up" is a tab job with Cancel, as are rename and delete;
  all three are refused, with the reason, while another job writes the instance
  (Settings' world backup dialog, an install, a modpack update). A world's "Backups & snapshots" button (its number counts
  both sections) opens a panel with "World backups" (its zips, with the same buttons,
  which open the dialog) and "In snapshots" (the snapshots holding it); both list
  Restore… (in place, asking first) then Restore as a new world. The lists follow
  `WorldBackups::watch`. Names the Launcher gave ("World backup", "Before restoring
  “<name>”") are shown translated.
- **Control API and CLI.** `world_backups.list` (`instance`, `world`; or `deleted`),
  `world_backups.create` (`instance`, `world`, `name`), `world_backups.restore`
  (`instance`, `backup`: an id, a name, or `latest` with `world`; `mode` `new`, the
  default, or `replace` with confirm; destructive), `world_backups.delete` (`backup` as
  for restore, `world`; confirm). `world` is a world's folder or name, matched first
  against the backups' own folders and world names, so a world no longer in the instance
  is found by its backups; backups are looked up in the background.
  `worlds.restore_snapshot` takes the same `mode` `new` or `replace` (destructive); making and restoring are jobs of
  kind `world_backups` that `jobs.cancel` stops, and no snapshot or world backup job of an
  instance starts while another runs. Records carry the zip's `path`. `worlds.list` counts
  each world's `world_backups` and `snapshots`. The CLI: `world-backups list|create|restore|delete`
  (`restore --mode new|replace`, `delete --world`), `worlds restore-snapshot --mode
  new|replace`, `storage clean deleted-world-backups`.

### On the Host

The core keeps a server's world backups beside its snapshots (`core/src/backups/world/`),
and the GPUI Host's Backups page is now called **Snapshots**, with a **World backups**
section above the snapshots (see "WB1 decisions").

- **What is in the zip.** The server's level (`level-name`) as it is on disk, and on Paper
  and Spigot its split `<level>_nether` and `<level>_the_end` folders, each under its own
  folder name, so the zip is a world any server or tool can use. Files are checked against
  the server's disk layout (`DiskLayout::from_disk`): what images never record
  (`session.lock`, `.lodestone-*`) is skipped, and links, things that aren't plain files
  (fifos, sockets, devices), unreadable files and names a restore can't place (a vanilla
  `DIM-1` inside a split level, a name that isn't UTF-8, the later of two names that are
  one file on a case-insensitive disk) are left out and counted. The level must be one folder name and hold a `level.dat`, and the
  world must fit what a restore reads (below), so every backup made can be restored.
- **Where it lives.** `<LODESTONE_PATH>/world-backups/<uuid>/<id>.zip` with a sidecar
  `<id>.json` (name, kind, time, level, the zip's top folders, server name, game version,
  flavour, size, file and left-out counts, `live`, `quiesced`, and for a safety backup the
  label of the backup it was taken before restoring). Outside every server folder and the
  image store, and protected from the file API like `backups/`. The zip is written as
  `.<id>.zip.part`, flushed, renamed, the folder flushed, then the sidecar written
  atomically, so a listed backup is always whole. Listing drops a sidecar whose zip is gone
  and adopts a whole zip with no sidecar (a crash between the two). Deleting removes the
  zip first, then the sidecar. Deleting a server deletes its folder.
- **Making one** (manual only: no schedule, no pruning). It holds the server's one job lock,
  shared with snapshots, so a world backup, a snapshot and a restore never run at once. A
  running server is quiesced exactly as a live snapshot is (`save-off`, `save-all flush`,
  wait for the save line, then `save-on` however it ends, the same marker and 15-minute
  cap); a stopped one can't start meanwhile. The free space for the uncompressed world is
  reserved among the core's large writes first. Progress events and Cancel; a cancelled or
  failed backup leaves no file.
- **Restoring one.** Only into a stopped server. Checked before anything is written: the
  zip (below), the world's folders (none a link or a file), that the safety backup can
  hold everything the restore replaces, and that the disks have room for both. Then a
  safety world backup of the current world ("Before restoring “…”"), then the zip extracted
  into a staging folder in the server and swapped in by renames with the snapshots'
  journal (`images/<uuid>/restore-journal.json`) and startup recovery, so a crash never
  leaves half a world. A restore that stops before the swap, or whose swap is undone,
  deletes its safety backup again (the world is as it was). Only the level and its
  `_nether` and `_the_end` folders change. The
  zip's paths go through canonical world paths into the server's current level name and
  split, so a vanilla backup restores into Paper and back.
- **The zip is untrusted** even though the core wrote it: at most 1,000,000 entries and
  64 GB declared (in all, and per entry), only Stored and Deflated, no encrypted or
  symlink entries, local headers that match the central directory, no duplicate names,
  UTF-8 names without `\`, a leading `/`, empty, `.` or `..` parts, every part a valid
  canonical component (Windows names, NFC, invisible characters), top folders only the
  backup's level and its split folders, no two paths that are one on a case-insensitive
  disk, files written with `create_new` and never more bytes than declared, and the
  server's disk must have room for the declared total.
- **Routes** (`core/src/handlers/world_backups.rs`), with the permissions of the matching
  snapshot routes: `GET` and `POST /instance/{uuid}/world-backups` (ReadInstanceFile; and
  AccessSetting to make one), `PATCH` and `DELETE …/world-backups/{id}` (AccessSetting to
  rename, WriteInstanceFile to delete), `POST …/{id}/restore` (WriteInstanceFile, and
  WriteGlobalFile or the owner with code execution for protected or code files it
  changes, as a snapshot restore), `POST …/{id}/download` (ReadInstanceFile; a link to the
  stored zip itself) and `POST …/world-backups/tasks/{task_id}/cancel` (the permission
  that started the job). Progressions `WorldBackup`/`WorldRestore` and
  `WorldBackupCompleted`/`WorldRestoreCompleted`. The world zip download
  (`GET …/world/download`) and the snapshot zip download stay.

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
   decisions"); then server images in place of the client pack sync (done: "M4b
   decisions").
5. Launcher: snapshots become images (exports drop downloadable jars), `LDS2:` as an
   encoding; world snapshots on demand and before risky operations, after sessions
   optional; restore with linked content. No `.lodesnap` import and no conversion of the
   old snapshot store or world backup zips: they are removed. (Done: see "M5 decisions".
   Its full-image option was removed in WB2: every snapshot covers the whole folder.)
6. Moving images between computers (Host to Host, Launcher to Host, Host to Launcher,
   Launcher to Launcher). Done in two parts: first the core's export of a backup as a
   `.lodeimg` and its import of a Host's or a Launcher's `.lodeimg` as a new server, with
   the GPUI Host's UI for both (done: "M6a decisions"); then the Launcher's "Host this" and
   "Download this world", which call those routes, the Launcher opening a core's
   `.lodeimg`, and the structured end of a creation from a file (done: "M6b decisions").
   CurseForge downloads stay out of the core: the developer decided so (see "M6a
   decisions", Making it); a CurseForge file the `.lodeimg` doesn't carry is left out and
   the summary says so.

Nothing converts old data in any milestone.

### ML decisions

Magic localhost for every server on the Launcher's Lodestone Host page was built
alongside these milestones but is not part of instance images: `lodestone_image` and
this format are unchanged (nothing added, renamed or removed). Its decisions (ports saved
in `servers.json`, forwarders for servers on other hosts, a server on this computer at
its own port) are in `launcher/docs/friends.md`, "Magic localhost for every server".

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
  id), it records "Changed the snapshot excludes". It waits at most 10 s for the last stop's
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
  content gate from before the safety backup to the swap, and record "Restored snapshot “…”"
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
  local `.mrpack`). An instance made from a server's image names its server's base the same
  way (`modrinth_pack`, from `modpack.json`, whose `from_server` keeps the instance from
  offering a Modrinth update of its own: `ModpackRecord::modrinth_ids` gives the base,
  `on_modrinth` the update), so its content images say which pack it is built on. The client settings are the recommended memory
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
  them as changed by hand, and the search's end would restore files the update replaced.
  Since milestones 4 (second half) and 5 the hook is
  `cores::image_sync::apply::before_server_update`, the one place a server's update (from
  its image, or a `.mrpack` client pack for a core older than milestone 3) takes a snapshot
  first: `Images::before_risky_change_with(RiskyChange::ServerUpdate)`, which records this
  and takes the snapshot ("M4b decisions", "M5 decisions").
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
  server's update takes the snapshot "Before updating from <server>" (its content record
  "Before a server update") through `image_sync::apply::before_server_update` and is the
  operation "Updated from <server>".
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
  the Trash) deletes `images/<id>/` and asks for a collection when it holds no snapshot;
  one that does is retired, and putting the instance back from the Trash takes its
  snapshots back, with the content they link to (milestone 5: "M5 decisions", Retiring). A record, check or history that finds `content-state.json`
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
  delta the server would, since the player keeps the view it applied (the Launcher needs
  no delta: it compares the two views' placed files in full, "M4b decisions"). No route takes a
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
  records "Changed the snapshot excludes" once at its next check before a start. A view's
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
  offered the mark only on Fabric servers (the only ones with a client pack), though the
  API takes it for any mod loader: since M4, which makes the client view what players
  install, the GPUI Host offers it for every mod loader (Fabric, Quilt, Forge, NeoForge;
  never plugins; the dashboard still only on Fabric). What Modrinth says about a jar later (a Mods-tab
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

### M4b decisions

Decisions the second half of milestone 4 made where this document left the Launcher's
side of "Server to player" open. The code is `launcher/crates/launcher-core/src/cores/`
(`player_image.rs`, `image_sync/`) and the Servers page (`lodestone-launcher/src/servers/`).

- **One client for both routes** (`cores::player_image::ServerImage`), on whichever
  `CoreClient` reaches the server: `/launcher/instances/{uuid}/` with this device's key
  (or a claimed core's owner key) when the Launcher has one, else
  `/public/instances/{uuid}/` (the public list, a Connect invite's info port, a friend's
  tunnel through its forwarded port). A server a signed-in Launcher sees only through the
  public list (merged into the signed-in one, though the key can't view it:
  `ServerInstance::public_only`) is asked through `/public/…` too, its `.mrpack` fallback
  included, since `/launcher/…` would refuse the key. Requests go through the core's pinned certificate
  like every other, never follow redirects, and log only the path without its query. No
  deadline for a whole request: each wait has its own (`Limits`): 120 s for `…/image`'s
  headers and between two reads of its body, 300 s for an object's (a download pauses
  while the server's store is collected). A view or delta past `MAX_VIEW_BYTES` (128 MiB,
  under the spec's `MAX_MANIFEST_BYTES`: a view the Launcher installs has at most 100,000
  entries, under 100 MiB of JSON, and parsing a body of tiny records costs a few times its
  size) is refused (a `Content-Length` past it before reading). A server's own error
  message (an error body's first cause) is shown cut to 300 characters, through
  `display_text`. `X-Lodestone-Rules` is read first
  on every answer: digits only, a number past 32 bits counts as newer, anything else says
  nothing; newer than `RULES` is "update Lodestone" whatever the status. A view of an
  unknown schema, or rules the view check doesn't know, is the same.
- **Waiting and giving up.** Every `Retry-After` is clamped to between 1 s and 30 s (5 s
  when missing or odd; a 0 is waited 1 s, so a hostile server can't make the Launcher ask
  without pausing), and waits are counted, not timed, so a test's instant sleep ends them
  too. `…/image` asks again through `Conflict` until its waits add up to 10 minutes (the
  first record of a big server), then says the server is still preparing; `Internal` stops
  at once ("couldn't prepare its files, try again in a few minutes"; the error carries the
  server's own `Retry-After`, unclamped up to a day). An object: `TooManyRequests` is
  waited out for up to 10 minutes of waits; `Conflict` for up to 2 minutes;
  a body cut short, a stall, a body past the size its view gives, a wrong hash, a 5xx and a
  transport error each count as a failure, and the file is fetched again at most 3 times
  (`MAX_RETRIES`: the fourth failure gives up, the spec's "at most 3 times in a row per
  file", counted per file in all: the busy and preparing answers between failures don't
  reset it, so alternating answers can't keep a file going for ever; bytes an attempt
  counted in the progress are taken back when a 200 starts the file over); `Internal`
  answers are counted on their own, and the third in a row gives up
  (`MAX_INTERNAL`, the spec's "3 `Internal` answers in a row"; any other answer resets
  that count). A 404 is "the view
  changed" (asking again, by Play or Create, gets the new view: "The server's files
  changed while they downloaded. Try again."), a 400 is refused. At most 4 objects download
  at once. A large file (over 1 MiB) resumes with `Range: bytes=<have>-`; a 206 must say
  exactly that range, a 200 starts over, a 416 deletes and starts over. Every object's
  SHA-256 is checked whole (ring, through `lodestone_image::hex`).
- **What a view must be.** Besides `check_received` and the id the server named
  (`image_sync::received`), a view must be a `content` image without worlds, on no base,
  a `loader` base or a `modrinth_pack` base (a `curseforge_pack` base is refused with its
  own message), of at most 100,000 entries. The cheap checks (the count, the kind, the
  id) come before `check_received`, and a delta that would make more than 100,000 entries
  is refused before it is applied, so a huge hostile answer isn't worked out again first.
  Parse errors a UI shows go through `display_text`. A delta is applied only to the kept view its
  `from` names; one the instance doesn't keep (or a 204 naming a view it doesn't keep)
  asks once more with `have` empty, for the whole view. So does a delta that can't be
  read (a newer core's change of a kind this Launcher doesn't know, say: the whole view
  then says whether the server needs a newer Lodestone) or doesn't apply or check out;
  when the whole view fails too, its own error is the one shown.
- **What a server may place** (`image_sync::placed`). The client view's rules let through
  files a server must never write on a player's machine (`options.txt`, `servers.dat`,
  `optionsof.txt`, screenshots), so an entry's path must be in a content folder (`mods/`,
  `config/`, `defaultconfigs/`, `kubejs/`, `resourcepacks/`, `shaderpacks/`, the Launcher's
  content scope) or at a path the base pack's own client share places (not one the content
  scope never takes from a pack); anything else refuses the whole view. Placed paths that
  differ only in letter case, and a placed total past 64 GB (the import bound), refuse it
  too, and so does a `modrinth` entry whose URL isn't a file on Modrinth's CDN
  (`https://cdn.modrinth.com/data/<project>/versions/<version>/<file>`) of the project and
  version the entry names: never CurseForge's CDN (which would get around a CurseForge
  author's choice) or another host a pack may use, and never a project the entry only
  claims (`addon::superseded_by` trusts it). Writes go to a staging folder inside the instance, every parent checked against
  links (`ParentCheck`), then are renamed in.
- **Kept views.** An instance keeps the views it applied in `<instance>/server-views/<id>.json`
  (outside the game folder, so never content): the applied one and the 3 newest others.
  `have` is the applied view first, then the others newest first, at most 16. A kept view
  is checked as a received one when read; a damaged one is deleted. Unlinking an instance
  deletes them. The link (`lodestone_server.json`) gains `view`, the applied view's id;
  `pack_etag` is kept only for a core older than milestone 3. No old link or record is
  converted: an instance from the old client pack sync has no `view`, gets the whole view
  at its next Play and converges through the merge below against its `modpack.json`.
- **Placing the difference without a diff.** What a view places is worked out in full each
  time (the base pack's client share, less `removed`, the entries over it), and compared
  with what `modpack.json` says the last applied view placed and with the disk, through
  the modpack updater's three-way merge (`modpack::merge`, which modpack updates use
  unchanged). So the Launcher needs no delta file list, no old base pack and no
  `lodestone_image::diff`: a whole view after pruning, a 204 naming another kept view, a
  base update, a `removed` change and `base` entries following the base all come out of
  the same comparison. `modpack.json` records each placed file's SHA-1 (empty when the
  view gives none) and, for files from the view's entries, its SHA-256 (`PackFile::sha256`),
  which the merge compares by when set; it records what the view placed, never the disk's
  hashes, so a change the player kept is still theirs at the next update.
- **Reading only what changed.** A server's plan (`merge::Rules::trust_recorded`) takes a
  file as unchanged without reading it when `modpack.json` records the hash the view now
  places and its size on disk is the view's: a delta of one entry reads only that entry's
  file (and the files removed). Modpack updates still read every file. A file changed by
  hand to the same size isn't seen then; the check on a 204 below reads the mods whose
  size or time changed, and the mods it finds missing or changed are read by the plan
  that follows (`merge::Rules::untrusted`), the rest still taken by their record.
- **A plan is checked again just before it is applied.** The merge notes what each file
  it overwrites or removes must still be (`merge::Expect`: the last version's file as
  read or as its record said, nothing there for a file it adds, and, for a file it
  replaces whatever it holds (a server's mod the player changed, the player's own file at
  a path the view now uses), the bytes it held when planned, so a change made while the
  files download isn't overwritten unseen), and the updater checks
  those after copying the files aside and before changing any (`ModpackError::
  ChangedSincePlanned`, "<file> changed after the update was worked out, so nothing was
  changed. Press Play again."): the prompt waits for the player, and the game may run
  meanwhile and change a config. Modpack updates get the same check.
- **A 204 is checked against the mods on disk.** When the server answers that the applied
  view is current, every `mods/` file `modpack.json` says it placed must be on disk as
  placed (by SHA-256 when recorded, else SHA-1; a mod the player turned off counts), as
  the client pack sync checked on a 304, but the files the applied view can't give (its
  needed withheld files and CurseForge files that may not be downloaded, which the player
  keeps as they like). A link or folder the player put at a mod's path counts as placed:
  the merge keeps it as theirs, so planning again would only record, at every Play. A
  needed withheld file the base pack places anyway isn't checked on a 204 (the check
  doesn't read the base pack, and nothing records which kept files the base placed); the
  next view that changes anything puts it back if the player deleted it. When one is missing or changed (deleted, updated through the
  Content tab, damaged) the kept applied view is planned again, reading those mods
  rather than taking them by their record, so the merge puts the server's mod back and
  the update prompt says so. A 204 is
  the common Play, so this reads only the mods whose size or modification time differ
  from what the last check found (`server-views/mods-checked.json`, with the hash each
  was placed with; one modified in the last 2 s isn't remembered), as the image store's
  stat cache does for snapshots; a mod changed to the same size with its time put back
  isn't seen until its entry next changes. Nothing fills that file when an instance is
  made or updated, so the first 204 after a Create or an update reads the mods once (a
  second or so for a 300-mod pack), and later ones read only what changed.
- **An update that stopped part way.** Before placing a view's files, the update keeps the
  view and adds it to `server-views/placing` (one id a line, at most 8, the oldest
  dropped; harmless: the link still names the old view); recording a view in the link
  removes the file. When the next Play plans another view while `placing` names kept
  views that aren't the applied one, the files on disk that are those views' entries,
  byte for byte, count as placed by them (added to what `modpack.json` says), so the next
  view removes the ones it drops instead of keeping them as the player's. (Only the view's entries, as a config's record or an index file's: a base
  file a stopped base update placed that isn't an entry and the next view drops stays,
  as a pack's dropped config does.) A 204 naming the applied view while `placing` names
  another isn't taken: the applied view is planned again the same way, so the files the
  stopped update added go. An update that fails having
  changed nothing (it stopped before the files, a file changed since the plan, or it put
  back what it had changed) takes its view back out of `placing` (leaving the views an
  earlier stopped update named, whose files are still on disk, and removing the file when
  none was named) and records no image, so a player's own copy of one of that view's
  files isn't taken for the server's later; only one whose undo couldn't put everything
  back (`ModpackError::NotUndone`) keeps it, and records "Updated from <server> (stopped
  part way)".
- **Merge rules for a server's files.** `mods/` is authoritative (a server's mod the player
  changed is replaced, so they can join), as the client pack sync had it. Entries in a
  config folder (`config/`, `defaultconfigs/`, `kubejs/`) are placed as a pack's overrides
  are: never over the player's own file, and left on disk when a later view drops them;
  other entries as a pack's index files (removed when unchanged), but a `base` entry,
  which is placed as the pack places that file: one of the pack's overrides never goes
  over the player's own file (CraftTweaker's `scripts/`, say), as a modpack update keeps
  it. Unlike a modpack update, a server's dropped pack file goes when it is unchanged
  wherever it is (`merge::Rules::dropped_overrides_go`), not only in `mods/`,
  `resourcepacks/` and `shaderpacks/`: a script the view's `removed` names or the pack's
  next version drops is removed, but a config and a file the game writes itself
  (`options.txt`) stay. Anything but a plain file at a path the view places (a link to a
  shared config, a folder) is the player's and stays as it is, never written over (in
  `mods/` too: it is listed as a file the player changed). So is a file in a folder the
  player linked out of the game folder (a shared `resourcepacks/`, a folder above a
  placed path resolving outside the instance): nothing is written into it or removed from
  it (`merge::Disk::outside`; a modpack update too), so a view or base pack that adds a
  resource pack there doesn't fail every Play; but in `mods/`, which is authoritative,
  the update still refuses to write through such a folder ("a path leaves the instance
  folder"). The player's own mods,
  their edited configs and a pack config the server deleted stay. A mod an added pack has
  that the view now has too is removed (`addon::superseded_by`).
- **Files a view can't give stay on disk when the player has them.** A needed withheld
  file the base pack places anyway (the core keeps withheld paths out of `removed`) isn't
  one the player lacks: the pack's copy is placed, and the summary and the "has files you
  won't get" prompt don't list it (`Placed::missing` leaves it out). A path the new view
  lists as a needed withheld file (it kept changing, couldn't be read, is a link), or as a
  CurseForge file to download by hand, is never removed, and its line in `modpack.json`
  stays while the player has the file (the line of one they deleted goes, so the check
  on a 204 doesn't take it for a server's mod gone missing at every Play): the server
  still has that file, and removing the player's copy would leave them without it. When a later view gives it again it is updated as usual. A CurseForge file
  to get by hand that `modpack.json` records with the entry's SHA-256 isn't listed as
  missing: the player has it.
- **Renames of letter case.** A file the next view places under the same name in other
  letter case is, on a disk that doesn't tell case apart (macOS's and Windows's
  defaults), the same file: it isn't removed (removals run first, so it would be the only
  copy). On a disk that tells case apart the old name goes and the new one is written.
- **Where files come from.** Modrinth entries from Modrinth's CDN (the URL the entry names,
  through the Modrinth client's host check), checked by SHA-1 when given (else SHA-256 as
  they download), SHA-512 and SHA-256; `object` entries from the server; `base` entries are
  the pack's file at that path, whose SHA-1, SHA-512 and size must be the entry's when it
  gives them (a missing one is refused: the pack must have it), checked by SHA-256 too.
  The pack's file is its client share's copy, or, when that isn't the entry's (a pack with
  a `client-overrides/` copy at the path), its server share's (`overrides/` with
  `server-overrides/` over them, which is where the core took it from); only those paths
  are read again from the pack. Downloadable CurseForge entries come through the
  CurseForge client (one request per 500, `files_by_id`, for at most 2,000 entries of a
  view: the rest are listed by hand, so a hostile view can't spend the Launcher's
  CurseForge quota; the URL CurseForge gives, once `download_url` says its host is
  CurseForge's CDN, fetched through the client as a CurseForge pack's files are), checked
  by CurseForge's SHA-1 and the entry's SHA-256; the others (marked not downloadable, or
  with no download on CurseForge) are listed for the player to download by hand and not
  placed. CurseForge out of reach (offline, an error, no CurseForge in this build) when a
  view has downloadable CurseForge entries refuses the install with its own message
  ("CurseForge couldn't be reached"): a file it would give isn't one its author keeps
  from other apps. CurseForge entries never come from a real core yet, so that path is tested end
  to end against the CurseForge and Modrinth mocks only (`tests/server_images.rs`). The
  trust advisory tells only Modrinth's files apart, so a downloadable CurseForge file
  Modrinth doesn't list is shown with the server's own uploads, though it comes from
  CurseForge's CDN, checked by CurseForge's SHA-1 and the entry's SHA-256.
- **The base pack.** A `modrinth_pack` base is taken only from Modrinth: the version must be
  the base's project, Modrinth's listing of its `.mrpack` must have the base's SHA-512, and
  the download (from the CDN) is checked against it. Packs are cached in
  `<data>/cache/server-packs/<sha512>.mrpack`, hashed again when used (a damaged one is
  downloaded again), the newest 8 kept and any used in the last day (an install prompt
  that waits for the player holds the pack its install copies from). A version Modrinth no longer has refuses the
  install with a message saying so (M3's "`base` entries need the pack"). The pack's
  client share is what any Modrinth launcher installs (`modpack::read_pack`).
- **The game is fixed.** The view's game becomes a new instance's (Forge's loader version
  as a full Maven version, `<minecraft>-<version>`; Quilt and NeoForge as they are). For an
  existing instance a view of another Minecraft version or loader is refused, telling the
  player to make a new instance, as soon as the view is checked (before its base pack is
  downloaded or CurseForge asked, so a view that will be refused costs nothing), and
  for a vanilla instance too (it isn't a failure to ask, below); only the loader version follows, saved after the files
  and before the link names the view. The `.mrpack` fallback follows the same rule (the old
  sync changed the Minecraft version in place). A view whose loader version differs from
  the instance's is placed again even when the server answers 204 (an update whose loader
  version wasn't saved). The game and loader themselves aren't installed by Create or
  an update: they install at the next launch, as for any instance.
- **Not applied from a server:** launch settings, Java, and the view's client settings
  (recommended memory, resource pack order, icon, background): a new instance starts with
  the Launcher's defaults.
- **Showing what will be installed.** "Create matching instance" always shows what the new
  instance gets (base pack, Minecraft and loader, mods the server added, files only the
  server has, files it couldn't share and CurseForge files to download by hand) before
  anything is installed (the mods the server added are its `mods/` entries that aren't the
  base pack's own file at that path: a `base` entry, or a Modrinth file with the SHA-1 or
  SHA-512 of the pack's), unless the view places nothing (a vanilla server) and lacks
  nothing (no needed withheld file, no CurseForge file to get by hand): from a core
  trusted for mods with Install and Cancel, otherwise inside the trust advisory. Nothing
  to install (a vanilla, Paper or Spigot server's view; an update that only removes files
  or changes the loader version) needs no advisory, whatever the core's trust: no file in
  it can run code. While it asks the server and gets the base pack, the server's row
  shows the pack's download (bytes of bytes), with a Cancel (the server may prepare for
  minutes, and a pack may be large); the row keeps its Cancel while the instance is made
  or updated (the server's own files come as fast as it sends them, and a hostile one may
  send a byte now and then for ever), until the files start to change. Every wait for the
  server (its answer's headers, each read of a body, a `Retry-After`) races the Cancel
  (`CancelToken::cancelled`), so it ends at once rather than when the wait's own limit
  runs out. Play's update prompt lists the changes, a new loader version and the files
  the server couldn't share. Paths a server names are shown through `display_text` in
  every list (the summary, the advisory, the update prompt), so a name can't span lines,
  and so are the base pack's name and version (from its index, which the server chose),
  in the summary, the progress and `modpack.json`. The summary and the update prompt say
  how much will be downloaded (`Summary::download_bytes`: from the server, Modrinth and
  CurseForge, not the files copied out of the base pack), and before anything is staged
  the data folder's disk must have room for every file the install writes
  (`apply::check_free_space`, refused with "There isn't enough free space…"; a view may
  place up to 64 GB).
- **Files a player lacks are said before Play, once per view.** A view that changes
  nothing on disk but lacks files the applied view didn't (`Ready::newly_lacking`: needed
  withheld files and CurseForge files to get by hand, compared with the applied view's,
  whose CurseForge files count as lacking when `modpack.json` doesn't list them) opens
  "<server> has files you won't get" with Play (which records the view: no advisory,
  nothing installed) and Cancel. Once that view is applied, Play joins without asking
  again until another view lacks something new. Without a kept applied view, every lacking
  file counts as new. The advisory takes a server's own files (no SHA-1 to ask Modrinth by)
  for uploaded ones, and a Modrinth file without a SHA-1 for Modrinth's (its CDN and
  SHA-512). Missing files are `lodestone_image::view::needed_withheld` (the shared crate
  gained it: the rule "Files a view can't give" states), shown through `display_text`.
- **Friend trust** for a server from the friend list is tied to the view's id
  (`FriendServer::trusted_pack` holds it; a client pack's ETag for an older core).
- **The fallback.** A 404 on `…/image` means a core older than milestone 3: creating an
  instance imports its `.mrpack` when its listing says `has_client_pack: true`, else makes
  a plain instance; Play keeps an instance from it in step through the `.mrpack` when its
  link has a `pack_etag` or the listing says it has a pack, else joins (`cores::sync`,
  kept only for this). An instance whose link names a view it applied from that server
  never falls back: a 404 then means the server stopped sharing its files
  (`SyncError::StoppedSharing`), not an older core, so a core that served views can't
  move its players back to the `.mrpack`, whose overrides may land where a view can't. A server listed publicly without the client-pack scope
  (`has_client_pack` missing) shares no files: a plain instance, as before. An instance
  with nothing of a server's on disk (vanilla, and no `modpack.json` or one that lists no
  files, as a vanilla, Paper or Spigot server's view leaves, whatever view the link
  names) joins when asking the server fails for any reason but "update Lodestone" or the
  player's Cancel, as Play joined those servers before images: it has nothing to install
  to lack. That includes a 404 when its link names a view (the server stopped sharing,
  say its owner turned the client-pack scope off while the listing stays): it has none
  of the server's files to keep in step, so it isn't stopped with "stopped sharing"; a
  404 to one whose link names no view is the older core's, which the `.mrpack` fallback
  above answers. A server that now runs another Minecraft version or loader isn't a failure to
  ask: the player would lack that game, so "The server now runs … Make a new instance" is
  shown for a vanilla instance too. For the same reason "Create matching instance" for a vanilla, Paper or Spigot
  server makes a plain instance when asking fails that way.
- **Records and crash safety.** A new instance is staged whole beside the instances (files,
  `content.json`, `modpack.json`, the link with its view, the kept view) and renamed into
  place, then recorded "Created from <server>"; nothing is left when it fails. An update
  runs through `image_sync::apply::ServerUpdate`, which the app and the tests both use, so
  its order lives in one place: `start` refuses while the instance's files are held (the
  caller says why: the game runs, another job writes its files or deletes it, a "Find it
  for me" search is saved), and the Servers page refuses an install while the server's
  row has another job under way (a row's job ends only its own entry), and refuses
  unlinking the instance meanwhile, and refuses an answered prompt when the instance is no
  longer linked to that server (unlinked while the prompt waited); the Content tab refuses a change while another job
  holds the instance's files (an update's undo would put back the `content.json` from
  before it); `place` deletes
  staging folders older updates left (over a day old), keeps the view as being placed
  (above), checks the free space, takes the snapshot before a server update through
  `before_server_update` (the one call site: `Images::before_risky_change_with(
  RiskyChange::ServerUpdate)`, "Before updating from <server>", whose content image,
  "Before a server update", holds every content file the update replaces; its reading
  shows as the update's progress and Cancel stops it, changing nothing). When the
  snapshot fails, nothing changes and the update stops (`SyncError::NoSnapshot`), as a
  modpack update does: the update prompt comes back saying why ("No snapshot could be
  taken first…") with "Update without a snapshot", which skips the advisory (answered
  already) and updates after only recording the content (`ServerUpdate::without_snapshot`;
  a failed record is logged and the update goes on, keeping its rollback copy); the
  `.mrpack` fallback's prompt does the same, keeping its downloaded pack meanwhile. The
  control API (`servers play`, `friends join`) fails with the reason (`failed`) instead of
  going on without one. Then it places the files as the operation "Updated
  from <server>" while Play is held (staged, a rollback copy, the plan checked again,
  swapped in; then `content.json`, `modpack.json`), the operation succeeding part way
  only when placing failed with files changed part way (it is dropped when nothing
  changed); once placed, the rollback copy (`backups/modpack-update/<time>/`, a folder
  of its own even for two updates within a second) is deleted when the record before the
  update was made and holds every file the update replaced or removed: each is in that
  record's scope (`ContentScope::holds_file`: the content folders, or a file the
  instance's packs wrote, as `modpack.json` named them before the update). A player's own
  file outside it (a script at a path the server's pack now places, which the server's
  own copy replaces) is in no image, so the rollback copy stays. The modpack updater
  deletes its rollback copy itself when an update that failed put everything back (the
  instance is as it was), a half-made copy when copying failed, and a whole one when the
  update stops (Cancel, a world backup that failed) after copying and before changing
  anything; an undo puts the instance's records (`content.json`, `modpack.json`, the
  added packs') back whole (written aside and renamed), so a full disk can't leave a
  `modpack.json` cut short; a removal through a folder that links out of the game folder
  (a shared `resourcepacks/`) is skipped, the file left alone (the merge already leaves it
  out, below; the updater skips it too, for a plan made before the link); `finish`, on
  the window's thread, saves the loader version and lets the operation succeed; only when
  the save worked, `AppliedUpdate::record`, off the window's thread (it writes the whole
  view), keeps the view and names it in the link. A view that changes nothing on disk
  goes through the same `ServerUpdate` (Play hands it to the window's thread, which
  checks the files are free) and is only recorded (`modpack.json` when it names other
  files, as a change of which files are content, then the link). A crash after
  `modpack.json` was saved and before the link leaves the old view named, and the next
  Play finds the files in place (no changes) and only records the view; a crash part way
  through placing leaves `content.json` and `modpack.json` as they were, so the next Play
  plans again against them (with the stopped update's files counted as the server's,
  above) and asks before placing the rest; its staging folder and rollback copy stay
  until a later update deletes the staging folder. Staged files aren't synced before
  they are renamed in (as modpack updates).
- **The control API and CLI.** The advisory's data gains `summary` (base, game, mods,
  files only the server has, `missing` with each path and reason, CurseForge files,
  `download_bytes`); the update prompt's gains `missing`, `curseforge`, `loader`,
  `download_bytes`, `only_lacking` (the prompt
  for a view that only lacks files, still `mods_changed`; either answer plays) and
  `snapshot_failed`. A trusted
  core's install prompt is answered Install, and `servers.create_instance` answers with
  the `summary` the instance got, so a caller can tell the user what is missing. A server
  that needs a newer Lodestone fails with the new code `update_required` (exit 1), which
  the CLI's skill says not to retry.
- **Errors in the player's language.** Every way installing a server's files can fail has
  a translated message (a base file the pack lacks, a cancelled install, an older core, a
  server that stopped sharing its files, too little free space, an instance unlinked
  while its prompt waited, a file changed since the update was worked out, and a local
  failure as "Couldn't install the server's files: <detail>", the detail in English).
- **The Host** offers the "For players" mark for every mod loader (see "M3 decisions").
- **Tests against the core's own answers.** The Launcher's tests run against a fake core
  whose answers `lodestone_image`'s `view` and `diff` compute, with scripted faults for
  every status above, and replay the real core's answers to a player (the 503 `Conflict`
  of the image and of an object before the first content image, the whole view, a 204, a
  delta, an object, and a 400 with `X-Lodestone-Rules`), so the error shapes the client
  depends on can't drift, which the core's test `records_the_answers_a_player_gets`
  writes to `launcher/crates/launcher-core/tests/fixtures/player_image/` when
  `LODESTONE_RECORD_PLAYER_FIXTURES` names that folder: the two workspaces can't link each
  other, so they share the bytes.
- **Open: "Not for players" on a pack's own file** still reaches players through the base's
  client share (M3 decisions); the fix needs a change to `Image::validate`, not made here.

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
  Together with its link it covers the whole game folder (since WB2 there is no option to
  ask for more: "Include mods and configs" was removed, "WB2 decisions"). A snapshot whose
  content couldn't be brought up to date takes a full image of the whole game folder
  instead, with its base and the files' Modrinth and CurseForge sources (recording failed, the store is suspended for
  content, a "Find it for me" search has mods renamed), so a snapshot always holds or
  links what was on disk. The image's worlds get `level_name`, `data_version`,
  `game_version`, `last_played`, `size` (from its entries) and `quiesced: true` (the game
  never runs during a snapshot: they are refused while it does, and one a session begins
  during, Play pressed again while the snapshot after the last session reads a big world,
  stops at the next file and records nothing, `ImageError::Running`). "While it does"
  covers a game this Launcher didn't start too, which its sessions can't know of (the
  Launcher restarted while the game ran, with Settings › After launch set to quit or
  after a crash): a snapshot, and an in-place restore, is refused while any world of the
  instance has its `session.lock` locked by another
  process (`worlds::open_in_game`: Minecraft 1.16 and later hold that lock while a world is
  open, Java taking an `fcntl` lock on Unix, which `F_GETLK` reports; on Windows the file
  can't be opened without sharing while the game has it open). Older versions lock
  nothing, which stays a known limit. A restore of one world is refused while another
  world is open too: its safety snapshot reads every world. It uses the `data` stat
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
  Kinds: `manual` (the user, the CLI), `before_change`,
  `after_play`, `before_restore`, `imported`. A name is the user's, cut to 200 characters,
  or the label of an automatic one; labels are plain English data like content labels:
  "Before updating <pack> to <version>", "Before updating from <server>", "Before
  updating <title>" or "Before updating N mods", "Before changing <loader> to
  <version>", "After playing", "Before going back to “<name>”",
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
  taken (and while that game still runs, snapshots and restores are refused by its
  worlds' `session.lock`, see the first bullet).
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
  adds "No snapshot was taken first, so take one in Settings › Snapshots before you play."
  with the reason, and the API's reply `snapshot_failed`. The update isn't paused to ask:
  the replaced jars are in the content history, and only the next launch, which the
  message warns about, can change the worlds. Modpack
  updates call it in place of `record_before_change`, and no longer zip worlds
  (`update::apply` lost `backup_worlds`; its rollback copy of changed files stays).
  **Server updates** call it through `cores::image_sync::apply::before_server_update`
  (milestone 4's second half, "M4b decisions"), for an update from a server's image and
  from a `.mrpack` client pack alike, and stop when it fails, as a modpack update does,
  offering to update without one. A change of the instance's game saved from the Settings tab,
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
  stat caches and the content images no snapshot links to are deleted. A restore journal
  stays: if the instance comes back, it says what its unfinished restore set aside, and
  Play's gate and startup's recovery look for it there too (see "The swap"). The
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
  another `created` time and never does. Until it is taken back, the folder of an
  instance that is back (put back while the Launcher runs, nothing has looked yet:
  `retired::returns_to`, the same test) is still the instance's: Storage's clean-up takes
  back each live instance's snapshots before it deletes anything and never deletes such
  a folder, the deleted instances' list takes them back first and leaves them out, and
  Storage's counts count them as the live instance's, so a pinned or manual snapshot of
  an instance that is back is never deleted as a deleted instance's. Restoring a deleted
  instance's snapshot into a new instance from the list isn't done.
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
  `restore_world_copy` (the Worlds tab's "Restore as a new world" on a snapshot: the world placed beside the
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
  snapshot's without a word. These refusals are checked before anything is staged
  (`Staged::new` run once as a check, the staging folder not there yet, with a carried
  path the restore also places counted as a clash) and again after staging. "As a new
  instance" given a name that is blank once control and invisible characters are gone
  names the instance "<instance> (<snapshot>)" (the snapshot's name without them), so the
  name never refuses what was staged. The window and the control API refuse an in-place restore
  (the Snapshots dialog, `snapshots.restore`, the Worlds tab's "Restore…" and
  "Restore as a new world" on a snapshot, `worlds.restore_snapshot`) while another job writes the instance's files
  (`Instances::files_or_pack_busy`: a tab's job holding them busy, an install, or a
  modpack update or add-on pack being applied, which mark nothing busy); and applying a
  modpack update or an add-on pack is refused while a tab's job (a restore among them)
  holds them busy. "As a new instance" only reads the store, so it runs while the game
  does. After the swap, a content
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
  before anything can be played, and only renames: a work folder it is done with (a
  staged copy, or the old folder after a finished swap) is renamed
  `<instance>/.lodestone-restore-trash-<16>/` and deleted in the background
  (`Images::empty_restore_trash`, from the startup task; after Play's retry, for that
  instance; and at the start of every restore), since a staged copy can be gigabytes. An
  instance put back after it was deleted, whose snapshots (and an unfinished restore's
  journal kept with them, see "Retiring") are still retired, takes them back first
  (`readopt_before_journal_check`), before any journal is looked for or a work folder is
  cleared. Then a journal is undone (or, when done, only cleaned up:
  the journal lists what the restore replaced, `replaced`, and anything else in the old
  folder, written meanwhile into an owned folder, is kept aside as below), and a journal
  whose instance folder isn't there stays, logged. `swap::recover_one` refuses outright,
  keeping the journal, when the instance folder can't be reached (a drive unplugged under
  a link): a missing old folder there would read as "nothing to undo". Play's retry and a
  restore's own recovery also refuse while the instance has no `instance.json`, so Play
  keeps refusing until the folder is back. Work folders no journal names
  (`swap::remove_leftovers`) are never simply deleted when they may hold the instance's
  files: a staging folder goes (a crash while staging leaves only copies from the store)
  except what a rollback set aside in it (`.rollback-<n>/`), and an old folder, which only
  outlives its journal when its clean-up failed or the journal was lost (a retired
  instance put back after the Launcher started), is renamed
  `<instance>/restore-leftovers-<16>/`. **One that can't be undone keeps the instance from being played**,
  like the core's locked server: while its journal is there (`Images::restore_unfinished`,
  a rollback that failed at startup or during the restore; also in the retired snapshots
  of an instance put back after it was deleted that nothing has taken back yet), Play
  first tries again (taking those snapshots back first)
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
  and the loader versions are compared. With "Only the worlds" chosen, the dialog says
  "These worlds last ran with other mods." when something under `mods/` or `coremods/`
  or the loader version differs, else "These worlds last ran with other files." (configs
  a mod rewrites at startup, now that every snapshot takes in what the game wrote), and
  a toggle that switches to the worlds and their mods, then, introduced by "Putting their
  mods back too:" (so they don't read as what the worlds-only restore does), a line each
  with up to five names: the files it puts back, removes and changes. With "The
  worlds and their mods" or "Everything" chosen, the same lines say what the restore will
  change, with the loader version it goes back to.
  The Worlds tab's "Restore…" on a snapshot says that only the world goes back and points to the
  Snapshots list for its mods. Known limit: the preview compares with the current content
  image without the quick check, so a mod dropped in by hand since the last record isn't
  in the list (the restore itself records it first, and replaces it). Known limit: a full
  image's content files are picked with the instance's current content scope, which
  counts today's `modpack.json` pack files outside the content folders, not the ones the
  snapshot's pack listed (an image's base names the pack, not its files); a pack file
  outside the content folders that only the snapshot's pack version listed (a
  `scripts/x.zs` a later version dropped) is therefore not put back by "The worlds and
  their mods" nor named in the preview; "Everything" puts it back.
- **Exports** (`.lodeimg`, `export::EXTENSION`): `container::export` of the snapshot's image
  (labelled with its name, no parent) and its content image, after `private_patterns`
  (`share::PLAYER_PRIVATE` and the JVM's crash files, `CRASH_FILES`, since milestone 6's
  second part)
  (`command_history.txt`, `hotbar.nbt`, `ias.json` at any depth: In-Game Account Switcher,
  `essential/microsoft_accounts.json` and `essential/mojang_accounts.json`: Essential; at
  the top of the game folder the official launcher's account files, in any case, as
  `import::copy` names them for its imports (`launcher_profiles.json`, older versions of
  which held tokens, `launcher_accounts*.json` and `launcher_msa_credentials*.bin`: a
  player who pasted a `.minecraft` folder into an instance has them); and at the top of
  the game folder the JVM's `hs_err_pid*`, `replay_pid*`, `java_pid*.hprof`,
  `*.mdmp`, `core` and `core.*`, and every top-level name `share::is_jvm_crash_name`
  matches in any case, exactly what snapshots leave out, for images that hold them however they were
  made, such as an imported one) are removed
  from both and added to their `excludes`, both re-sealed. Best effort: an unlisted
  account switcher's file still travels; the dialog lists what was left out, and the
  CurseForge files whose authors don't allow downloads elsewhere
  (`ExportReport::not_downloadable`), which whoever imports the file won't get. Written to
  `<data>/cache/snapshot-exports/<record id>/` for dragging out or Save as, or where the
  CLI says; leaving the dialog stops writing it. Save as moves the file out of the cache
  (a copy and delete to another disk), and the folder is cleared when the Launcher starts
  (renamed away at once, deleted in the background), so an export doesn't take its size
  twice; Storage's cache clean-up removes it too. The control API's reply names
  `not_downloadable` as the dialog does.
- **Imports** make a new instance: `container::open` (refused when its image isn't the one
  the preview the user approved showed: `expect_image`), then entries with Lodestone's
  names or paths the client can't place are refused, then the caller's free-space check
  for everything (unpacked objects, the files, every possible download, 1 GiB; the same
  figure as `ImportPreview::needed_bytes`, which the dialog shows beside the free space,
  saying 1 GB of it is to spare, refusing to start when it is short; a short disk is
  `ImageError::NoRoom`, which the window words itself), then `import_objects`.
  **Every Modrinth and CurseForge entry is downloaded from its source in that import**,
  even when the store has its bytes: the file's SHA-256 and SHA-512 come from the same
  untrusted file as its sources, so only a download says that a project's file has those
  bytes (a sender could otherwise give their own jar Sodium's project with their jar's
  hashes, and carry the bytes under another path). A Modrinth entry is a download only
  with its SHA-512 and when its URL is exactly
  `https://cdn.modrinth.com/data/<project>/versions/<version>/<file>` for its own project
  and version, written as the URL parser writes it (`Url::parse(url).as_str() == url`, so
  no `..`, `%2e%2e` or backslash that parsing would resolve to another project's path),
  with base62 ids, no query, and no encoded `.`, `/` or `\` in the file name (every export
  the Launcher writes is made that way), so a file of another project on Modrinth's CDN
  never passes for the one named. `Modrinth::cdn_url` also refuses a URL with dot segments
  or backslashes, for every caller. It is fetched through that check, SHA-256 and SHA-512
  checked, the SHA-512 read in pieces; the check and the store's write run on a thread of
  their own, so the other downloads go on meanwhile. A
  CurseForge entry is a download when its author allows it and there is a key, through
  the API's download URL for that project and file, looked up in batches of 500, each file
  id once. Each distinct file and claimed source is downloaded once, eight at a time; a
  file naming more than 20,000 downloads is refused when it is opened, by the preview and
  by the import before anything is unpacked (`MAX_DOWNLOADS`), so a file listing a million
  references asks nothing of Modrinth or CurseForge. Once eight downloads in a row got no
  answer at all (`DownloadError::Failed { answered: false }`: no network, a blocked host,
  each after every retry), no new download starts and the rest count as not downloaded,
  so an import offline ends in moments rather than after hours of retries (`Breaker`); a
  failure that stops the import stops new downloads too. **A download that fails doesn't stop
  the import** (a version its author deleted, a CDN error after the downloader's tries,
  bytes that don't match the SHA-512 or SHA-256): it is logged, and the entry is one that
  wasn't downloaded, as is one without a URL (a failed CurseForge lookup). Only Cancel, or
  a failure of the store or the disk, stops it. An entry that
  wasn't downloaded is placed when the file carries its bytes (since milestone 6's second
  part, never from the store's other files by hash: "M6b decisions") **as a plain file**: `content.json` learns no source for it,
  and the imported snapshot kept of the file records its source as `object` too
  (`without_unconfirmed_sources`, both images re-sealed), so a later restore can't record
  it as a project's file either; the rest are left out and listed, and the window says
  which in a notice after the import. In the kept snapshot those are withheld, reason
  `not_downloaded` (`export::NOT_DOWNLOADED`), not `object` entries the store can't back
  (which couldn't be saved): a restore of it leaves those paths as they are. The sender's
  content image the kept snapshot links to isn't a version of this instance's content:
  the content history leaves out content images only imported snapshots link to, and
  one the instance already has under the same id (the placed content exactly the
  sender's) isn't saved over. The preview's `mods` are every file under `mods/`
  at any depth (Quilt loads subfolders, old Forge `mods/<version>/`) and in `coremods/`,
  and `carried_mods` the mod files, in the image and in its content link each (a data
  image's entry hides its link's at the same path, but a later restore of the content can
  place the link's), whose bytes the file carries, Modrinth and CurseForge entries
  included: a download of one that fails places the carried bytes, as a plain file (an
  export the Launcher writes never carries those); the dialog and the control API's
  confirm name them as coming from whoever made the file. The placed files go into a new staging instance;
  `instance.json` gets the game, the recommended memory and default launch settings
  (images carry none), and a name: the one given, else the image's label without control
  and invisible format characters (direction overrides, zero-width ones), cut to 64
  characters; `content.json` the confirmed sources. It is recorded "Made from a
  snapshot", and the imported image (with its content) is kept as an `imported` snapshot.
  Collections stay held off (and the write counted) until that snapshot's manifests are
  saved: until then no manifest names the objects only it uses (its worlds), and a long
  import may have unpacked them longer ago than a collection's grace.
  Every path is also refused whose top-level folder is a content folder or `saves`
  spelled with other than ASCII letters (`modſ/`, a long s, is `mods/` on APFS, and a
  loader would load from it, while a comparison of letters misses it); and wherever a
  folder is recognised (the preview's mods, the content scope, a restore's owned paths) it
  is compared as a case-insensitive disk folds (`lodestone_image::paths::same_name`,
  `in_names`), not only ASCII letter case. The Minecraft and loader versions must be
  plausible version ids (letters, digits and ` .-_+`, at most 64 bytes): the dialog
  shows them, through `display_text`, and the instance keeps them. The dialogs list at most eight names of each kind ("…" for more): a
  file from someone else can name a million.
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
  world from one puts back the old folder name. Renaming moves no snapshot data; since WB2
  it moves the world's world backups with it (see "World backups", In the Launcher).
- **The UI.** Settings › Snapshots: "Take snapshot…", the after-play toggle (following the
  global default, which its hint says, until changed; then the instance's own, with "Use
  the default"; a toggle whose save fails is a notice), a row per snapshot (name, a badge:
  Automatic, After playing, Imported, Pinned, none for a safety snapshot, whose name says
  why; time, worlds by their own names, with their folders when two share a name, sizes)
  with Restore…, Export…, Pin or Unpin and Delete (a pin or delete that fails is a
  notice), a row for
  images that can't be read with Delete (asking a second time: they go for good), and the
  retention line. A row's details leave out "0 bytes new" (since WB2 they no longer say
  "With mods" for a full image: every snapshot covers the whole folder). The list follows the store: `Images::watch_snapshots` sends the instance's id
  after every change to its list (taken, pinned, deleted, pruned by retention, imported,
  taken back), from any thread, and `Instances` reads that instance's list again when it
  has one and counts the change (`snapshot_revision`), so the Worlds tab reads its backups
  again too: the snapshots the Launcher takes on its own (after playing, before an update
  or a loader change) appear without reopening anything. Settings › Instance defaults'
  toggle has its own hint ("for instances that don't set it themselves"). One dialog takes, restores (Only the worlds with world checkboxes,
  The worlds and their mods, Everything, As a new instance), exports (drag out or Save
  as…), deletes and imports (game, worlds, mods with the trust warning, downloads, files
  that can't be downloaded, disk space and what is free, name: the file's name, which
  exports make "<instance> - <snapshot>"). A restore opens on a choice it offers (as a new
  instance when the snapshot has no worlds and can't put content back); as a new instance
  it is named "<instance> (<snapshot>)"; work that reports no size shows no bar. A
  world's row shows its folder beside its size when its own name differs ("New World" is
  the game's default, so several often share it). "Only the worlds" names a loader
  version the mods choice would go back to among what it would change. An in-place
  restore that finishes stays open saying "Restored “<name>”." with Close. While taking,
  restoring or importing, the dialog can't be closed (no ×, Escape or click outside), but
  Cancel stops the work at its next check ("Cancelling…"; a restore's swap, once started,
  finishes) and the dialog says how it ended; deleting is short and can't be stopped,
  and an export stops when the dialog goes. The restore dialog is wider (680 px), so its
  four choices fit on one line. Since WB2 the
  Worlds tab's "Back up" makes a world backup, a zip of that world ("World backups"); the
  panel its "Backups & snapshots" button opens has an "In snapshots" section that lists the snapshots holding
  the world, each with the world's own size there and "with N other worlds", with
  "Restore…" (a world-only restore, in place) and "Restore as a new world" (both stop on
  Cancel while staging); a snapshot change re-reads only those, not the worlds. `.lodeimg` files open
  the import dialog when dropped or picked with Import…; a `.lodesnap` is refused as no
  modpack, whose message says snapshots shared from Lodestone end in `.lodeimg`.
- **Control API and CLI.** `snapshots.list` (`deleted`), `snapshots.create` (`name`),
  `snapshots.restore` (`scope` worlds, worlds_and_mods, everything or new;
  `worlds`, `name`, `preview`; confirm in place), `snapshots.export`, `snapshots.import`
  (`preview`, `name`; confirm with mods), `snapshots.delete`, `snapshots.pin`,
  `snapshots.policy`; `worlds.snapshots` and `worlds.restore_snapshot` (`snapshot`,
  `mode`; named `worlds.backups` and `worlds.restore` before WB2, when `worlds.backup` took
  a snapshot too) work on snapshots. Every snapshot job (`snapshots.create`, `restore`,
  `export`, `import`, `worlds.restore_snapshot`) can be cancelled with `jobs.cancel`, as in
  the window. `snapshots.import` reads the file in the background (its manifests come
  from someone else and can be large), and its confirm names at most eight mods of each
  kind, with "…" for more, and the full counts. Records carry `world_details`; `snapshots.export`'s reply
  has `not_downloadable`; a restore whose `kept_aside` is set also shows the window's
  notice. The handlers that name a snapshot (`snapshots.restore`, `export`, `delete`,
  `pin`, `worlds.restore_snapshot`) read the snapshots, and `snapshots.restore` its preview, in the
  background: reading them can wait up to 3 s for the instance's record lock, and the
  preview checks every object of the snapshot. `snapshots.restore` with scope new and no
  name names the instance after the snapshot without its invisible characters, as the
  import does. `snapshots.revert`, `worlds.delete_backup` and the modpack
  update's `backup_worlds` are gone; the CLI follows (`snapshots restore`, `pin`, `unpin`,
  `policy`; no `worlds delete-backup`, no `--no-backup-worlds`).
- **Not in M5:** a content history view (the CLI's `content history` stays the only one),
  restoring a deleted instance's snapshot into a new instance (they come back with the
  instance when it is put back from the Trash), restoring from images moved between computers
  other than `.lodeimg` imports (milestone 6). **The shared crate changed only additively**:
  `paths::same_name` and `paths::in_names` (crate-private before) are public, so the
  Launcher recognises a folder however an untrusted path spells it. Restores and imports
  report progress per download, not per byte placed. In launcher-core,
  `DownloadError::Failed` gained `answered` (whether a server answered the last try),
  which imports' `Breaker` reads.

### M6a decisions

Decisions the first part of milestone 6 made where this document left moving a server's
images open. The code is `core/src/images/share/` (`export.rs`, `uploads.rs`,
`inspect.rs`, `import.rs`), `core/src/handlers/backups.rs` (the export route) and
`core/src/handlers/instance_images.rs`; the Host's client is
`launcher-core/src/cores/admin/images.rs`, its UI `lodestone-host/src/pages/backups.rs`
(Export…) and `pages/create_instance/image.rs` ("From a file"). M5's Launcher export and
import are the model.

- **Routes.** `POST /instance/{uuid}/backups/{id}/export` (ReadInstanceFile, like the zip
  download; body `{ "include_plugin_data": false }`, answer `ExportedBackup`: a key for
  `GET /file/{key}`, the file's name `<server> - <backup id>.lodeimg` and size, and what it
  left out, blanked and can't carry, and `secrets_withheld`, the files it left out because
  they hold a password or token, each a `NameList`: at most 64 names with their count).
  `POST /instance/image/upload` (CreateInstance; multipart field `file`, a `.lodeimg`; answer
  `ImageUpload`: an `upload_id` and the file's `ImageSummary`).
  `DELETE /instance/image/upload/{upload_id}` (CreateInstance, the uploader only).
  `DELETE /file/{key}` (the user who made the link only): lets go of a download link and
  deletes the file made for it (a backup's zip or export) at once instead of an hour later;
  the Host calls it once an export's download ends, however it ends.
  `POST /instance/create_from_image` (CreateInstance, and WriteGlobalFile when the file
  carries protected files; `CreateFromImageRequest`: `upload_id`, `image` (the id the
  summary showed: a file that changed since is 409), `name`, `port`, `accept_eula`,
  `world`, `keep_operators`, optional `flavour` (one of the summary's `server_choices`),
  `description`, `min_ram`, `max_ram`; answer the new
  server's uuid, which comes as an `InstanceCreation` progression whose end message lists
  what the server didn't get). ts-rs bindings are in `dashboard/src/bindings`; the Tauri
  dashboard has no UI for them.
- **The export is a derived image.** It runs while the store's collections are held off
  (the hold lives in the blocking work, so it lasts to the end of the reads even when the
  request is dropped first) and only reads the store, writing a temporary file in
  `<lodestone>/tmp/backup-downloads/` (the carried objects' size checked against the free
  space first) that `GET /file/{key}` streams and then deletes, like the zip download,
  which writes there too: no copy of the store. Startup empties that folder in the
  background (`share::clear_leftovers`: renamed aside at once, then deleted), so a core
  stopped while writing or before the download doesn't keep a world-sized file. Progress is the bytes of the
  carried objects read (`container::export_with_progress`), sent as the progression's
  percent; a dropped request cancels it (the `Preparing` guard both downloads share). Both
  images are changed and sealed anew, so the export has ids of its own: (1) each data entry
  whose path and SHA-256 its content image has takes that entry's source, SHA-1, SHA-512,
  `env` and `env_from`, so Modrinth and CurseForge files travel as references and owner
  marks and sides travel; (2) `base` sources become `object` (the core's `base` entries are
  pack overrides, which no importer fetches from the `.mrpack`; the images keep their
  `base`); (3) the patterns below are taken out of both and added to their `excludes`; (4)
  secrets are blanked; (5) the label is "<server> – <backup name or time>", no parent,
  producer core. A backup whose content image is gone exports without one. Backup excludes
  near the 1,000-pattern budget plus the export's own refuse the export (409) rather than
  write an invalid manifest.
- **What an export takes out** (`export::SERVER_PRIVATE`, matched case-sensitively as
  patterns and in any letter case by `export::is_private`): `.lodestone*` at any depth
  (Lodestone's settings with the Java command and arguments, the modpack records, the
  marks file); `/eula.txt` (accepted per person); `/.console_history` (commands the admin
  typed); the server software, `/libraries/`, `/versions/`, `/*.jar`,
  `/user_jvm_args.txt` (the importer's installers make their own; an untrusted server jar
  never runs), every program or script at the top (`/*.sh`, `/*.bat`, `/*.cmd`, `/*.ps1`,
  `/*.command`, `/*.exe`, `/*.msi`, `/*.dll`, `/*.so`, `/*.dylib`: start scripts hold
  environment tokens, and Spigot's restart runs a script; a server from a file never takes
  them either) and the installers' logs, `/forge-installer.log`, `/installer.log`,
  `/*.jar.log` (Forge's and NeoForge's installer writes them on every server it makes,
  and again on the importer's); the JVM's `/hs_err_pid*`, `/replay_pid*`,
  `/java_pid*.hprof`, `/*.mdmp`, `/core`, `/core.*` and `share::is_jvm_crash_name`; the
  files that hold a key to the server or a service's login
  (`export::is_way_in`, which a server made from a file never takes either): Velocity's
  `/forwarding.secret`, the forwarding settings `/config/FabricProxy-Lite.toml`, the old
  FabricProxy's `/config/FabricProxy.toml` and Proxy Compatible Forge's settings
  (`/config/proxy-compatible-forge.toml` since 1.2.0, where it reads `forwarding.secret`
  with forwarding on by default, `/config/proxy-compatible-forge.cfg` for Minecraft
  1.12.2, `/config/pcf-common.toml` before), NeoForge's forwarding mods' settings
  (NeoVelocity's `/config/neovelocity-common.toml`, whose `forwarding-secret` it trusts
  whatever `online-mode` says, and NeoForwarding's `/config/neoforwarding-server.toml`),
  and every file or folder at the top of `config/` whose name says velocity, forwarding or
  proxy in any letter case (`/config/*velocity*`, `/config/*forwarding*`,
  `/config/*proxy*`: new forwarding mods keep appearing), Paper's
  `/config/legacy-backup/` folder (when Paper moved a server to 1.19's settings it moved
  the old `paper.yml` there as `paper.yml.old`, which still holds
  `settings.velocity-support.secret`, the secret `paper-global.yml` took over; Paper never
  reads the folder again), Floodgate's whole folder as a mod or a plugin
  (`/config/floodgate/`, `/plugins/floodgate/`: whoever holds its key joins as any
  Bedrock player through their own Geyser, and its `config.yml` names the key's file
  (`key-file-name`), so a key under any other name, `k2.pem`, would be read as surely as
  `key.pem`; without the folder Floodgate writes its defaults and a new key), and in
  Geyser's folders every `.pem` (`/config/Geyser-*/*.pem`, `/plugins/Geyser-*/*.pem`: a
  copy of Floodgate's key, under the name Geyser's `floodgate-key-file` gives) and
  `saved-refresh-tokens.json` (the Microsoft logins Geyser keeps for Bedrock players);
  and the settings of plugins and mods that hold a database's password, a bot's token or
  a token that lets its holder in (each folder as `config/<folder>/` and
  `plugins/<folder>/`, in any letter case; `export::WAY_IN_SETTINGS`): LuckPerms'
  (`/config/luckperms/luckperms.conf`, `/plugins/LuckPerms/config.yml`: its database
  password, and its database decides every permission), the Discord bots of DiscordSRV,
  EssentialsX Discord and Discord Integration (`/plugins/DiscordSRV/config.yml`, the
  copies of it DiscordSRV keeps when it updates its settings, `config.yml-build.<version>.old`
  with the token still in them (any name beginning `config.yml` in its folder,
  `/plugins/DiscordSRV/config.yml*`), and `/plugins/DiscordSRV/.token`, which DiscordSRV
  reads before its settings' `BotToken`,
  `/plugins/EssentialsDiscord/config.yml`, Discord Integration's
  `/config/Discord-Integration.toml` as a mod and `/plugins/DiscordIntegration/config.toml`
  as a plugin: the bot's token, whose Discord users, or its admin roles, can run
  commands), CoreProtect's, Plan's and LiteBans'
  databases (`/plugins/CoreProtect/config.yml`, `/plugins/Plan/config.yml`,
  `/config/plan/config.yml`, `/plugins/LiteBans/config.yml`), ServerTap's REST API key
  (`/plugins/ServerTap/config.yml`: its bare `key:` authenticates every call, the one that
  runs console commands included), AuthMe's whole folder
  (`/plugins/AuthMe/`: its database and mail passwords, and its players' password
  hashes), BungeeGuard's tokens (`/plugins/BungeeGuard/config.yml`: whoever holds one
  joins through their own proxy), and Votifier's and NuVotifier's tokens and keys
  (`/plugins/Votifier/config.yml`, `/plugins/Votifier/rsa/`, the same under
  `NuVotifier/` and `/config/nuvotifier/`: votes give rewards); and the map plugins'
  databases: BlueMap's whole storages folder (`/config/bluemap/storages/`,
  `/plugins/BlueMap/storages/`: every storage's file, whatever its name) and its web page
  for SQL (`sql.php` at any depth: its web root can be moved), Dynmap's
  `/dynmap/configuration.txt` and `/plugins/dynmap/configuration.txt` and its
  `web/standalone/` folder as a mod and a plugin (`/dynmap/web/standalone/`,
  `/plugins/dynmap/web/standalone/`: Dynmap writes the database's password into
  `MySQL_config.php` or `PostgreSQL_config.php` there at every start, so the folder comes
  back; those two names go at any depth, wherever its web path was moved); and the Discord
  bots and logins of mods: Simple Voice Chat Discord Bridge's bots' tokens
  (`/config/voicechat-discord.yml` as a mod, `/plugins/voicechat-discord/config.yml` as a
  plugin), SDLink's settings (`/config/simple-discord-link/`: its bot token, encrypted,
  and the Discord roles that run commands) and the key that decrypts the token
  (`/sdlinkstorage/`, `sdlink.enc`), EasyAuth's database folder (`/EasyAuth/`: its
  players' password hashes, in `easyauth.db` by default) and settings
  (`/config/EasyAuth/`: its MySQL, PostgreSQL and MongoDB logins in `storage.conf`, its
  proxy's shared secret in `proxy.conf`), and Ledger's `/config/ledger.toml` (its
  database extension's password); Discord-MC-Chat's bot token (2.x's
  `/config/discord-mc-chat.json`, the `/config/discord-mc-chat-backup.json` it makes when
  it upgrades them, 3.x's `/config/discord_mc_chat/` folder), Mc2Discord's
  (`/config/mc2discord.toml`), and Tebex's store key, which runs the store's commands on
  the server (`/mods/tebex/` as a mod, `/plugins/Tebex/config.yml` as a plugin, the old
  `/plugins/BuycraftX/`). Checked and left as they are, holding no credential:
  Simple Voice Chat itself (its addon Simple Voice Chat Discord Bridge holds bots' tokens,
  above), Chunky, squaremap, spark,
  EssentialsX's own settings, CrossStitch (Velocity's argument types for Fabric). The
  other plugins' settings go with "Include plugin settings" only, and a mod's config
  folder travels, but neither ever carries a file the generic rule finds a credential in
  (Credentials found by their names, below). A server from a file never takes any of these
  either. `/banned-ips.json` (banned
  players' IP addresses: personal data, and meaningless on another network;
  `banned-players.json` travels); a player's files (`share::PLAYER_PRIVATE`, the list the
  Launcher's export takes out, matched in any case by `share::is_player_private`), for a
  server made from a game folder: `command_history.txt`, `hotbar.nbt`, `ias.json`,
  `essential/microsoft_accounts.json`, `essential/mojang_accounts.json` at any depth, and
  the official launcher's `/launcher_profiles.json`, `/launcher_accounts*.json` and
  `/launcher_msa_credentials*.bin`; and `plugins/*/`, every plugin's folder (settings,
  databases, player data), unless the owner turns on "Include plugin settings (they can
  hold passwords)" (the Host offers it for Paper and Spigot servers; Floodgate's folder stays
  out even then). Plugin jars travel. Logs, crash reports and caches are never
  in a manifest. The answer lists the paths taken out; the Host's toast only counts them,
  beside the secrets made empty and the files that hold a password or token, which it
  names.
- **Blanked, not withheld.** `server.properties` is written again from its properties alone
  (`export::rebuilt_properties`), last value winning as Java reads them, with every key
  whose name says password, secret or token (`rcon.password`, `management-server-secret`
  among them) and `text-filtering-config` (its JSON holds the filtering service's API key)
  empty, so comments, repeated keys (an old password a later line overrides, one commented
  out) and lines the reader can't parse (`rcon.password=x\uD800`: Java's `Properties`
  takes unpaired surrogates, the reader doesn't) never travel; Minecraft rewrites the file
  at its next start anyway. It is read with every line break Java's `Properties` takes (a
  lone carriage return too), so a password after one isn't read as part of the line
  before, and a `#` or `!` line after a line holding only `\` is a comment that ends with
  its own line, as Java reads it, so the password on the next line isn't read as part of
  the comment's value (`PropertiesDocument::parse`). YAML files (`export::YAML_SECRETS`) get their values emptied in place: `secret`
  in Paper's `config/paper-global.yml` and, before 1.19, `paper.yml` (the Velocity
  forwarding secret), and `sentry-dsn` in `pufferfish.yml`. The cleaned copy is stored as
  an unnamed object (collected later) and the entry names the blanked keys in an unknown
  field, `"blanked": ["rcon.password", …]` (at most 8 keys of at most 64 bytes, within an
  entry's budget; readers keep it), which the importer's summary shows.
  **The YAML editor reads an allowlist** (`export::read_simple_yaml`): a YAML file it
  cleans or pins is edited only when every line is in a strict subset of block style that
  SnakeYAML (which Paper, Spigot and Bukkit use) provably reads the way the editor does,
  and is otherwise withheld whatever it holds. Three rounds found a way past checks that
  looked for odd forms (a flow collection over several lines, an escaped key, a byte order
  mark before a flow mapping), so nothing is looked for: anything not allowed is out. The
  subset: one byte order mark at the start at most (read past, as SnakeYAML does, and kept
  in the copy), no other U+FEFF, no control character but the tab (in comments only), no
  line break but `\n` and `\r\n` (no lone carriage return, U+0085, U+2028, U+2029);
  indentation by spaces; at most one `---`, before everything else, and no other document
  marker or directive; every other line blank, a comment, a key or a list item. A key is
  plain or quoted without escapes (`''`, `\`), of ASCII letters, digits, `_`, `-` and
  spaces, beginning with a letter, a digit or `_` (or such parts joined by colons, a
  namespaced id: Paper 1.21.4 writes `minecraft:elytra:`), and never with a `.`: Bukkit's
  settings (Spigot's, Bukkit's, the old `paper.yml`'s, Pufferfish's) split every key at
  its dots, so a top-level `settings.restart-script:` after the nested `settings:` is
  Spigot's restart script to them (and `settings.world-container:`,
  `settings.permissions-file:` Bukkit's, `settings.velocity-support.secret:` the old
  Paper's forwarding secret), out of the sight of an editor that matches one key at a
  time; the files Bukkit saves nest their keys, so a real one has no dotted key, followed by `: ` and its value or
  `:` alone, so no `?` or merge key, tag, anchor or alias. A value is a plain scalar on its
  line (no indicator first, no `: `, no `:` at its end, a comment only after a space),
  wrapped onto following lines more indented than its key that only go on with it (no
  indicator first, `: `, comment or tab: Paper writes its long `no-permission` message so);
  a single-quoted scalar, which may go on over following non-blank lines more indented than
  its key up to its closing quote (`paper.yml` before 1.19 wraps one so); a double-quoted
  one on its line; a one-line flow collection, only `{}` or a list of scalars (`[]`,
  `[a, 'b']`, `[minecraft:elytra]`; a quote inside a plain item, `a"b`, is refused, not
  read as the start of a quoted one); or a block scalar (`|`, `>`, with `-` or `+`), every
  non-blank line of which is more indented than its key and at least as indented as its
  first. A list item is one such scalar on its line (no `- key: value`, nested list or `-`
  alone), at its key's indentation or deeper. A deeper line comes only under a key with an
  empty value or going on with a value, and a less indented one must return to an
  indentation a line above opened. So every key SnakeYAML finds is a line's key here, at
  its line, and the lines the editor drops with a key's old value (`KeyLine::end`: its
  wrapped or block scalar's lines, or the keys and items under it) come from the same
  reading of indentation. Keys are matched in any letter case at any depth. The settings
  files Paper, Spigot and Bukkit write today (`paper-global.yml` and
  `paper-world-defaults.yml` of Paper 1.20 and 1.21, `spigot.yml`, `bukkit.yml`,
  `commands.yml`, the old `paper.yml`, `pufferfish.yml`) are all in the subset and travel
  cleaned (`real_settings_files_are_read_and_cleaned`). A copy of any of these files that
  can't be cleaned (over 1 MiB, unreadable from the store, a YAML file that isn't UTF-8 or
  isn't in the subset) is withheld instead: taken out of both images, its exact path
  (`/config/paper-global.yml`) added to their `excludes` and listed in the answer's
  `left_out`, so a file that leaves the computer never carries those secrets.
  Beyond these and the named bots', databases' and forwarding mods' settings above, the
  generic rule below withholds any other settings file with a credential in it; the export
  dialog says so ("Passwords in server.properties are blanked, and files that hold a
  password or token are left out").
- **Credentials found by their names** (`share::secrets`, on top of the named lists, which
  stay: they also cover files with no credential key, a database or a key under another
  name). Four rounds of checks each found more mods and plugins whose settings hold a bot
  token, a store key, a proxy's forwarding secret or permission data the lists didn't
  know, and the lists will never be complete, so an export also reads every file it would
  still carry and withholds one that holds a credential, whatever the "Include plugin
  settings" option says (a credential that reaches an outside account or lets someone in
  never travels), and a server made from a file never takes one (below).
  *Which files*: every file whose extension is `.toml`, `.yml`, `.yaml`, `.json`,
  `.json5`, `.jsonc`, `.properties`, `.conf`, `.cfg`, `.hocon`, `.ini`, `.env`, `.snbt`
  or `.txt` (any letter case) anywhere outside the worlds, and in a world's
  `serverconfig/` (Forge's per-world settings), plus files with no extension in a settings
  folder (`config/`, `plugins/`, `defaultconfigs/`, a folder in `mods/`, a world's
  `serverconfig/`: Plasmo Voice's `forwarding-secret`). A copy of a settings or key file
  is read as the file it was made from (`secrets::underlying_name`): when a name's own
  extension is neither read, a key's nor plainly data, it is judged by the first settings
  or key extension in it that a separator (`-`, `_`, `.`, `~`, a space, `(`, `+`, `,`) or
  the name's end follows, so DiscordSRV's `config.yml-build.1.28.0.old` (it moves its
  settings there when it updates them, token and all, and its `messages.yml`,
  `voice.yml`, `linking.yml`, `synchronization.yml` and `alerts.yml` likewise),
  `config.json.bak`, `settings.toml~`, `config.yml.disabled`, `bot.token.old` and
  `server.properties.old` are read or withheld as their originals are; `mods/x.jar.disabled`
  and `icon.png.bak` are not read. Not `server.properties` and the
  YAML files of `YAML_SECRETS`, which keep their precise blanking (edited, not withheld),
  and not a Modrinth or CurseForge file, which travels as a reference to its public bytes.
  *How a file is read* (`secrets::find_credential`): JSON, JSON5, JSONC and SNBT by their
  tokens (strings, bare words, braces, brackets; comments skipped), every other format
  line by line, following YAML's indentation, TOML's and INI's `[sections]`, HOCON's and
  Forge `.cfg`'s braces, lists (`- item`, `[`…`]`, `.cfg`'s `<`…`>`), Forge's type letters
  (`S:botToken=`), `.env`'s `export` and `.properties`' `key value`; a block scalar or a
  TOML multi-line string is judged by its first line. A file it can't follow is still read
  line by line, which errs towards finding a credential. Each key is judged with the key
  above it (`secret: { key: … }` is a secret key).
  *Which keys* (`secrets::credential_key`): the key is split into words at separators,
  dots and camelCase (`botToken`: bot token, `secret_key`, `apiKey`, `rcon.password`); it
  names a credential when its last word is `password`, `passwd`, `passphrase`, `secret`,
  `token`, `credential(s)`, `webhook`, `apikey` (or a word ending in one: `bottoken`,
  `rconpassword`; plurals too: `allowed-tokens`), `pwd`, `bearer` or `dsn`; when it is
  `key` after `api`, `secret`, `private`, `license`, `access`, `auth`, `store`, `server`,
  `signing`, `encryption`, `client`, `master`, `shared`, `hmac`, `rcon`, `bot`, `webhook`,
  `session` and a few services' names (also as one word: `secretkey`; the word before may
  be the parent key's last); `pass` after a word that makes it a password (`db`, `mysql`,
  `rcon`, `smtp`, `user`, `admin`…); `url` after `webhook`; or `value` (`string`, `text`,
  `data`) under such a key. Bare `key`, `keys` or `pass` only in a file where a key whose
  words include `auth` is switched on (`useKeyAuth: true`, `auth: true`; ServerTap's REST
  API `key:` authenticates its calls), never otherwise; nor a key whose last word only
  begins like one (`keyFileName`, `keepInventory`, `passive`, `tokenExpiry`,
  `forwarding-secret-file`, `passwordMinLength`). A `dsn` that is a database's address
  with no login in it (`jdbc:mysql://localhost/worldguard`: no `user@` and no
  `password=`) doesn't count; a Sentry DSN, its key before an `@`, does.
  *Which values* (`secrets::real_value`): a placeholder doesn't count: empty or blank, a
  boolean, `~`, a number of fewer than 8 digits (`max-tokens: 64`), one character
  repeated (`xxxxxx`, `******`), a reference (`${BOT_TOKEN}`, `{{token}}`, `%token%`,
  `<token>`, `[]`, `{}`), `changeme`, `changeit`, `none`, `null`, `default`, `example`,
  `password`, `token`, `secret`, `key` and the like, passwords settings files ship with
  (HuskHomes' `pa55w0rd`, `passw0rd`, `password123`, `defaultpass`, `guest`), the key's own
  name or last word (`BOT_TOKEN` for `botToken`, `TOKEN`), a value that repeats the value
  of a key in the same file naming a user or a database (`username`, `user`, `login`,
  `database`, `db`, `db-name`: WorldGuard's `password: worldguard` beside `username:
  worldguard`), and an instruction (`your-token-here`, `INSERT-TOKEN-HERE`,
  `INSERT BOT TOKEN HERE`, `Paste your key`). A password is not discounted because the
  settings say its database isn't in use (HuskHomes' `type: SQLITE`, WorldGuard's
  `use: false`): a real password the owner once set stays a real password, so such a file
  is withheld, named in `secrets_withheld`, rather than sent. So the defaults of a
  fresh install, whose secrets are empty or such placeholders, travel: DiscordSRV's
  `BotToken: ""`, EssentialsX Discord's and Discord Integration's `INSERT…HERE`, Plan's
  `Password: password`, LuckPerms' `password: ''`, FabricProxy-Lite's `secret = ""`,
  Sponge's `secret=""`, HuskHomes' (as a mod and a plugin), WorldGuard's, ServerTap's
  `key: 'change_me'`, Simple Voice Chat, Chunky, spark, Geyser, Forge's server settings
  and Paper's, Spigot's and Bukkit's (`real_default_settings_hold_no_credential`).
  *Values that are logins by their shape*, whatever the key (`secrets::credential_value`):
  a URL with a password (`mongodb://user:secret@host`, unless the password is a
  placeholder), a `password=` parameter of a JDBC URL, a Discord webhook URL with its token
  and a Discord bot token.
  *By name* (`secrets::withheld_by_name`), files that are a credential with no keys,
  anywhere but in a world's region, entity and POI data: `.token` and `*.token`, `*.key`,
  `*.pem`, `*.p12`, `*.pfx`, `*.jks`, `*.keystore`, `*.secret`, `*.kdbx`, `id_rsa*`,
  `id_ed25519*`, `id_ecdsa*`, `id_dsa*`, `.env*`, `.netrc`, `.pgpass`, `.htpasswd`,
  `credentials`, `credentials.json`, `token.txt`, `secret*.txt`, and a `.txt` file or one
  with no extension whose name is a credential key (`bot-token.txt`, `forwarding-secret`).
  *Too big* (`secrets::too_large`, over `secrets::MAX_SCANNED_FILE`, 4 MiB): a file the
  scan would read, and any file in a settings folder whose extension isn't plainly data
  (`.jar`, `.zip`, `.gz`, images, sounds, fonts, `.nbt`, `.dat`, `.mca`, `.mcr`, `.mcc`,
  schematics, and databases, `.db`, `.sqlite`, which the scan never reads at any size),
  is withheld rather than sent unread.
  A file withheld so is taken out of both images, its exact path added to their
  `excludes` (so it is in `left_out` too) and listed in the answer's `secrets_withheld`,
  which the Host's toast names ("Left out, as they hold a password or token (the other
  server writes its defaults): …"); the log names the file and the key, never the value.
  A file the export can't read from the store is withheld too.
- **Large writes share the disk.** Every large write (a file written into `tmp/` for a
  download link: a folder zipped, a backup zipped or exported; an upload arriving; a
  server being made from a file, from its unpack to the end of placing its files)
  reserves what it is still to write in one count per core
  (`util::disk::LargeWrites`, `ImageService::large_writes`), and each write's free-space
  check asks for what all the others still need too, so no two writes at the same time
  ever count on the same space (an export no longer passes its check against space an
  import is filling, nor the reverse). A user's links may hold at most 3 backup files (zips and exports, counting those
  being written; `global_fs::MAX_BACKUP_FILES_PER_USER`, as `MAX_ZIPPED_FOLDERS_PER_USER`
  caps zipped folders; more are 429), so repeated requests can't keep the disk at its
  margin for the links' hour; the place goes with the write onto its blocking thread, so a
  dropped request doesn't free it before the write ends. `DELETE /file/{key}` frees one at
  once: the Host lets go of each backup file it saved, zip or export, once its download
  ends however it ended (and of an export whose task was dropped, from a drop guard), so
  files it already saved never hold the user's places.
- **Zips kept.** The plain backup zip (`…/download`) and the world zip download stay: they
  are for using a world outside Lodestone.
- **Uploads.** Streamed into `<lodestone>/tmp/image-uploads/image-*.lodeimg` (wiped at
  start), at most 64 GB (the container's own limit), refused at once when the request's
  `Content-Length` says more or the disk can't take it, the free space checked again every
  256 MiB written; then read on a blocking thread. At most 4 arrive at once and 2 of one
  user's (more are 409), and each one arriving reserves its declared size, less what has
  arrived, among the core's large writes (`uploads::Receiving`), so it never counts on the
  same space as another upload, an import or an export. Kept an hour
  for the uploader alone, at most 2 of each user's (their own oldest goes: one user's
  uploads never push out another's); creating a server forgets it once placed, the Host
  discards it when the person backs out or closes the dialog (and when Cancel is pressed
  as the core answers: the file it took is discarded), and a task deletes uploads past
  their hour every 5 minutes (`Uploads::prune_periodically`), so a file whose uploader
  went away doesn't keep its space until the next upload. Wiping the folder at start runs
  in the background (`share::clear_leftovers`). Uploads let go of are deleted outside the
  uploads' lock, on a blocking thread (a file of up to 64 GB can take seconds to unlink).
  Files are read one at a time (`Uploads::reading`, for the upload and for the creation's
  second read): a small file can unpack to manifests of 512 MiB and a million entries,
  which reading holds several times over. The turn (an owned permit) and the upload's slot
  among those arriving go into the blocking read, so a client that hangs up mid-read
  doesn't free them for the next file while its read goes on. At most one server is made
  from an upload at a time (`Uploads::take`; another request for it is 409 "A server is
  already being made from this file" until that creation ends, however it ends), so a
  client retrying can't queue creations that each hold a file's manifests and plan in
  memory while they wait for their turn to place.
- **Reading a file** (`inspect`): the container's checks, then a file is refused (400) for
  a `.lodestone*` name at any depth, a top-level `mods`, `config`, `defaultconfigs`,
  `kubejs`, `plugins`, `libraries`, `versions` or `saves` spelled with other than ASCII
  letters (`share::spelled_like`), a Minecraft or loader version `share::plausible_version`
  refuses, more than `share::MAX_DOWNLOADS` (20,000) distinct downloads, two paths that are
  one on a disk that ignores letter case (`paths::case_collisions`: a server made from it
  would hold one of them on macOS and Windows and both on Linux, where the summary could
  read `OPS.JSON` and the server `ops.json`), or a top-level settings file
  (`import::SETTINGS_FILES`: `server.properties`, `ops.json`, `permissions.yml`,
  `commands.yml`) spelled in other letters than the server reads it (`SERVER.PROPERTIES`)
  or over 1 MiB. `server.properties` and `ops.json` are read by their exact names before
  anything is imported (`Container::read_object`, at most 1 MiB, checked by SHA-256; a
  file whose copy can't be read whole is refused, so a padded `server.properties` can't
  turn online mode off unseen; a server takes them, and `permissions.yml` and
  `commands.yml`, only from the file's own bytes, never as a download, so the summary
  describes what is placed). `server.properties` is read with Java's line breaks, as it
  is placed; `offline_mode` is set when `online-mode` is anything but exactly `true` (any
  letter case, nothing around it), as Minecraft reads it (`Boolean.valueOf` on the value
  as `Properties` parsed it, which trims nothing: `online-mode=\ true` with an escaped
  space, an escaped tab or a no-break space before `true` is off, though a reader that
  trims Unicode whitespace would call it on). Starting a server reads its `online-mode` and
  `enable-rcon` the same way (`server::LaunchSettings`: the default when the key is
  missing, otherwise on only for `true`), so Lodestone's own player lookups agree with
  the server. A file is refused when its `server.properties` has any
  line the reader can't parse (`inspect::check_properties`: a malformed or
  unpaired-surrogate `\u` escape, an empty key; the line isn't shown, it may hold a
  password): Java's `Properties` may still read it (`online-mode=false\uD800`, which
  Minecraft reads as off), and the summary would say online mode is on; a server takes the
  file written again from the parsed properties alone, so nothing unparsed reaches it
  either. An `ops.json` that strict JSON can't read as a list of objects (a byte order
  mark, text after the list, a comment, a key twice in an entry: Minecraft's Gson reads
  some of these) is refused too, rather than shown as no operators; an empty one has none.
  These are the only settings files the summary reads (`permissions.yml`, `commands.yml`
  and LuckPerms' data are named, not read), and the only ones the import edits are
  `server.properties` (rebuilt) and the YAML files of `YAML_SECRETS` and `PINNED_YAML`
  (the allowlist above, deleted when outside it), so no file the core can't read passes
  through it to the server. `operators` lists every `ops.json` entry (ops are kept by
  uuid: a name, with the uuid when another entry has that name too, or the uuid of an
  entry with none), the file's `permissions.yml` and `commands.yml`, and the permission
  mods' and plugins' data it has (each shown once by its folder, `plugins/LuckPerms/`,
  `saves/world/serverconfig/ftbranks/`, or file, `config/roles.json`), which
  `keep_operators` also takes. The summary: who made it
  (`made_by` from the producer: it only steers the dialog), game, loader and flavour, the
  server Lodestone makes and why not, worlds with sizes and the default (a Host's level, a
  Launcher's last played), the sender's excludes and withheld files, blanked keys (a
  `NameList`, counted up to 100,000), what a
  server never takes, the operators and offline mode. What the server takes is worked out
  by `import::plan` for the default world (the whole file's when no server can be made):
  the mods it downloads from Modrinth and their size, its CurseForge mods and the ones it
  can't get, `carried_mods` (mod and plugin files it takes whose bytes the file carries:
  the trust warning names them; a Launcher jar that says it runs only on clients is only
  found once placed, so it is still named), `client_only` (mods left out as their server
  side is `unsupported`), whether WriteGlobalFile is needed, scripts skipped,
  `unpacked_bytes` and `needed_bytes` (creating the server checks the free space again for
  the world picked, so the Host doesn't stop Create when only the default world doesn't
  fit), `server_mods` (every mod and plugin file it takes, from any source: the Host's
  trust warning, "It has N mods or plugins: programs that run on the server. Only use
  files from people you trust.", shows whenever there is one, as the Launcher's import
  does, with the carried ones named and the Modrinth ones listed on the download line; a
  file with none that still carries protected files of its own, jars or programs, gets
  "It has protected files of its own … Only use files from people you trust.") and
  `secrets_not_taken` (the `is_way_in` files the file has, and every file the generic
  rule finds holding a credential: `inspect::credential_files`, below). Create stays off while the
  file carries protected files the person may not add, which the core always refuses. Every text goes through `display_text`, but a world's `folder` comes as the file
  names it, an id the request sends back, beside `folder_shown`, written visibly; lists
  hold at most 64 names. The core's prose fields (`unsupported`, `server_note`) are for
  the dashboard: the Host writes its own, translated, from the loader and flavour, and the
  Paper note only while Paper is picked.
- **Which server.** Fabric, Forge and NeoForge images make those servers; a vanilla game
  makes a vanilla server, or Paper when its flavour is Paper, and Paper for the Bukkit
  family's other servers (Spigot, Bukkit, Purpur, Folia: they run its plugins and share its
  world layout; the core can't make Spigot servers), which the summary says. A vanilla
  game's file may become either (`server_choices` is `["Vanilla", "Paper"]`, and the
  request's `flavour` picks; the Host offers "Server software" then), so a player's
  singleplayer world can be hosted on Paper for plugins, its world split the Paper way; a
  modded game's only makes its own loader's server (another `flavour` is 400). Quilt and
  unknown loaders are refused ("Lodestone can't make Quilt servers"). The loader version is
  the file's when it can still be downloaded, else the newest, as the folder import does
  (`ServerSetup::resolve`).
- **The server's view, never software or the EULA** (`import::plan`, pure and run before
  anything is made): the placed entries (the image's over its content link's) in the
  server's view, by the shared crate's own rule (`view::in_view(entry, Side::Server)`: a
  path a server can hold, whose server side isn't `unsupported`; a player's own files and
  client-only mods are left out), which `inspect` uses too. A Host's file is a server's
  own folder, so its files at a server's path all stay, client-only mods included: those
  were the mods the old server gave players ("For players"), and the new server's client
  view gives them to players too (dropping them would stop players getting them). Never
  server software (every program or script at the top included), `eula.txt` (the EULA is
  accepted only when the request says the user ticked it), `ops.json`, Bukkit's
  `permissions.yml` (which can give every player the operator commands) and Bukkit's
  `commands.yml` (its aliases replace any command, and run their commands as whoever typed
  it: the core types `stop`, `save-off` and `save-all` into the console, so an alias of
  `stop` would run as the console when the owner stops the server; without it the server
  writes its defaults), and permission mods' and plugins' data
  (`import::is_permissions_data`): LuckPerms' (everything in `config/luckperms/` or
  `plugins/LuckPerms/` but its settings, which are a way in: its H2 or SQLite database,
  `yaml-storage/` and `json-storage/` decide who has which permission, `*` included); FTB
  Ranks' ranks and players in any world's `serverconfig/ftbranks/` and the pack's
  `defaultconfigs/ftbranks/` (copied into a new world: its `command.*` nodes override any
  command's permission level, `command.op` among them) and FTB Utilities' rank files in
  `local/ftbutilities/`; the folders of PermissionsEx, GroupManager, UltraPermissions,
  bPermissions, zPermissions and PowerRanks (`plugins/<folder>/`); player-roles'
  `config/roles.json` (roles that give commands and operator powers); ForgeEssentials'
  `ForgeEssentials/` folder and its world data (`<world>/FEData/`: permissions and
  groups); and Command Aliases' `config/commandaliases*` (aliases that can run any command
  as the console, like Bukkit's `commands.yml`); unless `keep_operators` (off by default:
  an operator runs any command; `import::is_operators_file`), each named in the summary's
  operators by its folder (`saves/world/serverconfig/ftbranks/`, `plugins/PermissionsEx/`)
  or file, scripts unless the
  owner may add them while code execution is on (counted), nor a file that has no bytes
  and no source this core downloads. Every file is placed without its execute bits
  (`mode & 0o666`), so nothing the sender placed runs as a program by name.
  Launch settings never come from a file: `.lodestone*` is refused, and the installer
  writes the server's own.
- **Worlds.** A Host's file keeps every world; its level is the request's `world` or the
  file's `level-name`. A Launcher's file gives only the picked world, renamed to the
  server's level (`ServerLayout::unused_level` of the other top-level names, normally
  `world`); none when `world` is left out (the server makes a new one). Worlds are laid out
  in the server layout, split the Paper way for a Paper server; a file the layout can't
  place (a world named like server content) is refused before anything is made. The
  singleplayer player's inventory in `level.dat` doesn't become a server player's.
- **Making it.** Every new server goes through `ServerSetup` (`AppState::server_setup`):
  `Installers` in the core, `FakeSetup` in tests, so creating, importing and making from an
  image are tested with no network, no server jar and no EULA. A file's server goes
  through this creation flow (the installer, then `materialize` straight into the folder
  `.lodestone_creating` guards), not restore-as-new's staging and rename
  (`backups::restore::stage_all`): restore-as-new copies the backup's server software and
  Lodestone files and runs no installer, while a file gives neither and the installer
  must run first. The installer sets the server up in its new folder; then, one creation
  from a file at a time (`Uploads::placing`: each counts on the free space it checks), with
  collections held off from the import of the objects to the end of placing them: the
  objects the plan's files name are imported (`Container::import_objects_where_verified`,
  every byte checked, the objects the store already has too: a file must carry every byte
  it places, or a hash and a size alone would get another server's file out of the shared
  store; a Launcher file's other worlds, a player's files and client-only mods are never
  unpacked; the creation reserves these, the downloads and the placed files among the
  core's large writes until its files are placed, and the free-space check asks for them
  plus what the other large writes still need); every
  Modrinth entry with a SHA-512 and its own project's and version's canonical CDN URL
  (`inspect::names_modrinth_file`, the Launcher's rule) is downloaded once per file and
  claimed source, 8 at a time, through `<server>/.lodestone-download-<n>`, its size and
  SHA-512 checked, then stored and its SHA-256 checked; a download that fails is logged and
  the import goes on, and after 8 failures in a row no new one starts (the core's
  downloads don't say whether anyone answered); an entry no download confirmed is placed
  from the file's own bytes as a plain file, or left out and listed; confirmed files are noted
  as Modrinth's, so the first content image records their sources. The core has no
  CurseForge client (it has no CurseForge API key; the Launcher's is compiled into the
  Launcher), so a CurseForge file is placed only from bytes the file carries, and the rest
  are left out and listed: the Host's summary says so plainly as a warning ("Lodestone
  servers can't download from CurseForge yet, so these files won't be installed"). This
  differs from the brief, which asked for CurseForge downloads: the developer decided that
  CurseForge downloads stay out of the core for now, so they are out of scope (no
  CurseForge key or client in the core). The core's own content images
  never record CurseForge sources, so only a Launcher's file loses mods this way. The
  installer's `server.properties` is deleted when the file has one, the free space is
  checked once more for the placed files (the downloads took their share), and the files
  are materialized straight into the folder, each created new, so nothing the installer wrote
  is overwritten: a file the installer already wrote (a loader's log, say) keeps the
  installer's copy and the file's is skipped, rather than failing the creation.
  Then a Launcher file's placed `mods/*.jar` whose server side nothing knew is deleted
  when its own
  `fabric.mod.json` or `quilt.mod.json` says client only (a server loading it crashes;
  jars that declare nothing stay, and Forge's are read as the recorder reads them,
  without this check; a Host's file keeps its jars as its server had them);
  `server.properties` is read with Java's line breaks, written again from the properties
  the reader parsed alone (`export::rebuilt_properties`: no comment, repeated key or line
  it couldn't parse reaches the server) and pinned (the port, `query.port` set
  to the same port (the old computer's could be another server's query listener here),
  `level-name`, an empty `server-ip` (the old computer's address wouldn't bind here),
  every secret empty, `enable-rcon=false`, `management-server-enabled=false`,
  `online-mode` written as the `true` or `false` Minecraft reads from it (so the placed
  file says plainly what the summary showed: `\ true` is placed as `false`), and
  `function-permission-level` at most 2, Minecraft's default: at 3 or 4 a world's datapack
  could run `/op`); the YAML secrets an export blanks are emptied in the placed copies
  (Paper's Velocity forwarding secret: with it empty Paper refuses forwarded players until
  the owner sets their own), and `export::PINNED_YAML` is set whatever the file said:
  Spigot's `settings.restart-script` to its default `./start.sh` (Spigot's restart and
  watchdog run `sh <restart-script>`, which could be any placed file; no file places a
  `start.sh`), Bukkit's `settings.permissions-file` to `permissions.yml` (which comes only
  with the operators) and its `settings.world-container` to `.` when the file sets it (it
  could put the worlds anywhere the core can write, outside the server's folder, its
  backups and its users' permissions), and Bukkit's old command aliases, a top-level
  `aliases:` section in `bukkit.yml`, to `now-in-commands.yml`, the string Bukkit writes
  there once it has moved them (several keys per file, set one after the other,
  `export::pin_yaml_all`): with a section there and no `commands.yml`, Paper's, Spigot's,
  Purpur's and Folia's `CraftServer` move the section into a new `commands.yml` at the
  first start (each alias's commands with ` $1-`), and those aliases replace any command,
  `stop` and `save-all` included, which the core types into the console, so the sender's
  `stop: [op sender, stop]` would make them an operator the first time the owner stopped
  the server, although `commands.yml` comes only with the operators. The section is pinned
  with or without the operators (real files have said `now-in-commands.yml` since Bukkit
  moved aliases out, so only a crafted file loses anything), and an export leaves it as
  it is, since the owner's own aliases are no secret and every server made from the file
  pins it; a copy the cleaning can't read is deleted (the server writes
  its defaults), and that is any copy with a line outside the editor's allowlist
  (`export::read_simple_yaml`, as an export withholds one: a top-level
  `settings.restart-script:`, which Bukkit's settings split at its dots,
  `"restart\x2Dscript"`, a
  `settings: {` with `bungeecord: false, restart-script: world/evil.txt` on the next line,
  or a byte order mark before `{settings:{` would otherwise be Spigot's restart script to
  SnakeYAML, unpinned), each deleted copy named in the
  creation's end message ("settings file(s) couldn't be checked for keys to the server and
  were left out", `import::Placed::settings_removed`), not only the log, since the owner's
  settings from the file are gone; and the `is_way_in` files are never placed
  (`import::Why::Secret`, listed in the summary and the creation's end message), nor any
  file the generic rule finds a credential in: `inspect::credential_files` reads the
  carried bytes of every file the export's scan would read (`Container::read_object_any`,
  whole or from its chunk list, up to 4 MiB, every byte checked; a settings file whose
  bytes don't match its hash refuses the whole file at once) and names the files that
  hold one (a sender's bot token, proxy secret, store key or webhook), the key and token
  files by their names, the files too big to read, and any such settings file the server
  would download rather than take from the file's bytes (it can't be checked before it is
  placed); `import::plan_checked` drops them as `Why::Secret`, so a sender-supplied
  credential never configures the new server, and the end message says "file(s) holding a
  key to the server, a password or a token were left out, so the server writes its
  defaults". So a hostile file's settings give its
  sender no way in: not RCON, the management server, a proxy's forwarding, Floodgate (its
  whole folder, so not a key its settings rename), a LuckPerms database or a Discord bot
  of theirs, a script Spigot runs, worlds outside the server's folder, or operators'
  powers through functions, Bukkit's permissions, its command aliases or LuckPerms' own
  storage (taken only with the operators). Its mods and plugins are code from whoever made it, which the trust warning is
  for, and plugins' own settings (`plugins/*/`) come as the file has them. The owner's
  marks go to the marks file; a base is followed
  (`follow_base`), its origin taken from Modrinth when its version is a plausible id
  (`check_api_segment`). The first content image is "Created from <file name>", the name
  written visibly (`display_text`); no backup record is made, as a restore as new makes
  none. The server is never started.
- **Unfinished creations aren't servers.** Every new server's folder (from a file or not)
  holds `.lodestone_creating` (its uuid) until the installer and any step after it (a
  file's placing, a modpack's install) are done, and gets its `.lodestone_config` only
  then, written atomically before the marker goes: the core never loads a server whose
  creation a stop interrupted (half a world, a partial `mods/`, the file's own
  `server.properties`). At startup, before the servers are read, a folder with the marker
  and no `.lodestone_config` is deleted with its images
  (`instance::remove_unfinished_creations`), the images first while the marker still names
  them (a core stopped in between finds the folder again at the next start); one with both
  only loses the marker. Such a folder can hold a half-unpacked world, so startup only
  moves it into
  `<instances>.unfinished` beside the servers' folder (a rename) and empties that in the
  background (`share::clear_leftovers`, read-only files cleared as Windows needs); a
  folder that can't be moved is deleted in place. A creation that fails while the core
  runs discards its folder the same way and in the same order
  (`instance::discard_unfinished`, shared with startup): its images first, keeping the
  folder and its marker for the next start when they can't be deleted, then the folder
  renamed aside in one step and emptied in the background, so a stop or a failed delete
  never leaves part of a folder with no marker, nor images naming its objects. A folder
  whose marker couldn't be written is removed at once, so no folder without a marker is
  left.
- **Downloads that fail.** A download whose host failed or answered no file (or one over
  its size) is one file left out; any other failure (its file couldn't be written: a full
  disk) stops the creation, which then deletes what it made, so a server is never made
  without its mods with the network blamed. An entry no download confirmed is placed from
  the file's own bytes only (an object the file carries, imported and checked), never from
  another object the shared store has under that hash, which would let a file name any
  server's files by hash alone. The same holds for the file's plain objects: the unpack
  reads and checks the file's bytes of every object it places even when the store has it
  already (`import_objects_where_verified`), so a file naming another server's object by
  its hash and size, with other bytes of that length, is refused.
- **Not cancelled once made.** An upload is cancelled with its request (the Host's Cancel
  drops it at once, while it is sent or read); making the server, like every creation,
  runs to its end or fails and deletes what it made: its images and then its folder at
  once (Unfinished creations aren't servers), and
  the objects it unpacked or downloaded, which no image names but the store's grace keeps
  for an hour, by a collection asked for once that hour is past
  (`ImageService::request_gc_after(gc::AFTER_GRACE)`, and at every startup: the timer of a
  creation that failed in the previous run is lost with it). The Host shows how a creation ended when the creation flow says
  more than its plain success (what a file's server didn't get) or it failed, from the
  `InstanceCreation` progression's end, for every way of making a server; a backup
  restored as a new server says nothing more, as its page already said it started. The
  text is the core's English, as the Files tab's zip toasts are; the rest of the dialog is
  translated. A structured outcome (the lists of what was left out in the progression's
  end value) would let the Host translate it, but the end value is shared with every
  creation and the dashboard. Milestone 6's second part made it ("M6b decisions",
  Structured creation outcome): the end value carries an optional `outcome` beside the
  server's info, which the Host's toast and the Launcher's "Host this" show translated.
- **The Host's export says what it is for.** "Export as a file for another Host or a
  Launcher" (milestone 6's second part; it was "for another Lodestone Host" while the
  Launcher couldn't open a core export). The dialog shows an indeterminate bar while the core writes the file and the bytes
  once it downloads (known gap: the core sends the writing's percent as a progression the
  Host doesn't follow yet, and every container entry is deflated, so a world of 20 GB
  spends minutes "Preparing"); the request waits up to 4 hours (Cancel ends it sooner).
  The default file name is "<server> - <backup name or id>.lodeimg", never a relative
  time. The ticker redraws the page only while the dialog shows the export. Once the
  download ends, however it ends (Cancel pressed just as the core answered too), the
  client lets go of the core's file (`DELETE /file/{key}`), so a world-sized file doesn't
  wait out the link's hour on the core's disk.
- **The Host's client.** `upload_image` waits an hour plus a second per 256 KiB of the
  file (a fixed hour would end a large world sent over a home connection);
  `upload_image_named` sends a file under another name (for "Host this", which uploads a
  temporary export: the server is "Created from" the name given). Its Cancel makes the
  body fail with an error the async adapter doesn't retry (`Interrupted` would spin the
  HTTP client's only worker forever). Its Cancel watcher ends with the request's future,
  also when that is dropped (a dialog closed mid-upload, which also cancels the upload).
  `create_from_image` waits up to 15 minutes: before it answers, the core opens the upload
  again (its index and manifests, which for a file of a million entries take a while),
  checks the disk and looks the server software up over the network. Milestone 6's
  second part uses these for "Host this" (`cores::hosting`) and adds the creation's
  outcome (`admin::events::CreatedInstance`) and `AdminClient::wait_progression`.
- **The Host's "From a file" summary.** The new server's name starts as the file's label
  with the characters a server's name can't have replaced (`:` becomes `.`, the others
  `-`: an unnamed backup's label has the time, "14:10"), and the Name field says so before
  the request for a name typed with them (`modpack::NAME_FORBIDDEN`, for every way of
  making a server). The disk line and the no-room warning follow the world picked
  (`ImageSummary::needed_bytes_for`: a Launcher's file places only that world, so the
  default world's size comes off and the picked one's goes on, each twice, unpacked then
  placed; a Host's file places every world); the core checks again.
- **Interop.** One container both ways: the core's files are what `lodestone_image::container`
  writes and the Launcher reads, and the core reads any valid `.lodeimg`. The Launcher's
  import (M5) can't place a core export (a server's layout: `server.properties`, plugins),
  so it refuses a file whose producer is the core before anything is unpacked, saying so
  plainly ("This file is a server's backup exported from Lodestone Host. The Launcher can't
  open server backups yet") instead of naming the first path the client layout refuses.
  Milestone 6's second part replaced this refusal: the Launcher opens a core export as
  the client view of its content with its worlds (`images::from_server`, "M6b
  decisions"). Exports from the two programs differ in bytes (deflate), so compare them
  by image id.
- **Names from a core are shown safely.** Whatever a core sends is untrusted to the
  Launcher: `NameList::joined` (every list the summary, the outcome and the import show
  from a core) shows at most the first 8 names, each through `display_text` and cut at 120
  characters, with "…" for the rest, so a name can't span lines, hide its direction or
  fill the dialog. The core's English reasons shown in the window (a refused creation, a
  failed backup) go through `display_text` too. In the Launcher's "Host this" the file is
  the person's own game, so the summary leaves out who made it and the warning to trust
  its sender (`Picked::own_file`).
- **The shared crate changed only additively**: `container::export_with_progress`
  (`export` calls it), `Container::read_object`, `Container::read_object_any` (a carried
  object whole or from its chunk list, for the import's credential scan),
  `Container::unpacked_bytes_of`,
  `Container::import_objects_where` and `Container::import_objects_where_verified`
  (`import_objects` shares their loop), and the `share` module (`MAX_DOWNLOADS`,
  `placed_entries`, `plausible_version`, `strip`, `is_jvm_crash_name`,
  `is_launcher_account_file`, `PLAYER_PRIVATE`, `is_player_private`, `spelled_like`).
  Milestone 6's second part moved the Launcher's own copies of these to calls to `share`
  ("M6b decisions").

### M6b decisions

Decisions the second part of milestone 6 made where this document left moving images
between the Launcher and a core open. The code is `launcher-core/src/cores/hosting.rs`
("Host this"), `cores/world_download.rs` and `images/from_server.rs` ("Download this
world" and a core's `.lodeimg` in the Launcher), `images/export.rs` (`export_current`, the
import's request and preview), `core/src/images/share/outcome.rs` (the structured
creation outcome), `lodestone-ui/src/sharing.rs` (the shared summary and outcome text),
the Launcher's `servers/host.rs`, `servers/download_world.rs` and the snapshot import
dialog, and the control API's and CLI's `servers host` and `servers download-world`.

- **One flow each way, no `.mrpack`.** "Host this" no longer exports a `.mrpack`
  (`/modpack/upload`, `create_modpack`): the instance goes as a `.lodeimg` through the
  core's import, for Fabric, Forge, NeoForge and vanilla (Paper or vanilla, picked in the
  summary) instances alike. Nothing falls back to the old path: a core without the image
  routes is told to update. The window and the control API call the same functions. The
  Launcher's own `.mrpack` export (instances "Export as .mrpack") and the core's modpack
  creation for the Host and dashboard stay.
- **Asking the core first.** Before anything is written, `hosting::probe` sends
  `POST /instance/image/upload` with an empty multipart body: a core that makes servers
  from files refuses it as 400 ("No file was uploaded"), a user who may not create servers
  gets 403, and a core older than milestone 6 has no such route (404, or 405 if another
  route matches the path): "<core>'s Lodestone is too old to make servers from files.
  Update it on that computer, then try again." The probe holds one of the user's arriving
  uploads only while its empty body is read; a 409 (too many arriving) also says the core
  takes files. "Download this world" reads the export route's 404 as too old only when
  the core gave no message of its own (no such route answers an empty body), or a 405: a
  404 the core explains ("Backup not found" for a backup pruned or deleted meanwhile, a
  server gone) is shown as the core's message.
- **What "Host this" sends** (`Images::export_current`): under the instance's record lock,
  with collections held off to the end, refused while the game runs (its worlds would be
  read mid-write), the content is recorded as before a snapshot (a content image when it
  changed, kept by retention like any), and a data image of only the worlds picked
  (`saves/` and `saves/<picked>/…`: `scope::in_worlds` on top of the snapshot's filter)
  is built; when the content can't be recorded (a "Find it for me" search is saved) one
  full image of the content scope and those worlds goes instead. Nothing else of the
  player's is sent: not `options.txt`, `servers.dat`, `logs/`, `crash-reports/`,
  `screenshots/`, `usercache.json` or mods' own folders (minimaps' waypoints of other
  servers). It is written straight to a temporary `.lodeimg` in
  `<data>/cache/host-uploads/` (emptied at start with the other temporary folders,
  `DataDir::temporary_dirs`), after the free space for the carried objects and 1 GiB is
  checked. No manifest or snapshot record is saved, so the instance keeps nothing of it;
  the objects only it read (a world no snapshot recorded) are named by nothing, and a
  collection keeps unused objects for an hour (`store::gc::GRACE`), so however the export
  ends (sent, refused, cancelled) it asks for a collection a minute past that
  (`Images::request_gc_after_grace`: one waiting thread, the time moved on by a later
  export); a Launcher that quits first leaves them to the next collection. Private files are taken out as the
  snapshot export takes them out, and both images are labelled with the instance's name.
  A session that begins while it reads (Play pressed: the launch waits only 10 seconds
  for the record lock, then goes ahead) stops it at the next file, and a session seen
  after the read or the write refuses it (`ImageError::Running`, the file deleted), as a
  snapshot does, so a world the game writes is never sent half old, half new. The bar
  shows the read as its first half and the write as its second. Of `config/` and the
  other content folders only the private name list (`share::PLAYER_PRIVATE`) is taken out:
  the Launcher doesn't scan settings files for credentials before sending (best effort,
  as M5's export; the player sends their own files to a core they chose), so the file
  leaves the computer with those configs. The core's import then runs its own scan on
  every uploaded file, the Launcher's included, and withholds what it finds (listed in
  the outcome's `secrets`). Only a credential kept under a name neither the private list
  nor the scan recognises ends up in the server's folder, where every user of that core
  who may read its files can read it.
  It is uploaded as "<instance>.lodeimg", so the server's first image says "Created from
  <instance>.lodeimg". The worlds sent default to the last played; the server's world is
  picked in the summary among them, or none (a new world). A Quilt instance is refused
  before anything is written ("Lodestone can't make Quilt servers"), as the core would.
- **Nothing left behind.** The temporary file is deleted as soon as the core has
  answered the upload (on the upload's own thread: nothing reads it after, a refused
  creation is retried with the same upload id), so a world-sized file never waits in the
  cache while the summary is read. The core's upload belongs to an `Uploaded`, which lets
  go of it (`DELETE /instance/image/upload/{id}`, from a small thread) when dropped,
  unless a creation took the upload: backing out, Cancel, a refusal the person abandons
  and the dialog closing all drop it. Closing the dialog while the instance is written or
  sent (Escape, the close button, the window released) cancels that work at once, so the
  export and upload stop and leave nothing on either side. The temporary folders are
  renamed away at start to a name no run used before, and deleted in the background with
  any such folder an earlier run quit before deleting. A Launcher killed or quit
  mid-dialog (the upload is let go of from a detached thread, which the quit doesn't
  wait for) leaves the upload to the core's hourly prune, at most two per user. The
  probe's empty upload, were a core ever to keep one, is let go of the same way.
  The window keeps the owner key or the session from the password in memory only, for
  the dialog's life; neither is stored or logged, and the session isn't revoked at the
  end (as before).
- **The EULA and starting.** The summary's EULA toggle starts off and Create stays off
  until the person ticks it; the control API keeps `eula_required` (not for `preview`,
  which makes nothing). `create_from_image` never starts the server and nothing in the
  Launcher does. The tests check that no power route is called.
- **Following the creation.** The core answers a creation with the server's uuid; the
  Launcher reads `GET /events/all/buffer` filtered to progression events about once a
  second (`AdminClient::wait_progression`): it finds the start whose value names the new
  server, sums its updates for the bar, and ends at its end, also recognised by the end's
  value when the start was missed. The buffer holds the core's last 512 events, so on a
  busy core both can be lost: after 5 looks without a sign of the creation, the server
  being listed counts as made, without an outcome. After 30 minutes the dialog says the
  host is still setting it up, and the instance isn't linked. Once the creation was asked
  for, the dialog ends on that or on a failure (Close, or the Lodestone Host page), never
  back at the first step, where Continue would send the instance and make a second
  server. Cancel (or closing the
  dialog) stops only the waiting; the background task still links the instance if the
  Launcher runs. A WebSocket follower (`cores::live`) would be exact, but much more code
  for a rare case.
- **Permissions.** After the upload the Launcher reads the signed-in user
  (`GET /user/info`): the owner may change global files, a user only when given it. A
  file whose own jars need that permission then shows the summary's error and Create stays
  off for a user without it, as in the Host; the control API refuses before asking the
  core. A creation the core still refuses as forbidden shows the core's own reason
  (`HostError::Refused`), never "can't create servers", which is kept for the probe's 403.
- **The control API's `servers host`** follows the window: its job has the bytes written
  and sent as progress and `jobs.cancel` (the probe, the export and the upload stop, the
  temporary file and the core's upload go; once the creation was asked for, Cancel stops
  only the wait, and says the core goes on); the worlds are read away from the window; the
  server's name defaults to the instance's as the core takes it (`sharing::server_name`),
  and a name given is checked as the core checks it (at most 100 characters, no control
  characters or `NAME_FORBIDDEN`) before anything is written, by the same check the
  dialog uses (`admin::instances::name_problem`; the dialog says it translated, the
  control API in English); `no_world` (the CLI's
  `--no-world`) asks for a new world, and `world` is only ever a folder's name. The worlds
  are checked before anything is written (`hosting::worlds_to_send`): each folder named
  in `worlds` or `world` must be one of the instance's, spelt exactly as the export
  matches it (a wrongly-cased name is refused with a hint, never sent as no world, which
  would make the server a fresh one), and `world` must be among `worlds` when both are
  given. Without `worlds`, `world` alone is sent, nothing with `no_world`, or else the
  last played. `server_software` must be one of the summary's `server_choices`, matched
  whatever its case and sent as the core spells it; another is refused (`invalid_params`,
  naming the choices), the preview too, never quietly swapped for the file's own server.
  `servers download_world`'s job shows the download's bytes, then the import's, and
  checks that Modrinth can be reached before the download starts.
- **Linking.** The instance is linked to the new server only once its creation ended
  well (before, the link was saved as soon as the core answered); it replaces any earlier
  link, which the result names. A failed creation, or a wait that stopped first, leaves
  the old link. The window and the control API both decide it with
  `hosting::link_when_made`. An instance deleted while its server was made (the wait goes
  on for up to 30 minutes after the dialog closes) gets no link, which would make its
  folder again holding only the link: `Linked::InstanceGone`, which the dialog says ("The
  instance was deleted meanwhile, so nothing is linked to it") and the control API returns
  as `linked: false`. The link is saved only into a folder still there
  (`storage::save_in_existing_dir`), so a folder deleted between the check and the save
  isn't made again either (`NotFound` is `InstanceGone`). An older core's English end
  message is shown escaped (`display_text`), as every other text from a core, in the
  Launcher and the GPUI Host alike.
- **Structured creation outcome.** The `InstanceCreation` end value is now a
  `CreatedInstance`: the server's `InstanceInfo` fields (flattened, so the JSON and the
  dashboard's reading are unchanged) and an optional `outcome` (`CreationOutcome`:
  `not_downloaded`, `no_source`, `client_jars`, `secrets` and `settings_removed` as
  `NameList`s, and a count of `scripts`; boxed in Rust so every event stays small, the
  same on the wire), sent only for a server made from a file. The
  English message is built from the same data (`outcome::describe`), first 8 names of
  each list. The Host's toast and the Launcher translate it (`lodestone_ui::sharing::
  outcome_lines`); a core without it gives its English. ts-rs bindings are regenerated
  into `dashboard/src/bindings`.
- **Shared text.** What a file holds before a server is made and what the server didn't
  get are said by `lodestone_ui::sharing` for both apps (the summary lines and helpers
  moved out of the Host with their translations); each app lays the lines out with its
  own components. `NAME_FORBIDDEN` moved to `launcher-core` (`admin::instances`).
- **"Download this world"** (`cores::world_download`): a button on a server's page on a
  signed-in core. The export route needs ReadInstanceFile (as the backup zip download),
  which a Launcher's own key never has, so the dialog signs in with a password unless this
  computer is the core's owner. It lists the backups newest first, marking those taken
  while the server ran without a save as possibly caught mid-write, and offers "Back up
  now" (`create_backup`, AccessSetting, waited for by its task id's `BackupCompleted`).
  The export never includes plugins' settings. What the core says it took out of the file
  (`left_out`, and the secrets it `blanked`) is shown in the import dialog and returned
  by `servers.download_world` (`exported`, with `secrets_withheld` and `not_downloadable`). The control API's
  preview downloads the newest (or a named) backup to read it and never backs the
  server up: `backup: "new"` with `preview` is refused, so a preview changes nothing on
  the core. A preview costs a whole download (the CLI's help and SKILL.md say so), as a
  `servers.host` preview costs a whole upload. The core states the file's size before it
  is downloaded: when twice that (the file, and at least as much for what the import
  places while the file is still there) and 1 GiB to spare don't fit, it is refused and
  the core's file let go of before a byte comes, so the disk never fills up under a game
  saving in another instance and a download isn't waited through for an import that
  can't fit (the import checks the exact room again). The download is held to the stated
  size (`AdminClient::download_export`): a core sending more, or less, than it said is
  refused, the Host's backup export too. The file is streamed
  into `<data>/cache/world-downloads/` with a bar and Cancel, the core's file let go of
  however it ends, and handed to the snapshot import dialog (queued, never replacing a
  snapshot dialog that is taking, restoring or importing: it arrives on its own when the
  download ends, and closing that dialog would cancel its work); the file is deleted when its
  last holder lets go of it, always from a small thread (`Downloaded`'s drop), since the
  last holder can be the window's thread. Cancel ends the download at once, also while
  the core is yet to answer or a read waits, and a read that waits 30 seconds
  (`DOWNLOAD_IDLE_TIMEOUT`) fails, so a link gone dead mid-download (Wi-Fi dropped, a lid
  closed) neither holds Cancel nor waits for the 3-hour limit (`CoreClient::download_file`,
  for every download from a core: the Host's backup exports and files too). The dialog
  starts at "preparing the file" and says the server is being backed up only while "Back
  up now" runs. Cancel or a failed download goes back to the signed-in
  list of backups with why, so a password isn't asked again; when "Back up now" made a
  backup first, the list is read again and that backup picked
  (`DownloadProgress::backup_made`), so trying again doesn't make another. Escape or Close
  stops a download at once, as Host this does. A 403 says the user can't
  read the server's files; a 403 for "Back up now" says the user can't back the server up
  and should pick a backup, and the newest is picked, so Download doesn't repeat the
  refused request; on a server with no backups, Download is turned off and the dialog
  says to ask the server's owner to back it up. A Cancel while the import places the
  files is a stop (`ImageError::Cancelled`), not a failure. Of what the core says it left out, the import dialog shows
  only what a player would miss, which the export's answer lists itself
  (`ExportedBackup::left_out_for_players`, beside `left_out`: not the server software,
  the EULA, the JVM's crash files, the console history, a proxy's `forwarding.secret`,
  Lodestone's files, or anything the client view never carries by the shared crate's
  rules, such as `server.properties`, the operators and ban lists, Bukkit's settings files
  and `plugins/`; a `config/` file the owner didn't share with players can still be
  listed, as the export doesn't look up the content image's marks, though the instance
  wouldn't have taken it anyway): `left_out`'s first 64 names are mostly `libraries/`' files on a modded
  server and plugins' data on a Paper one (which "Download this world" always leaves
  out), so a Launcher filtering them would show nothing of what matters. The control API returns both. A file the
  export route sent that doesn't say a core made it is refused ("The host sent a file that
  isn't a server's backup"): labelled a Launcher's, it would skip every rule below. The
  import dialog's preview and the control API's preview refuse it the same way
  (`export::inspect_from_host`). A server's world carries
  every player's playerdata, stats and advancements: users who can read the server's
  files could already download the backup zip, so this exposes nothing new.
- **A core's `.lodeimg` in the Launcher** (`images::from_server::client_side`), replacing
  M6a's refusal: the content is the backup's non-world files over its content image's
  (the content image's when it has none), in the client view (`view(Client)`), then only
  what the Launcher's content scope holds (`scope::is_content`: a path the client view
  lets through that a server may never place on a player's machine is left out, never
  placed); the data image takes the world files without `share::is_server_world_file`'s
  (Paper's per-world `paper-world.yml`, the split Nether's and End's own files; the
  dimensions are already `DIM-1`/`DIM1`) and a world's permissions data (FTB Ranks'
  `serverconfig/ftbranks/`, ForgeEssentials' `FEData/`, which name the server's players
  and their ranks, as the core's import counts them among the operators), the worlds, the
  withheld files under `saves/` and no server settings. The content is the union of the
  backup's files and its content image's, the backup's copy winning: a file the content
  image still lists that was deleted outside Lodestone while the server ran (which the
  core records only at the next start's check) comes back in the downloaded instance, as
  it does in a server made from the core's own export. The core's export copies a content
  entry's sides and owner mark into the backup's only when their bytes match, so a newer
  copy in the backup (a config the server rewrote on load, a jar replaced on disk) would
  come with unknown sides; the content image's `env` and `env_from` carry over to it
  (`from_server::with_content_sides`: the owner's mark belongs to the path, and a jar's
  sides are the best guess for its newer copy), so a config marked "For players" comes and
  a server-only mod doesn't, as for a player joining through M4b. Forge versions become full Maven versions, as for players.
  Both images are made and sealed by this Launcher, labelled "Downloaded from <server>"
  (the server's name, or for a file opened by hand the part of the core's label before
  " – "), and the import names the instance after the server, without control or
  invisible characters (`export::shown_name`, applied to any name an import is given,
  since it may be a core's). Who made a file is written
  by its sender, so a file opened by hand that holds a server's files (a top-level
  `server.properties` or `plugins/`, or server settings) is taken as a core's whatever it
  says (`from_server::holds_server_files`). The preview's
  `from_server` lists the server-only mods left out and how many of the file's files
  aren't taken. M4b's rules for players hold: Modrinth files downloaded and checked, a
  failure left out and listed. Server configs and kubejs/ not marked "For players" don't
  come (singleplayer may then differ from the server); Forge or NeoForge server-only mods
  whose sides are unknown come, as they do for players.
- **The modpack's client share.** A server built on a Modrinth pack lists every file it
  has in its content images, so the client view holds every pack file a player needs that
  the server also runs; the pack's client-only index files and `client-overrides/`, which
  a player of the server installs from the base, are placed too
  (`from_server::place_pack_share`): the pack comes from Modrinth through M4b's
  `image_sync::base::base_pack` (the version must be the named project's and the file the
  base's SHA-512, kept in `<data>/cache/server-packs/`), and every file of its client
  share is placed that the images don't place at its path (compared in any letter case),
  that the server didn't remove (the client view's `removed`) and that isn't under
  `saves/` (the server's world is the one that comes), each checked by the pack's hashes
  (`modpack::fetch::stage`, staged in the instance's staging folder and then moved). The
  server's own copy of a file always wins over the pack's. Downloaded mods are recorded in
  `content.json` as a pack install records them, and the snapshot kept of what was sent
  links to the content recorded once they are placed. When the pack can't be had
  (offline, gone from Modrinth, not the file the base names, a CurseForge pack, which the
  Launcher can't install from a server as for players) or one of its files can't be
  placed, none of it is placed, the instance is made without those files and the import
  says why (`Imported::server_pack`: `Skipped { why, reason }`, `why` one of `gone`,
  `curseforge` or `failed`, which the window's notice says in its language, and `reason`
  in English for the control API); only cancelling stops the import. The preview says which (`FromServer::modpack`),
  and for a Modrinth pack warns, as for the images' own mods, that its files may include
  mods that run when the game starts (no count of the images' mods names them). The
  images still claim no pack
  (`base` and `removed` cleared) and the instance has no `modpack.json`: the copy follows
  neither the server nor the pack's updates. The import's free-space check can't count the
  pack's share, whose size is known only once the pack is read, so the share is checked on
  its own before it is downloaded: one that doesn't fit with 1 GiB to spare is skipped
  (`failed`) and the instance is made without it.
- **A failed import leaves nothing named.** An import (a Launcher's file or a core's)
  that fails or is cancelled once it began unpacking (Cancel during the mods' downloads,
  a full disk while placing files) makes no instance, and the objects it put in the store
  are named by no manifest: it asks for a collection past the grace
  (`Images::request_gc_after_grace`), as "Host this" does, so a whole server's world
  doesn't stay in the store for days; so does an import whose snapshot couldn't be kept.
- **The import trusts only the file.** Both Launcher imports (a Launcher's file and a
  core's) unpack only the objects the placed images name, each read from the file and
  checked even when the store has it (`Container::import_objects_where_verified`), and a
  file no download confirmed is placed only from bytes the file carries: before, M5
  placed it from the store when the store happened to have those bytes, so a file could
  name the user's own stored files by hash. An offline import of a pack whose mods the
  store already has now leaves those mods out (listed) instead.
- **The shared crate changed only additively**: `layout::BUKKIT_NETHER` and
  `layout::BUKKIT_END` are public, and `share::is_server_world_file` is new. The Launcher
  calls `share` instead of its copies (private patterns, `strip`, placed entries, folder
  spellings, the version check, the download cap, the crash and account names; its
  snapshot scope folds Unicode case for crash names now, so it leaves out slightly more
  exotic spellings). `names_modrinth_file` and `download_key` stay in both programs: the
  shared crate has no URL parser, and adding one would be a new dependency.
- **Not in scope.** CurseForge downloads in the core (the developer decided they stay
  out, "M6a decisions", Making it; the summary keeps warning). Launcher to Launcher needs nothing new: M5's `.lodeimg`
  export and import already do it, and `LDS2:` codes stay as they are.

### WB1 decisions

Decisions the world backups milestone on the Host made where "World backups" left them
open. The code is `core/src/backups/world/` (`store.rs`, `writer.rs`, `reader.rs`,
`service.rs`), `core/src/handlers/world_backups.rs`, `restore::stage_folders`,
`launcher-core/src/cores/admin/world_backups.rs` and the Host's `pages/backups.rs` with
`pages/backups/world.rs`.

- **One job lock per server.** World backups and restores take the snapshots' job mutex
  (`BackupService::take_guards`, `lock_stopped`), so a snapshot (manual or scheduled)
  waits for a world job and the reverse; a scheduled snapshot skipped meanwhile is tried
  again by the scheduler. Both lists' `running` report whichever job holds it, with the
  new `JobKind`s `world_backup` and `world_restore`. Deleting a world backup is refused
  (409) while a world job of that server runs.
- **Sidecars, not a list.** Each backup has its own `<id>.json` written after its zip, so a
  crash leaves at worst a whole zip with no sidecar (adopted as a manual backup named by
  nothing, its level from its top folder) or a sidecar with no zip (dropped). A sidecar that
  can't be read is an error, never "no backups". Ids are `YYYYMMDD-HHMMSS-xxxx`, checked
  exactly, since they name files.
- **What the zip leaves out.** Links, unreadable files and names the server layout can't
  place are counted (`left_out`) and the Host shows the count; `session.lock` and
  `.lodestone-*` are skipped silently, as images do. So are fifos, sockets and devices,
  which are never opened (a fifo's open would wait forever; files are opened without
  following links and without blocking, then checked to be plain files through the
  handle). Of two canonical paths that are one file on a case-insensitive disk (possible
  on Linux), the later in the walk's name order is left out, a folder with all it holds
  (`lodestone_image::paths::CaseTree`), since the reader refuses such pairs: a backup made
  on a case-sensitive disk still restores, and a restore whose safety backup would drop
  one is refused like any other left-out path. Region files and other compressed
  formats (`.mca`, `.mcr`, `.mcc`, `.dat`, `.dat_old`, `.nbt`, `.png`, `.zip`, `.gz`,
  `.jar`) are Stored, everything else Deflated, which keeps the save-off window short.
- **Restore mapping.** The zip's paths map through canonical paths
  (`saves/<level>/…`) into the server's current `level-name` and split, rewriting the level
  name, so a vanilla backup restores into a Paper server (its `DIM-1` into
  `<level>_nether/DIM-1`), a Paper backup into a vanilla server (its split folders' own files
  under `.bukkit_nether/` and `.bukkit_the_end/`), and a backup into a renamed level. The
  restore owns the current level, and on a split server its `_nether` and `_the_end`; one
  the zip lacks is moved aside and removed (the safety backup has it).
- **Safety backup.** Taken whenever any folder the restore replaces exists: the level
  (with or without a `level.dat`, so a damaged world is kept too) or, on a split server,
  its `_nether` and `_the_end` even when the level is gone (the zip then holds just those,
  and adopting such a zip takes their level). A `before_restore` world backup named
  "Before restoring “<label>”" cut to 100 characters; its failure refuses the restore.
  None when the server has none of the world's folders.
- **The safety backup holds everything the swap deletes.** The swap replaces whole folders,
  so what the safety zip can't hold (links, files Lodestone can't read, names a restore
  can't place, such as a vanilla `DIM-1` inside a Paper level) would be lost. A restore is
  refused (409) when the safety backup would leave anything out, naming the first five
  paths: checked from the walk before anything is written, and again by the safety backup
  itself (a file that can't be read as it is copied), which then leaves nothing. There is
  no override; the operator moves those paths out or fixes their permissions. Snapshots
  refuse likewise ("Couldn't include … in the safety snapshot"). What the restore
  replaces is listed for the permission check only when the requester can't change
  global files or code (so never for the owner), after the safety plan has bounded the
  world, and never past the reader's entry limit.
- **Within what a restore reads.** A world over the reader's bounds (64 GB uncompressed,
  1,000,000 files and folders) is refused before anything is reserved or paused (400,
  "Take a snapshot instead"), and a world that grows past them while it is copied stops
  the backup. A restore over such a world is refused too, since its safety backup
  couldn't be made.
- **Free space.** No time limit between world backups, and no cap on their count or total
  size: they are manual, and the large writes' 1 GB free-space floor is the only bound.
  Making one reserves the world's uncompressed size (an upper bound) on the world backups'
  disk among the core's large writes (`LargeWrites`), with the 1 GB margin. A restore
  first checks room for the safety backup and the unpacked world together (added up when
  `world-backups/` and the server are on one disk, `util::disk::same_disk`), then the
  steps reserve their own space as they write.
- **Download.** The link (`global_fs::register_stored_download`) points at the stored zip
  with a display name (`<server>-<level>-<id>.zip`), holds no copy and takes no backup-file
  place; `DELETE /file/{key}` only forgets it, and the zip is never deleted with the link.
  It re-checks ReadInstanceFile when used, like every link.
- **Cancel.** `…/tasks/{task_id}/cancel` cancels a world job until its swap begins (409
  after); snapshot jobs have no cancel. A cancelled backup removes its `.part` file (also
  one the save pause's 15-minute cap stopped as it finished), a cancelled restore its
  staging folder. The Host shows Cancel only to those with the permission that started
  the job, turns a world backup's Delete off while a world job runs, and its restore
  dialog's Restore while any job runs. A world job leaves the service however its task
  ends (a drop guard that removes only its own entries, by task id, since it may run after
  the next job took the lock), and a restore of a server whose last restore couldn't be
  undone says to restart Lodestone rather than to stop the server. Only a server in the
  Stopped state is restored into (`lock_stopped` refuses every other, Error included).
- **Names.** Two backups of a server finishing in the same second with the same random
  suffix never replace each other: under the record lock the final name is checked free
  and another id drawn if not. A `<id>.zip` that is a link is neither listed nor loaded.
  Errors that reach clients name a backup by id, never by its path. Zips without a sidecar
  that can't be adopted are skipped until the core restarts rather than read on every
  list. A zip that took a file out again at its end is cut to its central directory.
- **Deleting a server** deletes `world-backups/<uuid>/` beside its snapshots, while the job
  lock is held.
- **Startup.** The sweep of part files and temporary sidecars runs after the snapshots'
  recovery and is awaited before the core serves requests, so it can never remove the part
  file of a backup started meanwhile.
- **Snapshot wording.** The core's snapshot messages (progress, refusals, free-space
  reasons, completions) and the Host's text say "snapshot" ("Taking a snapshot of …",
  "Saving the snapshot", "This snapshot doesn't know …", "Restored snapshot “…”"); the
  texts world backups share with snapshots are neutral ("Names can be at most …", "The
  server stopped while it was being copied"). Routes, types, message keys and log lines
  keep their backup names.
- **Renaming** is a route (`PATCH …/world-backups/{id}`) for API clients only: the Host
  renames neither snapshots nor world backups, and its admin client has no call for it.
- **Not in scope.** Uploading or importing a world zip from elsewhere, scheduled world
  backups, pruning, a cancel for snapshot jobs. `crates/lodestone-image` only gained
  `paths::CaseTree` and `store::flush` (`write_out`, `write_out_dir`, `full_flush`, the
  store's batched flushing made public; additive).
- **Flushing a restore.** Unpacking a world backup and writing a snapshot's files for a
  restore flush in one batch, as materializing does: each file and folder is handed to the
  drive (`store::flush`), and the staging folder gets one full flush before the swap (on
  Apple platforms a plain `fsync(2)` per file and one `F_FULLFSYNC`, on Linux one
  `syncfs(2)`), not a full flush per file, which took minutes for a world of many files.
  Only on unix: on Windows a folder can't be flushed, so the swap can't be made durable
  either way.
- **Late cancels.** A world backup closes its cancel (under the job's lock, as a restore
  does before its swap) when the copy is done and it starts saving, so a cancel from then
  on gets 409 rather than a success for a backup that is then listed. A restore registers
  its job as soon as it holds the job lock and the stopped lock, before it plans: the file
  API's refusal during a restore and the refusal to delete a world backup while a world job
  runs hold while it plans; its Start event, and so the list's `running`, wait until the
  restore is checked, so a refusal is only the route's response.
- **Adopted zips.** A zip found without its sidecar (a crash between the zip's rename and
  the sidecar's write) whose only top folder is named like a split folder (`…_nether`,
  `…_the_end`) and has no `region` folder of its own is a split server's dimension, so its
  level is the name's prefix; with a `region` folder it is a vanilla level of that name.
- **Zip times.** A file's time is written in the zip's DOS fields in the core's local time,
  as other zip tools read them, and read back the same way (only for show).
- **Fewer walks.** The plan a request makes (a backup's check, a restore's safety plan) is
  the one the job writes from, so a stopped world is walked once to plan and once to copy;
  the copy's own walk is what takes region files a running server creates during its save
  pause. Entries between 3 and 4 GiB get zip64's large file fields, so a file that grows
  past 4 GiB while copied doesn't fail the backup. Downloads stream in 256 KiB reads.

### WB2 decisions

Decisions the Launcher's world backups ("World backups", In the Launcher) made where the
developer's brief left them open. The developer decided: world backups are manual only
(no schedule, no pruning), plus the safety backup before a restore; the Worlds tab's
"Back up" makes one; "Include mods and configs" goes.

- **Layout and ids.** `<data>/world-backups/<instance id>/<world folder>/<id>.zip` with
  `<id>.json` beside it (`DataDir::world_backups_dir`); ids are snapshot record ids
  (`images::snapshots::new_record_id`, `valid_record_id`), checked before any path join,
  and a world folder must be a plain name (`content::is_plain_name`). A `.json` over
  64 KiB, of another version, or naming another id is ignored; the folder it is in wins
  over the folder it names.
- **Ownership by (instance id, `created`)** rather than M5's retiring and taking back:
  nothing is renamed when an instance is deleted, put back from the Trash or its id is
  reused, and a crash can't leave backups half moved. Entries of the instances folder
  that aren't readable instances are never cleaned.
- **World identity is the folder.** A world renamed in the Launcher takes its backups
  along (`WorldBackups::rename_world`), so a world later given the old name never inherits
  them. Every world the Launcher places itself (a rename, an import, a restore as a new
  world from a world backup or a snapshot) takes a folder name free in both `saves/` and
  the instance's world backups folder (`worlds::unused_in`,
  `WorldBackups::folders_of`), so it never takes over a deleted world's backups. A world renamed or deleted outside the Launcher leaves its backups under the old
  folder, which Settings lists as a world no longer in the instance, and which the control
  API's `world` finds by the backups' folder or world name; restoring one in place
  recreates the old folder.
- **The zip** has one top folder, the world's; `session.lock` and `.lodestone-…` names are
  never in it; links and names a canonical path refuses are left out, counted and listed
  in the `.json` and the row's details ("N files left out"). Restoring in place carries
  such files on disk back into the restored world.
- **Locking.** Making, restoring, deleting, the startup sweep and Storage's clean-up run
  under the instance's record lock in a counted write of the image store
  (`Images::with_instance_locked`, `with_restore_locked`, sharing `restore()`'s start), so
  they never overlap a snapshot, a restore or a record, and a data move waits for them or
  refuses them (`WorldBackupError::Suspended`). Making one is refused while a session runs
  or the game has that world open, and stopped when a session begins or the world opens;
  restoring in place is refused while any world is open (as M5's restore, since the
  safety backup and the swap must not race the game), checked again just before the
  swap. Making one is refused while any game session of the instance runs, not only
  while that world is open, deliberately stricter than the brief: a world the running
  game hasn't opened yet could be opened mid-zip, and a session is what the record lock
  and Play already coordinate on. Restoring as a new world isn't
  refused by the core while the game runs (it replaces nothing, as `restore_world_copy`),
  but the dialog and the control API refuse it. The dialogs and the control API also
  refuse making or restoring one while another job writes the
  instance's files (`files_or_pack_busy`: an install, a modpack update, a window job),
  as Take a snapshot does: a backup could zip a file mid-write, a swap would lose what
  the job writes, and the work would otherwise wait for the record lock with Cancel
  doing nothing. The dialog refuses deleting one then too, since the delete would sit
  waiting for that lock, as long as the job runs; its progress line names the backup
  being moved to the Trash. The control API's delete waits instead (a script can). Restoring in place is refused when
  `saves/` or a folder above it is a link (shared saves, another drive): the swap could
  rename through it, but its rollback refuses to (it could move files out of somewhere
  else), so a failed or crashed swap could never be undone and would leave the instance
  unplayable. The refusal comes before the safety backup and suggests Restore as a new
  world. The swap itself now refuses a link above any owned path
  (`Staged::with_folders`, `swap::no_links_above`), so snapshot restores get the same
  protection, as does a carried path inside a folder where the restore places a file of
  that name (found before anything is staged, not by a failed rename).
- **Deleting** moves the zip to the Trash; Storage's clean-up of deleted instances'
  backups deletes them for good. There is no rename, export (the zip itself is the
  export, Show in Finder), schedule or pruning. Safety backups are kept like any other
  and named "Before restoring “<name>”" (plain English data, like content labels), the
  restored name cut so the whole name fits 200 characters. A restore that leaves the
  world unchanged (refused after the safety backup, cancelled while unpacking, rolled
  back) deletes its safety backup, since each is a full-size zip that nothing prunes;
  the swap's own checks run before the safety backup from the list it will zip
  (`worlds::zip::plan`), so most refusals come before it is made.
- **A world without `level.dat`.** A manual backup of it is refused (no restore could take
  it). The safety backup before a restore is made anyway (the world may be broken, the
  reason for restoring) and keeps the files the restore replaces; such a zip can't be
  restored by Lodestone but opens with any zip tool.
- **Names in the window.** The `.json` keeps the English names the Launcher gives
  (`labels::WORLD_BACKUP`, `labels::before_restoring`), which the control API and CLI
  show. The window shows them in the user's language (`world_backups::shown_name`): a name
  equal to "World backup" as `WorldBackupDefaultName`, and a `before_restore` backup whose
  name is still `before_restoring(restored)` as `WorldBackupBeforeRestoring` with the
  restored backup's name (itself translated when it is the default). A name the user typed
  is shown as it is.
- **Folder names a restore wouldn't take** are refused before zipping
  (`worlds::zip::check_world_folder`: `check_pack_path` on `<folder>/` and
  `CanonicalPath::new("saves/<folder>")`), rather than made into a backup every restore
  of which fails. Names inside the world that the zip check refuses are left out like
  other names no disk keeps, and a folder holding nothing else is left out with them.
- **One zip checker** for world imports and world backups (`worlds::zip::check` and
  `extract_into`, replacing `inspect_zip`, `extract`, `extract_mapped`, `plan_extract` and
  `check_collisions`): canonical-path names and the shared crate's case folding instead of
  `to_lowercase`, encrypted entries refused, and a free-space check (stated size and 1 GiB)
  before an import too. A third-party world zip with such a name is now refused, with a
  message naming it, where it used to import.
- **Free space.** Making one: the world's size and 1 GiB on the backups' disk. Restoring
  in place: when the instance folder and the backups folder are on one disk (the same
  device on Unix, the same volume elsewhere; taken as one when that can't be told), the
  zip's stated unpacked size plus the world's size plus 1 GiB, once; otherwise the stated
  size and 1 GiB on the instance's disk and the world's size and 1 GiB on the backups'.
  All before the safety backup. As a new world: the stated size and 1 GiB.
- **Zip contents and speed.** Files already compressed (region files, gzip NBT, mods'
  SQLite databases such as Distant Horizons' `DistantHorizons.sqlite` of chunks it
  compressed itself, images, sound, archives) are stored, and so is any file of 64 MiB or
  more whatever its name (a large file in a world is region-like data or a database, never
  text); the rest is deflated. Deflating such files gains nothing and runs on one core
  (about 60 to 70 MB/s: 8 s for 512 MiB, against 0.2 s stored), so a world with a 5 GiB
  database would spend over a minute on every backup, and before every in-place restore,
  whose safety backup runs first. Cancel is asked
  between 8 MiB chunks of a large file too. Files are opened without following links and
  must be plain files. Unpacking makes its files durable as one batch
  (`lodestone_image::store::flush`) rather than with a drive cache flush per file
  (`F_FULLFSYNC` on macOS, about 6 ms each), which also speeds up world imports.
- **Crashes.** Every `.part` and every temp file of a `.json` being written
  (`storage::TEMP_PREFIX`) is deleted by the startup sweep (it runs under the record
  lock, which every zip is written under, so none is being written; the Launcher runs once
  per data folder), and so is the instance's own restore-as-a-new-world staging folder in
  `saves/`; the sweep covers every live instance, with world backups or not. A `.json`
  without its zip (a crash during a delete) is deleted; a zip without its `.json` (a crash
  between them: the user's copy) is kept as a manual backup named "World backup" of its
  live instance, with what its `level.dat` says (nothing when it has none, as a safety
  backup of a broken world may: `Shape::OneFolder`, so it can be listed and deleted),
  unless the zip is older than that instance (a deleted instance whose id was reused made
  it): such a zip is left alone. A restore's crash is M5's journal's.
- **Snapshots cover the whole folder.** "Include mods and configs" (`Take::full`, the
  dialog's checkbox, `snapshots.create`'s `everything`, `--everything`) and the row's
  "With mods" are gone; a snapshot is still a full image when its content can't be linked
  (`SnapshotRecord::full` stays). The section's hint says "Copies of the whole instance
  folder to go back to or share."
- **Ids and order.** A new id is unique among all of the instance's worlds' backups
  (whichever instance made them), so an id alone finds one backup, and its suffix counts
  up from the highest of the same second (the second's first is random below `8000`), so
  newest first (by `created`, then id) and the API's `latest` never take the older of two
  backups made in one second. The last id handed out per instance is remembered while
  the Launcher runs and counts as taken, so a backup made in the second of one just
  deleted never reuses its id.
- **Renaming a world moves only this instance's backups.** A deleted instance that had
  the same id keeps its backups under the old folder name, as its world was named; the
  folder is moved in one rename when it holds only this instance's backups, else file by
  file (zip, then `.json`), each put back if one fails.
- **Names differing only in case.** A world on a disk that keeps case apart can hold
  `region/` and `Region/`; the zip leaves out the later name in sorted order (counted and
  listed), since `check` refuses such a zip and it could never be restored.
- **Restoring as a new world** unpacks into
  `saves/.lodestone-restore-staging-<instance id>-<16>/` (a name world lists and snapshots
  skip), so placing it is a rename on one disk even when `saves` is a link to another
  drive; the instance's id in it keeps another instance sharing that `saves/` from
  sweeping it mid-restore.
- **Speed.** Unpacking copies with a 1 MiB buffer (std's copy moves 8 KiB per call,
  about 2.5 times slower on stored region files) and asks Cancel and moves the bar every
  8 MiB of a large file. Making one looks at the world's `session.lock` at most every
  250 ms, not per file. Opening a zip reads only its end records and central directory:
  the archive is taken to start at byte 0 (`ArchiveOffset::Known(0)`), since `zip`'s
  default scans the whole file from its first byte to find the start (about 4 s for a
  5 GiB backup from cache, minutes from a hard drive, with no progress or Cancel). Only a
  world import falls back to that scan, for a zip with something before it (a
  self-extracting stub), and only when the file doesn't start with a zip record (a local
  header, or an empty zip's end record): a damaged zip that starts like one is refused
  without reading it through. A world backup with something before it is refused. `check`
  lists entries from the central directory alone (`by_index_data`), reading no local
  header. Files of 2 GiB or more are written with zip64 sizes: a deflated file just
  under 4 GiB that doesn't compress would otherwise overflow its 32-bit sizes and fail
  every backup of its world. Storage's clean-up of deleted instances' backups takes an
  instance's lock only when it has some, so a long job of a live instance doesn't hold
  it up. Storage sizes the world backups folder once,
  in its scan (`WorldBackups::usage` leaves `bytes` to it).
- **Window details.** Back up a world's picker, progress and result name a world by its
  name, with its folder when another world has the same name ("New World (New World
  (1))"). A world's label in Settings shows only with its card, so a search never leaves a
  label without rows. The Worlds tab's world backup rows say "N files left out" as
  Settings' do, and Delete there waits only for the tab's job, not for the game to stop
  (deleting a zip doesn't touch the world). The restore confirmation's wording (whether
  the world is there) comes from the list's background read, not a look at the disk.
  Its done message names the safety backup ("The world as it was is in the world backup
  “…”"), as the Worlds tab's snapshot restore names its safety snapshot; its title,
  "Restore <world>", is not a question, as it stays over that message (as Restore
  <snapshot> does). When the Worlds tab refuses a job because another one writes the
  instance's files, it says so ("Wait for the instance's files to finish changing"), as
  the dialog does, rather than to wait for a task it doesn't show.
  `worlds.delete`, `rename` and the other world changes in the control API are refused
  while a world backup job of the instance runs or the window's jobs write its files.
- **Words.** "World backup" is the zip and "snapshot" the image everywhere in the
  Launcher: the Worlds tab's button that opens a world's panel is "Backups & snapshots"
  (counting its world backups and the snapshots holding it), the after-play hints say a
  snapshot copies the whole instance folder, the Worlds tab's snapshot rows say
  "Restore…" and "Restore as a new world" as its world backup rows do, the confirmation
  "Restore <world> from this snapshot?" (a snapshot of the instance is taken first) with
  "Snapshot and restore", and the job "Restoring the world from the snapshot". A modpack
  update's replaced files are "kept inside the instance's folder, in backups/", never
  "the instance's backups". Storage's row says how many world backups are deleted
  instances' only when some are. `labels::backed_up` ("Backed up <world>") is gone.
- **API and CLI names.** `world_backups.list`, `create`, `restore`, `delete`; job kind
  `world_backups`; `worlds.backups` became `worlds.snapshots`, `worlds.restore` became
  `worlds.restore_snapshot` (mode `new` or `replace`, as `world_backups.restore`; both
  default to `new` and both are destructive in `meta.methods`), `worlds.backup` is gone, and
  `worlds.list`'s `backups` became `world_backups` and `snapshots`. The CLI follows
  (`world-backups …`, `worlds snapshots`, `worlds restore-snapshot`). Storage's clean-up is
  `deleted-world-backups`; `storage.show` has `world_backups` (`bytes`, `backups`,
  `deleted`, `deleted_bytes`), and the world backups folder isn't "other".
- **Test hook.** `WorldBackups::before_swap` (tests only) runs just before an in-place
  restore's swap, to write into the world meanwhile and check what is kept aside.
- **Only within the limits a restore reads.** A world backup is made only when the zip it
  would write passes the zip check a restore runs first: at most 500,000 entries (the top
  folder, every folder and every file) and 64 GiB of files, compared with what
  `worlds::zip::plan` lists (`ZipPlan::within`) before anything is written. Otherwise the
  backup would be listed like any other and refused the day it is needed. A larger world
  is refused with a message suggesting a snapshot. An in-place restore over a world past
  those limits is refused before anything changes (its safety backup is checked the same
  way and could never be restored), naming restore as a new world, which needs no safety
  backup. Restores check zips against the same limits (`WorldBackups::limits`, smaller in
  tests through `fake_limits`).
- **The shared crate** gains only `lodestone_image::store::flush` (`write_out`,
  `write_out_dir`, `full_flush`: public wrappers of the store's batch flush), an additive
  change. The Host's world backups ("WB1 decisions") added the same module, word for word,
  so the two apps share one.
