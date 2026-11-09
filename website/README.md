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
node website/serve.mjs                  # http://localhost:8888, with netlify.toml's redirects
```

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
  the page; if a source can't be read, the page keeps its default, "100k+". Set
  `GITHUB_TOKEN` in Netlify's environment to avoid the API's unauthenticated
  limit.
- **Screenshots.** `src/images/<view>.webp` (2240×1400) and `<view>-1120.webp`
  (half size), taken from the launcher at 150% zoom in a 1493×933 window. The
  tabs above the screenshot list the views.

## Deploying

Netlify builds the site from `website/netlify.toml`, so the site's **Base
directory** setting in Netlify must be `website` (the build and publish paths in
that file are relative to it). Keeping the file out of the repository root means
other Netlify sites built from this repository, like Storybook, are unaffected.
`netlify.toml` also redirects the dashboard's old addresses (`/first_setup`,
`/login/*`, `/dashboard/*`, `/settings/*`) to `/core`, and sends any other
`/core/*` path to the dashboard's `index.html`.
