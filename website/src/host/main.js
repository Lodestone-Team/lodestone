// Points the download button at Lodestone CLI for the visitor's platform and
// shows the repo's star count. Both come from the GitHub API. The page works
// without them: the button falls back to the Windows build and the star count
// stays hidden. Same code as the launcher page's (launcher/main.js) otherwise.

const REPO = 'Lodestone-Team/lodestone';
const CLI_REPO = 'Lodestone-Team/lodestone_cli';
const RELEASE_PAGE = `https://github.com/${CLI_REPO}/releases/latest`;
const CACHE_MS = 60 * 60 * 1000; // the unauthenticated API allows 60 calls an hour

const PLATFORMS = {
  windows: { name: 'Windows' },
  macos: { name: 'macOS' },
  linux: { name: 'Linux' },
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

// Lodestone CLI installs Host, keeps it updated and starts it. Its releases
// (Lodestone-Team/lodestone_cli) carry one file per platform:
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
  const platform = detectPlatform();
  if (!platform) {
    // Phones and anything unrecognised: point at the full list.
    label.textContent = 'Download';
    button.href = RELEASE_PAGE;
    return;
  }
  label.textContent = `Download for ${PLATFORMS[platform].name}`;

  let release;
  try {
    release = await githubJson(`repos/${CLI_REPO}/releases/latest`, 'lodestone:cli-release');
  } catch (e) {
    // Offline or rate limited. Windows keeps the default link; other
    // platforms go to the release page.
    if (platform !== 'windows') button.href = RELEASE_PAGE;
    return;
  }
  const picked = pickAsset((release && release.assets) || [], platform, detectArch());
  button.href = picked ? picked.url : RELEASE_PAGE;
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
