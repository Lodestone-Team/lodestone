# www.lodestone.cc

The site has four parts, built together by `build.mjs` into `website/dist`:

| Path        | What                                       | Source                  |
| ----------- | ------------------------------------------ | ----------------------- |
| `/`         | Home page: all the Lodestone products      | `website/src`           |
| `/launcher` | Lodestone Launcher page, with the download | `website/src/launcher`  |
| `/host`     | Lodestone Host page, with the download     | `website/src/host`      |
| `/host/app` | Lodestone Host web dashboard               | `dashboard/`            |

The three pages are plain HTML, CSS and JavaScript with no build step. They
share the fonts in `src/fonts`; everything else of the launcher and Host pages'
is under `src/launcher` and `src/host`. The
dashboard is built with `DASHBOARD_BASE_PATH=/host/app`, which sets Next's
`basePath` and prefixes the dashboard's own `public/` files (see
`dashboard/src/utils/basePath.ts`). Builds without that variable, like the Tauri
app, are served from the root as before.

## Working on it

```bash
node website/build.mjs                  # everything
node website/build.mjs --landing-only   # just the pages
node website/serve.mjs                  # http://localhost:8888, with _redirects applied
```

To run it on Cloudflare's own runtime instead, build first, then run
`npx wrangler dev` in `website/`.

## The home page

- **Logo.** The launcher's gem (`src/images/mark.svg`) and Host's mark
  (`src/images/host-mark.svg`) take turns in the hero, every 4.5 seconds while
  it's on screen (`switchMark()` in `main.js`), with the product's name under
  it as a link. Both are drawn inline in `index.html` so `styles.css` can
  animate them: on load the gem turns into place and the L rises into it; at a
  switch the logo leaving turns away and the one arriving turns in. The
  switching holds while the pointer is on the logo or its name, and with
  reduced motion the gem stays. The header shows the wordmark alone
  (`src/images/wordmark.svg`).
- **Iron filings.** `main.js` draws a grid of short strokes on a canvas behind
  the pair and turns each one along the field of a magnet at the pair; the
  pointer is a second magnet. It stops drawing while the hero is off screen,
  and with reduced motion it draws them once, already lined up.
- **Lodestone Connect.** Its own section between the hero and the cards
  (`#connect`, also in the header), in Lodestone's
  teal and green: your Lodestone Host's invite link and a friend's launcher joined by a
  line, then the three steps of sharing by link. The friend list gets one line
  under the steps: it's an extra, not the way in. The two panels are illustrations, not live data. When the
  section scrolls into view, `main.js` fills the launcher's bar once while it
  "gets the server's mods"; without JavaScript or with reduced motion it shows
  the end. Connect isn't a download of its own; the header and the cards
  below link to the launcher and Host, which have it built in.
- **Cards.** The Host card's server list and the CLI card's commands are
  illustrations, not live data. The CLI commands are real `lodestone_cli`
  commands; their output is shortened.
- **Questions.** Three answers above the footer. Keep them true: the apps send
  no analytics (the dashboard's `react-ga4` is never initialized), game traffic
  goes directly between computers, and connect.lodestone.cc only introduces them.
- **Social preview.** `src/images/og.jpg` is a 1200×630 capture of the hero.
- **Download count and star count** work as on the launcher page (below).

## Speed sections

The launcher and Host pages each end with a comparison ("Faster than the rest",
"Light enough to run beside your game"). Every number has a source, named in
the section's footnote; change one only with a new measurement.

- **Launcher:** the six-launcher benchmark on one Windows 11 PC (Ryzen 9
  9950X3D, 64 GB), 10 and 11 November 2026: window on screen, ready, CPU at
  idle, installed size. Prism uses slightly less memory at idle, and the
  footnote says so.
- **Host:** published download sizes only (GitHub releases and Docker Hub,
  12 November 2026). Lodestone Host's is the new Host app's developer build 3845;
  update it when a host-v release is out. Nothing has been benchmarked against
  other server managers yet.

## The Host page

The launcher page's layout in Host's green, with the dashboard's logo.

- **Download button.** `host/main.js` offers the Lodestone Host app from the
  newest published stable `host-vX.Y.Z` release: `LodestoneHostSetup.exe` on
  Windows, `LodestoneHost.dmg` on macOS. Until there is one for the visitor's
  platform (and on Linux), it picks Lodestone CLI from the latest
  `Lodestone-Team/lodestone_cli` release (`lodestone_cli_<os>_<arch>[.exe]`; no
  Intel Mac build), which installs Host, keeps it updated and starts it.
  Without JavaScript it links to the CLI's Windows build. "Open the dashboard"
  goes to `/host/app/`.
- **Claims.** Only what Host does today: Vanilla, Fabric, Forge and Paper
  servers; Fabric modpacks only; sharing with friends through Lodestone Connect,
  from the friend list or by invite link; players' launchers getting the
  server's mods; only this computer's apps reaching it until other devices are
  allowed; users and permissions; API keys and the CLI. Docker management isn't
  finished, so the page leaves it out. The page doesn't call Host a desktop app
  until the download button offers one.
- **Screenshots.** `src/host/images/<view>.webp` (2240×1400) and
  `<view>-1120.webp`: the dashboard at `/host/app` in Edge, 1493×933 at 150%,
  dark theme, against a local Host with three servers on 26.3 (a Fabric one
  running, with Fabric API, Lithium, FerriteCore, Chunky and spark), the owner
  `mossy_toast` and two friends.

## The launcher page

- **Download button.** `main.js` detects the visitor's OS and picks the
  matching file from the newest published stable launcher release (tagged
  `launcher-vX.Y.Z`, the one with the highest build) through the GitHub API:
  `LodestoneLauncherSetup.exe` on Windows, `LodestoneLauncher.dmg` on macOS.
  Without JavaScript, or when the API is unavailable, it links to
  `LodestoneLauncherSetup.exe` in the old `launcher-latest` release, an older
  build that updates itself. Platforms with no build yet show "Coming soon".
  Nothing here needs changing when a Linux build is published, as long as it
  follows `publish_release.py`'s file names. Betas (pre-releases,
  `launcher-vX.Y.Z-beta.N`) aren't offered here.
- **Star count.** It comes from the GitHub API too, cached in `localStorage` for
  an hour (the unauthenticated API allows 60 calls an hour per visitor).
- **Download count.** The badge above the headline ("109k+ downloads") counts
  Lodestone Host's downloads (it was called Lodestone Core): pulls of its Docker image plus downloads from its GitHub releases
  (see `stats.mjs`). `build.mjs` works it out on each deploy and writes it into
  the page. A source that can't be read counts as its last known figure (`KNOWN`
  in `stats.mjs`, raise it now and then), so the number never drops. Set
  `GITHUB_TOKEN` as a build variable in Cloudflare to avoid the API's
  unauthenticated limit.
- **Points.** The list under the screenshots: joining a friend's server from
  an invite link, automatic setup of new instances, Lodestone Host's servers in
  the launcher, and, last, the friend list (servers and LAN worlds without a
  link). Invite links lead; the friend list is an extra.
- **Screenshots.** `src/launcher/images/<view>.webp` (2240×1400) and `<view>-1120.webp`
  (half size), taken from the launcher at 150% zoom in a 1493×933 window. The
  tabs above the screenshot list the views.

## Deploying

The site is a Cloudflare Worker with static assets, built from this repository
by Cloudflare's Git integration (Workers Builds). Its build settings:

| Setting                    | Value                      |
| -------------------------- | -------------------------- |
| Root directory             | `website`                  |
| Build command              | `node build.mjs`           |
| Deploy command             | `npx wrangler deploy`      |
| Build watch paths: include | `website/*`, `dashboard/*` |

The watch paths are relative to the repository root and `*` covers subfolders.
They make Cloudflare build only for commits that change the site or the
dashboard it serves at `/host/app`. Commits to the launcher or Host alone don't
redeploy the site, and their PRs don't get a Cloudflare preview build.

`wrangler.jsonc` names the Worker, puts it on Wenwei's account and on
`www.lodestone.cc` (a custom domain, so Cloudflare manages its DNS record), and
points it at `dist`. `.node-version` asks
for Node 24, which the dashboard needs.

How a request is answered, in order:

1. `src/_redirects` sends the dashboard's old addresses (`/first_setup`,
   `/login/*`, `/dashboard/*`, `/settings/*`) and its later homes, `/core` and
   `/host`, to `/host/app`, keeping the query string.
2. A file in `dist`, with the cache headers in `src/_headers`.
3. `worker.js`: any other `/host/app` path gets the dashboard's page, since it is a
   single-page app. Cloudflare's `_redirects` can't do this: a catch-all rule
   there would also catch the dashboard's own files.

Redirecting `lodestone.cc` to `www.lodestone.cc` is a Cloudflare setting (a
redirect rule on the zone), not part of this repository. Until one is added,
the apex record still forwards to Netlify, which does the redirect.
