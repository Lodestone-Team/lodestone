// lodestone.cc home page. Everything here is decoration on top of a page that
// works without it: the star count, the iron filings behind the logo, the
// logo switching between the launcher and Host, the light on the cards, the
// Host card's chart, the CLI card's typing and the Connect section's join.

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
// second magnet: strokes within about 170px of it turn toward its field. The
// ones closest to it (about 90px) fade almost away, and a ring just outside
// that brightens, pushing those strokes toward the full gem colors they
// already carry, so the pointer reads as a dark spot with a lit rim that the
// field bends around. On load the strokes start at random angles and swing into line,
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
  // While Host's mark shows, the strokes take its greens instead.
  const HOST_COLORS = [
    [46, 242, 154],
    [42, 228, 158],
    [38, 214, 162],
    [34, 202, 165],
    [31, 191, 168],
  ];
  // When the logo switches, the edge the strokes fade along morphs from one
  // shape to the other over MORPH_MS.
  const MORPH_MS = 700;
  // ...and a light ripple runs out from the logo: a ring RIPPLE_WIDTH px wide
  // moving RIPPLE_SPEED px a millisecond, fading out by RIPPLE_REACH px. As it
  // passes, strokes swing by up to RIPPLE_SWING radians and brighten a little,
  // then ease back into line.
  const RIPPLE_SPEED = 0.75;
  const RIPPLE_WIDTH = 46;
  const RIPPLE_REACH = 820;
  const RIPPLE_SWING = 0.55;
  const RIPPLE_GLOW = 0.22;
  // Filings run right up to the gem and fade out toward its edge: fully shown
  // from GEM_FADE px out, gone at GEM_MARGIN px, following the hexagon.
  const GEM_MARGIN = 3; // px between the gem's edge and the nearest stroke tip
  const GEM_FADE = 40; // px from the gem's edge where strokes are fully shown
  const POINTER_REACH = 170; // px; the pointer's pull halves at about this distance
  const POINTER_SPREAD = 2 * (POINTER_REACH / 1.18) ** 2;
  const FADE_REACH = 90; // px; strokes are half faded at about this distance
  const FADE_SPREAD = 2 * (FADE_REACH / 1.18) ** 2;
  const RING_RADIUS = 125; // px from the pointer to the brightest part of the ring
  const RING_WIDTH = 2 * 42 ** 2;
  let width = 0;
  let height = 0;
  let spacing = 22;
  let filings = [];
  let center = { x: 0, y: 0 };
  let gemScale = 1; // CSS px per unit of the logo's viewBox
  let shapeMix = 0; // 0 = the gem's hexagon, 1 = Host's square
  const morph = { from: 0, to: 0, start: -Infinity };
  let rippleStart = -Infinity;
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

    // the logo's viewBox is 580 units wide
    gemScale = mark.offsetWidth / 580;

    spacing = width < 600 ? 24 : 22;
    const old = filings;
    filings = [];
    // line the grid up on the logo, so the fade around it looks the same on every side
    const ox = center.x - Math.ceil(center.x / spacing) * spacing;
    const oy = center.y - Math.ceil(center.y / spacing) * spacing;
    const cols = Math.ceil((width - ox) / spacing) + 1;
    const rows = Math.ceil((height - oy) / spacing) + 1;
    for (let j = 0; j < rows; j++) {
      for (let i = 0; i < cols; i++) {
        const x = ox + i * spacing;
        const y = oy + j * spacing;
        const d = Math.hypot(x - center.x, y - center.y);
        const prev = old[filings.length];
        filings.push({
          x,
          y,
          clear: 0,
          nx: 0,
          ny: 0,
          gemFade: 0,
          dist: d,
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
    fitShape();
  }

  // How each stroke sits against the logo: the room it has before it reaches
  // the logo, which way is away from it, and how far it has faded near it.
  function fitShape() {
    for (const f of filings) {
      const edge = markDistance(f.x, f.y);
      // room for the stroke before it reaches the logo
      f.clear = Math.max(0, edge - GEM_MARGIN);
      // which way is away from the logo here
      f.nx = markDistance(f.x + 0.5, f.y) - markDistance(f.x - 0.5, f.y);
      f.ny = markDistance(f.x, f.y + 0.5) - markDistance(f.x, f.y - 0.5);
      // 0 at the logo's edge, 1 from GEM_FADE out, eased so it fades quickly near it
      const t = f.clear < 2 ? 0 : Math.min(1, edge / GEM_FADE);
      f.gemFade = t * t * (3 - 2 * t);
    }
  }

  // Distance in px to the edge of the logo that shows, or a blend of the two
  // while it switches.
  function markDistance(x, y) {
    if (shapeMix <= 0) return gemDistance(x, y);
    if (shapeMix >= 1) return hostDistance(x, y);
    return gemDistance(x, y) * (1 - shapeMix) + hostDistance(x, y) * shapeMix;
  }

  // Distance in px from (x, y) to the edge of Host's mark: a square 440
  // viewBox units across with corners rounded like its Ls (9.5 of its 70).
  function hostDistance(x, y) {
    const HALF = 220;
    const ROUND = 59.7;
    const qx = Math.abs(x - center.x) / gemScale - (HALF - ROUND);
    const qy = Math.abs(y - center.y) / gemScale - (HALF - ROUND);
    const outside = Math.hypot(Math.max(qx, 0), Math.max(qy, 0));
    return (outside + Math.min(Math.max(qx, qy), 0) - ROUND) * gemScale;
  }

  // Distance in px from (x, y) to the gem's edge (negative inside): a hexagon
  // with points at the top and bottom (flat sides 259.8 viewBox units from its
  // middle, rounded corners), the same shape as the logo's path.
  function gemDistance(x, y) {
    const ROUND = 78;
    const inner = 259.81 - ROUND;
    // swap x and y so the flat sides are on top for the usual formula
    let px = Math.abs(y - center.y) / gemScale;
    let py = Math.abs(x - center.x) / gemScale;
    const kx = -0.866025404;
    const ky = 0.5;
    const fold = 2 * Math.min(kx * px + ky * py, 0);
    px -= fold * kx;
    py -= fold * ky;
    const lim = 0.577350269 * inner;
    px -= Math.max(-lim, Math.min(lim, px));
    py -= inner;
    return (Math.hypot(px, py) * Math.sign(py) - ROUND) * gemScale;
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

    // the logo switched: move the fade edge toward its shape
    if (morph.start > -Infinity) {
      const k = Math.min(1, (now - morph.start) / MORPH_MS);
      const e = k * k * (3 - 2 * k);
      shapeMix = morph.from + (morph.to - morph.from) * e;
      fitShape();
      if (k >= 1) morph.start = -Infinity;
    }

    // where the ripple's ring is now, and how strong it still is
    const rippleR = (now - rippleStart) * RIPPLE_SPEED;
    const rippleOn = rippleR >= 0 && rippleR < RIPPLE_REACH + RIPPLE_WIDTH * 2;
    const rippleFade = rippleOn ? Math.max(0, 1 - rippleR / RIPPLE_REACH) : 0;

    for (const p of paths) p.length = 0;

    for (const f of filings) {
      b.x = 0;
      b.y = 0;
      dipole(f.x, f.y, center.x, center.y, mx, my, 1, b);
      let target = Math.atan2(b.y, b.x);
      let near = 0; // how much the pointer pulls on this stroke, 0 to 1
      let fade = 0; // how much the pointer fades it, 0 to 1
      let ring = 0; // how much the ring around the pointer lights it, 0 to 1
      if (pointer.strength > 0.01) {
        const dx = f.x - pointer.x;
        const dy = f.y - pointer.y;
        const d2 = dx * dx + dy * dy;
        near = pointer.strength * Math.exp(-d2 / POINTER_SPREAD);
        fade = pointer.strength * Math.exp(-d2 / FADE_SPREAD);
        const fromRing = Math.sqrt(d2) - RING_RADIUS;
        ring = pointer.strength * Math.exp(-(fromRing * fromRing) / RING_WIDTH);
        if (near > 0.003) {
          pb.x = 0;
          pb.y = 0;
          dipole(f.x, f.y, pointer.x, pointer.y, px / pl, py / pl, 1, pb);
          target += halfTurn(Math.atan2(pb.y, pb.x) - target) * Math.min(1, near * 1.15);
        }
      }
      // the ripple: a nudge to one side as the ring passes, which the stroke's
      // easing turns into a smooth swing and settle
      let wave = 0;
      if (rippleOn) {
        const off = (f.dist - rippleR) / RIPPLE_WIDTH;
        wave = Math.exp(-off * off) * rippleFade;
        target += wave * RIPPLE_SWING;
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
      const shown = Math.min(
        1,
        (strength * (1 - 0.97 * fade) + 0.42 * ring + RIPPLE_GLOW * wave) * appear * f.gemFade
      );
      if (shown < 0.05) continue;
      const level = Math.min(BUCKETS - 1, Math.floor(shown * BUCKETS));
      const ax = Math.cos(f.angle);
      const ay = Math.sin(f.angle);
      // shorten strokes that point at the gem so their tips never touch it
      const toward = Math.abs(ax * f.nx + ay * f.ny);
      const half = Math.min(spacing * (0.24 + 0.14 * strength), f.clear / Math.max(toward, 0.001));
      const cx = ax * half;
      const cy = ay * half;
      paths[f.band * BUCKETS + level].push(f.x - cx, f.y - cy, f.x + cx, f.y + cy);
    }

    ctx.clearRect(0, 0, width, height);
    ctx.lineCap = 'round';
    ctx.lineWidth = 1.5;
    for (let band = 0; band < BANDS; band++) {
      const [gr, gg, gb] = BAND_COLORS[band];
      const [sr, sg, sb] = HOST_COLORS[band];
      const hr = gr + (sr - gr) * shapeMix;
      const hg = gg + (sg - gg) * shapeMix;
      const hb = gb + (sb - gb) * shapeMix;
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

  mark.addEventListener('markswitch', (e) => {
    morph.from = shapeMix;
    morph.to = e.detail === 'host' ? 1 : 0;
    morph.start = performance.now();
    rippleStart = morph.start;
  });

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

// The Host card's CPU chart: a random walk that moves along every second.
function chartHost() {
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

// The Connect section's join: when it scrolls into view, the launcher's bar
// fills once while it gets the server's mods. Without JavaScript, or with
// reduced motion, it shows full.
function playJoin() {
  const link = document.querySelector('[data-link]');
  if (!link || reducedMotion) return;
  const bar = link.querySelector('[data-join-bar]');
  bar.style.transform = 'scaleX(0)';

  let started = false;
  new IntersectionObserver(
    ([entry]) => {
      if (!entry.isIntersecting || started) return;
      started = true;
      bar.style.transition = 'transform 1.8s cubic-bezier(0.4, 0, 0.2, 1) 0.4s';
      requestAnimationFrame(() => (bar.style.transform = 'scaleX(1)'));
    },
    { threshold: 0.6 },
  ).observe(link);
}

// The hero's logo takes turns between the launcher's gem and Host's mark,
// every few seconds while the hero is on screen. It holds still while the
// pointer is on the logo or its name, and with reduced motion it stays on the
// gem. Each switch tells the iron filings (startField) to follow the new shape.
function switchMark() {
  const mark = document.querySelector('[data-mark]');
  const label = document.querySelector('[data-mark-label]');
  if (!mark || !label || reducedMotion) return;
  const EVERY_MS = 4500;
  let held = false;
  let onScreen = true;

  function show(name) {
    mark.dataset.show = name;
    for (const a of label.querySelectorAll('a')) {
      if (a.dataset.for === name) a.removeAttribute('tabindex');
      else a.setAttribute('tabindex', '-1');
    }
    mark.dispatchEvent(new CustomEvent('markswitch', { detail: name }));
  }

  setInterval(() => {
    if (held || !onScreen || document.hidden) return;
    show(mark.dataset.show === 'launcher' ? 'host' : 'launcher');
  }, EVERY_MS);

  for (const el of [mark, label]) {
    el.addEventListener('pointerenter', () => (held = true));
    el.addEventListener('pointerleave', () => (held = false));
  }
  label.addEventListener('focusin', () => (held = true));
  label.addEventListener('focusout', () => (held = false));
  new IntersectionObserver(([entry]) => (onScreen = entry.isIntersecting)).observe(mark);
}

showStars();
startField();
switchMark();
lightCards();
chartHost();
typeTerminal();
playJoin();
