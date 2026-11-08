// Parses console lines into styled text segments: ANSI SGR escape sequences
// (colours and text styles) and Minecraft § formatting codes. Every other escape
// sequence and control character is removed, so the text never shows them.
//
// The console text is untrusted: it is only ever turned into React text and
// inline styles, never into HTML.
//
// This module has no imports, so its tests run with plain `node --test`.

export type ConsoleLevel = 'warn' | 'error';
export type ConsoleTheme = 'light' | 'dark';

/**
 * A colour. 0-255 is an xterm palette index (0-15 are the 16 theme colours),
 * 256 + 0xRRGGBB is a 24-bit colour.
 */
export type ConsoleColor = number;

export const RGB_BASE = 256;

export const BOLD = 1;
export const DIM = 2;
export const ITALIC = 4;
export const UNDERLINE = 8;
export const STRIKE = 16;
export const INVERSE = 32;

export type ConsoleSegment = {
  text: string;
  fg: ConsoleColor | null;
  bg: ConsoleColor | null;
  /** BOLD | DIM | ... */
  flags: number;
};

export type ParsedConsoleLine = {
  /** The text without escape sequences or formatting codes */
  plain: string;
  /** The text in runs of the same style, in order; they add up to `plain` */
  segments: ConsoleSegment[];
  /** Whether any of the text has a style (a colour, bold, ...) */
  styled: boolean;
  /** The log level, for lines without styles; null for INFO and unknown */
  level: ConsoleLevel | null;
};

type Style = {
  fg: ConsoleColor | null;
  bg: ConsoleColor | null;
  flags: number;
};

const ESC = 0x1b;
const BEL = 0x07;
const SECTION = 0xa7; // §

// Minecraft colour codes 0-f in the ANSI palette, as Paper and Spigot map them
// for the console: §4 dark red is ANSI red, §c red is bright red, and so on.
const MINECRAFT_COLORS = [0, 4, 2, 6, 1, 5, 3, 7, 8, 12, 10, 14, 9, 13, 11, 15];

const hexDigit = (code: number): number => {
  if (code >= 0x30 && code <= 0x39) return code - 0x30; // 0-9
  if (code >= 0x61 && code <= 0x66) return code - 0x61 + 10; // a-f
  if (code >= 0x41 && code <= 0x46) return code - 0x41 + 10; // A-F
  return -1;
};

// '' is 0, as in "\x1b[m" and "\x1b[;1m"; anything else that isn't a number
// (the private markers < = > ?) is NaN
const toInt = (value: string | undefined): number =>
  value === undefined || value === '' ? 0 : /^\d+$/.test(value) ? +value : NaN;

// a colour component; a missing one (a sequence cut short) is NaN
const byte = (value: string | undefined): number => {
  const n = value === undefined ? NaN : toInt(value);
  return n >= 0 && n <= 255 ? n : NaN;
};

const rgb = (r: number, g: number, b: number): ConsoleColor | null =>
  Number.isNaN(r) || Number.isNaN(g) || Number.isNaN(b)
    ? null
    : RGB_BASE + ((r << 16) | (g << 8) | b);

/** Applies an SGR parameter string (the part of "\x1b[...m" between [ and m) */
const applySgr = (style: Style, params: string) => {
  const parts = params.split(';');
  for (let k = 0; k < parts.length; k++) {
    const part = parts[k];
    let code: number;
    if (part.includes(':')) {
      // sub-parameters: 38:5:n, 38:2:r:g:b, 38:2:colourspace:r:g:b, 4:3, ...
      const sub = part.split(':');
      code = toInt(sub[0]);
      if (code === 38 || code === 48 || code === 58) {
        const mode = toInt(sub[1]);
        let color: ConsoleColor | null = null;
        if (mode === 5) {
          const index = byte(sub[2]);
          color = Number.isNaN(index) ? null : index;
        } else if (mode === 2) {
          const c = sub.length >= 6 ? sub.slice(3) : sub.slice(2);
          color = rgb(byte(c[0]), byte(c[1]), byte(c[2]));
        }
        if (color !== null && code === 38) style.fg = color;
        if (color !== null && code === 48) style.bg = color;
        // 58 is the underline colour, which isn't shown
        continue;
      }
      if (code === 4) {
        // 4:0 is no underline, 4:1-5 are underline styles
        if (toInt(sub[1]) === 0) style.flags &= ~UNDERLINE;
        else style.flags |= UNDERLINE;
        continue;
      }
    } else {
      code = toInt(part);
    }
    if (code === 38 || code === 48 || code === 58) {
      // 38;5;n or 38;2;r;g;b
      const mode = toInt(parts[k + 1]);
      let color: ConsoleColor | null = null;
      if (mode === 5) {
        const index = byte(parts[k + 2]);
        color = Number.isNaN(index) ? null : index;
        k += 2;
      } else if (mode === 2) {
        color = rgb(byte(parts[k + 2]), byte(parts[k + 3]), byte(parts[k + 4]));
        k += 4;
      } else {
        // unknown colour mode: the rest can't be read reliably
        return;
      }
      if (color !== null && code === 38) style.fg = color;
      if (color !== null && code === 48) style.bg = color;
      continue;
    }
    if (code === 0) {
      style.fg = null;
      style.bg = null;
      style.flags = 0;
    } else if (code === 1) style.flags |= BOLD;
    else if (code === 2) style.flags |= DIM;
    else if (code === 3) style.flags |= ITALIC;
    else if (code === 4 || code === 21) style.flags |= UNDERLINE;
    else if (code === 7) style.flags |= INVERSE;
    else if (code === 9) style.flags |= STRIKE;
    else if (code === 22) style.flags &= ~(BOLD | DIM);
    else if (code === 23) style.flags &= ~ITALIC;
    else if (code === 24) style.flags &= ~UNDERLINE;
    else if (code === 27) style.flags &= ~INVERSE;
    else if (code === 29) style.flags &= ~STRIKE;
    else if (code >= 30 && code <= 37) style.fg = code - 30;
    else if (code === 39) style.fg = null;
    else if (code >= 40 && code <= 47) style.bg = code - 40;
    else if (code === 49) style.bg = null;
    else if (code >= 90 && code <= 97) style.fg = code - 90 + 8;
    else if (code >= 100 && code <= 107) style.bg = code - 100 + 8;
    // anything else (blink, conceal, overline, fonts, ...) is ignored
  }
};

/**
 * Skips a control string (OSC, DCS, SOS, PM, APC) that starts at `i`, the first
 * character after its introducer. It ends with BEL or ST (ESC \); one that is
 * never terminated ends at the end of the line, so the rest of the console
 * stays readable.
 */
const skipControlString = (s: string, i: number): number => {
  for (; i < s.length; i++) {
    const c = s.charCodeAt(i);
    if (c === BEL || c === 0x9c) return i + 1;
    if (c === ESC && s.charCodeAt(i + 1) === 0x5c) return i + 2;
    if (c === 0x0a) return i;
  }
  return i;
};

/**
 * Reads a CSI sequence whose parameters start at `i`. Returns where it ends and,
 * for SGR ("m" without a private marker or intermediates), its parameters. A
 * sequence cut short by a character that can't be part of it ends there, and
 * that character is read as text again.
 */
const readCsi = (s: string, i: number): { end: number; sgr: string | null } => {
  const start = i;
  while (i < s.length) {
    const c = s.charCodeAt(i);
    if (c >= 0x30 && c <= 0x3f)
      i++; // parameters
    else break;
  }
  const paramsEnd = i;
  while (i < s.length) {
    const c = s.charCodeAt(i);
    if (c >= 0x20 && c <= 0x2f)
      i++; // intermediates
    else break;
  }
  const final = i < s.length ? s.charCodeAt(i) : -1;
  if (final < 0x40 || final > 0x7e) return { end: i, sgr: null };
  const params = s.slice(start, paramsEnd);
  const isSgr = final === 0x6d && paramsEnd === i && !/^[<=>?]/.test(params);
  return { end: i + 1, sgr: isSgr ? params : null };
};

const sameStyle = (a: Style, b: Style) =>
  a.fg === b.fg && a.bg === b.bg && a.flags === b.flags;

const LEVEL_BRACKETS = /^\s*\[([^\]\n]*)\](?:\s*\[([^\]\n]*)\])?/;
const LEVEL_TOKEN =
  /(?:^|[\s/])(TRACE|DEBUG|INFO|WARN|WARNING|ERROR|SEVERE|FATAL)$/;
const LEVEL_PREFIX = /^\s*(WARN|WARNING|ERROR|SEVERE|FATAL):\s/;

const toLevel = (token: string): ConsoleLevel | null =>
  token.startsWith('WARN')
    ? 'warn'
    : token === 'ERROR' || token === 'SEVERE' || token === 'FATAL'
      ? 'error'
      : null;

/**
 * The log level of a line, from the level in its first one or two bracketed
 * groups, which covers the common server formats:
 * `[12:00:00] [Server thread/WARN]: ...` (vanilla, Fabric, Forge),
 * `[12:00:00 WARN]: ...` (Paper, Velocity) and `[WARN] ...`; or from a
 * `WARN: ` / `ERROR: ` prefix.
 */
export const detectLevel = (plain: string): ConsoleLevel | null => {
  const brackets = LEVEL_BRACKETS.exec(plain);
  if (brackets) {
    for (const group of [brackets[1], brackets[2]]) {
      if (group === undefined) break;
      const token = LEVEL_TOKEN.exec(group.trimEnd());
      // the first group with a level decides: "[INFO]: [x] [ERROR]" is INFO
      if (token) return toLevel(token[1]);
    }
  }
  const prefix = LEVEL_PREFIX.exec(plain);
  return prefix ? toLevel(prefix[1]) : null;
};

/**
 * Parses one console line (one output event). The style starts plain at the
 * start of the line and again after each newline in it, so a colour that is
 * never reset doesn't run into the following lines. The trailing line break is
 * dropped.
 */
export const parseConsoleLine = (message: string): ParsedConsoleLine => {
  const s = message.replace(/[\r\n]+$/, '');
  const segments: ConsoleSegment[] = [];
  const style: Style = { fg: null, bg: null, flags: 0 };
  let text = '';
  // the style of `text`
  let textStyle: Style = { ...style };

  const flush = () => {
    if (text === '') return;
    const last = segments[segments.length - 1];
    if (last && sameStyle(last, textStyle)) last.text += text;
    else segments.push({ text, ...textStyle });
    text = '';
  };
  const append = (chunk: string) => {
    if (chunk === '') return;
    if (text !== '' && !sameStyle(textStyle, style)) flush();
    if (text === '') textStyle = { ...style };
    text += chunk;
  };
  const reset = () => {
    style.fg = null;
    style.bg = null;
    style.flags = 0;
  };

  let runStart = 0;
  let i = 0;
  while (i < s.length) {
    const c = s.charCodeAt(i);
    const special =
      (c < 0x20 && c !== 0x09) || (c >= 0x7f && c <= 0x9f) || c === SECTION;
    if (!special) {
      i++;
      continue;
    }
    append(s.slice(runStart, i));

    if (c === 0x0a) {
      // a new line inside the event starts plain again
      reset();
      append('\n');
      i++;
    } else if (c === SECTION) {
      i = applySection(s, i, style, append);
    } else if (c === ESC || c === 0x9b || c === 0x9d || c === 0x90) {
      i = applyEscape(s, i, style);
    } else if (c === 0x98 || c === 0x9e || c === 0x9f) {
      i = skipControlString(s, i + 1); // C1 SOS, PM, APC
    } else {
      i++; // other control characters (\r, BEL, backspace, ...) are dropped
    }
    runStart = i;
  }
  append(s.slice(runStart));
  flush();

  const plain = segments.map((segment) => segment.text).join('');
  const styled = segments.some(
    (segment) =>
      segment.fg !== null || segment.bg !== null || segment.flags !== 0
  );
  return {
    plain,
    segments,
    styled,
    level: styled ? null : detectLevel(plain),
  };
};

/** Handles the escape (ESC, or a C1 CSI / OSC / DCS) at `i`; returns the next index */
const applyEscape = (s: string, i: number, style: Style): number => {
  const c = s.charCodeAt(i);
  let next: number;
  let at: number;
  if (c === ESC) {
    if (i + 1 >= s.length) return i + 1;
    next = s.charCodeAt(i + 1);
    at = i + 2;
  } else {
    // C1 forms: 0x9b is CSI, 0x9d OSC, 0x90 DCS
    next = c === 0x9b ? 0x5b : c === 0x9d ? 0x5d : 0x50;
    at = i + 1;
  }
  if (next === 0x5b) {
    // CSI: ESC [
    const { end, sgr } = readCsi(s, at);
    if (sgr !== null) applySgr(style, sgr);
    return end;
  }
  if (
    next === 0x5d || // OSC (window titles, hyperlinks, ...)
    next === 0x50 || // DCS
    next === 0x58 || // SOS
    next === 0x5e || // PM
    next === 0x5f // APC
  ) {
    return skipControlString(s, at);
  }
  if (next >= 0x20 && next <= 0x2f) {
    // ESC with intermediates and a final character, e.g. ESC ( B
    let j = at;
    while (j < s.length && s.charCodeAt(j) >= 0x20 && s.charCodeAt(j) <= 0x2f)
      j++;
    const final = s.charCodeAt(j);
    return final >= 0x30 && final <= 0x7e ? j + 1 : j;
  }
  if (next >= 0x30 && next <= 0x7e) return at; // ESC 7, ESC M, ESC c, ...
  // ESC followed by something that can't continue it: drop the ESC alone
  return i + 1;
};

/**
 * Handles the § at `i`; returns the next index. § followed by anything that
 * isn't a formatting code is kept as text.
 */
const applySection = (
  s: string,
  i: number,
  style: Style,
  append: (chunk: string) => void
): number => {
  const code = s.charCodeAt(i + 1);
  const digit = hexDigit(code);
  if (digit >= 0) {
    // a colour; in Minecraft it also ends bold, italic, ...
    style.fg = MINECRAFT_COLORS[digit];
    style.flags = 0;
    return i + 2;
  }
  switch (code | 0x20) {
    case 0x6b: // k: obfuscated, shown as plain text
      return i + 2;
    case 0x6c: // l
      style.flags |= BOLD;
      return i + 2;
    case 0x6d: // m
      style.flags |= STRIKE;
      return i + 2;
    case 0x6e: // n
      style.flags |= UNDERLINE;
      return i + 2;
    case 0x6f: // o
      style.flags |= ITALIC;
      return i + 2;
    case 0x72: // r
      style.fg = null;
      style.bg = null;
      style.flags = 0;
      return i + 2;
    case 0x78: {
      // x: a hex colour, §x§R§R§G§G§B§B (Spigot / BungeeCord)
      let value = 0;
      for (let k = 0; k < 6; k++) {
        const at = i + 2 + k * 2;
        const d = hexDigit(s.charCodeAt(at + 1));
        if (s.charCodeAt(at) !== SECTION || d < 0) {
          append('§');
          return i + 1;
        }
        value = value * 16 + d;
      }
      style.fg = RGB_BASE + value;
      style.flags = 0;
      return i + 14;
    }
  }
  append('§');
  return i + 1;
};

// ---- Colours ----

// The 16 colours used on badges (text with a background colour), in both
// themes. Such text carries its own background, so these are the usual
// terminal colours rather than the theme's.
export const BADGE_PALETTE = [
  0x000000, 0xcd3131, 0x0dbc79, 0xe5e510, 0x2472c8, 0xbc3fbc, 0x11a8cd,
  0xe5e5e5, 0x666666, 0xf14c4c, 0x23d18b, 0xf5f543, 0x3b8eea, 0xd670d6,
  0x29b8db, 0xffffff,
];

/** The 0xRRGGBB value of an xterm palette index 16-255 */
export const xterm256 = (index: number): number => {
  if (index >= 232) {
    const v = 8 + (index - 232) * 10;
    return (v << 16) | (v << 8) | v;
  }
  const n = index - 16;
  const level = (x: number) => (x === 0 ? 0 : 55 + x * 40);
  return (
    (level(Math.floor(n / 36)) << 16) |
    (level(Math.floor(n / 6) % 6) << 8) |
    level(n % 6)
  );
};

export const toHex = (value: number) =>
  '#' + value.toString(16).padStart(6, '0');

const channel = (v: number) => {
  const x = v / 255;
  return x <= 0.03928 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
};

/** WCAG relative luminance of 0xRRGGBB */
export const luminance = (value: number) =>
  0.2126 * channel((value >> 16) & 255) +
  0.7152 * channel((value >> 8) & 255) +
  0.0722 * channel(value & 255);

/** WCAG contrast ratio between two 0xRRGGBB colours */
export const contrast = (a: number, b: number) => {
  const x = luminance(a);
  const y = luminance(b);
  return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
};

// The background console text has to be readable on, per theme: the harder of
// the console (gray-900) and a hovered row (gray-800), which is the row in the
// dark theme and the console in the light one (see globals.css)
const READABLE_ON: Record<ConsoleTheme, number> = {
  dark: 0x2b2d32,
  light: 0xf3f5f8,
};
export const MIN_CONTRAST = 4.5;

const toHsl = (value: number): [number, number, number] => {
  const r = ((value >> 16) & 255) / 255;
  const g = ((value >> 8) & 255) / 255;
  const b = (value & 255) / 255;
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  const l = (max + min) / 2;
  if (max === min) return [0, 0, l];
  const d = max - min;
  const sat = l > 0.5 ? d / (2 - max - min) : d / (max + min);
  const hue =
    max === r
      ? (g - b) / d + (g < b ? 6 : 0)
      : max === g
        ? (b - r) / d + 2
        : (r - g) / d + 4;
  return [hue / 6, sat, l];
};

const fromHsl = (h: number, sat: number, l: number): number => {
  const q = l < 0.5 ? l * (1 + sat) : l + sat - l * sat;
  const p = 2 * l - q;
  const hue = (t: number) => {
    if (t < 0) t += 1;
    if (t > 1) t -= 1;
    if (t < 1 / 6) return p + (q - p) * 6 * t;
    if (t < 1 / 2) return q;
    if (t < 2 / 3) return p + (q - p) * (2 / 3 - t) * 6;
    return p;
  };
  const to = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 255);
  return (to(hue(h + 1 / 3)) << 16) | (to(hue(h)) << 8) | to(hue(h - 1 / 3));
};

const readableCache = new Map<number, number>();

/**
 * A 256 or 24-bit text colour made readable on the console: if it has less
 * than 4.5:1 contrast, its lightness is moved towards white (dark theme) or
 * black (light theme) until it has. The hue stays the same.
 */
export const readableColor = (value: number, theme: ConsoleTheme): number => {
  const key = theme === 'dark' ? value : value | 0x1000000;
  const cached = readableCache.get(key);
  if (cached !== undefined) return cached;
  const bg = READABLE_ON[theme];
  let result = value;
  if (contrast(value, bg) < MIN_CONTRAST) {
    const [h, sat, l0] = toHsl(value);
    const step = theme === 'dark' ? 0.02 : -0.02;
    for (let l = l0 + step; l >= 0 && l <= 1; l += step) {
      result = fromHsl(h, sat, l);
      if (contrast(result, bg) >= MIN_CONTRAST) break;
    }
    if (contrast(result, bg) < MIN_CONTRAST)
      result = theme === 'dark' ? 0xffffff : 0x000000;
  }
  if (readableCache.size > 4096) readableCache.clear();
  readableCache.set(key, result);
  return result;
};
