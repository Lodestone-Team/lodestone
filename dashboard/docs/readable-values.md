# Readable names and values

The dashboard (the Tauri Host's web UI) follows the same names-and-values design
as the GPUI Host and the Launcher: the object an action is about leads, each
value has one role style, facts are keyed, lists come out of sentences and
status shows as a badge. This file lists every place in `dashboard/src` that had
the problem, what it became, and the components to use for new UI.

## Rules

- **The object is never inside the sentence.** Dialog titles name the action and
  the kind ("Delete server?", "Revoke API key?"); the object is an `ObjectCard`
  under the title. Toasts lead with an `ObjectChip`, then a short
  placeholder-free outcome ("Deleted", "Couldn't update"), then the reason.
- **One role, one style.** Object: `font-mediumbold text-gray-300`. Mod, plugin
  and modpack versions: `VersionTag`. Numbers: `Num` (tabular figures, the unit
  lighter). Exact text (file names, paths, addresses, ids, key prefixes):
  `MonoText` / `MonoValue` / `ValueField`. Time: `text-ink-muted`. Keys:
  `text-small text-ink-muted`. Status: `StatusBadge`.
- **The game and its loader are one plain phrase**: "Minecraft 1.21.1",
  "Fabric 0.16.9" (`gamePhrase`, `loaderPhrase`, `instanceLoaderPhrase` in
  `utils/values.ts`). In rows they stand alone; in key-over-value strips and
  fact tables the key is "Game" or "Loader". They never use a `VersionTag`.
- **Facts are flex children**, never joined with " · ". Rows use a `FactLine`,
  headers a `FactStrip`, dialogs and panels a `FactList`. Lists where every row
  has the same facts use right-aligned columns (`ListHeader`, `ListCell`).
- **Lists leave sentences**: a `NameList` with a heading and a `CountBadge`,
  5 rows, then a counted "Show N more". Never an uncounted "…".
- **Counts are a `CountBadge`**, never "(7)". Kinds are a `KindIcon`, never
  "(folder)".
- **Sentences that still need a value** use `rich()` with element params
  (`MonoValue`, `ObjectChip`, links), so translators keep their word order.

## Components

Atoms are in `src/components/Atoms/Values/`, molecules in
`src/components/Values/`; both have a Storybook story each (`Values/…`). The
converted places can be seen in the stories of `InstalledMods`,
`ClientPackPanel`, `ModpackPanel`, `FileToast`, `ContextMenuButton`,
`NotificationCard`, `InstanceCard`, `InstanceLoadingCard`,
`InstanceLoadingPill`, `UserBox`, `ConfirmDialog`, `ClipboardTextfield` and
`ApiKeysSection`; the rest need a running core (`npm run dev`).

| Component | Use it for |
| --- | --- |
| `Num`, `Bytes`, `Fraction` | Any number: tabular figures in gray-400, unit in ink-muted, printed once after the total ("1.2 / 3.4 MB"); `–` when unknown. |
| `MonoText` | A file name, path, address or id. `truncate="middle"` keeps the extension, `"path"` shows the home folder as `~` and keeps the last two parts. The full value is in the title and in screen-reader text. |
| `MonoValue` | A short exact value inside a sentence (`rich()` param). |
| `VersionTag` | Mod, plugin and modpack versions only. `tone="from"` / `"to"` in a `DiffValue`. Long versions are middle-truncated. |
| `StatusBadge`, `stateTone` | State (running, stopped, expired, update available). Colour means status and nothing else. |
| `CountBadge` | A count after a heading, tab or button label. Compact figures, exact value in the title. |
| `KindIcon` | The kind of an object (folder, file, archive, mod…), with the kind as screen-reader text. |
| `ObjectChip` | An object in a toast, notification or fact value: icon + name, truncated with a title. |
| `MeterBar` | A 4px meter (`role="meter"`). |
| `Fact` | One keyed value: a visible key, or an icon with the key as screen-reader text. The accessible text is "key: value". |
| `FactLine` | Line 2 of a list row. Facts in priority order; the ones that don't fit wrap onto a hidden second line, and the row's title lists them all ("key: value" per line). |
| `FactStrip` | Facts under a page or card title: key over value, dividers, wraps. |
| `FactList` | Facts in a dialog or panel: a two-column key/value grid; `framed` puts it on a card. |
| `DiffValue` | A from → to change; the arrow is an icon (mirrored in RTL) with "to" as screen-reader text. |
| `CompareTable` | Two values that must match (core vs dashboard version, this version vs this server); mismatched cells are yellow. |
| `ObjectCard` | The object of a dialog, under its title: icon, name, version, facts, badge. |
| `NameList` | Lists of names: groups with a heading + count, per-item reasons, 5 rows then "Show N more". |
| `ValueField` | A long value the person copies: path, URL, code or secret, with Copy (and Open for URLs, Show/Hide for secrets). |
| `StatTile`, `StatRow` | The single number a reading depends on (CPU load, memory used). |
| `ListHeader`, `ListCell` | Right-aligned Size and date columns; they hide below the `@2xl` container width, where the row's `FactLine` shows the same facts. |
| `ObjectToast` | Toast layout: object chip and status, outcome, reason, then a `NameList` or counts. |
| `ConfirmDialog` `object` | The slot under a dialog title for an `ObjectCard`. |

## Token mapping

The dashboard's ink tokens are alpha-based, so the design's roles map onto the
dashboard's own classes (and `src/light-overrides.css` keeps Daylight right):

| Role | Class |
| --- | --- |
| Object, name, value | `text-gray-300 font-mediumbold` |
| Number | `text-gray-400 tabular-nums` |
| Unit, time | `text-ink-muted` |
| Key | `text-small font-medium text-ink-muted` (the design's ink-faint; the dashboard's `ink-faint` is white/30 and fails contrast) |
| Mono | `font-mono` |
| Status | `bg-pill-blue text-blue-150`, `bg-pill-green text-green`, `bg-pill-yellow text-yellow`, `bg-pill-red text-red-200`, neutral `bg-gray-700 text-gray-400` |
| Lines | `border-line` |

## Inventory

Paths are relative to `dashboard/src`; line numbers are from main at 458e113f,
before this change. Status says whether the place now follows the design
(Converted) or was kept on purpose, and why. Patterns: name-version, row-meta, object-in-sentence,
object-in-title, list-in-prose, inline-values, facts-as-prose, comparison-prose,
header-facts, suffix-kinds, status-values, unlabeled-stat.

### Mods

| # | Where | Was | Pattern | Now | Status |
| --- | --- | --- | --- | --- | --- |
| 1 | `components/Mods/InstalledMods.tsx:176-184`, `:219-231` | "Sodium mc1.21.1-0.6.0-fabric"; "Update to {version}" | name-version | Name, then a `VersionTag`; with an update, a `DiffValue` of two tags and a plain "Update" button. | Converted |
| 2 | `InstalledMods.tsx:204-215` | "fabric-api-0.116.17+1.21.1.jar · 2.5 MB", "since Nov 3, 2026" | row-meta | `FactLine` with the mono file name (middle-truncated); Size and Added are right-aligned columns under a `ListHeader`, at the end of the row so they line up whatever buttons a row has, folded into the line on narrow widths. Added's title is "Came with: {entry}". | Converted |
| 3 | `InstalledMods.tsx:85-93` | "Failed to enable {name}: {error}" | object-in-sentence | `ObjectToast`: mod chip, "Couldn't turn it on/off", reason line. | Converted |
| 4 | `InstalledMods.tsx:116-131` | "Players get {name} the next time they join" | object-in-sentence | `ObjectToast`: mod chip, "Players get it when they join" / "Players no longer get it" / "Couldn't change it". | Converted |
| 5 | `InstalledMods.tsx:143-162` | "Updated {name} to {version}" | object-in-sentence | `ObjectToast`: mod chip + To `VersionTag`, "Updated", then the install `NameList`. | Converted |
| 6 | `InstalledMods.tsx:326-331` | "Uploaded {names}" | list-in-prose | "Uploaded" + `CountBadge`, then a `NameList`. | Converted |
| 7 | `InstalledMods.tsx:356-360` | "Only .jar files can be uploaded: README.txt, sodium.zip" | list-in-prose | Outcome, then a `NameList` "Not uploaded" of mono names. | Converted |
| 8 | `InstalledMods.tsx:383-398` | "Some updates failed: Sodium: HTTP 404; Iris: reset" | list-in-prose | "Some updates failed", then a `NameList` with each core error as the row's reason. | Converted |
| 9 | `InstalledMods.tsx:404-408` | "Deleted {name}" (showed the file name) | object-in-sentence | `ObjectToast`: mod chip, mono file, "Deleted". | Converted |
| 10 | `InstalledMods.tsx:437-452` | "{name} ({fileName}) will be deleted from the server…" | object-in-title | Title "Delete mod?" / "Delete plugin?", `ObjectCard` (icon, name, version, File fact), body "It's deleted from the server. This can't be undone." | Converted |
| 11 | `InstalledMods.tsx:453-466` | "{message} Replace it with the uploaded file?" | inline-values | "Replace the file on the server with the uploaded one?", the core's message on its own muted line. | Converted |
| 12 | `InstalledMods.tsx:477-489` | "158 mods in mods/" | suffix-kinds | "Mods"/"Plugins" + `CountBadge`, a caption "Folder" with the mono "mods/". | Converted |
| 13 | `InstalledMods.tsx:494-498` | "Update all ({count})" | suffix-kinds | "Update all" + `CountBadge`. | Converted |
| 14 | `components/Mods/common.tsx:91-170`, `InstalledMods.tsx:105-110`, `ClientPackPanel.tsx:103-109` | "Needed by {mods}" | list-in-prose | Tooltip `NameList` "Needed by" + count. | Converted |
| 15 | `common.tsx:225-277` | "Installed: a 1.0, b 2.0" / "Removed: …" / "Skipped: id (reason)" | list-in-prose, name-version | `InstallResult` `NameList` groups: Installed (name + tag), Removed (mono), Skipped (mono id + core reason), Players need these too. | Converted |
| 16 | `common.tsx:305-311` | "{title} by {author}" | row-meta | Title alone; `FactLine` with Author and Downloads icon facts. | Converted |
| 17 | `components/Mods/ModrinthProjectView.tsx:208-217` | "1.21.1, 1.21 +3 more" | list-in-prose | Up to 3 versions as one plain phrase ("Minecraft 1.21.1, 1.21, 1.20.6", `gameVersionsPhrase`; the game never gets a `VersionTag`) + a `+N` `CountBadge` whose title lists the rest. | Converted |
| 18 | `ModrinthProjectView.tsx:246-272` | "gv, gv · fabric, quilt · Mar 4, 2025" | row-meta | `VersionTag` + type; `FactLine` of the game phrase and loader names; Published column. | Converted |
| 19 | `ModrinthProjectView.tsx:256-264` | "Not for {loader} {gameVersion}" | comparison-prose | Warning `StatusBadge` "Not for this server". | Converted |
| 20 | `ModrinthProjectView.tsx:394-410` | "{title} {version} is made for {loaders} {gameVersions}, not …" | comparison-prose, object-in-title | `ObjectCard`, then a `CompareTable` (This version / This server; Loader, Game, as plain phrases), body "It may crash the server." | Converted |
| 21 | `ModrinthProjectView.tsx:355-375` | "Installed {title} {version}" | object-in-sentence | `ObjectToast`: project chip + tag, "Installed", install `NameList`. | Converted |
| 22 | `ModrinthProjectView.tsx:452-466` | "No versions for {loader} {gameVersion}." | inline-values | "No versions for this server" + a `FactLine` of the two phrases. | Converted |
| 23 | `ModrinthProjectView.tsx:549-584` | "by author · mod", bare stats, "server: optional" | header-facts | `FactStrip`: Author, Type, Downloads, Followers, Server side. | Converted |
| 24 | `ModrinthProjectView.tsx:607-614`, `i18n/en.ts` `create.modpack.notAModpack` | "This is a {type}." (raw Modrinth type) | inline-values | Translated project type (`mods.projectType.*`) as a `rich()` param. | Converted |
| 25 | `components/Mods/ModsTab.tsx:70-78` | "Only show versions for {gameVersion} / {loader}" | inline-values | "Only compatible versions", then the two phrases inside the label. | Converted |
| 26 | `components/Mods/ModpackPanel.tsx:56-62`, `:120-140` | "+3 ↑2 ~1 −4" | suffix-kinds | Icon `Fact`s (plus, arrow-up, right-left, minus) with keys as screen-reader text and titles. | Converted |
| 27 | `ModpackPanel.tsx:146-150` | "a.jar → b.jar" | comparison-prose | `DiffValue` of two mono names. | Converted |
| 28 | `ModpackPanel.tsx:205-235` | "time  Minecraft 1.21.1 · Fabric 0.16.9" | row-meta | `FactLine`: the date and time in ink-muted, then the two phrases (the loader left out for vanilla); the row's title keys them. | Converted |
| 29 | `ModpackPanel.tsx:73-88` | blameLabel → "since {date}" | row-meta | Returns `{ date, change }` for item 2's Added column. | Converted |
| 30 | `ModpackPanel.tsx:250-322` | "{pack} goes from {from} to {to}.", a dotted software line, counts joined with ", ", "Files you changed stay as they are: {files}" | comparison-prose, list-in-prose | Compact `ObjectCard` + `FactList` (pack `DiffValue`, Game, Loader, Added/Replaced/Removed), then a `NameList` (Removed, Stay as you changed them). Restore: the entry's card + one sentence. | Converted |
| 31 | `ModpackPanel.tsx:411-455` | "Updated {pack} to {version}." + counts + skipped, joined with "\n" | object-in-sentence | `ObjectToast`: pack chip + To tag, "Updated"/"Restored", count facts, skipped note as the reason. | Converted |
| 32 | `ModpackPanel.tsx:386-392` | "{name} isn't a .mrpack file." | object-in-sentence | "Not a .mrpack file" + the file as a mono chip. | Converted |
| 33 | `ModpackPanel.tsx:497-503`, `:519-534` | "{name} {version}", "This server runs the modpack {pack}.", "● {version} is out", "Update to {version}" | name-version, header-facts | Pack icon and name alone; `FactStrip`: Installed tag, Available To tag + "Update available" badge; the button says "Update". | Converted |
| 34 | `ModpackPanel.tsx:556-562` | "History (7)" | suffix-kinds | "History" + `CountBadge`. | Converted |
| 35 | `components/Mods/ClientPackPanel.tsx:37-48` | "Players can join with plain Fabric (Minecraft {gameVersion}, {loader})." | inline-values | The sentence, then a `FactLine` of the two phrases. | Converted |
| 36 | `ClientPackPanel.tsx:79-131` | "Players need {count} mods to join", "Minecraft x · Fabric loader y", glued chips, "Show all {count} mods" | suffix-kinds, header-facts, name-version | Title + `CountBadge`; `FactStrip` Game/Loader; chip + `VersionTag`; "Show {count} more". | Converted |

### Create

| # | Where | Was | Pattern | Now | Status |
| --- | --- | --- | --- | --- | --- |
| 37 | `components/Instance/Create/ModpackCreateForm.tsx:112-147` | Modpack "Name 6.4.0", Loader "fabric 0.16.9", "{count} ({size} to download)" | facts-as-prose, name-version | `FactList`: Modpack (name + tag), Game, Loader, Server files (`Num`), Download (`Bytes`). | Converted |
| 38 | `ModpackCreateForm.tsx:231-238` | "gvs · loaders · date · size" | row-meta | As item 18, plus a Size column. | Converted |
| 39 | `ModpackCreateForm.tsx:629-634` | "name · Minecraft x · Fabric y" | header-facts | Compact `ObjectCard`: name + tag, the two phrases as facts. | Converted |
| 40 | `ModpackCreateForm.tsx:592-597` | "Downloading the modpack file ({size}) to read it…" | inline-values | The status line + a Size fact. | Converted |
| 41 | `i18n/en.ts` `create.modpack.showMore`, `mods.versions.showMore` | "Show more ({count} left)" | suffix-kinds | `values.showMore` "Show {count} more". | Converted |

### Instance

| # | Where | Was | Pattern | Now | Status |
| --- | --- | --- | --- | --- | --- |
| 42 | `components/Instance/InstanceOverview.tsx:90-117` | Labels: state, "Version {v}", "Player Count {p}/{max}", address | header-facts, status-values | State `StatusBadge`; `FactStrip`: Game, Loader, Players (`Fraction`), Address (mono + copy). | Converted |
| 43 | `components/Instance/InstanceDetails.tsx:56-77` | "Created from modpack": "{name} {version}" | name-version | Key "Modpack", value chip link + `VersionTag`. | Converted |
| 44 | `InstanceDetails.tsx:131-140` | "Minecraft version", Loader "{variant} {version}" | facts-as-prose | Game: "Minecraft 1.21.1", Loader: "Fabric 0.16.9". | Converted |
| 45 | `InstanceDetails.tsx:104-115`, `:155` | "Min / Max RAM": "{min} MB / {max} MB" | facts-as-prose | Min RAM and Max RAM rows, `Num` with a lighter unit. | Converted |
| 46 | `InstanceDetails.tsx:144-149` | Players "3 / 20" with "?" | facts-as-prose | `Fraction`, "–" when unknown. | Converted |
| 47 | `InstanceDetails.tsx:117-128`, `:157-165`, `:186-197` | UUID and Folder in break-all text fields | inline-values | `ValueField` (code, path); the section is a framed `FactList`. | Converted |
| 48 | `components/Instance/InstancePerformance.tsx:52-59`, `:82-87` | "CPU LOAD: 12.4%", "MEMORY USED: 1.5 GiB" | unlabeled-stat | `StatTile` "CPU load" / "Memory used" with a tabular value. | Converted |
| 49 | `components/Instance/InstanceShareSection.tsx:141-142` | State `Label` | status-values | `StatusBadge`. | Converted |
| 50 | `InstanceShareSection.tsx:184-190` | "{count} people connected ({count} joins so far)" | suffix-kinds | "People connected" + `CountBadge`, a Joins fact. | Converted |
| 51 | `InstanceShareSection.tsx:196-208` | "203.0.113.4:51234  over the internet, 42 ms" | row-meta | Mono address, route `StatusBadge`, Ping fact. | Converted |
| 52 | `components/Instance/InstanceSettings.tsx:189-221` | "Delete “{name}”" | object-in-title | "Delete server?" + `ObjectCard` (game, loader, state). | Converted |
| 53 | `components/Instance/PublicListingSetting.tsx:33-41`, `:110-119` | "Anyone… sees its {list}" | list-in-prose | The sentence, then a row of chips, one per shown field. | Converted |
| 54 | `components/Instance/ResourcePackSetting.tsx:104-112` | "Downloaded {size}." | facts-as-prose | A Downloaded fact with `Bytes`. | Converted |
| 55 | `components/InstanceCard.tsx:382-392` | "{players}/{max} Online" in the state colour | status-values | Neutral Players fact + state `StatusBadge`. | Converted |
| 56 | `InstanceCard.tsx:154-162`, `components/InstanceLoadingPill.tsx:34-40`, `components/InstanceLoadingCard.tsx:89` | "{name} only starts after you agree…", "Setting up… (45%)", "00/20" | object-in-sentence, status-values | Compact `ObjectCard` + a placeholder-free sentence; a "Setting up" badge and a fixed-width percent; Players "–". | Converted |

### Shell

| # | Where | Was | Pattern | Now | Status |
| --- | --- | --- | --- | --- | --- |
| 57 | `data/EventStream.ts:118-196`, `:260-275`, `data/NotificationContext.tsx`, `components/Atoms/NotificationCard.tsx`, `components/DashboardLayout/NotificationPanel.tsx` | "Server {name} started!", "{players} Joined {name}", "{player} said {message} on {name}", "{pack} {version} is out for {name}", 'API key “{name}” created' | object-in-sentence, list-in-prose | Structured `NotificationContent`; the card leads with the server or key chip (+ state badge), then the outcome; players as chips; chat as player chip + quoted message; modpack as pack chip + To tag and a Server fact. | Converted |
| 58 | `components/DashboardLayout/CoreConnectionStatus.tsx:86-120` | Address, Version, Uptime as plain rows | facts-as-prose | Compact `FactList`: mono Address, Version tag, tabular Uptime; status badge. | Converted |
| 59 | `components/UserMenu.tsx:84` | "Signed in as {name}" | object-in-sentence | Key "Signed in as" over the name. | Converted |
| 60 | `components/UserBox.tsx:80-115` | "Are you sure you want to delete {name}? …" | object-in-title | "Delete user?" + `ObjectCard` (avatar, name, role tags), "This can't be undone." | Converted |
| 61 | `components/DashboardLayout/DashboardLayout.tsx:88-110` | "Core Version: x / Dashboard Version: y" | comparison-prose | `CompareTable` (Core / Dashboard). | Converted |
| 62 | `DashboardLayout.tsx:151-160` | "Version mismatch! … Core: {core}, Dashboard: {dashboard}" | comparison-prose | "Versions differ" + two facts + the question. | Converted |
| 63 | `DashboardLayout.tsx:182`, `:200`, `:220` | "…setup {name}?", "{name} is not setup yet…", "…connecting to {address}…" | object-in-sentence, inline-values | Compact `ObjectCard` for the core, or a `ValueField` for the address; placeholder-free sentences. | Converted |
| 64 | `components/ClipboardTextfield.tsx:27` | 'Copied “{text}”' | inline-values | "Copied" + the value on a mono line. | Converted |
| 65 | `components/GameConsole.tsx:255-260` | "{count} older lines hidden (showing the newest {max})" | suffix-kinds | "Older lines hidden" + `CountBadge`, a Showing fact. | Converted |

### Files

| # | Where | Was | Pattern | Now | Status |
| --- | --- | --- | --- | --- | --- |
| 66 | `utils/apis.ts:112`, `:145`, `:207`, `:340`, `:413`, `:416`, `components/FileViewer/FileViewer.tsx:248-254` | "Deleted file: {name}", "Failed to download {name}: {error}", "Unzipping {name}…" | object-in-sentence | `FileToast`: mono chip with a `KindIcon`, the outcome, the reason. | Converted |
| 67 | `FileViewer.tsx:426-439` | "Delete {count} files", "• world_nether (folder)" | list-in-prose, suffix-kinds | "Delete files?" + `CountBadge`; a `NameList` with a `KindIcon` per row. | Converted |
| 68 | `components/FileViewer/FileContextMenu.tsx:18-33`, `:163`, `:233` | "Zip {name} to {archive}", "Paste into {name}", "Unzip to {name}" | object-in-sentence | A mono header naming the file once ("{count} items" for several); the items are verbs ("Zip", "Paste into", "Unzip to a folder") with the archive or folder name as a mono caption under them (`ContextMenuButton` `caption`). | Converted |
| 69 | `i18n/en.ts` `files.nameTaken` | "{name} already exists in this folder" | object-in-sentence | "Something with this name is already in this folder". | Converted |
| 70 | `FileViewer.tsx:522-527`, `FileContextMenu.tsx:110-115` | "{count} items cut to clipboard" | — | Kept: a count in a short sentence with no object. | Kept: a count in a short sentence, no object |

### Settings

| # | Where | Was | Pattern | Now | Status |
| --- | --- | --- | --- | --- | --- |
| 71 | `components/Settings/ApiKeys/ApiKeyList.tsx:117-180` | Name + prefix glued; "Custom: 3 servers, 5 permissions"; "Created {date}", "Last used: {time}" | row-meta | Name + mono prefix; `FactLine`: scope tag, Servers, Permissions, Created, Expires (or an "Expired" badge), Last used. | Converted |
| 72 | `ApiKeyList.tsx:85-115` | 'Revoke "{name}" ({display})?', 'Revoked API key "{name}"' | object-in-title, object-in-sentence | "Revoke API key?" + `ObjectCard` (Prefix, Created); toast: key chip + "Revoked". | Converted |
| 73 | `ApiKeysSection.tsx:34`, `CreatedApiKeyDialog.tsx:91` | 'Created API key "{name}"', 'API key "{name}" created' | object-in-title, object-in-sentence | "API key created" + `ObjectCard`; toast: chip + "Created". | Converted |
| 74 | `components/Settings/MinecraftLinks.tsx:50-80` | "Remove the link to {name}? …", "Removed {name}" | object-in-title, object-in-sentence | "Remove account link?" + `ObjectCard` (player, mono UUID); toast: chip + "Link removed". | Converted |
| 75 | `MinecraftLinks.tsx:142`, `:148`; `settings.apiKeys.userLead/userEmpty`; `settings.users.permissionsLead` | "{username}" inside leads under a page that names the user | object-in-sentence | Placeholder-free leads ("this user"). | Converted |
| 76 | `pages/settings/UserSettings.tsx:146-149` | "All members ({count})" | suffix-kinds | Label + `CountBadge`. | Converted |
| 77 | `pages/settings/CoreSettings.tsx:362` | "Port {port} opened" | object-in-sentence | "Port opened" + a mono Port fact. | Converted |
| 78 | `components/Instance/Create/form.ts` | "{name} is required" | — | Kept: validation messages sit next to their own field. | Kept: next to its own field |
| 79 | `common.combo.addCustom`, `create.combo.add` | 'Add “{query}”' | inline-values | "Add" + a `MonoValue` of the typed text, no typographic quotes. | Converted |

### Found while applying

| # | Where | Was | Pattern | Now | Status |
| --- | --- | --- | --- | --- | --- |
| 80 | `pages/InstanceTabs/InstanceTabs.tsx:131-138` | The console title's state as a coloured `Label` | status-values | `StatusBadge`; `stateToLabelColor` is gone. | Converted |
| 81 | `components/DashboardLayout/SystemStat.tsx` | "CPU: 12%", uppercase, no caller | unlabeled-stat | Deleted with its story. | Converted |
| 82 | `mods.project.side.*` | "server: optional" | facts-as-prose | Sentence-case values under the "Server side" key. | Converted |
| 83 | `server.share.connection.*` | "over the internet" | row-meta | Standalone route names on a badge ("Internet", "Local network"). | Converted |

### Kept on purpose

- Error messages with no object, "Failed to load …: {error}" (`mods.search.failed`,
  `console.sendFailed`, `create.createFailed` and about 20 more). Where such a
  message is a toast this work touches, it uses the outcome + reason layout.
- Counted clipboard toasts ("3 items cut to clipboard"), item 70.
- Form validation messages next to their field, item 78.
- English the core sends (modpack history entry labels, plan notes, skipped
  notes, unsupported reasons, error strings) is shown as it is, on its own
  reason line, until the core sends structured data.
- The `Label` atom stays for tags that aren't status (categories, version
  types); `StatusBadge` is for state.
