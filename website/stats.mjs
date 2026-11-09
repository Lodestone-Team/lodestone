// Lodestone Host's download count (Host was called Lodestone Core), for the
// landing pages. build.mjs calls it
// and writes the result into dist/index.html, so the number is as fresh as the
// last deploy.
//
// The count is:
//   - pulls of Core's Docker image (ghcr.io/lodestone-team/lodestone_core),
//     read from its GitHub package page: the packages API doesn't expose it;
//   - downloads of the files on Core's GitHub releases: this repo's (except the
//     launcher's), lodestone_core's (the old Core repo) and lodestone_cli's.
//
// Set GITHUB_TOKEN to avoid the API's limit of 60 unauthenticated calls an hour.

const ORG = 'Lodestone-Team';
const IMAGE_PAGE = `https://github.com/orgs/${ORG}/packages/container/package/lodestone_core`;
const RELEASE_REPOS = ['lodestone', 'lodestone_core', 'lodestone_cli'];
const isLauncherRelease = (tag) => tag.startsWith('launcher-');

async function get(url, json) {
  const headers = { 'User-Agent': 'lodestone-website-build' };
  if (json) headers.Accept = 'application/vnd.github+json';
  if (json && process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  const res = await fetch(url, { headers, signal: AbortSignal.timeout(15000) });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  return json ? res.json() : res.text();
}

/** "76.2K" -> 76200; the page shows the total rounded like this. */
export function parseCount(text) {
  const m = text.trim().match(/^([\d.,]+)\s*([KMB]?)$/i);
  if (!m) throw new Error(`can't read a count from "${text}"`);
  const scale = { '': 1, K: 1e3, M: 1e6, B: 1e9 }[m[2].toUpperCase()];
  return Math.round(Number(m[1].replace(/,/g, '')) * scale);
}

export async function imagePulls() {
  const html = await get(IMAGE_PAGE, false);
  // <span ...>Total downloads</span> <h3 title="76226">76.2K</h3>: the title
  // holds the exact number, the text a rounded one.
  const exact = html.match(/Total downloads<\/span>\s*<h3[^>]*\btitle="(\d+)"/i);
  if (exact) return Number(exact[1]);
  const rounded = html.match(/Total downloads<\/span>\s*<h3[^>]*>\s*([^<]+?)\s*<\/h3>/i);
  if (!rounded) throw new Error(`no "Total downloads" on ${IMAGE_PAGE}`);
  return parseCount(rounded[1]);
}

export async function releaseDownloads(repo) {
  let total = 0;
  for (let page = 1; ; page++) {
    const releases = await get(
      `https://api.github.com/repos/${ORG}/${repo}/releases?per_page=100&page=${page}`,
      true
    );
    for (const release of releases) {
      if (isLauncherRelease(release.tag_name)) continue;
      for (const asset of release.assets) total += asset.download_count;
    }
    if (releases.length < 100) return total;
  }
}

// Each source's count on 2026-11-09. Counts only go up, so when a source can't
// be read (GitHub rate-limits or blocks some build machines), its last known
// count still gives a true lower bound. Raise these now and then.
const KNOWN = {
  image: 76226,
  lodestone: 24216,
  lodestone_core: 3255,
  lodestone_cli: 5621,
};

async function count(name, read) {
  try {
    return Math.max(await read(), KNOWN[name]);
  } catch (err) {
    console.warn(`Couldn't read ${name} downloads, using ${KNOWN[name]}: ${err.message}`);
    return KNOWN[name];
  }
}

/** The total, from live counts where they can be read. */
export async function coreDownloads() {
  const sources = [
    ['image', imagePulls],
    ...RELEASE_REPOS.map((repo) => [repo, () => releaseDownloads(repo)]),
  ];
  const counts = await Promise.all(sources.map(([name, read]) => count(name, read)));
  const total = counts.reduce((a, b) => a + b, 0);
  console.log(
    `Core downloads: ${total} (${sources.map(([name], i) => `${name} ${counts[i]}`).join(', ')})`
  );
  return total;
}

/** 109312 -> "109k+", 1250000 -> "1.2M+". */
export function formatDownloads(n) {
  if (n >= 1e6) return `${(Math.floor(n / 1e5) / 10).toString()}M+`;
  if (n >= 1e3) return `${Math.floor(n / 1e3)}k+`;
  return String(n);
}
