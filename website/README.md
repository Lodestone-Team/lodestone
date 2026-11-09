# www.lodestone.cc

The site has two parts, built together by `build.mjs` into `website/dist`:

| Path    | What                                   | Source           |
| ------- | -------------------------------------- | ---------------- |
| `/`     | Lodestone Launcher landing page        | `website/src`    |
| `/core` | Lodestone Core web dashboard           | `dashboard/`     |

The landing page is plain HTML, CSS and JavaScript with no build step. The
dashboard is built with `DASHBOARD_BASE_PATH=/core`, which sets Next's
`basePath` and prefixes the dashboard's own `public/` files (see
`dashboard/src/utils/basePath.ts`). Builds without that variable, like the Tauri
app, are served from the root as before.

## Working on it

```bash
node website/build.mjs                  # everything
node website/build.mjs --landing-only   # just the landing page
node website/serve.mjs                  # http://localhost:8888, with _redirects applied
```

To run it on Cloudflare's own runtime instead, build first, then run
`npx wrangler dev` in `website/`.

## The landing page

- **Download button.** `main.js` detects the visitor's OS and picks the
  matching file from the `launcher-latest` release through the GitHub API.
  Without JavaScript, or when the API is unavailable, it links to
  `LodestoneLauncherSetup.exe`. Platforms with no build yet show "Coming soon".
  Nothing here needs changing when a macOS or Linux build is published, as long
  as it follows `publish_release.py`'s file names.
- **Star count.** It comes from the GitHub API too, cached in `localStorage` for
  an hour (the unauthenticated API allows 60 calls an hour per visitor).
- **Download count.** The badge above the headline ("109k+ downloads") counts
  Lodestone Core's downloads: pulls of its Docker image plus downloads from its GitHub releases
  (see `stats.mjs`). `build.mjs` works it out on each deploy and writes it into
  the page. A source that can't be read counts as its last known figure (`KNOWN`
  in `stats.mjs`, raise it now and then), so the number never drops. Set
  `GITHUB_TOKEN` as a build variable in Cloudflare to avoid the API's
  unauthenticated limit.
- **Screenshots.** `src/images/<view>.webp` (2240×1400) and `<view>-1120.webp`
  (half size), taken from the launcher at 150% zoom in a 1493×933 window. The
  tabs above the screenshot list the views.

## Deploying

The site is a Cloudflare Worker with static assets, built from this repository
by Cloudflare's Git integration (Workers Builds). Its build settings:

| Setting         | Value               |
| --------------- | ------------------- |
| Root directory  | `website`           |
| Build command   | `node build.mjs`    |
| Deploy command  | `npx wrangler deploy` |

`wrangler.jsonc` names the Worker and points it at `dist`. `.node-version` asks
for Node 24, which the dashboard needs.

How a request is answered, in order:

1. `src/_redirects` sends the dashboard's old addresses (`/first_setup`,
   `/login/*`, `/dashboard/*`, `/settings/*`) to `/core`, keeping the query
   string.
2. A file in `dist`, with the cache headers in `src/_headers`.
3. `worker.js`: any other `/core` path gets the dashboard's page, since it is a
   single-page app. Cloudflare's `_redirects` can't do this: a catch-all rule
   there would also catch the dashboard's own files.

Redirecting `lodestone.cc` to `www.lodestone.cc` is a Cloudflare setting (a
redirect rule on the zone), not part of this repository.
