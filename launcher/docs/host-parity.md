# Lodestone Host in GPUI: the parity plan

Lodestone Host is being rewritten in GPUI, in the launcher's workspace
(`launcher/crates/lodestone-host`), to replace the Tauri app in `dashboard/src-tauri`.
The web dashboard (`dashboard/src`) stays as it is: it is also served for remote
access. The new Host must do everything the dashboard does, in the launcher's design
language, from the components it shares with the launcher.

This file is the plan: how the Host is built, what is done, and an exhaustive
checklist of the dashboard's features, split into four work packages (A to D) that can
be built in parallel with little file overlap. Tick items (`[x]`) as you build them.

## How the Host is built

- **The core is a child process.** The Host can't link the core: the core's
  dependencies would break the launcher workspace's rule that every crate comes from
  Zed v1.27.0's lockfile. It runs the core's own program, `lodestone_core`, built from
  the root workspace **without the `deno` feature** (`cargo build -p lodestone_core
  --no-default-features`; never build Deno, in development, bundles or releases), and
  shipped next to the Host's executable. `core_process.rs` finds it, picks the port
  (16662; a debug build takes the next free one; `LODESTONE_HOST_CORE_PORT` overrides),
  and starts it with `--is-host --lodestone-path <data> --port <port>
  --stop-with-stdin`. The data folder is `LODESTONE_PATH`, else `~/.lodestone` (debug
  builds: `~/.lodestone-host-dev`). `supervisor.rs` waits until `/info` answers,
  restarts a core that exits (giving up after 4 quick exits, showing the end of its
  log), and on Quit closes the core's stdin: the core stops its servers and exits
  (killed after 20 s). If the Host dies, the system closes the pipe and the core stops
  too. The core's output goes to `<host folder>/logs/core.log`.
- **The Host signs in as the owner without a password**, through the core's local
  claim (`core/src/local_claim.rs`), with `"session": true`: the Host proves it can
  write the core's data folder (only the same OS user can), and the core writes an
  owner sign-in token (a JWT, as `POST /user/login` gives) into the claim's private
  folder. The token is tied to the core's HTTPS certificate (pinned, strict client) and
  never saved: the Host claims again at each start (`session.rs`,
  `launcher_core::cores::local_claim::claim_session`). On the first run (a core without
  an owner) the Host asks for the owner's username and password, which the claim uses
  to make the account (no setup key needed), then for the core's name and public
  address. If the owner turned off "Let apps on this computer claim owner access", the
  Host asks for a username and password (`POST /user/login` over loopback).
- **Shared code.** `launcher/crates/lodestone-ui`: components, theme, fonts and icons,
  the i18n plumbing and the Common/Format words, undo, file dialogs, the skin paint
  layer, toasts (`components::toast`: `HostState::toast(Toast::success(…), cx)`, drawn
  by the shell over everything). `launcher/crates/launcher-core/src/cores/admin`: the core's full API
  (`AdminClient`), its models and the typed event stream (`events.rs`); the live
  streams are `cores::live` (`StreamKind::All` for every event, `Console` for a
  console).
- **State.** `state.rs`'s `HostState` holds the session's `AdminClient`, `/info`, the
  signed-in user, the instances (live from the event stream), the core's settings, the
  connection and instances being made. Pages read it, subscribe to its `HostEvent`s,
  ask the shell to navigate (`HostState::navigate`) or open the "New server" dialog,
  and fetch what only they show through `state.read(cx).admin`.
- **Navigation.** `routes.rs`: `Route` (Home, an instance's `InstanceTab`, a
  `SettingsPage`) with the dashboard's tab visibility rules; `pages/mod.rs::open` makes
  each route's page. The shell (`shell/`) is the top bar, the sidebar (selected
  instance card with power buttons and tabs, all instances with live state, "New
  server", connection, user, settings) and the page.
- **Window.** Closing the window keeps the Host, the core and its servers running
  (macOS: the app hides; elsewhere it minimizes). The macOS menu bar icon (`tray.rs`,
  AppKit through `objc2`) shows the window or quits; the Dock icon and a second start
  (`single_instance.rs`) show it too. Development builds show a DEV badge, an amber
  line under the top bar, and the DEV Dock icon.

### Build, run, test, bundle

```sh
cargo build -p lodestone_core --no-default-features   # in the repository's root
cd launcher
cargo run -p lodestone-host                           # finds ../target/debug/lodestone_core
cargo test -p lodestone-host                          # includes tests/core_child.rs: a real core, end to end
python3 scripts/host_dev_bundle.py --open             # "Lodestone Host Dev.app" in a temp folder
```

- `LODESTONE_PATH`: the core's data folder. `LODESTONE_HOST_DATA_DIR`: the Host's own
  folder (logs, the single-instance socket). `LODESTONE_HOST_CORE`: the core program.
  `LODESTONE_HOST_CORE_PORT`: the port. Tests and bundles must never use `~/.lodestone`.
- The dev bundle's Info.plist points both folders into its output folder.
- `cargo run -p lodestone-host --features screenshot -- --screenshot out.png [--wait 10]
  [--select-first] [--tab console] [--toasts]` renders the window offscreen once the core
  has started and the Host has signed in (optionally on the first instance's tab, with
  sample toasts), then quits (stopping the core). Always set `LODESTONE_PATH` and
  `LODESTONE_HOST_DATA_DIR` to temporary folders when running it.
- Never start a Minecraft server while testing (that accepts Mojang's EULA): test
  power buttons against an unknown instance id, or a fixture instance you don't start.

## Work packages

Each package owns its files; touch others' only to fix a bug, and say so in the PR.
Shared files (marked *shared*) take small additive changes only (new variants, new
helpers at the end), merged often, to keep conflicts small.

| Package | Scope (sections below) | Owns |
|---|---|---|
| **A: instances** | instance list polish (icons, tooltips, creation progress), "New server" for every game and the Modrinth modpack flow (§2.1–2.4), the Overview tab (§3) | `pages/create_instance.rs` (and a `pages/create_instance/` folder), `pages/overview.rs`, `cores/admin/instances.rs`, the modpack part of `cores/admin/mods.rs`, `i18n/*/instances.rs` |
| **B: console and files** | the Console tab (§4), the Files tab with its editor (§5) | `pages/console.rs`, `pages/files.rs` (and folders of the same names), `cores/admin/console.rs`, `cores/admin/files.rs`, `i18n/*/console_files.rs` |
| **C: instance tabs** | the instance Settings tab with the settings-manifest form, public listing, resource pack and delete (§2.5, §6.1), Mods/Plugins (§6.2), Event logs (§6.4), Playit.gg (§6.7) | `pages/instance_settings.rs`, `pages/mods.rs`, `pages/event_logs.rs`, `pages/playit.rs`, `cores/admin/instance_settings.rs`, `cores/admin/playit.rs`, the mods part of `cores/admin/mods.rs`, `i18n/*/instance.rs` |
| **D: core and account** | Settings › General, Users and permissions, Profile, API keys, Minecraft links (§7), notifications (§1.7), connection dialogs and session (§1.5, §1.6), theme, language and zoom settings (§1.8), connecting to other cores (§1.3), the updater (§8.7), the tray on Windows and Linux (§8.5), persisted settings (§9) | `pages/core_settings.rs`, `pages/users.rs`, `pages/profile.rs`, a new `notifications.rs`, `cores/admin/users.rs`, `cores/admin/core_settings.rs`, `tray.rs` (Windows/Linux), `i18n/*/core.rs`; the notification bell's slot in `shell/top_bar.rs` |

*Shared*: `state.rs`, `routes.rs`, `pages/mod.rs`, `shell/*`, `app.rs`,
`cores/admin/{mod,models,events}.rs`, `i18n/*/shell.rs`, and everything in
`lodestone-ui` (a new component or icon there helps both apps: add it at the end of
`components/mod.rs` or `Icon::ALL`, and keep the launcher looking the same). `cores/admin/mods.rs` is split between A and C
by section: put each side's routes under its own heading comment.

**Extension points already in place.** Every page has a stub view with a fixed
constructor (`new(state, uuid, cx)` or `new(state, cx)`), wired into `pages::open`;
the "New server" dialog is `pages/create_instance.rs`'s `CreateInstance`, which the
shell opens from the sidebar and Home and closes on `CreateInstanceEvent::Close`. Each
package has its i18n area (`InstancesMsg`, `ConsoleFilesMsg`, `InstanceMsg`,
`CoreMsg`), empty in all four locales: add a message in `i18n/en/<area>.rs` and its
translation in `fr`, `zh_hans` and `zh_hant` (a missing one doesn't compile). Each
package has its `cores/admin/<area>.rs` with the routes to add listed at its top.

**Out of scope: everything that needs Deno.** The Host's core is built without the
`deno` feature, so macros, tasks, instance scripts, extensions and atom (scripted)
instances don't exist there. The Host shows them as unavailable, as the dashboard does
for a core without the capability (`core/src/capabilities.rs`): the Macros tab only
shows for a core with the `macros` capability and then says scripting is unavailable
(`pages/macros.rs`), and atom instances' controls are disabled without
`generic_instances`. Don't build them. They are marked **out of scope** below.

## Done in the foundation

- [x] The core as a child process: find, port, start, wait, restart, stop; log file
  (`core_process.rs`, `supervisor.rs`; core: `--port`, `--stop-with-stdin`).
- [x] Owner sign-in without a password (local claim session), first-run owner account
  and core name/address, password sign-in when claims are off (`session.rs`,
  `onboarding.rs`, `app.rs`; core: `"session": true` claims).
- [x] The shell: top bar with logo and DEV badge, sidebar with the selected instance
  (state, players, power buttons by state and permission), its tabs by the dashboard's
  visibility rules, every instance with a live status dot, instances being made, "New
  server" (permission), connection status, signed-in user, Settings with its three
  pages (`shell/`, `routes.rs`).
- [x] Live state from the event stream: state changes, players, instances made and
  deleted, permission and username changes, settings changes; reconnects and reloads;
  signing in again when the session ends (`state.rs`).
- [x] Start / stop / restart / kill through the API (`HostState::power`).
- [x] Closing the window keeps everything running; menu bar icon (macOS) with Show and
  Quit; Dock icon and a second start show the window; one Host at a time; DEV Dock icon.
- [x] Zoom with ⌘/Ctrl + − 0 (not persisted yet: §1.8, D).
- [x] A dev bundle (`launcher/scripts/host_dev_bundle.py`) and offscreen screenshots.

## Known gaps and risks

- **Windows and Linux** build but are untested by the foundation: no tray icon yet
  (closing minimizes), the system draws the window frame (the launcher draws its own
  caption buttons on Windows: reuse `components::window_frame` and the launcher's
  `top_bar` approach), and the Windows executable has no icon resource yet.
- **Packaging and updates** (§8.7): no release packaging, signing or updater for the
  Host yet. The Tauri Host's updater feed is different from the launcher's channels;
  the Host should use the launcher's (`launcher_core::update`) with its own channel.
- **Light theme** (§1.8): `lodestone-ui`'s theme is dark only. A light theme touches
  the shared theme and every component; do it in one dedicated change.
- **Text editor** (§5): the dashboard uses Monaco; the Host uses `lodestone-ui`'s
  `TextEditor` (multi-line, virtualized, undo/redo, colours for configuration formats).
  It has no word wrap and no find and replace yet.
- **Charts** (§3): no chart component yet; package A draws the performance graphs
  (GPUI `canvas` paths).
- **Error messages** of `CoreError` mention "Lodestone Launcher" in a few places; make
  them app-neutral when touching `cores/client.rs`.
- The session token lasts 60 days; a Host left running that long signs in again when
  the core refuses it (handled by `HostEvent::SignedOut`).

---

# The dashboard, feature by feature

The rest of this file is the inventory of `dashboard/src` and `dashboard/src-tauri`.
Each section names its package. Items marked **(dead)** exist in the dashboard's code
but can't be reached from its UI; **(bug?)** marks dashboard behaviour that looks broken:
decide before copying it. Paths are relative to `dashboard/` (taken at commit
`7ab870c9`).

Conventions in the code that every area relies on:

- REST base: `{protocol}://{address}:{port}/api/{apiVersion}` (`utils/util.ts` `coreApiBaseUrl`). WebSocket
  base: same with `ws`/`wss` (`coreWebSocketUrl`). Default local core `http://localhost:16662`, api `v1`.
- Auth header `Authorization: Bearer <token>` on every REST call (set globally in `pages/index.tsx`).
  WebSockets cannot send headers, so the token goes in the query string (`?token=Bearer%20…`) or in the
  `EventQuery.bearer_token` JSON for the event stream.
- Default HTTP timeout 5 s (`pages/_app.tsx`); Modrinth calls 30 s; uploads/installs/modpack downloads no
  timeout (`data/Mods.ts`); file uploads no timeout.
- File-system paths are sent as URL-safe Base64 (`Base64.encode(path, true)`, js-base64) in the URL.
- Errors: core replies `ClientError {kind: ErrorKind, causes: string[]}`; shown as "Kind Words: cause, cause"
  (`bindings/ClientError.ts`, `utils/util.ts` `errorToString`). Network errors have their own messages.
- Owner bypasses all permission checks client-side (`data/UserInfo.ts` `isUserAuthorized`): `is_owner` →
  true; boolean perms → value; per-instance perms (arrays of instance UUIDs) → membership.
- "Atom" instance = `game_type.type === 'Generic'` and UUID not starting `DOCKER-`; owner-only to control
  (`data/CodeExecution.ts`). "Docker" instance = UUID starts with `DOCKER-`, normal permissions apply.
- Code-execution switch: `useCodeExecution()` → `enabled` (= !`code_execution_locked` && `code_execution_enabled`,
  default true), `locked`, `canManage` (= `is_owner`), `canRun` (= canManage && enabled).
- Core capabilities (`/info` → `capabilities`, missing = available): `macros`, `generic_instances`
  (`docker` exists in the type but the dashboard never reads it).
- "Saved" toast: one small reusable toast (`components/Settings/savedToast.ts`, id `settings-saved`,
  1.5 s) after any auto-saving setting.

---

## 1. App shell

### 1.1 Routing / navigation (`pages/index.tsx`, `pages/_app.tsx`, `data/BrowserLocationContext.tsx`)  
**Package: foundation (done: `routes.rs`; D: persisted window state).**

- [x] Route table — `pages/index.tsx`
  - Login layout (background image, centred card): `/first_setup` (FirstTime), `/login/core/select`
    (needs ≥1 saved core else → `/login/core/new`), `/login/core/new` (CoreConnect).
  - Login layout + RequireCore: `/login/core/first_setup` (CoreSetupNew), `/login/core/first_config`
    (CoreConfigNew), `/login/user/select` + `/login/user` (both wrapped in RequireSetup).
  - Dashboard (RequireCore + RequireToken): `/` (Home) and `/dashboard/{overview|settings|console|files|mods|macros|logs|playitgg}` (InstanceTabs).
  - `/settings/general`, `/settings/users`, `/settings/profile`: settings **modal** over the page it was
    opened from (that page is still rendered underneath; return path kept in sessionStorage
    `settingsReturnTo`, `utils/settingsReturn.ts`).
  - `*` → 404 page (title "404", "Go home" button → `/`) — `pages/notfound.tsx`.
- [x] Route guards — `utils/router/RequireCore.tsx` (no saved cores → `/first_setup`), `RequireToken.tsx`
  (no token → `/login/user/select`), `RequireSetup.tsx` (core `is_setup === false` → `/login/core/first_setup`;
  "lenient": waits ≤1 s for `/info`, then shows the page anyway).
- [ ] URL state used as app state — `data/BrowserLocationContext.tsx`, `utils/hooks.ts` `useQueryParam`
  - `?instance=<uuid>` selected instance; `?user=<uid>` selected user in settings; `?path=<dir>` file
    browser folder; `?modrinth=<link>` deep link into Mods › Browse (consumed once);
    legacy `?address=&port=&protocol=&apiVersion=` core links (see 1.3).
  - Back/forward history is meaningful (navigate pushes; some setters replace).
- [ ] Lazy loading / preloading of dashboard chunks after sign-in (not needed natively, but note the
  skeleton/loading states each page has) — `pages/index.tsx`, `components/DashboardLayout/LayoutSkeletons.tsx`.
- [ ] Document title per page: `"<Page> - Lodestone"` style via `common.pageTitle` — every page.
- [ ] `pages/settings.tsx` legacy tabbed settings page **(dead)**.

### 1.2 Main layout (`components/DashboardLayout/*`)  
**Package: foundation (done, see the ticks); A: game icons and tooltips.**

- [ ] Top bar — `components/DashboardLayout/TopNav.tsx`
  - Centred Lodestone logo; click → clears `instance` and `user` params and goes to `/`.
  - DEV badge next to logo + amber bottom border when dev build (`useIsDevBuild`: Next dev server, or Tauri
    `is_debug_build` command) — `utils/tauriUtil.ts`.
  - Right side: notification bell popover (1.6).
  - macOS Tauri: whole bar is a window drag region (`data-tauri-drag-region`), left column leaves room for
    traffic lights (overlay title bar).
  - "Degraded" banner above the bar when connection status is `degraded`: text + "refresh" link
    (`window.location.reload()`) + "Learn more" (wiki Known-Issues#firefox) — `components/Atoms/TopBanner.tsx`.
- [x] Left sidebar — `components/DashboardLayout/LeftNav.tsx`, `InstanceViewLayout.tsx`
  - Resizable (drag east edge, 200–280 px, persisted `localStorage.leftNavSize`, default 220) —
    `components/Atoms/ResizePanel.tsx`.
  - Sections top→bottom: "Selected instance" (1.2/§3 card + tab list), "All instances" list (§2.1),
    "New server" button, sticky footer (core status + user menu + settings gear).
  - `IntegrationsList.tsx` is imported but never rendered **(dead)**.
- [x] Selected-instance block — `components/DashboardLayout/SelectedInstanceInfo.tsx`
  - Empty state: dashed box "No instance selected" (faExpand icon).
  - Loading skeleton when URL names an instance but list still loading.
  - Instance card (§2.2) then the tab list. Tab visibility rules:
    - `playitgg` only if `global_settings.playit_enabled`.
    - `macros` only if owner (`canManage`) and core capability `macros` (and capabilities loaded).
    - `mods` only if the game supports mods/plugins (Fabric/Forge → "Mods", Paper/Spigot → "Plugins",
      `utils/mods.ts getModSupport`) **and** user has `can_read_instance_file` on the instance.
    - Tabs + icons (`pages/InstanceTabs/tabList.tsx`): Overview (chart-line), Settings (cog),
      Console (server), Files (folder), Mods/Plugins (puzzle), Macros (code-compare), Event Logs (inbox),
      Playit.gg (car-tunnel). Active tab highlighted.
- [x] Footer: core connection status row — `components/DashboardLayout/CoreConnectionStatus.tsx`
  - Icon + core name; colour by status: success (plug-check, green), loading (plug, gray), degraded
    (plug-exclamation, yellow), error (plug-x, red).
  - Hover/focus tooltip: status label + detail text + Address (`proto://addr:port`), Version, Host
    (`host_name`), System (`os arch`, macos→macOS, aarch64→arm64), Uptime (from `up_since`, d/h/m).
- [ ] Footer: user menu — `components/UserMenu.tsx`
  - Button: avatar (boring-avatars from uid) + username (skeleton while loading, "Guest" if logged out),
    opens upward menu: "Signed in as <name>" / "Not signed in"; **Sign out** (clears token for this core,
    clears `instance` & `user` params → redirected to login); **Change core** (→ `/login/core/select`).
  - Gear icon button → remembers current path, opens `/settings/general`.
- [x] Home page — `pages/home.tsx`
  - Placeholder cube image, title + subtitle, "Create a new server" button (only when logged in; disabled
    without `can_create_instance`). Skeleton while user info loads.
- [ ] Instance tab container — `pages/InstanceTabs/InstanceTabs.tsx`
  - Heading = tab display title (Mods/Plugins dynamic); Console heading also shows a state label.
  - "Server not found" when URL instance isn't in the list; "Tab not found: <path>" for unknown tab.
  - Per-instance command history provider (§4).
  - Max widths per tab (4xl / 6xl).
  - If URL has `instance` and list loaded and instance exists but path isn't `/dashboard…` → go to
    `/dashboard/overview`; if instance missing → `/` (`InstanceViewLayout.tsx`).
  - Selecting `/` clears the selection.

### 1.3 Cores: selecting, adding, persistence (`pages/index.tsx`, `pages/login/*`)  
**Package: D.**

- [ ] Current core persisted in localStorage keys `address`, `port`, `protocol`, `apiVersion`
  (defaults localhost/16662/http/v1). Switching core clears the whole query cache and `?instance`.
- [ ] Saved core list `localStorage.cores` (array of `{address,port,protocol,apiVersion}`); a core is
  added after a successful connection; same address+port replaces protocol/apiVersion.
- [ ] Per-core token map `localStorage.tokens` keyed by `"address:port"`.
- [ ] Legacy link params `?address&port&protocol&apiVersion`: if equal to current core they are just
  stripped; if different → open `/login/core/new` pre-filled (never switches silently).
- [ ] **Connect to a core** page — `pages/login/CoreConnect.tsx` (`/login/core/new`)
  - Fields: API version (select, only `v1`), Protocol (select http/https), Port (number, default 16662),
    Address (text). All required (yup).
  - Protocol auto-follows address (http for localhost/127.0.0.1/::1, https otherwise) until user picks one.
  - IPv6 address auto-wrapped in `[...]`.
  - Submit: `GET /info` against the candidate base URL (unauthenticated) → on 200: setCore + addCore; if
    `is_setup === false` → `/login/core/first_setup`, else → `/login/user/select`.
  - Errors: warning "Check fields: <error>"; for remote network errors shows connection help
    (`components/CoreConnectionHelp.tsx`): https → "trust the certificate" link to
    `https://addr:port/` + fingerprint hint; http → plain-HTTP hint.
  - https page + http core: warning banner and a "mixed content" danger dialog (wiki link) — browser-only.
  - Buttons: Back, "Download core" (opens GitHub releases), "Add and continue".
- [ ] **Select a saved core** — `pages/login/CoreSelectExisting.tsx` (`/login/core/select`)
  - Select of saved cores (label `address:port`), "or" "Connect to a new core" button, Download core,
    Continue (same `/info` probe and routing as above; help box with `savedCore` wording).
- [ ] Local-core discovery while signed in to a remote core — `DashboardLayout.tsx`, `data/SystemInfo.ts`
  - If the local core is reachable (Tauri, or page served over http) and the current core isn't local:
    poll `GET http://localhost:16662/api/v1/info` every 3 s (2.5 s timeout) until it answers.
  - Answer `is_setup:false` → dialog "A local core was found" with **Set up** (switch to local core →
    `/login/core/first_setup`) / **Skip**. `is_setup:true` → silently add local core to saved list.

### 1.4 First run / owner setup / login (`pages/login/*`)  
**Package: foundation (done, through the local claim).**

- [x] **Welcome page** `/first_setup` — `pages/login/FirstTime.tsx`
  - If any core is saved → redirect `/login/core/select` immediately.
  - Logo, "Wiki" link button, title + dashboard version (green), red "Beta" notice with GitHub issues link.
  - Browser: buttons "Download core" (GitHub releases) and "Connect to an existing core" (→ `/login/core/new`).
  - Tauri: invokes `is_setup`; adds+selects default local core; shows **Continue** (→ `/`) if set up,
    else **Set up Lodestone Core** (→ `/login/core/first_setup`). Invisible placeholder button meanwhile.
  - **(bug?)** compares the boolean result with the string `'true'`, so it always shows "Set up"; the
    setup page then redirects to user select because `/info.is_setup` is true.
- [x] **Owner setup** `/login/core/first_setup` — `pages/login/CoreSetupNew.tsx`
  - Shows core name + `addr:port`; polls `/info` every 3 s; if `is_setup` becomes true → `/login/user/select`.
  - Fields: Username (required), Setup key (password field, required), Password (required),
    Confirm password (must match). Hint "find the key in the core's console/log" unless pre-filled.
  - Tauri: `invoke('get_first_time_setup_key')` pre-fills the key and disables the field
    ("Setup key (filled in)").
  - Submit: `POST /setup/{setupKey}` body `{username,password}` → `LoginReply {token,user}`; stores token,
    invalidates queries, → `/login/core/first_config`. Error → warning box.
  - Analytics event "Setup Owner Account".
- [x] **Core first config** `/login/core/first_config` — `pages/login/CoreConfigNew.tsx`
  - Fields: Core name (required), Public domain/IP (max 253, placeholder `123.123.123.123`), prefilled
    from `GET /global_settings`.
  - Submit: `PUT /global_settings/name` (JSON string), `PUT /global_settings/domain`; updates caches; → `/`.
- [x] **User select** `/login/user/select` — `pages/login/UserSelectExisting.tsx`
  - Not signed in and (not Tauri or not local core) → redirect to `/login/user` (renders nothing).
  - Tauri + local core + no token: `invoke('get_owner_jwt')` → if a token comes back, store it (auto sign-in
    as owner, no password).
  - Signed in: "Continue as <username>" (avatar) / skeleton while loading; "Switch user" → `/login/user`.
  - Not signed in: disabled "Continue as current user" + "Sign in" button.
  - "Change core" → `/login/core/select`.
- [x] **Sign in** `/login/user` — `pages/login/UserLogin.tsx`
  - Username/password form (both required): `POST /user/login` with HTTP Basic auth, body `{}` →
    `LoginReply`; store token, → `/`.
  - 429 → "Too many failed sign-in attempts, try again in N seconds/minutes" from `Retry-After`.
    Network failure → "Login failed: …"; other → "Error: …".
  - Toggle link "Sign in with an API key" / "Sign in with password": API key field (password type,
    required, must start `lsk_`, trimmed) + hint text; `POST /user/login_api_key {key}` → `{user}`; the key
    itself becomes the session token.
  - **(bug?)** the token handling in `pages/index.tsx` JWT-decodes every token; a `lsk_…` key is not a JWT,
    so it is rejected with an "invalid token" toast and signed out immediately.
  - Back button: Tauri+local core → "Switch account" (history back); otherwise "Change core".
  - Analytics "Logged in" (User / API key).

### 1.5 Session / token lifecycle (`pages/index.tsx`, `data/EventStream.ts`)  
**Package: D (the foundation signs in again when the session ends).**

- [ ] Token = JWT (claims `uid`, `exp`), decoded without verification; malformed / no `exp` / no `uid` /
  expired → error toast and sign-out.
- [ ] Auto sign-out timer at `exp` with warning toast "Session expired" (toast id `sessionExpired`).
- [ ] 401 handling: any 401 on a request carrying the current token → re-check `GET /user/info`; if that
  401s → toast "session invalid" and sign out. No refresh tokens.
- [ ] Changing token or core clears the whole data cache; notifications cleared on sign-out.
- [ ] `UserDeleted` event for self → sign out.
- [ ] Analytics: Google Analytics (`react-ga4`, id `G-LZQ3VZ6N26`) in production builds; events from
  `useAnalyticsEventTracker` in many places (login, create instance, notification panel, instance list…) —
  `utils/hooks.ts`. Decide whether the native app keeps any telemetry.

### 1.6 Connection status, reconnect, global dialogs (`DashboardLayout.tsx`, `data/EventStream.ts`, `data/SystemInfo.ts`, `utils/websocket.ts`)  
**Package: D (the foundation shows the connection in the sidebar).**

- [ ] Connection status state machine `loading | success | degraded | error`:
  - `/info` success → success (unless degraded); `/info` failure → error.
  - Event websocket open → success. Websocket failed before opening → probe `GET /user/info`: OK → degraded
    (REST works, WS doesn't); 401 → sign-out path; network error → error; other → degraded.
- [ ] Reconnecting websocket helper: exponential backoff 1 s → max 30 s, reset on open; URL rebuilt each
  attempt; close code 1000 by client stops it — `utils/websocket.ts`.
- [ ] After an event-stream reconnect: invalidate instance list, user info, global settings.
- [ ] **Core error dialog** (status `error`): "Can't reach <addr:port>", **Back** (→ `/login/core/select`) /
  **Continue** (reload page).
- [ ] **Setup required dialog** (`/info.is_setup === false` while in dashboard): **Set up** (→ first_setup)
  / **Change core**.
- [ ] **Version mismatch** (`DASHBOARD_VERSION` vs `/info.version`, semver): different major or minor, or
  core 0.4.x with patch < 4 → danger dialog (core/dashboard versions, "incompatible", wiki Updating link,
  "Continue anyway"); patch-only difference → warning toast top-centre 10 s (id `patchVersionMismatch`).
- [ ] **Beta thank-you dialog** once per new core version containing "beta" (`localStorage.latestVersion`).

### 1.7 Notifications (`data/NotificationContext.tsx`, `data/EventStream.ts`, `components/DashboardLayout/Notification*.tsx`, `components/Atoms/NotificationCard.tsx`)  
**Package: D.**

- [ ] Event history on sign-in: `GET /events/all/buffer?filter=<EventQuery JSON, all null>` → replay
  (non-console) events as non-fresh; failure toast "Couldn't load past events" (id `eventBufferError`);
  "historyLoaded" flag drives skeleton.
- [ ] Live events: WS `/events/all/stream?filter=<EventQuery JSON with bearer_token>` (console-type events
  `InstanceOutput/InstanceInput/SystemMessage` ignored here).
- [ ] Notification list: max 200, deduped by snowflake; each item has title, message, timestamp (from
  snowflake: `(id >> 22) + 1667530800000`), level (Info/Warning/Error), fresh flag.
- [ ] Ongoing (progress) notifications from `ProgressionEvent`: Start (title, total, start value) → Update
  (add `progress`, message; clamp to total) → End (moves to normal list with end message). Progress shown
  as bar (capped at 99 % while ongoing).
- [ ] Bell icon in top bar with red dot when a new fresh notification or any ongoing one arrives; cleared
  on click. Popover 480 px wide, 80 vh: title, "In progress" section, "All notifications" section
  (newest first), skeleton while history loads, empty state.
- [ ] Notification titles produced per event (i18n `shell.event.*`): state transitions per state
  (`Starting/Running/Stopping/Stopped/Error` "<name> is …"), instance warning (+ error toast
  "warningToast"), instance error, players joined/left/both, player chat message, API key created/revoked,
  macro started/detached/stopped (with exit status type), playit runner started/loading/stopped.
- [ ] Cache side effects of events (fresh only): StateTransition → instance state; InstanceError → state
  Error; PlayerChange → player list/count; UserCreated → refetch user list; UserDeleted → drop user (self →
  sign out); UsernameChanged / PermissionChanged → update self + list; ApiKeyCreated/Revoked → refetch key
  lists; Macro Started → refetch task+macro lists; Macro Stopped → refetch task+history; Playit runner →
  runner status; ProgressionEnd InstanceCreation → add instance; InstanceDelete → remove instance;
  FSOperationCompleted → success/error toast with core message + refetch that instance's file lists;
  GlobalSettingsChanged → refetch global settings. `FSEvent`, `UserLoggedIn/Out` ignored.
- [ ] Global toasts (react-toastify): bottom-right, click to close, pause on hover, Zoom transition,
  level icon; colours info gray / error red / warning yellow / success green — `pages/_app.tsx`.

### 1.8 Theme, language, zoom, keyboard  
**Package: D.**

- [ ] Theme preference `system | light | dark` in `localStorage.theme`; system follows OS live; syncs across
  tabs; applied before first paint; Tauri window theme set to match (`setTheme(null)` for system) —
  `utils/theme.ts`, `components/Settings/AppearanceSetting.tsx` (segmented control with per-option hint).
- [ ] Separate light/dark colour tokens, graph colours, Monaco themes (`lodestone-dark` bg `#26282C`,
  `lodestone-light` white), console palette per theme — `globals.css`, `light-overrides.css`.
- [ ] Languages: English, Français, 简体中文 (zh-Hans), 繁體中文 (zh-Hant); preference `auto` (browser
  languages; zh-TW/HK/MO → Hant) or explicit, `localStorage.language`; `<html lang>` set; plural forms via
  `Intl.PluralRules`; locale-aware number/date/relative-time/bytes/list formatting — `i18n/*`,
  `components/Settings/LanguageSetting.tsx`. ~1,600 message keys in `i18n/en.ts`.
- [ ] Zoom (Tauri only): Ctrl/⌘ `+`/`=`/NumpadAdd zoom in, `-`/NumpadSubtract out, `0` reset; levels
  0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2; persisted `localStorage.lodestone-zoom` —
  `utils/zoom.ts`, `utils/zoomLevels.ts`.
- [ ] Other keyboard behaviour: Ctrl/⌘+S saves in file editor; ↑/↓ console command history; Enter saves /
  Esc cancels inline-edit text fields (`components/EditableTextfield.tsx`); Esc resets a setting input
  (`components/Atoms/Config/InputBox.tsx`); Esc in settings modal closes the narrow-window page list first,
  then the modal; dialogs close on Esc/backdrop except the "new API key" dialog.

---

## 2. Instances: list, cards, create, delete

### 2.1 Instance list (sidebar) — `components/DashboardLayout/InstanceList.tsx`, `components/InstancePill.tsx`, `data/InstanceList.ts`  
**Package: A (the foundation lists instances with live state).**

- [x] `GET /instance/list` → `InstanceInfo[]` (keyed by uuid; insertion order = core order). Only fetched
  when signed in.
- [ ] Header "All instances"; each row: game icon (or modpack icon), name, coloured status dot
  (Running green, Starting/Stopping yellow, Stopped gray, Error red). Click selects (`?instance=`).
- [ ] Skeleton rows while loading; logged-out state: dashed box "Log in to view".
- [ ] In-progress creations appear as "Setting up… NN %" pills with circular progress (from ongoing
  `InstanceCreation` progression events) — `components/InstanceLoadingPill.tsx`.
- [ ] "New server" button under the list (disabled + "No permission" label without `can_create_instance`);
  opens create dialog (§2.3).
- [x] Live updates via events (state, players, add/remove).
- [ ] Game icon tooltip = game title (Minecraft / Forge (Minecraft) / Fabric (Minecraft) / Paper (Minecraft)
  / Spigot (Minecraft) / `<name> (Minecraft)` / Minecraft Bedrock / `<name>` generic); modpack icon only
  from `https://cdn.modrinth.com/`, falls back to game icon on load error — `components/Atoms/GameIcon.tsx`,
  `data/GameTypeMappings.tsx`.

### 2.2 Instance card (selected instance) — `components/InstanceCard.tsx`  
**Package: A (the foundation has the power buttons).**

- [ ] Shows icon, name, power button with menu, "Copy IP" chip (`<domain or core address>:<port>`, toast
  "Copied …"), player-count label coloured by state (`players/max online`).
- [x] Power menu items by state: Stopped → Start; Running → Stop, Restart; Starting/Stopping → Kill;
  Error → Restart.
  - Endpoints: `PUT /instance/{uuid}/start`, `/stop`, `/kill`, `/restart`; error → toast.
  - Permissions: needs `can_view_instance`; start needs `can_start_instance`; stop/kill need
    `can_stop_instance` (per instance).
  - Atom instances: whole menu disabled with tooltip if core lacks `generic_instances` ("no scripting") or
    user isn't owner; Start/Restart disabled with note when code execution is disabled.

### 2.3 Create instance dialog — `components/Instance/InstanceCreateForm.tsx`, `components/Instance/Create/*`, `components/DashboardLayout/LeftNav.tsx`  
**Package: A.**

- [ ] Modal with a left "wizard steps" rail (done = check, current = filled number, can click done steps to
  go back) — `Create/WizardSteps.tsx`.
- [ ] Step 1 **Select game** — `Create/GameTypeSelectForm.tsx`, `Create/SelectGameCard.tsx`
  - `GET /games` → `HandlerGameType[]` (core offers `MinecraftJavaVanilla`, `MinecraftFabric`,
    `MinecraftForge`, `MinecraftPaper`; `MinecraftBedrock` exists in the type but isn't offered).
  - Full-width card "Modrinth modpack" with "New" badge → modpack flow (§2.4).
  - One card per game type with title + description; selected card highlighted. Skeleton cards while loading.
  - Next disabled until a game is chosen (initial type is `'Generic'` with no URL).
- [ ] Step 2 **Basic settings** (client-side manifest, `Create/form.ts`): Name (required, default
  "Minecraft Server"-style default from i18n), Description (optional).
- [ ] Step 3 **Server settings**: rendered from `GET /setup_manifest/{HandlerGameType}` → `SetupManifest`
  (sections of `SettingManifest`) — `Create/FormFromManifest.tsx`, `Create/FieldFromManifest.tsx`.
  - Field by `value_type`: String → text/password (is_secret); Integer/UnsignedInteger/Float → number;
    Boolean → true/false radio; Enum → filterable combobox (prefix match). Optional fields marked.
  - `java_cmd` / `cmd_args` disabled with reason unless owner, or (`can_write_global_file` and safe mode
    off), and never while code execution is disabled — `data/CodeExecution.ts useCommandLineBlockedReason`.
  - Validation generated per field (`generateValidationSchema`): required, numeric, integer, ≥0 for
    unsigned, min/max, enum membership. Initial values from `default_value` then `value`, else type zero.
- [ ] Step 4 **Auto settings**: Auto start (bool), Restart on crash (bool).
- [ ] Create: `POST /instance/create/{HandlerGameType}` body `SetupValue {name, description, auto_start,
  restart_on_crash, setting_sections: {sectionId: {settings: {id: {value}}}}}`; failure → toast, dialog
  stays open; success closes dialog; the instance appears through progression events (pill → list).
- [ ] **Out of scope** (needs Deno). Generic/atom path: `PUT /generic_setup_manifest {url}` and `POST /instance/create_generic
  {url, setup_value}` exist in code but there's no URL field in the UI **(dead)**. No Docker-instance UI.
- [ ] Dialog skeleton while code loads (`CreateInstanceFlow.tsx`); Back keeps typed values.

### 2.4 Create from Modrinth modpack — `components/Instance/Create/ModpackCreateForm.tsx`, `components/Mods/ModrinthSearch.tsx`, `data/Mods.ts`  
**Package: A.**

- [ ] Steps: Select game (go back) → Select modpack → Select version → Server settings.
- [ ] Can be opened pre-set from Mods › Browse ("Create server from this modpack", `ModpackPreset
  {projectId, versionId}`), skipping the search step.
- [ ] **Select modpack**: Modrinth search (project_type `modpack`), "Only Fabric" checkbox (loader `fabric`,
  default on), sort select (Relevance/Downloads/Follows/Newest/Updated), **Upload .mrpack** button.
  - Search box accepts a modrinth.com link → `GET /modrinth/resolve?url=` → picks that project (and
    version); non-modpack → toast "not a modpack".
  - Upload: file picker `.mrpack` only → `POST /modpack/upload` (multipart `file`) → `ModpackInfo` with
    `upload_id` → goes to version step showing summary.
  - Search state and scroll position are kept when going back.
- [ ] **Select version**: project header (icon/title/summary, skeleton), version list from
  `GET /modrinth/project/{id}/versions` (no filter), 20 at a time + "Show N more"; row: version number,
  release/beta/alpha label, game versions · loaders · date · size, Select button.
  - Select → `POST /modpack/inspect {version_id}` (no timeout; "Downloading <size>…" status) →
    `ModpackInfo` summary: Modpack name+version, Minecraft version, Loader+version, server files
    count/size, summary text, warning if `unsupported_reason`. Only the latest inspect wins.
  - Next requires info + source and no `unsupported_reason`.
- [ ] **Server settings** (formik/yup): Name (required, default pack name), Description (default pack
  summary), Port (1–65535, default first free from 25565 among existing instances), Min RAM MB (≥1, default
  1024), Max RAM MB (≥ min, default 4096), Auto start, Restart on crash.
- [ ] Create: `POST /instance/create_modpack` `CreateModpackInstanceRequest {source: {type:'modrinth',
  version_id} | {type:'upload', upload_id}, name, port, description, min_ram, max_ram, auto_start,
  restart_on_crash}` → `InstanceUuid`; progress via progression events.

### 2.5 Delete instance — `components/Instance/InstanceSettings.tsx`  
**Package: C.**

- [x] In Settings › Danger zone: "Delete" button → danger confirm dialog ("Delete <name>?", cannot be
  undone) → `DELETE /instance/{uuid}`; refetch list, deselect; error toast.
- [x] Permission: `can_delete_instance` (global) and, for atom instances, owner.
- [ ] No clone, rename-folder, import-existing, backup or export features exist in the dashboard.

---

## 3. Overview tab — `components/Instance/InstanceOverview.tsx`, `InstancePerformance.tsx`, `InstancePlayerList.tsx`, `InstanceDetails.tsx`  
**Package: A.**

- [ ] Inline-editable **name** (heading) and **description**: click or pencil → input, Enter/floppy saves,
  Esc/blur cancels; trimmed; inline error text; spinner — `components/EditableTextfield.tsx`.
  - `PUT /instance/{uuid}/name` / `PUT /instance/{uuid}/description` (JSON string body); updates list cache.
  - Read-only for non-owners on atom instances (tooltip). No explicit permission check client-side.
- [ ] Labels row: game icon, state label, "Version x", "n/m players", address chip `<domain|core
  address>:<port>` (click copies).
- [ ] **Performance**: two line charts, last 60 s (1 sample/s): CPU % (max 100) and memory GiB (max =
  core `total_ram`); headings show latest value; gaps drawn dashed; tooltip "N seconds ago"; skeleton while
  waiting for the first report of a running/starting instance — `components/Graphs/PerformanceGraph.tsx`.
  - WS `/monitor/{uuid}?token=Bearer …` → `PerformanceReport {memory_usage, cpu_usage, disk_usage,
    start_time}`; server replays last 60 s on each connect.
- [ ] **Players**: sort by name toggle (asc/desc); rows with 16 px head from `https://mc-heads.net/avatar/
  {uuid}/16.png`, name, and the linked Lodestone username if the player's Minecraft account is linked
  (owner sees all links, others only their own — `data/MinecraftLinks.ts`, `utils/linkedPlayers.ts`);
  empty state "No players online". No kick/ban/op/whitelist actions.
- [ ] **Details** (collapsible, open state in `localStorage.instanceDetailsOpen`): Name, Description, UUID
  (copy), Game, Minecraft version / version, Loader + loader version, Port, State, Players, Created
  (`creation_time`), Auto start, Restart on crash, RAM (min–max MB), Folder path (copy), Modpack (link to
  modrinth.com/modpack/<id> if from Modrinth). Rows without value hidden.
- [ ] Ports / playit tunnels are not on Overview (Playit is its own tab, §6.7).
- [ ] **Share with friends** (Lodestone Connect, added after the inventory) —
  `components/Instance/ShareWithFriends.tsx`, `data/InstanceShare.ts`, core `src/connect.rs`
  - `GET /instance/{uuid}/share` → `InstanceShare` (fails for users who can't manage the
    instance, and on cores without sharing: then hidden); polled every 5 s while sharing,
    2 s while starting or reconnecting.
  - Off: lead text and **Start sharing** → `POST /instance/{uuid}/share` (answers once the
    link is known). On: state label (sharing green / reconnecting yellow / starting gray),
    the stable link (read-only field, select on focus, **Copy** with toasts), "preparing"
    note, how friends join, a "hard network" note, "N connected" and "N joins", one line per
    friend with its connection kind, **Stop sharing** → danger confirm →
    `DELETE /instance/{uuid}/share` (the link stops working for good).
  - Messages: `server.share.*` in `i18n/en.ts`.

## 4. Console tab — `components/GameConsole.tsx`, `data/ConsoleStream.ts`, `components/ConsoleLine.tsx`, `utils/consoleFormat.ts`  
**Package: B.**

- [x] History: `GET /instance/{uuid}/console/buffer` → `ClientEvent[]` (only `InstanceOutput` used).
- [x] Live: WS `/instance/{uuid}/console/stream?token=Bearer …`; reconnect with backoff and re-fetch buffer
  to fill the gap; merge by snowflake, dedupe, keep newest 5,000 lines; render newest 2,000 with a
  "N older lines hidden" row. *Host: the list is virtualized, so all 5,000 kept lines are
  shown and there is no "hidden" row (`pages/console.rs`).*
- [x] Status dot (top-right) with tooltip: no permission / loading / buffered / live (green) / live but
  history failed (yellow) / closed (red) / error (red); "Server stopped" overrides when Stopped.
- [x] Filter box + "Regex" checkbox (default on; case-insensitive; invalid regex matches nothing; plain
  mode = case-insensitive substring) on the plain text.
- [x] Auto-scroll sticks to bottom unless user scrolled up (>10 px). *Host: and a "Jump to
  latest" button while scrolled up.*
- [x] Coloured output (toggle in Profile: *Host: `pages::console::set_colored`, for package D's
  Profile toggle and settings store; on until then*, `localStorage.coloredConsole`, default on): ANSI SGR (16/256/24-bit
  colours, bold, dim, italic, underline, strike, inverse) and Minecraft `§` codes; other escapes stripped;
  low-contrast colours adjusted for the theme (≥4.5:1); text on background colours rendered as badges;
  plain lines coloured by detected log level (WARN/ERROR from `[..]` prefixes or `WARN:`/`ERROR:`).
- [x] Linked-player badge appended to lines that mention a linked player.
- [x] Command input: `POST /instance/{uuid}/console` body JSON string; error toast "Couldn't send: …";
  history of last ~50 commands per tab session, ↑/↓ navigation. *Host: kept per instance
  while the Host runs.*
  - Input disabled with message when: no `can_access_instance_console`; atom instance and not owner;
    instance Stopped; stream closed.
- [x] Empty/no-permission states with server icon graphic; skeleton code lines while loading.
- [ ] The instance tab container's Console heading with the state label (§1.2): not done,
  the heading is the shell's; the sidebar and the status dot show the state.

## 5. Files tab — `components/FileViewer/*`, `utils/apis.ts`, `data/FileSystem.ts`  
**Package: B.**

- [x] Permissions: browse/read/download need `can_read_instance_file`; upload/new/rename/move/copy/paste/
  delete/zip/unzip/save need `can_write_instance_file` (per instance). No read perm → "No permission" graphic.
- [x] Directory listing: `GET /instance/{uuid}/fs/{b64(path)}/ls` → `ClientFile[]`, sorted by type then
  name; root path `.`; current folder in `?path=` (reset to `.` when switching instance); previous folder
  kept dimmed while the next loads; error text; "Empty folder". *Host: the folder is
  remembered per instance while the Host runs, not in a URL; the list is virtualized.*
- [x] Separator detection: `/` if instance path contains `/`, else `\` (Windows cores).
- [x] List row: checkbox, icon by extension (folder, `.lodestone_config`, txt/md/properties/cfg/ini/conf,
  yml/yaml, exe, images, json/mcmeta, zip/gz/tar, jar, java, class, ts, js, bin/dll/dylib/so/o,
  dat/nbt/mca/obj, lock, log, ps1, sh, mcfunction, unknown → yellow question icon), name (click: folder →
  open, file → open in editor), modified time ("x ago" < 1 day, else date; hidden in narrow widths), size
  (files only). ".." row to go up. Clicking empty space closes editor and clears selection. Cut files
  shown dimmed.
- [x] Breadcrumb: instance folder name then each path segment clickable, then opened file name.
- [x] Resizable split list/editor (`localStorage.fileListSize`, default 200, min 200). *Host: default
  300, kept while the Host runs (persisting it is package D's settings store).*
- [x] Toolbar "Add / Remove" menu: Select all / Deselect all; Cut selected (toast "N cut"); Download
  selected; Unarchive selected (exactly one, Smart); New file; New folder; Delete selected (danger).
- [x] Toolbar buttons: Paste (N) when clipboard non-empty; Save / Discard changes when editor open (enabled
  only if content changed); Upload.
- [x] Status line under the panel: "N selected", "N in clipboard".
- [x] Right-click context menu (on a file or empty area; acts on the selection if the clicked file is
  selected) — `FileContextMenu.tsx`:
  - Copy, Cut (toasts), Paste / "Paste into <folder>" (needs clipboard).
  - Rename (modal), Delete (confirm dialog).
  - Zip: label varies ("Zip", "Zip to <name>.zip", "Zip <file> to <file>.zip", "Zip N files to
    Archive.zip") → `PUT /instance/{uuid}/fs/zip` `ZipRequest {target_relative_paths,
    destination_relative_path}`; toast "Zipping N…".
  - For `.zip/.gz/.tgz` files: Unzip here (`Normal`), Unzip here smart (`Smart`), Unzip to "<stem>/"
    (`ToDirectoryWithFileName`) → `PUT /instance/{uuid}/fs/{b64}/unzip` body `UnzipOption`; toast.
    Unsupported extension → error toast.
  - New folder, New file (in right-clicked folder or current folder).
  - Menu positioned to stay inside the list; closes on outside click.
- [x] Zip/unzip completion comes back as `ProgressionEnd FSOperationCompleted` events → toast + refresh.
- [x] Delete: confirm dialog listing names ("(folder)", "(unknown type)"); files `DELETE
  /instance/{uuid}/fs/{b64}/rm`, folders `DELETE …/rmdir`; one by one; success toast each; closes editor if
  affected.
- [x] New file / New folder modals (name required): `PUT …/fs/{b64(dir/name)}/new` / `…/mkdir`; inline
  error; refresh folder.
- [x] Rename modal (name required): `PUT …/fs/{b64(src)}/move/{b64(dest)}` (same folder); error inline +
  toast.
- [x] Cut+Paste = move each (`…/move/…`); Copy+Paste = `PUT /instance/{uuid}/fs/cpr`
  `CopyInstanceFileRequest {relative_paths_source[], relative_path_dest}`; clipboard cleared after paste.
- [x] Upload: file picker (multiple) or drag-and-drop anywhere on the tab (overlay "Drop to upload");
  folders rejected ("only files"); `PUT …/fs/{b64(dir)}/upload` multipart field `file` (×N), no timeout;
  toast "Uploading N files"; refresh. *Host: files are read into memory before they are
  sent, so a very large upload needs as much memory.*
- [x] Download: `GET …/fs/{b64}/url` → key string → open `/file/{key}` (new tab / system browser).
  Folders are zipped by the core (60 s timeout, toast "Zipping for download"). *Host: saved
  where the user picks (one file: a save dialog; several: a folder), through the pinned
  client, with a "Show in folder" toast.*
- [x] Editor (Monaco): `GET …/fs/{b64}/read` (raw text); language from extension, `.lodestone_config` →
  JSON, `.toml` → custom TOML grammar (`utils/monaco-languages/toml.ts`); word wrap on, minimap off; themed;
  Ctrl/⌘+S saves; Save → `PUT …/fs/{b64}/write` raw body; updates size/mtime in list; error toast.
  Skeleton while loading; error graphic with message on read failure. *Host: the editor is
  `lodestone_ui::components::TextEditor` (line numbers, JetBrains Mono, undo/redo, only the
  lines in view drawn); colours for `.properties`/INI, TOML, YAML and JSON.*
- [x] *Host, beyond the dashboard:* opening another file or folder with unsaved changes asks
  (Save / Discard / Keep editing); edits left unsaved when the tab closes come back when the
  file opens again (while the Host runs, unless the file changed); a file over the core's
  16 MB read limit says so instead of failing; the editor is read-only without
  `can_write_instance_file`.
- [ ] Editor gaps against Monaco: no word wrap (long lines scroll sideways), no find and
  replace, and colours for fewer languages.

## 6. Other instance tabs

### 6.1 Settings tab — `components/Instance/InstanceSettings.tsx`, `components/SettingField.tsx`, `components/Atoms/Config/*`, `InstanceSettingsCreate/*`  
**Package: C.**

- [x] Layout: sections with a table of contents (scroll-spy highlighting, click to scroll) —
  `components/Settings/SettingsToc.tsx`, `SettingsBlocks.tsx`. Skeleton on first load only.
- [x] Manifest-driven sections: `GET /instance/{uuid}/settings` → `ConfigurableManifest {auto_start,
  restart_on_crash, setting_sections}`, refetched every visit. Each setting row by `value_type`:
  - Boolean → toggle (saves immediately, optimistic); Enum → dropdown (saves on change); Integer/
    UnsignedInteger → number input with min/max; Float → text input parsed as float; String → text, or
    password (with show/hide) if `is_secret`.
  - Text/number inputs: save on Enter or floppy icon, revert icon, Esc resets; validation after typing
    pause and on submit (not empty for numbers, strict integer/float parse, min/max).
  - Save: `PUT /instance/{uuid}/settings/{sectionId}/{settingId}` body `ConfigurableValue` JSON → "Saved" toast.
  - Disabled when `!is_mutable`, no `can_access_instance_setting` (row shows "No permission"), atom &
    not owner, or `java_cmd`/`cmd_args` blocked (reason appended).
  - Section with no settings: "Not available" row.
  - Special preview: `motd` setting shows rendered MOTD (Minecraft formatting, `\uXXXX`/`\n` unescaped) on a
    dirt background with the Minecraft font — `SettingOverrides.tsx`, `components/Atoms/MOTDRender.tsx`,
    `utils/motd.ts`.
  - (Which settings exist — port, RAM, java, flags, server.properties fields… — comes entirely from the
    core's manifest; the dashboard has no hard-coded list.)
- [x] **Automation** section: Auto start, Restart on crash toggles → `PUT /instance/{uuid}/auto_start` /
  `/restart_on_crash` (JSON bool); refetch list; needs `can_access_instance_setting` (+ owner for atom).
- [ ] **Sharing / public listing** (Minecraft Java only; hidden on cores without scopes) —
  `components/Instance/PublicListingSetting.tsx`
  - `GET /instance/{uuid}/public_listing` → `InstancePublicListing`; "Public" toggle + checkboxes
    Status / Address / Client pack; sentence describing what's shown; only owner can edit; listing can't be
    turned on while core setting "List instances publicly" is off (unlisting always allowed).
  - `PUT /instance/{uuid}/public_listing` body `PublicScopeChange` → `PublicScope`.
- [x] **Server resource pack** (Minecraft Java only) — `components/Instance/ResourcePackSetting.tsx`
  - `GET /instance/{uuid}/resource_pack` → `ServerResourcePack {url, sha1, required, prompt}`.
  - Fields: URL (http/https, no spaces, max 2048), SHA-1 (empty or 40 hex, max 40) + **Compute** button
    (`POST /instance/{uuid}/resource_pack/hash {url}` → `{sha1,size}`, note "Downloaded <size>"),
    Required toggle, Prompt (max 2048). Draft kept until **Save** (`PUT …/resource_pack`); **Remove** saves
    empty pack. Toast "Saved" or "Saved, restart to apply" if running. Needs `can_access_instance_setting`.
- [x] **Danger zone**: Delete server (§2.5).
- [x] **Minecraft EULA** (PR #626, Minecraft Java only): `GET /instance/{uuid}/eula`; a
  section with the status, a "Read the EULA" link (https://aka.ms/MinecraftEULA) and
  "I agree to the Minecraft EULA" (`PUT /instance/{uuid}/eula`, needs start permission).
  When Start or Restart fails and the EULA isn't accepted, a dialog asks the same, with
  the link and "I agree, start the server" (`pages/eula_prompt.rs`, after the dashboard's
  instance card). Only the user's click accepts it.
- Host notes for §6.1: the list of sections shows beside them when the window is wide
  enough; there is no skeleton, only a loading card on first load. The MOTD preview
  draws the Minecraft colours and formatting on a dark panel, without the dirt
  background and the Minecraft font. The public listing ("Sharing") reuses package D's
  row (`pages/public_listing.rs`) and comes with D's change.

### 6.2 Mods / Plugins tab — `components/Mods/*`, `data/Mods.ts`, `utils/mods.ts`  
**Package: C.**

- [ ] Shown for Fabric/Forge (mods) and Paper/Spigot (plugins); needs `can_read_instance_file`; changes need
  `can_write_instance_file` (disabled buttons get a tooltip "No write permission").
- [ ] "Restart required" yellow banner when `restart_required`.
- [ ] Sub-views: **Installed** | **Browse** (Browse stays mounted after first open). `?modrinth=<link>`
  opens Browse with the link.
- [ ] **Installed** — `InstalledMods.tsx`
  - `GET /instance/{uuid}/mods[?check_updates=true]` → `InstalledModList {kind, loader, game_version,
    directory, restart_required, loader_version, client_required_count, mods[]}`.
  - Header: "N mods in <directory>"; buttons **Update all (N)**, **Check for updates** (refetch with
    check_updates), **Upload** (`.jar` only, multiple).
  - Row: icon (Modrinth), name (Modrinth title / jar metadata / file name), version, client requirement
    label (mods only: Required / Server only / Unknown with tooltips incl. "needed by …"), file name · size;
    **Update to vX** button when update available; link to Modrinth page; enable/disable toggle; delete
    (trash, confirm dialog).
  - Endpoints: `PUT /instance/{uuid}/mods/{fileName}/enable|disable` → `InstalledMod`;
    `DELETE /instance/{uuid}/mods/{fileName}`; `POST /instance/{uuid}/mods/install`
    `ModInstallRequest {version_id, install_dependencies: true}` → `ModInstallResult` (update/update all);
    `POST /instance/{uuid}/mods/upload[?overwrite=true]` multipart `file` → `InstalledMod[]`, 409 →
    "Overwrite?" confirm dialog with the core's message.
  - Result toast lists installed / removed files / skipped (with reasons) / "players need these too".
  - Empty, error and skeleton states.
- [ ] **Client modpack panel** (Fabric only) — `ClientPackPanel.tsx`: "Players need N mods to join",
  Minecraft version · Fabric loader version, chips of required mods (tooltip "needed by"), **Download client
  modpack** → `GET /instance/{uuid}/mods/client_pack` → `ClientPackDownload {key,…}` → open `/file/{key}`;
  import hint (mentions included jars not on Modrinth). Plain-Fabric one-liner when no mods needed.
- [ ] **Browse** — `ModsTab.tsx` Browse, `ModrinthSearch.tsx`, `ModrinthProjectView.tsx`
  - Search (debounced 300 ms) `GET /modrinth/search?query&project_type&game_version&loader&index&offset&limit=20`
    → `ModrinthSearchResponse`; pagination prev/next with "Page x of y"; "Only compatible with <gameVersion>
    <loader>" checkbox (default on); sort select.
  - Pasted/typed modrinth.com links resolved via `GET /modrinth/resolve?url=` and opened directly; "Open
    <slug>" link; error line.
  - Result card: icon, title, "by author", Installed label, client-side label (mods), description,
    downloads (compact number).
  - Project view: Back, icon, title, author/slug · type, description, downloads, followers, server-side
    support, client-side label, category labels, links (Modrinth, Source, Issues, Wiki, Discord), gallery
    (https images only, opens externally), full description rendered from untrusted Markdown with a safe
    renderer (`utils/markdown.ts`, `components/SafeMarkdown.tsx`: headings, lists, tables, code, links
    http/https only, images https only, raw HTML stripped).
  - Data: `GET /modrinth/project/{idOrSlug}`, `GET /modrinth/project/{id}/versions?game_version&loader`,
    `GET /modrinth/version/{id}` (linked version listed first). 5-min client cache.
  - Versions list: "Install dependencies" checkbox (default on), "Show all versions" checkbox; 20 per page
    + "Show N more"; row: version, type label, Installed label, "Not for <loader> <version>" label, game
    versions (3 + "N more"), loaders, date; **Install** → `POST …/mods/install {version_id,
    install_dependencies, force}`; incompatible version → danger confirm "Install anyway" with `force:true`.
  - Paper accepts paper/bukkit/spigot versions (filtered client-side).
  - Warning when project's `server_side === 'unsupported'`; wrong project type warning.
  - Modpack project: **Create server from this modpack** (needs `can_create_instance`) → opens §2.4 preset.

### 6.3 Macros tab — `pages/macros.tsx`, `components/Table.tsx`, `components/ButtonMenu.tsx`  
**Package: **out of scope** (needs Deno).**

- [ ] Visible only to the owner on cores with the `macros` capability; otherwise "owner only" /
  "no scripting" text. Banner when code execution is disabled (listing, kill and history still work).
- [ ] Sub-tabs: **Macros**, **Tasks**, **History** (tables with skeleton rows, "No data", zebra rows, per-row
  "…" menu).
  - Macros: `GET /instance/{uuid}/macro/list` → `MacroEntry[]`; columns Name, Last run. Row menu:
    **Run** → `PUT /instance/{uuid}/macro/run/{name}` body `[]` (args); on error toast, refetch global
    settings if "code execution disabled", else open config dialog; **Edit config** → config dialog.
  - Tasks: `GET /instance/{uuid}/task/list` → `TaskEntry[]`; columns Name, Created, PID; menu **Kill** →
    `PUT /instance/{uuid}/macro/kill/{pid}`.
  - History: `GET /instance/{uuid}/history/list` → `HistoryEntry[]`; columns Name, Created, Finished, PID.
- [ ] Macro config dialog: `GET /instance/{uuid}/macro/config/get/{name}` → `GetConfigResponse {config:
  {name: SettingManifest}, message, error}`; renders each setting with the same setting-field controls
  (edits local only); red "config required" note if `error`; **Save** → `POST
  /instance/{uuid}/macro/config/store/{name}` body the config map; failure keeps dialog open.
- [ ] Lists refetch on macro events and every visit. `MacroCreateForm.tsx` ("create macro") **(dead)**.

### 6.4 Event Logs tab — `pages/InstanceTabs/InstanceTabs.tsx`  
**Package: C.**

- [ ] Placeholder only ("Coming soon" with cube image). No per-instance event log exists.

### 6.5 Players / whitelist / ops / backups / scheduling  
**Package: nobody (not in the dashboard).**

- [ ] Not present in the dashboard (only the read-only player list on Overview). Any such feature would
  come only through the core's settings manifest.

### 6.6 Ports  
**Package: C (through the settings manifest).**

- [ ] Instance port appears in card/overview/details; editing comes from the settings manifest. Core-wide
  "Open port" UPnP action is in Core settings (§7.1).

### 6.7 Playit.gg tab — `components/PlayitggOverview.tsx`, `components/PlayitggSignupModal.tsx`  
**Package: C.**

- [ ] Tab only when `global_settings.playit_enabled`. (Core-wide, not per instance.)
- [ ] On open: `POST /playitgg/verify_key` → bool (refetched each visit), `GET /playitgg/cli_is_running`
  → runner status, `GET /playitgg/get_tunnels` → `PlayitTunnelInfo[]` (failure → empty list).
- [ ] Not verified → signup modal opens automatically: "Generate link" (`GET
  /playitgg/generate_signup_link` → `{url, claim_code}`) → shows link → "Verify" (re-run verify_key,
  refetch tunnels). Error toasts.
- [ ] "Playit runner" heading + power button (disabled until verified) toggling `POST
  /playitgg/start_cli` (status → Loading) / `POST /playitgg/stop_cli` (→ Stopped); status label
  Stopped/Loading/Started; status also driven by `PlayitggRunnerEvent`s.
- [ ] Tunnels list with refresh icon (spins): name, active dot (green only if active and runner started),
  public address (click copies), "Local server ip:port"; empty states "No tunnels" / "Not signed up";
  help link to wiki.

---

## 7. Core / account settings (modal: `components/DashboardLayout/SettingsLayout.tsx`, `SettingsLeftNav.tsx`)

- [ ] Settings modal over the dashboard (full-window below 760 px wide or 559 px tall, with a hamburger page
  list); left nav groups **Core** (General, Users) and **Account** (Profile), then a **Users** list (owner
  only: avatar, name, "You" tag, sorted self first then name; "Unselect" link). Close button / Esc /
  backdrop → back to the page it was opened from. Clicks on toasts don't close it.

### 7.1 Core › General — `pages/settings/CoreSettings.tsx`  
**Package: D.**

- [ ] Data: `GET /global_settings` → `GlobalSettingsData {core_name, safe_mode, domain, playit_enabled,
  code_execution_enabled, code_execution_locked, allow_lan_http, list_instances_publicly,
  allow_local_claim}`; `GET /info`. Edits only by owner (`is_owner`); others see read-only values.
- [ ] Warning banner when `allow_lan_http` is on (owner/other wording).
- [ ] **Core** section:
  - Core name: text, not empty, ≤32 → `PUT /global_settings/name`.
  - Public domain/IP: text, may be empty, ≤253, placeholder current address, tooltip → `PUT
    /global_settings/domain`. Used for "Copy IP" everywhere.
  - Enable Playit.gg toggle → `PUT /global_settings/playit_enabled` then `POST /playitgg/stop_cli`.
- [ ] **Public instances** section (owner, cores that support it): "List instances publicly" toggle →
  `PUT /global_settings/list_instances_publicly`; per-instance Public toggle + Status/Address/Client pack
  checkboxes for every Minecraft Java instance from `GET /public_listing` → `PublicListingOverview` (same
  row component as §6.1, `PUT /instance/{uuid}/public_listing`); "No servers" empty row.
- [ ] **Information** (read-only): Core version, Dashboard version, OS, Architecture, CPU, CPU cores, Total
  RAM, HTTPS certificate mode (self-signed / user-provided / none), certificate SHA-256 fingerprint.
- [ ] **Danger zone**:
  - Safe mode toggle (turning on saves; turning off asks confirm) → `PUT /global_settings/safe_mode`.
  - Code execution toggle (owner; disabled if locked, with "locked by administrator" note; hint differs
    when core has no Deno: only custom Java commands) → turning off asks danger confirm → `PUT
    /global_settings/code_execution`, then refetch.
  - Allow plain HTTP from LAN (owner, if supported) → turning on asks danger confirm → `PUT
    /global_settings/allow_lan_http`.
  - Allow local claim (owner, if supported) → `PUT /global_settings/allow_local_claim`.
  - Open port (UPnP): button (owner) → modal with port (1–65535, default 25565) → `PUT
    /gateway/open_port/{port}`; error inline; toast "Port N opened".
- [ ] All setting PUTs send a JSON value body; success → "Saved" toast and cache update.

### 7.2 Core › Users — `pages/settings/UserSettings.tsx`, `components/UserBox.tsx`, `components/Settings/*`  
**Package: D.**

- [ ] User list only fetched for the owner: `GET /user/list` → `PublicUser[]`. Selected user in `?user=`.
- [ ] List view: "All members (N)", **Create user** button, rows (avatar, name — click opens user, tags
  You/Admin/Owner, "…" menu: **Change password**, **Delete user** (disabled for self)).
  - Create user dialog: Username (required), Password (required), Confirm (match) → `POST /user
    {username,password}` → `PublicUser`; added to list.
  - Change password dialog: self → Old password + new + confirm; other user → new + confirm (no old) →
    `PUT /user/{uid}/password {uid, old_password|null, new_password}`; error box.
  - Delete user confirm → `DELETE /user/{uid}`; removed from cache (no error handling).
- [ ] User detail view (header "username · All users" link):
  - **Permissions** (normal): `can_create_instance`, `can_delete_instance`, `can_read_global_file` (toggles);
    `can_view_instance`, `can_start_instance`, `can_stop_instance`, `can_access_instance_console`,
    `can_access_instance_setting`, `can_read_instance_resource`, `can_read_instance_file` (per-instance
    checkbox grid of instance names) — `components/Settings/permissionDescriptions.ts`,
    `components/Atoms/MultiSelectGrid.tsx`.
  - **Unsafe permissions** (danger card; disabled while safe mode is on): `can_write_instance_resource`,
    `can_write_instance_file` (per instance), `can_write_global_file`, `can_manage_permission` (toggles);
    plus a disabled "Owner only" row explaining macros/extensions/atoms are owner-only (or "no scripting").
  - Each change: `PUT /user/{uid}/update_perm` body full `UserPermission`; changes are queued and each is
    computed from the latest cached permissions; "Saved" toast; edits only by owner.
  - Not editable/listed: `can_access_instance_macro`, `can_install_extension`.
  - **API keys** of that user (§7.4, revoke only).
  - **Minecraft accounts** of that user (owner): list + Remove (§7.5).
- [ ] Non-owners see no user list (the page is effectively empty for them).

### 7.3 Account › Profile — `pages/settings/profile.tsx`  
**Package: D.**

- [ ] Toast warning if not logged in; skeleton while loading.
- [ ] Username (inline save) → `PUT /user/{uid}/rename` (JSON string); updates self + list caches.
- [ ] Password: **Change password** button → dialog (old/new/confirm) → `PUT /user/{uid}/password`.
- [ ] **Appearance**: Language select (§1.8), Theme segmented control (§1.8), "Coloured console" toggle
  (local only).
- [ ] **API keys** section (own keys, §7.4).
- [ ] **Minecraft accounts** section (own links, §7.5).

### 7.4 API keys — `components/Settings/ApiKeys/*`, `data/ApiKeys.ts`  
**Package: D.**

- [ ] Own keys: `GET /user/api_keys` (refetched every visit) → `ApiKeyInfo[]`. Other user's keys (owner/
  admin): `GET /user/{uid}/api_keys`. 4xx not retried; 403 for API-key sessions shown as "only from a
  signed-in session".
- [ ] Key row: name, display prefix `lsk_<id>_…`, scope summary ("Full access", "Custom: N servers, M
  permissions", launcher variants with/without console read/write), Created date, Expires/Expired (red)/
  Never, Last used ("x ago"/never); **Revoke** → confirm → `DELETE /user/api_keys/{id}` or
  `DELETE /user/{uid}/api_keys/{id}`; toast.
- [ ] **Create key** dialog (own only): Name (required, ≤64), Expires in (30 days / 90 days (default) /
  1 year / Never — warning for Never), Access (Full / Custom). Custom → scope editor with the same
  permission list as §7.2, limited to what the user can grant (owner all; admin implicit set: view, start,
  stop, console, settings, read resource, read file, create, delete; else own grants); per-instance grids
  with non-grantable instances disabled; must pick ≥1 permission. → `POST /user/api_keys
  CreateApiKeyRequest {name, expires_at (unix s|null), scope}` → `CreatedApiKey {key, info}`.
- [ ] **New key** dialog (can't be dismissed by Esc/backdrop, only Done): key in a copyable code block,
  CLI example `LODESTONE_URL=<proto://addr:port> LODESTONE_TOKEN=<key> lodestone_cli instance list`
  (copyable), "store it now" warning; key never cached. Copy toasts never include the key.

### 7.5 Minecraft account links — `components/Settings/MinecraftLinks.tsx`, `data/MinecraftLinks.ts`  
**Package: D.**

- [ ] Own: `GET /user/minecraft_links` → `MinecraftLink[]`; remove `DELETE /user/minecraft_links/{mcUuid}`.
- [ ] Owner, all users: `GET /minecraft_links` → `MinecraftLinkInfo[]`; remove `DELETE
  /minecraft_links/{mcUuid}`.
- [ ] Row: Minecraft name, UUID (mono), Remove (confirm). 404 → "core too old". Links are created in
  Lodestone Launcher, not here.
- [ ] Also used to annotate players (Overview) and console lines with Lodestone usernames.

### 7.6 Not present  
**Package: nobody.**

- [ ] No system-stats page (`/system/ram|disk|cpu` hooks and `SystemStat.tsx` exist but are unused
  **(dead)**), no core updater UI, no global event-log page, no extension management, no backup UI.
  Version/info appears only in Core › General › Information and the connection tooltip.

---

## 8. Tauri host (`src-tauri/*`, `src/utils/tauriUtil.ts`)

### 8.1 Process / core in-process — `src-tauri/src/main.rs`, `core/src/tauri_export.rs`  
**Package: foundation (done, as a child process).**

- [x] Runs `lodestone_core::run(Args { is_cli: false, is_host: true, lodestone_path: None,
  disable_code_execution: false })` on the same tokio runtime before creating the window; core reports
  `product: "host"`, `launched_by: "host"` in `/info`.
- [x] Installs rustls aws-lc-rs crypto provider first.
- [x] Core start failure → native desktop notification "Lodestone Host failed to start: <error>"
  (`notify-rust`) and exit.
- [x] If the core future ends on its own (not via Quit) → `process::exit(128)`.
- [x] **No Deno, ever:** the Host bundles a core built with `--no-default-features`, and shows scripting as unavailable. (The Tauri Host had:) Cargo feature `deno` (default) passes through to core (macros/atoms); `--no-default-features` build
  has no scripting (dashboard reacts via capabilities).

### 8.2 Commands exposed to JS (all permission-free) — `src-tauri/src/main.rs`  
**Package: foundation (done: replaced by the local claim).**

- [x] `is_setup() -> bool` — owner account exists (`tauri_export::is_owner_account_present`). Used by
  FirstTime.
- [x] `setup_owner_account(username, password)` — creates owner directly, clears setup key; error if owner
  exists. **Not called anywhere in the dashboard (dead)**; the dashboard uses `POST /setup/{key}` instead.
- [x] `get_first_time_setup_key() -> String` (Err if none) — pre-fills owner setup form.
- [x] `get_owner_jwt() -> JwtToken` (Err if no owner) — silent owner sign-in on the local core.
- [x] `is_debug_build() -> bool` — DEV badge.

### 8.3 Window — `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`  
**Package: foundation (done); D: theme and zoom.**

- [ ] Single window `main`, 1100×600, min 1000×550, resizable, title "Lodestone Host", macOS overlay title
  bar with hidden title, traffic lights at (16, 21); drag regions provided by the page
  (`core:window:allow-start-dragging`).
- [ ] OS drag-and-drop into the webview disabled (`dragDropEnabled: false`) so HTML5 file drops work.
- [x] Close button **hides** the window (prevent_close); app keeps running in the tray. No macOS Dock
  "reopen" handler.
- [ ] JS permissions: `core:default`, set-theme (window appearance follows dashboard theme), set-webview-zoom
  (zoom shortcuts), opener open-url/default-urls (`<a target=_blank>` http(s) links open in system browser).
- [x] Debug builds: DEV window icon (non-macOS) / DEV Dock icon set at runtime via AppKit (macOS).

### 8.4 Serving the dashboard — `main.rs`  
**Package: not needed (no web view).**

- [ ] Release builds serve the static export via `tauri-plugin-localhost` on the first free port of
  16663–16672 (stable origin so localStorage — cores, tokens, settings — survives restarts), else any free
  port; IPC enabled on that URL. Dev builds use `http://localhost:3001`.
- [ ] `withGlobalTauri: true`, CSP disabled; JS detects Tauri via `window.__TAURI__.core`
  (`utils/tauriUtil.ts`: `tauri`, `useTauri`, `isTauriMacOS`, `setTauriWindowTheme`, `useIsDevBuild`).

### 8.5 Tray — `main.rs`  
**Package: foundation (macOS done); D: Windows and Linux.**

- [ ] Template tray icon (`icons/icon.png`); menu with single **Quit** item.
- [ ] Left click (button up) shows + focuses the main window; on macOS left click also opens the menu.
- [x] Quit: signals core shutdown (stop servers), waits up to 10 s for it, then exits.

### 8.6 Single instance — `main.rs`  
**Package: foundation (done).**

- [x] `tauri-plugin-single-instance`: a second launch emits event `single-instance` `{args, cwd}` to the
  first; **nothing in the dashboard listens to it**, and the window isn't shown/focused.

### 8.7 Updater — `main.rs`, `tauri.conf.json`  
**Package: D.**

- [ ] On startup (Linux: only AppImage or dev), checks `https://raw.githubusercontent.com/Lodestone-Team/
  updates/main/updates.json` (minisign pubkey in config). If an update exists: native Yes/No dialog "A new
  version of Lodestone Host is available!" with versions and release notes → download+install → second
  dialog "Ready to restart" → restart. Errors logged only. Bundles `v1Compatible` updater artifacts.

### 8.8 Other Tauri facts  
**Package: D (packaging).**

- [ ] Product "Lodestone Host" v0.5.1, identifier `com.lodestone.cc`, bundle targets all (dmg/msi/deb/
  AppImage…), WiX upgrade code fixed.
- [ ] No deep links / URL scheme, no app menu bar, no global shortcuts, no auto-start at login, no
  in-app notifications from Rust beyond the startup-failure notification.
- [ ] Download links (`/file/{key}`) are opened with `window.open(url, '_blank')` — in Host this relies
  on the opener/webview behaviour; a native app should save files itself.

---

## 9. Local storage / persisted client state (checklist for the native settings store)  
**Package: D (each package stores its own page state through D's settings store).**

- [ ] `address`, `port`, `protocol`, `apiVersion` — current core.
- [ ] `cores` — saved core list.
- [ ] `tokens` — `{ "addr:port": token }`.
- [ ] `latestVersion` — last seen core version (beta dialog).
- [ ] `leftNavSize` (200–280), `fileListSize`.
- [ ] `coloredConsole` (bool, default true).
- [ ] `theme` (`system|light|dark`), `language` (`en|fr|zh-Hans|zh-Hant`, absent = auto).
- [ ] `lodestone-zoom` (Host only).
- [ ] `instanceDetailsOpen` (bool).
- [ ] sessionStorage `settingsReturnTo`.

---

## 10. Core API checklist (everything the dashboard calls)  
**Package: each package, for its routes.**

All paths are under `/api/v1`. Bodies are JSON unless noted. Types are in `dashboard/src/bindings`.
"str" = JSON string body (`axiosPutSingleValue`), "bool" = JSON boolean.

### 10.1 Unauthenticated / setup / auth

| Method | Path | Request | Response | Used by |
|---|---|---|---|---|
| GET | `/info` | – | `CoreInfo` (local type in `data/SystemInfo.ts`: version, is_setup, os, arch, cpu, cpu_count, total_ram, total_disk, host_name, uuid, core_name, up_since, tls?: `TlsInfo`, capabilities?: `CoreCapabilities`, product?: `CoreProduct`, launched_by?: `LaunchedBy`) | connect, guards, status, settings |
| POST | `/setup/{setupKey}` | `{username, password}` | `LoginReply` | owner setup |
| POST | `/user/login` | `{}` + HTTP Basic auth | `LoginReply {token: JwtToken, user: PublicUser}`; 429 + `Retry-After` | sign in |
| POST | `/user/login_api_key` | `{key}` | `{user: PublicUser}` | API-key sign in |
| GET | `/file/{key}` | – | file download (key-authorised) | downloads, client pack |

### 10.2 Users, permissions, API keys, Minecraft links

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/user/info` | – | `PublicUser` |
| GET | `/user/list` | – | `PublicUser[]` |
| POST | `/user` | `{username, password}` | `PublicUser` |
| DELETE | `/user/{uid}` | – | – |
| PUT | `/user/{uid}/password` | `{uid, old_password: string\|null, new_password}` | – |
| PUT | `/user/{uid}/rename` | str | – |
| PUT | `/user/{uid}/update_perm` | `UserPermission` | – |
| GET | `/user/api_keys` | – | `ApiKeyInfo[]` |
| POST | `/user/api_keys` | `CreateApiKeyRequest` | `CreatedApiKey` |
| DELETE | `/user/api_keys/{id}` | – | – |
| GET | `/user/{uid}/api_keys` | – | `ApiKeyInfo[]` |
| DELETE | `/user/{uid}/api_keys/{id}` | – | – |
| GET | `/user/minecraft_links` | – | `MinecraftLink[]` |
| DELETE | `/user/minecraft_links/{minecraftUuid}` | – | – |
| GET | `/minecraft_links` | – | `MinecraftLinkInfo[]` (owner) |
| DELETE | `/minecraft_links/{minecraftUuid}` | – | – (owner) |

### 10.3 Global settings / core

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/global_settings` | – | `GlobalSettingsData` |
| PUT | `/global_settings/name` | str | – |
| PUT | `/global_settings/domain` | str | – |
| PUT | `/global_settings/safe_mode` | bool | – |
| PUT | `/global_settings/code_execution` | bool | – |
| PUT | `/global_settings/allow_lan_http` | bool | – |
| PUT | `/global_settings/playit_enabled` | bool | – |
| PUT | `/global_settings/allow_local_claim` | bool | – |
| PUT | `/global_settings/list_instances_publicly` | bool | – |
| GET | `/public_listing` | – | `PublicListingOverview` |
| PUT | `/gateway/open_port/{port}` | – | – |
| GET | `/system/ram`, `/system/disk`, `/system/cpu` | – | `{total,free}` / `{total,free}` / `{cpu_speed,cpu_load}` — defined, unused |

### 10.4 Instances

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/games` | – | `HandlerGameType[]` |
| GET | `/setup_manifest/{HandlerGameType}` | – | `SetupManifest` |
| PUT | `/generic_setup_manifest` | `{url}` | `SetupManifest` (dead path) |
| POST | `/instance/create/{HandlerGameType}` | `SetupValue` | – (progress via events) |
| POST | `/instance/create_generic` | `{url, setup_value: SetupValue}` | – (dead path) |
| POST | `/instance/create_modpack` | `CreateModpackInstanceRequest` | `InstanceUuid` |
| GET | `/instance/list` | – | `InstanceInfo[]` |
| DELETE | `/instance/{uuid}` | – | – (progress via events) |
| PUT | `/instance/{uuid}/start` \| `/stop` \| `/kill` \| `/restart` | – | – |
| PUT | `/instance/{uuid}/name` | str | – |
| PUT | `/instance/{uuid}/description` | str | – |
| PUT | `/instance/{uuid}/auto_start` | bool | – |
| PUT | `/instance/{uuid}/restart_on_crash` | bool | – |
| GET | `/instance/{uuid}/settings` | – | `ConfigurableManifest` |
| PUT | `/instance/{uuid}/settings/{sectionId}/{settingId}` | `ConfigurableValue \| null` | – |
| GET | `/instance/{uuid}/public_listing` | – | `InstancePublicListing` |
| PUT | `/instance/{uuid}/public_listing` | `PublicScopeChange` | `PublicScope` |
| GET | `/instance/{uuid}/resource_pack` | – | `ServerResourcePack` |
| PUT | `/instance/{uuid}/resource_pack` | `ServerResourcePack` | `ServerResourcePack` |
| POST | `/instance/{uuid}/resource_pack/hash` | `{url}` | `ResourcePackHash` |
| POST | `/instance/{uuid}/console` | str (command) | – |
| GET | `/instance/{uuid}/console/buffer` | – | `ClientEvent[]` |
| GET / POST / DELETE | `/instance/{uuid}/share` | – | `InstanceShare` (Share with friends) |

### 10.5 Instance files (`{b64}` = URL-safe Base64 of the instance-relative path)

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/instance/{uuid}/fs/{b64}/ls` | – | `ClientFile[]` |
| GET | `/instance/{uuid}/fs/{b64}/read` | – | raw text |
| PUT | `/instance/{uuid}/fs/{b64}/write` | raw text | – |
| PUT | `/instance/{uuid}/fs/{b64}/new` | – | – |
| PUT | `/instance/{uuid}/fs/{b64}/mkdir` | – | – |
| DELETE | `/instance/{uuid}/fs/{b64}/rm` | – | – |
| DELETE | `/instance/{uuid}/fs/{b64}/rmdir` | – | – |
| PUT | `/instance/{uuid}/fs/{b64src}/move/{b64dest}` | – | – |
| PUT | `/instance/{uuid}/fs/cpr` | `CopyInstanceFileRequest` | – |
| PUT | `/instance/{uuid}/fs/zip` | `ZipRequest` | – (completion via `FSOperationCompleted`) |
| PUT | `/instance/{uuid}/fs/{b64}/unzip` | `UnzipOption` | – (completion via event) |
| PUT | `/instance/{uuid}/fs/{b64dir}/upload` | multipart, field `file` ×N | – |
| GET | `/instance/{uuid}/fs/{b64}/url` | – | string key → `GET /file/{key}` |

### 10.6 Mods, Modrinth, modpacks

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/instance/{uuid}/mods[?check_updates=true]` | – | `InstalledModList` |
| POST | `/instance/{uuid}/mods/install` | `ModInstallRequest` | `ModInstallResult` |
| POST | `/instance/{uuid}/mods/upload[?overwrite=true]` | multipart `file` ×N | `InstalledMod[]` (409 on conflict) |
| PUT | `/instance/{uuid}/mods/{fileName}/enable` \| `/disable` | – | `InstalledMod` |
| DELETE | `/instance/{uuid}/mods/{fileName}` | – | – |
| GET | `/instance/{uuid}/mods/client_pack` | – | `ClientPackDownload` → `/file/{key}` |
| GET | `/modrinth/search` | query: `query, project_type, game_version, loader, index, offset, limit` | `ModrinthSearchResponse` |
| GET | `/modrinth/project/{idOrSlug}` | – | `ModrinthProject` |
| GET | `/modrinth/project/{idOrSlug}/versions` | query: `game_version, loader` | `ModrinthVersion[]` |
| GET | `/modrinth/version/{versionId}` | – | `ModrinthVersion` |
| GET | `/modrinth/resolve` | query: `url` | `ModrinthUrlTarget` |
| POST | `/modpack/inspect` | `ModpackInspectRequest {version_id}` | `ModpackInfo` |
| POST | `/modpack/upload` | multipart `file` | `ModpackInfo` (with `upload_id`) |

### 10.7 Macros / tasks

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/instance/{uuid}/macro/list` | – | `MacroEntry[]` |
| GET | `/instance/{uuid}/task/list` | – | `TaskEntry[]` |
| GET | `/instance/{uuid}/history/list` | – | `HistoryEntry[]` |
| PUT | `/instance/{uuid}/macro/run/{name}` | `string[]` args | – |
| PUT | `/instance/{uuid}/macro/kill/{pid}` | – | – |
| GET | `/instance/{uuid}/macro/config/get/{name}` | – | `GetConfigResponse` |
| POST | `/instance/{uuid}/macro/config/store/{name}` | `{[name]: SettingManifest}` | – |

### 10.8 Playit.gg

| Method | Path | Request | Response |
|---|---|---|---|
| GET | `/playitgg/generate_signup_link` | – | `PlayitSignupData {url, claim_code}` |
| POST | `/playitgg/verify_key` | – | bool |
| POST | `/playitgg/start_cli` | – | – |
| POST | `/playitgg/stop_cli` | – | – |
| GET | `/playitgg/cli_is_running` | – | bool |
| GET | `/playitgg/get_tunnels` | – | `PlayitTunnelInfo[]` |

### 10.9 Events (REST + WebSocket)

| Kind | Path | Notes |
|---|---|---|
| GET | `/events/all/buffer?filter=<EventQuery JSON>` | all filters null, `bearer_token` null; → `ClientEvent[]` |
| WS | `/events/all/stream?filter=<EventQuery JSON>` | `bearer_token` = raw token; messages `ClientEvent` |
| WS | `/instance/{uuid}/console/stream?token=Bearer%20<token>` | messages `ClientEvent` (`InstanceOutput`); close 1000 = closed by server |
| WS | `/monitor/{uuid}?token=Bearer%20<token>` | messages `PerformanceReport`, replays last 60 s |

`ClientEvent {event_inner: EventInner, details, snowflake, level: EventLevel, caused_by: CausedBy}`.
Event variants the dashboard handles (everything else must at least be tolerated):

- [ ] `InstanceEvent` {instance_uuid, instance_name, instance_event_inner}: `StateTransition {to}`,
  `InstanceWarning {message}`, `InstanceError {message}`, `PlayerChange {player_list, players_joined,
  players_left}`, `PlayerMessage {player, player_message}`; `InstanceOutput` (console only),
  `InstanceInput`, `SystemMessage` (ignored).
- [ ] `UserEvent` {user_id, user_event_inner}: `UserCreated`, `UserDeleted`, `UsernameChanged
  {new_username}`, `PermissionChanged {new_permissions}`, `ApiKeyCreated {key_id, name}`, `ApiKeyRevoked
  {key_id, name}`; `UserLoggedIn`, `UserLoggedOut` (ignored).
- [ ] `MacroEvent` {instance_uuid, macro_pid, macro_event_inner}: `Started`, `Detach`, `Stopped
  {exit_status: ExitStatus}`.
- [ ] `ProgressionEvent` {event_id, progression_event_inner}: `ProgressionStart {progression_name, total,
  inner: ProgressionStartValue (InstanceCreation|InstanceDelete {instance_uuid})}`, `ProgressionUpdate
  {progress_message, progress}`, `ProgressionEnd {success, message, inner: ProgressionEndValue
  (InstanceCreation & InstanceInfo | InstanceDelete {instance_uuid} | FSOperationCompleted {instance_uuid,
  success, message})}`.
- [ ] `PlayitggRunnerEvent`: `RunnerStarted`, `RunnerLoading`, `RunnerStopped`.
- [ ] `FSEvent {operation, target}` (ignored).
- [ ] `GlobalSettingsChanged` (no payload).

### 10.10 Tauri IPC (Host only)

| Command | Args | Returns | Used |
|---|---|---|---|
| `is_setup` | – | bool | FirstTime |
| `setup_owner_account` | username, password | () / Error | unused |
| `get_first_time_setup_key` | – | string / Err | CoreSetupNew |
| `get_owner_jwt` | – | JwtToken / Err | UserSelectExisting |
| `is_debug_build` | – | bool | DEV badge |
| event `single-instance` (Rust → JS) | – | `{args, cwd}` | not listened to |
| JS → window API | `getCurrentWindow().setTheme`, `getCurrentWebview().setZoom`, drag regions | | theme, zoom, macOS title bar |
