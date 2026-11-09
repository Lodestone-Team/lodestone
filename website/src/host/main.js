// Points the download button at Lodestone Host for the visitor's platform and
// shows the repo's star count. Both come from the GitHub API. The page works
// without them: the button falls back to the list of Host releases and the star
// count stays hidden. Same code as the launcher page's (launcher/main.js)
// otherwise.

const REPO = 'Lodestone-Team/lodestone';
// Each Lodestone Host version is a release of its own, tagged host-vX.Y.Z; betas
// (host-vX.Y.Z-beta.N) are pre-releases and aren't offered here.
const TAG_PREFIX = 'host-v';
// Lodestone CLI installs and runs Lodestone Core where there's no Host app (Linux),
// and while no Host release has a build for this platform.
const CLI_REPO = 'Lodestone-Team/lodestone_cli';
const CLI_PAGE = `https://github.com/${CLI_REPO}/releases/latest`;
const CACHE_MS = 60 * 60 * 1000; // the unauthenticated API allows 60 calls an hour

const PLATFORMS = {
  windows: { name: 'Windows', note: 'Windows 10 and 11 · Updates itself' },
  macos: { name: 'macOS', note: 'Apple silicon and Intel · Updates itself' },
  linux: { name: 'Linux', note: 'Installs and updates with Lodestone CLI' },
};

function detectPlatform() {
  const ua = navigator.userAgent;
  const hint = (navigator.userAgentData && navigator.userAgentData.platform) || '';
  if (/android|iphone|ipad|ipod/i.test(ua)) return null;
  if (/win/i.test(hint) || /windows/i.test(ua)) return 'windows';
  if (/mac/i.test(hint) || /macintosh|mac os x/i.test(ua)) return 'macos';
  if (/linux/i.test(hint) || /linux|x11/i.test(ua)) return 'linux';
  return null;
}

function detectArch() {
  const ua = navigator.userAgent;
  if (/arm64|aarch64/i.test(ua)) return 'aarch64';
  return 'x86_64';
}

function readCache(key) {
  try {
    const entry = JSON.parse(localStorage.getItem(key));
    if (entry && Date.now() - entry.at < CACHE_MS) return entry.value;
  } catch (e) {
    // storage blocked or unreadable: fetch instead
  }
  return undefined;
}

function writeCache(key, value) {
  try {
    localStorage.setItem(key, JSON.stringify({ at: Date.now(), value }));
  } catch (e) {
    // storage blocked: nothing to do
  }
}

async function githubJson(path, cacheKey) {
  const cached = readCache(cacheKey);
  if (cached !== undefined) return cached;
  const res = await fetch(`https://api.github.com/${path}`, {
    headers: { Accept: 'application/vnd.github+json' },
  });
  if (!res.ok) throw new Error(`GitHub API ${res.status}`);
  const value = await res.json();
  writeCache(cacheKey, value);
  return value;
}

function formatCount(n) {
  if (n < 1000) return String(n);
  const k = n / 1000;
  return `${k >= 10 ? Math.round(k) : k.toFixed(1).replace(/\.0$/, '')}k`;
}

async function showStars() {
  try {
    const repo = await githubJson(`repos/${REPO}`, 'lodestone:repo');
    const count = repo && repo.stargazers_count;
    if (typeof count !== 'number') return;
    const el = document.querySelector('[data-stars]');
    el.querySelector('[data-stars-count]').textContent = formatCount(count);
    el.title = `${count.toLocaleString()} stars on GitHub`;
    el.hidden = false;
  } catch (e) {
    // leave the count hidden
  }
}

// The newest published stable Host release: the one with the highest build.
// GitHub lists releases newest first, the launcher's and the old Host's among them.
function newestRelease(releases) {
  const build = (release) =>
    Math.max(
      0,
      ...release.assets.map((a) => Number((a.name.match(/^lodestone-host-(\d+)-/) || [])[1] || 0)),
    );
  return releases
    .filter((r) => r.tag_name.startsWith(TAG_PREFIX) && !r.draft && !r.prerelease)
    .sort((a, b) => build(b) - build(a))[0];
}

// A Host release's installers (launcher/scripts/publish_release.py --product host):
// lodestone-host-<build>-windows-x86_64-setup.exe and
// lodestone-host-<build>-macos-universal.dmg, also under names that never change,
// LodestoneHostSetup.exe and LodestoneHost.dmg. The Host needs its core beside it,
// so the -standalone and -core files are only for its updater.
function pickHostAsset(assets, platform) {
  const byName = (re) =>
    assets
      .map((a) => ({ asset: a, m: a.name.match(re) }))
      .filter((x) => x.m)
      .sort((a, b) => Number(b.m[1]) - Number(a.m[1]))[0];
  if (platform === 'windows') {
    const stable = assets.find((a) => a.name === 'LodestoneHostSetup.exe');
    const setup = byName(/^lodestone-host-(\d+)-windows-x86_64-setup\.exe$/);
    if (stable) return { url: stable.browser_download_url, build: setup && setup.m[1] };
    if (setup) return { url: setup.asset.browser_download_url, build: setup.m[1] };
  }
  if (platform === 'macos') {
    const stable = assets.find((a) => a.name === 'LodestoneHost.dmg');
    const dmg = byName(/^lodestone-host-(\d+)-macos-[a-z0-9_]+\.dmg$/);
    if (stable) return { url: stable.browser_download_url, build: dmg && dmg.m[1] };
    if (dmg) return { url: dmg.asset.browser_download_url, build: dmg.m[1] };
  }
  return null;
}

// Lodestone CLI installs Lodestone Core, keeps it updated and starts it. Its
// releases (Lodestone-Team/lodestone_cli) carry one file per platform:
// lodestone_cli_<os>_<arch>[.exe]. There's no Intel Mac build.
function pickAsset(assets, platform, arch) {
  const os = { windows: 'windows', macos: 'macos', linux: 'linux' }[platform];
  const archs = platform === 'macos' ? ['aarch64'] : [arch];
  for (const a of archs) {
    const hit = assets.find((x) => x.name === `lodestone_cli_${os}_${a}${platform === 'windows' ? '.exe' : ''}`);
    if (hit) return { url: hit.browser_download_url };
  }
  return null;
}

async function setUpDownload() {
  const button = document.querySelector('[data-download]');
  const label = document.querySelector('[data-download-label]');
  const note = document.querySelector('[data-download-note]');
  const platform = detectPlatform();
  if (!platform) {
    // Phones and anything unrecognised: the button keeps the list of releases.
    label.textContent = 'Download';
    return;
  }
  const info = PLATFORMS[platform];
  label.textContent = `Download for ${info.name}`;

  // The Host app, from its newest release, on Windows and macOS.
  if (platform !== 'linux') {
    try {
      const releases = await githubJson(`repos/${REPO}/releases?per_page=100`, 'lodestone:releases');
      const release = newestRelease(releases || []);
      const picked = pickHostAsset((release && release.assets) || [], platform);
      if (picked) {
        button.href = picked.url;
        note.textContent = picked.build ? `${info.note} · Build ${picked.build}` : info.note;
        return;
      }
    } catch (e) {
      // Offline or rate limited: the button keeps the list of releases.
      return;
    }
  }

  // Linux, or no Host build for this platform yet: Lodestone CLI.
  note.textContent = PLATFORMS.linux.note;
  let release;
  try {
    release = await githubJson(`repos/${CLI_REPO}/releases/latest`, 'lodestone:cli-release');
  } catch (e) {
    button.href = CLI_PAGE;
    return;
  }
  const picked = pickAsset((release && release.assets) || [], platform, detectArch());
  button.href = picked ? picked.url : CLI_PAGE;
}

// The tabs above the screenshot swap it for another view of the dashboard.
function setUpTour() {
  const tabs = [...document.querySelectorAll('[data-shot]')];
  const panel = document.getElementById('shot');
  const img = panel.querySelector('img');

  function select(tab, focus) {
    const name = tab.dataset.shot;
    for (const t of tabs) {
      const selected = t === tab;
      t.setAttribute('aria-selected', String(selected));
      t.tabIndex = selected ? 0 : -1;
    }
    panel.setAttribute('aria-labelledby', tab.id);
    if (focus) tab.focus();
    const next = new Image();
    next.srcset = `/host/images/${name}-1120.webp 1120w, /host/images/${name}.webp 2240w`;
    next.sizes = img.sizes;
    img.classList.add('is-loading');
    const show = () => {
      img.srcset = next.srcset;
      img.src = `/host/images/${name}.webp`;
      img.alt = tab.dataset.alt;
      img.classList.remove('is-loading');
    };
    next.decode().then(show, show);
  }

  tabs.forEach((tab, i) => {
    tab.tabIndex = i === 0 ? 0 : -1;
    tab.addEventListener('click', () => select(tab, false));
    tab.addEventListener('keydown', (e) => {
      const step = { ArrowRight: 1, ArrowLeft: -1 }[e.key];
      if (!step) return;
      e.preventDefault();
      select(tabs[(i + step + tabs.length) % tabs.length], true);
    });
  });

  // Warm the cache for the other views once the page has settled.
  window.addEventListener('load', () => {
    const wide = window.matchMedia('(min-width: 1129px)').matches || devicePixelRatio > 1;
    for (const tab of tabs.slice(1)) {
      new Image().src = `/host/images/${tab.dataset.shot}${wide ? '' : '-1120'}.webp`;
    }
  });
}

showStars();
setUpDownload();
setUpTour();
