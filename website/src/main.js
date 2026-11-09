// lodestone.cc home page. Everything here is decoration on top of a page that
// works without it: the star count, the iron filings behind the logo, the
// tilt of the logo, the light on the cards, the Core card's chart and the CLI
// card's typing.

const REPO = 'Lodestone-Team/lodestone';
const CACHE_MS = 60 * 60 * 1000; // the unauthenticated API allows 60 calls an hour
const reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
const finePointer = window.matchMedia('(pointer: fine)').matches;

// Star count. Same cache as the launcher page (launcher/main.js), so moving
// between the two pages costs one API call an hour.

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
    // storage blocked: skip caching
  }
}

function formatCount(n) {
  if (n < 1000) return String(n);
  const k = n / 1000;
  return `${k >= 10 ? Math.round(k) : k.toFixed(1).replace(/\.0$/, '')}k`;
}

async function showStars() {
  try {
    let repo = readCache('lodestone:repo');
    if (repo === undefined) {
      const res = await fetch(`https://api.github.com/repos/${REPO}`, {
        headers: { Accept: 'application/vnd.github+json' },
      });
      if (!res.ok) return;
      repo = await res.json();
      writeCache('lodestone:repo', repo);
    }
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

// Iron filings. A grid of short strokes, each turned along the field of a
// magnet at the logo (a dipole whose axis sways a little). The pointer is a
// second magnet: strokes within about 170px of it turn toward its field, and
// the ones closest to it (about 90px) fade almost away, so the pointer reads as
// a dark spot the field bends around. On load the strokes start at random angles and swing into line,
// nearest the logo first.

function startField() {
  const canvas = document.querySelector('[data-field]');
  const mark = document.querySelector('[data-mark]');
  if (!canvas || !mark || !canvas.getContext) return;
  const hero = canvas.parentElement;
  const ctx = canvas.getContext('2d');

  // Strokes are drawn in batches by brightness and by color: the gem's
  // gradient, green at the top left to violet at the bottom right.
  const BUCKETS = 10;
  const BAND_COLORS = [
    [79, 224, 165],
    [74, 196, 200],
    [106, 174, 238],
    [125, 158, 242],
    [154, 143, 245],
  ];
  const BANDS = BAND_COLORS.length;
  const CLEAR_RADIUS = 84; // no filings under the logo
  const POINTER_REACH = 170; // px; the pointer's pull halves at about this distance
  const POINTER_SPREAD = 2 * (POINTER_REACH / 1.18) ** 2;
  const FADE_REACH = 90; // px; strokes are half faded at about this distance
  const FADE_SPREAD = 2 * (FADE_REACH / 1.18) ** 2;
  let width = 0;
  let height = 0;
  let spacing = 22;
  let filings = [];
  let center = { x: 0, y: 0 };
  const pointer = { x: 0, y: 0, tx: 0, ty: 0, strength: 0, target: 0 };
  let start = performance.now();
  let running = false;
  let visible = true;

  function layout() {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    width = hero.clientWidth;
    height = hero.clientHeight;
    canvas.width = Math.round(width * dpr);
    canvas.height = Math.round(height * dpr);
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

    const heroBox = hero.getBoundingClientRect();
    const markBox = mark.getBoundingClientRect();
    center = {
      x: markBox.left + markBox.width / 2 - heroBox.left,
      y: markBox.top + markBox.height / 2 - heroBox.top,
    };

    spacing = width < 600 ? 24 : 22;
    const old = filings;
    filings = [];
    const cols = Math.ceil(width / spacing) + 1;
    const rows = Math.ceil(height / spacing) + 1;
    const ox = (width - (cols - 1) * spacing) / 2;
    const oy = (height - (rows - 1) * spacing) / 2;
    for (let j = 0; j < rows; j++) {
      for (let i = 0; i < cols; i++) {
        const x = ox + i * spacing;
        const y = oy + j * spacing;
        const d = Math.hypot(x - center.x, y - center.y);
        if (d < CLEAR_RADIUS) continue;
        const prev = old[filings.length];
        filings.push({
          x,
          y,
          angle: prev ? prev.angle : Math.random() * Math.PI,
          // the ripple: strokes near the logo line up first
          delay: 250 + d * 0.9,
          rate: 0.05 + Math.random() * 0.05,
          band: Math.max(
            0,
            Math.min(BANDS - 1, Math.floor(((x - center.x + (y - center.y)) / 900 + 0.5) * BANDS))
          ),
        });
      }
    }
  }

  // Field of a dipole with moment (mx, my) and strength k at (cx, cy), at (x, y).
  function dipole(x, y, cx, cy, mx, my, k, out) {
    const rx = (x - cx) / 100;
    const ry = (y - cy) / 100;
    const r2 = rx * rx + ry * ry + 0.05;
    const r = Math.sqrt(r2);
    const ux = rx / r;
    const uy = ry / r;
    const dot = mx * ux + my * uy;
    const s = k / (r2 * r);
    out.x += s * (3 * dot * ux - mx);
    out.y += s * (3 * dot * uy - my);
  }

  const b = { x: 0, y: 0 };
  const pb = { x: 0, y: 0 };
  // strokes have no head or tail, so angles wrap at half a turn
  const halfTurn = (d) => ((((d + Math.PI / 2) % Math.PI) + Math.PI) % Math.PI) - Math.PI / 2;
  const paths = Array.from({ length: BUCKETS * BANDS }, () => []);

  function frame(now) {
    const t = (now - start) / 1000;
    const sway = -Math.PI / 2 + (reducedMotion ? 0 : 0.28 * Math.sin(t * 0.18));
    const mx = Math.cos(sway);
    const my = Math.sin(sway);

    pointer.x += (pointer.tx - pointer.x) * 0.12;
    pointer.y += (pointer.ty - pointer.y) * 0.12;
    pointer.strength += (pointer.target - pointer.strength) * 0.06;
    // the pointer's magnet points at the logo, so the two pull together
    const px = center.x - pointer.x;
    const py = center.y - pointer.y;
    const pl = Math.hypot(px, py) || 1;

    for (const p of paths) p.length = 0;

    for (const f of filings) {
      b.x = 0;
      b.y = 0;
      dipole(f.x, f.y, center.x, center.y, mx, my, 1, b);
      let target = Math.atan2(b.y, b.x);
      let near = 0; // how much the pointer pulls on this stroke, 0 to 1
      let fade = 0; // how much the pointer fades it, 0 to 1
      if (pointer.strength > 0.01) {
        const dx = f.x - pointer.x;
        const dy = f.y - pointer.y;
        const d2 = dx * dx + dy * dy;
        near = pointer.strength * Math.exp(-d2 / POINTER_SPREAD);
        fade = pointer.strength * Math.exp(-d2 / FADE_SPREAD);
        if (near > 0.003) {
          pb.x = 0;
          pb.y = 0;
          dipole(f.x, f.y, pointer.x, pointer.y, px / pl, py / pl, 1, pb);
          target += halfTurn(Math.atan2(pb.y, pb.x) - target) * Math.min(1, near * 1.15);
        }
      }
      const elapsed = now - start - f.delay;
      if (reducedMotion) {
        f.angle = target;
      } else if (elapsed > 0) {
        // turn the short way
        f.angle += halfTurn(target - f.angle) * f.rate;
      }
      const appear = reducedMotion ? 1 : Math.min(1, Math.max(0, elapsed / 600));
      if (appear <= 0) continue;
      const strength = Math.min(1, Math.pow(Math.hypot(b.x, b.y), 0.3) * 0.85);
      const shown = strength * appear * (1 - 0.97 * fade);
      if (shown < 0.05) continue;
      const level = Math.min(BUCKETS - 1, Math.floor(shown * BUCKETS));
      const half = (spacing * (0.24 + 0.14 * strength)) / 1;
      const cx = Math.cos(f.angle) * half;
      const cy = Math.sin(f.angle) * half;
      paths[f.band * BUCKETS + level].push(f.x - cx, f.y - cy, f.x + cx, f.y + cy);
    }

    ctx.clearRect(0, 0, width, height);
    ctx.lineCap = 'round';
    ctx.lineWidth = 1.5;
    for (let band = 0; band < BANDS; band++) {
      const [hr, hg, hb] = BAND_COLORS[band];
      for (let i = 0; i < BUCKETS; i++) {
        const seg = paths[band * BUCKETS + i];
        if (!seg.length) continue;
        const s = (i + 0.5) / BUCKETS;
        // slate when weak, the gem's colors when strong
        const r = Math.round(104 + (hr - 104) * s);
        const g = Math.round(116 + (hg - 116) * s);
        const bl = Math.round(134 + (hb - 134) * s);
        ctx.strokeStyle = `rgba(${r}, ${g}, ${bl}, ${0.08 + 0.68 * s})`;
        ctx.beginPath();
        for (let k = 0; k < seg.length; k += 4) {
          ctx.moveTo(seg[k], seg[k + 1]);
          ctx.lineTo(seg[k + 2], seg[k + 3]);
        }
        ctx.stroke();
      }
    }

    if (running) requestAnimationFrame(frame);
  }

  function setRunning(on) {
    if (reducedMotion) return;
    if (on && !running) {
      running = true;
      requestAnimationFrame(frame);
    } else if (!on) {
      running = false;
    }
  }

  layout();
  if (reducedMotion) {
    frame(performance.now() + 1e6);
  } else {
    setRunning(true);
  }

  let resizeTimer;
  window.addEventListener('resize', () => {
    clearTimeout(resizeTimer);
    resizeTimer = setTimeout(() => {
      layout();
      if (reducedMotion) frame(performance.now() + 1e6);
    }, 100);
  });

  if (!reducedMotion && finePointer) {
    hero.addEventListener('pointermove', (e) => {
      const box = hero.getBoundingClientRect();
      pointer.tx = e.clientX - box.left;
      pointer.ty = e.clientY - box.top;
      if (pointer.target === 0) {
        pointer.x = pointer.tx;
        pointer.y = pointer.ty;
      }
      pointer.target = 1;
    });
    hero.addEventListener('pointerleave', () => {
      pointer.target = 0;
    });
  }

  new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
    setRunning(visible && !document.hidden);
  }).observe(hero);
  document.addEventListener('visibilitychange', () => setRunning(visible && !document.hidden));
}

// The logo leans toward the pointer.
function tiltMark() {
  const mark = document.querySelector('[data-mark]');
  if (!mark || reducedMotion || !finePointer) return;
  window.addEventListener('pointermove', (e) => {
    const box = mark.getBoundingClientRect();
    const dx = (e.clientX - (box.left + box.width / 2)) / window.innerWidth;
    const dy = (e.clientY - (box.top + box.height / 2)) / window.innerHeight;
    mark.style.setProperty('--tilt-y', `${(dx * 24).toFixed(2)}deg`);
    mark.style.setProperty('--tilt-x', `${(-dy * 24).toFixed(2)}deg`);
  });
}

// A light follows the pointer over each card (see .card::before).
function lightCards() {
  if (!finePointer) return;
  for (const card of document.querySelectorAll('[data-card]')) {
    card.addEventListener('pointermove', (e) => {
      const box = card.getBoundingClientRect();
      card.style.setProperty('--mx', `${e.clientX - box.left}px`);
      card.style.setProperty('--my', `${e.clientY - box.top}px`);
    });
  }
}

// The Core card's CPU chart: a random walk that moves along every second.
function chartCore() {
  const line = document.querySelector('[data-spark-line]');
  const fill = document.querySelector('[data-spark-fill]');
  if (!line || !fill) return;
  const N = 40;
  let v = 0.45;
  const points = [];
  const next = () => {
    v += (Math.random() - 0.5) * 0.22 + (0.42 - v) * 0.12;
    v = Math.min(0.9, Math.max(0.12, v));
    return v;
  };
  for (let i = 0; i < N; i++) points.push(next());

  function draw() {
    const step = 300 / (N - 1);
    const d = points
      .map((p, i) => `${i ? 'L' : 'M'}${(i * step).toFixed(1)},${(48 - p * 40).toFixed(1)}`)
      .join(' ');
    line.setAttribute('d', d);
    fill.setAttribute('d', `${d} L300,48 L0,48 Z`);
  }

  draw();
  if (reducedMotion) return;
  setInterval(() => {
    if (document.hidden) return;
    points.shift();
    points.push(next());
    draw();
  }, 1000);
}

// The CLI card types its commands out, then starts over.
function typeTerminal() {
  const term = document.querySelector('[data-term]');
  if (!term || reducedMotion) return;
  const lines = [...term.children].map((el) => ({ cls: el.className, text: el.textContent }));

  // Fix the terminal at the height of its last frame (every line, then a
  // fresh prompt), so typing never resizes the card or the grid.
  function reserve() {
    const ghost = document.createElement('pre');
    ghost.className = term.className;
    for (const [i, line] of lines.entries()) {
      if (i) ghost.append('\n');
      const span = document.createElement('span');
      span.className = line.cls;
      span.textContent = line.text;
      ghost.append(span);
    }
    ghost.append('\n');
    const prompt = document.createElement('span');
    prompt.className = 't-cmd';
    ghost.append(prompt);
    ghost.style.cssText = `position:absolute;visibility:hidden;width:${term.offsetWidth}px;height:auto;min-height:0`;
    document.body.append(ghost);
    term.style.height = `${ghost.offsetHeight}px`;
    ghost.remove();
  }
  reserve();
  let reserveTimer;
  window.addEventListener('resize', () => {
    clearTimeout(reserveTimer);
    reserveTimer = setTimeout(reserve, 100);
  });
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const caret = document.createElement('span');
  caret.className = 'caret';
  let started = false;

  async function play() {
    for (;;) {
      term.textContent = '';
      for (const [i, line] of lines.entries()) {
        if (i) term.append('\n');
        const span = document.createElement('span');
        span.className = line.cls;
        term.append(span);
        if (line.cls === 't-cmd') {
          span.after(caret);
          await wait(500);
          for (const ch of line.text) {
            span.textContent += ch;
            await wait(18 + Math.random() * 40);
          }
          await wait(300);
        } else {
          caret.remove();
          await wait(250 + Math.random() * 300);
          span.textContent = line.text;
        }
      }
      term.append('\n');
      const prompt = document.createElement('span');
      prompt.className = 't-cmd';
      term.append(prompt);
      prompt.after(caret);
      await wait(4000);
    }
  }

  new IntersectionObserver(([entry]) => {
    if (entry.isIntersecting && !started) {
      started = true;
      play();
    }
  }).observe(term);
}

showStars();
startField();
tiltMark();
lightCards();
chartCore();
typeTerminal();
