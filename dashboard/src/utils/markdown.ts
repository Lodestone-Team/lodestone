/*
 * A small Markdown parser for untrusted text (Modrinth project bodies).
 *
 * It turns Markdown into a plain syntax tree that SafeMarkdown renders as
 * React elements. Nothing here produces HTML strings, and nothing is ever
 * passed to dangerouslySetInnerHTML.
 *
 * - Supported: ATX and setext headings, paragraphs, bold, italic,
 *   strikethrough, inline code, fenced and indented code blocks, block
 *   quotes, ordered and unordered lists, horizontal rules, tables, links
 *   (inline, reference and autolinks) and images.
 * - Raw HTML tags are removed and their text kept. `<a href>`, `<img src>`
 *   and `<br>` become the matching nodes. The content of `<script>` and
 *   `<style>` is dropped, and so are comments.
 * - Every link URL goes through safeLinkUrl (http and https only) and every
 *   image URL through safeImageUrl (https only). A rejected link keeps its
 *   text, a rejected image keeps its alt text.
 *
 * Pure functions with no imports, so they can be tested on their own.
 */

export type MdInline =
  | { type: 'text'; text: string }
  | { type: 'strong'; children: MdInline[] }
  | { type: 'em'; children: MdInline[] }
  | { type: 'del'; children: MdInline[] }
  | { type: 'code'; text: string }
  /** href is null when the URL was rejected; render the children only */
  | { type: 'link'; href: string | null; children: MdInline[] }
  /** src is null when the URL was rejected; render the alt text only */
  | { type: 'image'; src: string | null; alt: string }
  | { type: 'br' };

export type MdBlock =
  | { type: 'heading'; level: 1 | 2 | 3 | 4 | 5 | 6; children: MdInline[] }
  | { type: 'paragraph'; children: MdInline[] }
  | { type: 'code'; text: string }
  | { type: 'blockquote'; children: MdBlock[] }
  | { type: 'list'; ordered: boolean; start: number; items: MdBlock[][] }
  | { type: 'hr' }
  | {
      type: 'table';
      align: Array<'left' | 'center' | 'right' | null>;
      header: MdInline[][];
      rows: MdInline[][][];
    };

/** Longer bodies are cut off (Modrinth itself allows 65,536 characters). */
export const MAX_MARKDOWN_LENGTH = 200_000;
/** Block quotes and lists nested deeper than this are shown as text. */
const MAX_BLOCK_DEPTH = 12;
/** Inline nodes nested deeper than this are shown as text. */
const MAX_INLINE_DEPTH = 12;
/** How far ahead an emphasis or link looks for its closing delimiter. */
const SCAN_WINDOW = 2_000;

/* ------------------------------------------------------------------------ */
/* URL filtering                                                            */
/* ------------------------------------------------------------------------ */

const parseAbsoluteUrl = (raw: string): URL | null => {
  const trimmed = raw.trim();
  if (trimmed === '') return null;
  try {
    // no base: relative and protocol-relative URLs are rejected
    return new URL(trimmed);
  } catch {
    return null;
  }
};

/**
 * The URL to use for a link, or null if it is not an absolute http: or
 * https: URL. Returns the parsed, normalized form, so what is rendered is
 * exactly what was checked.
 */
export function safeLinkUrl(raw: string): string | null {
  const url = parseAbsoluteUrl(raw);
  if (!url) return null;
  if (url.protocol !== 'http:' && url.protocol !== 'https:') return null;
  return url.href;
}

/** The URL to use for an image, or null if it is not an absolute https: URL. */
export function safeImageUrl(raw: string): string | null {
  const url = parseAbsoluteUrl(raw);
  if (!url) return null;
  if (url.protocol !== 'https:') return null;
  return url.href;
}

/* ------------------------------------------------------------------------ */
/* HTML entities and tags                                                   */
/* ------------------------------------------------------------------------ */

const NAMED_ENTITIES: Record<string, string> = {
  amp: '&',
  lt: '<',
  gt: '>',
  quot: '"',
  apos: "'",
  nbsp: ' ',
  copy: '©',
  reg: '®',
  trade: '™',
  hellip: '…',
  mdash: '—',
  ndash: '–',
  laquo: '«',
  raquo: '»',
  middot: '·',
  bull: '•',
  times: '×',
  rarr: '→',
  larr: '←',
  deg: '°',
};

/** Decodes HTML character references. Unknown names are left as they are. */
export function decodeEntities(text: string): string {
  if (!text.includes('&')) return text;
  return text.replace(
    /&(#[0-9]{1,7}|#[xX][0-9a-fA-F]{1,6}|[a-zA-Z][a-zA-Z0-9]{1,9});/g,
    (match, body: string) => {
      if (body[0] === '#') {
        const codePoint =
          body[1] === 'x' || body[1] === 'X'
            ? parseInt(body.slice(2), 16)
            : parseInt(body.slice(1), 10);
        const valid =
          codePoint > 0 &&
          codePoint <= 0x10ffff &&
          !(codePoint >= 0xd800 && codePoint <= 0xdfff);
        return valid ? String.fromCodePoint(codePoint) : '�';
      }
      return NAMED_ENTITIES[body] ?? match;
    }
  );
}

// An HTML tag at the start of the string: closing slash, name, attributes.
// The attribute part has no nested ambiguity, and it is only ever matched
// against a bounded slice, so it can't backtrack badly.
const TAG_RE =
  /^<(\/?)([a-zA-Z][a-zA-Z0-9-]*)((?:\s+[^\s"'>/=]+(?:\s*=\s*(?:"[^"]*"|'[^']*'|[^\s"'=<>`]+))?)*)\s*\/?>/;
const ATTRIBUTE_RE =
  /([^\s"'>/=]+)(?:\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s"'=<>`]+)))?/g;
const MAX_TAG_LENGTH = 2_000;
/** Tags whose content is dropped along with the tag. */
const DROP_CONTENT_TAGS = ['script', 'style', 'textarea', 'title'];

type HtmlTag = {
  closing: boolean;
  name: string;
  attributes: Record<string, string>;
  length: number;
};

const matchTag = (text: string, pos: number): HtmlTag | null => {
  const match = TAG_RE.exec(text.slice(pos, pos + MAX_TAG_LENGTH));
  if (!match) return null;
  const attributes: Record<string, string> = {};
  for (const attr of match[3].matchAll(ATTRIBUTE_RE)) {
    const name = attr[1].toLowerCase();
    if (name in attributes) continue;
    attributes[name] = decodeEntities(attr[2] ?? attr[3] ?? attr[4] ?? '');
  }
  return {
    closing: match[1] === '/',
    name: match[2].toLowerCase(),
    attributes,
    length: match[0].length,
  };
};

/** Finds `</name>` from pos, case-insensitively. Returns its index or -1. */
const findClosingTag = (text: string, name: string, pos: number): number => {
  const lower = text.toLowerCase();
  const re = new RegExp(`</${name}\\s*>`, 'g');
  re.lastIndex = pos;
  const match = re.exec(lower);
  return match ? match.index : -1;
};

/**
 * The text a reader sees for a piece of inline Markdown: HTML tags and
 * comments removed with the text between them kept, script and style
 * content removed, character references decoded, and Markdown markup gone.
 */
export function inlineMarkdownToText(text: string): string {
  return inlineToText(parseInline(text, emptyRefs()));
}

/* ------------------------------------------------------------------------ */
/* Inline parsing                                                           */
/* ------------------------------------------------------------------------ */

type RefMap = Map<string, string>;
const emptyRefs = (): RefMap => new Map();
const normalizeLabel = (label: string) =>
  label.trim().replace(/\s+/g, ' ').toLowerCase();

const ASCII_PUNCTUATION = /[!-/:-@[-`{-~]/;
const isWhitespace = (c: string | undefined) => !c || /\s/.test(c);
const isAlphanumeric = (c: string | undefined) =>
  !!c && /[\p{L}\p{N}]/u.test(c);

/** Concatenates the text of inline nodes, e.g. for image alt text. */
export function inlineToText(nodes: MdInline[]): string {
  return nodes
    .map((node) => {
      switch (node.type) {
        case 'text':
        case 'code':
          return node.text;
        case 'image':
          return node.alt;
        case 'br':
          return '\n';
        default:
          return inlineToText(node.children);
      }
    })
    .join('');
}

type InlineState = {
  refs: RefMap;
  depth: number;
  /** links can't contain links */
  inLink: boolean;
  /** code span lengths with no closing run at or after the given index */
  noCodeCloser: Map<number, number>;
};

/** Finds the `]` matching the `[` at pos, or -1. */
const findLabelEnd = (text: string, pos: number): number => {
  let depth = 0;
  const end = Math.min(text.length, pos + SCAN_WINDOW);
  for (let i = pos; i < end; i++) {
    const c = text[i];
    if (c === '\\') {
      i++;
    } else if (c === '`') {
      // brackets inside a code span don't count
      let run = 1;
      while (text[i + run] === '`') run++;
      const close = text.indexOf('`'.repeat(run), i + run);
      if (close !== -1 && close < end) i = close + run - 1;
      else i += run - 1;
    } else if (c === '[') {
      depth++;
    } else if (c === ']') {
      depth--;
      if (depth === 0) return i;
    }
  }
  return -1;
};

type LinkTail = { url: string; length: number };

/** Parses `(url "title")` at pos. */
const parseInlineDestination = (text: string, pos: number): LinkTail | null => {
  if (text[pos] !== '(') return null;
  let i = pos + 1;
  while (i < text.length && /[ \t\n]/.test(text[i])) i++;
  let url = '';
  if (text[i] === '<') {
    const close = text.indexOf('>', i + 1);
    if (close === -1) return null;
    url = text.slice(i + 1, close);
    if (url.includes('\n')) return null;
    i = close + 1;
  } else {
    let parens = 0;
    const start = i;
    const end = Math.min(text.length, pos + SCAN_WINDOW);
    for (; i < end; i++) {
      const c = text[i];
      if (c === '\\' && i + 1 < text.length) {
        i++;
      } else if (c === '(') {
        parens++;
      } else if (c === ')') {
        if (parens === 0) break;
        parens--;
      } else if (/\s/.test(c)) {
        break;
      }
    }
    url = text.slice(start, i).replace(/\\([!-/:-@[-`{-~])/g, '$1');
  }
  while (i < text.length && /[ \t\n]/.test(text[i])) i++;
  // an optional title, which is not shown
  const quote = text[i];
  if (quote === '"' || quote === "'" || quote === '(') {
    const closer = quote === '(' ? ')' : quote;
    const close = text.indexOf(closer, i + 1);
    if (close === -1 || close - pos > SCAN_WINDOW) return null;
    i = close + 1;
    while (i < text.length && /[ \t\n]/.test(text[i])) i++;
  }
  if (text[i] !== ')') return null;
  return { url: decodeEntities(url), length: i + 1 - pos };
};

/**
 * Parses what follows a link label `[...]` that ends at labelEnd: an inline
 * destination, a full or collapsed reference, or a shortcut reference.
 */
const parseLinkTail = (
  text: string,
  labelStart: number,
  labelEnd: number,
  refs: RefMap
): LinkTail | null => {
  const after = labelEnd + 1;
  const inline = parseInlineDestination(text, after);
  if (inline) return inline;
  if (text[after] === '[') {
    const refEnd = text.indexOf(']', after + 1);
    if (refEnd !== -1 && refEnd - after < 1000) {
      const refLabel = text.slice(after + 1, refEnd);
      const key = normalizeLabel(
        refLabel === '' ? text.slice(labelStart + 1, labelEnd) : refLabel
      );
      const url = refs.get(key);
      if (url !== undefined) return { url, length: refEnd + 1 - labelEnd - 1 };
    }
  }
  const url = refs.get(normalizeLabel(text.slice(labelStart + 1, labelEnd)));
  if (url !== undefined) return { url, length: 0 };
  return null;
};

/** Finds a closing emphasis run of `delimiter` after pos, or -1. */
const findEmphasisCloser = (
  text: string,
  delimiter: string,
  pos: number
): number => {
  const c = delimiter[0];
  const end = Math.min(text.length, pos + SCAN_WINDOW);
  let i = pos;
  while (i < end) {
    const found = text.indexOf(c, i);
    if (found === -1 || found >= end) return -1;
    if (text[found - 1] === '\\') {
      i = found + 1;
      continue;
    }
    let run = 0;
    while (text[found + run] === c) run++;
    // a single delimiter skips longer runs, which belong to nested emphasis
    const closes =
      run >= delimiter.length &&
      !(delimiter.length === 1 && run > 1) &&
      !isWhitespace(text[found - 1]) &&
      found > pos &&
      (c !== '_' || !isAlphanumeric(text[found + run]));
    if (closes) return found + run - delimiter.length;
    i = found + run;
  }
  return -1;
};

const BARE_URL_RE = /^https?:\/\/[^\s<>"]+/i;
const AUTOLINK_RE = /^<([a-zA-Z][a-zA-Z0-9+.-]{1,31}:[^\s<>]*)>/;

/** Removes trailing punctuation from a bare URL, as GitHub does. */
const trimBareUrl = (url: string): string => {
  let result = url;
  for (;;) {
    const last = result[result.length - 1];
    if (/[.,:;!?'"*_~]/.test(last)) {
      result = result.slice(0, -1);
    } else if (last === ')') {
      const opens = (result.match(/\(/g) ?? []).length;
      const closes = (result.match(/\)/g) ?? []).length;
      if (closes > opens) result = result.slice(0, -1);
      else break;
    } else {
      break;
    }
  }
  return result;
};

const parseInlineWith = (text: string, state: InlineState): MdInline[] => {
  const nodes: MdInline[] = [];
  let buffer = '';
  const flush = () => {
    if (buffer !== '') {
      nodes.push({ type: 'text', text: decodeEntities(buffer) });
      buffer = '';
    }
  };
  const push = (node: MdInline) => {
    flush();
    nodes.push(node);
  };
  const nested = (inner: string, inLink = state.inLink): MdInline[] =>
    state.depth >= MAX_INLINE_DEPTH
      ? [{ type: 'text', text: stripTagsShallow(inner) }]
      : parseInlineWith(inner, { ...state, depth: state.depth + 1, inLink });

  let i = 0;
  while (i < text.length) {
    const c = text[i];

    // backslash escapes and hard line breaks
    if (c === '\\') {
      const next = text[i + 1];
      if (next === '\n') {
        push({ type: 'br' });
        i += 2;
        continue;
      }
      if (next !== undefined && ASCII_PUNCTUATION.test(next)) {
        // escaped characters are literal, including `&`
        flush();
        nodes.push({ type: 'text', text: next });
        i += 2;
        continue;
      }
    }

    // soft and hard line breaks
    if (c === '\n') {
      if (buffer.endsWith('  ')) {
        buffer = buffer.trimEnd();
        push({ type: 'br' });
      } else {
        buffer = buffer.trimEnd() + ' ';
      }
      i++;
      while (text[i] === ' ' || text[i] === '\t') i++;
      continue;
    }

    // code spans
    if (c === '`') {
      let run = 1;
      while (text[i + run] === '`') run++;
      const knownMissing = state.noCodeCloser.get(run);
      let close = -1;
      if (knownMissing === undefined || i + run < knownMissing) {
        let from = i + run;
        for (;;) {
          const found = text.indexOf('`'.repeat(run), from);
          if (found === -1) break;
          let foundRun = run;
          while (text[found + foundRun] === '`') foundRun++;
          if (foundRun === run && text[found - 1] !== '`') {
            close = found;
            break;
          }
          from = found + foundRun;
        }
        if (close === -1) state.noCodeCloser.set(run, i + run);
      }
      if (close !== -1) {
        let code = text.slice(i + run, close).replace(/\n/g, ' ');
        if (
          code.length > 2 &&
          code.startsWith(' ') &&
          code.endsWith(' ') &&
          code.trim() !== ''
        )
          code = code.slice(1, -1);
        push({ type: 'code', text: code });
        i = close + run;
      } else {
        buffer += '`'.repeat(run);
        i += run;
      }
      continue;
    }

    // images
    if (c === '!' && text[i + 1] === '[') {
      const labelEnd = findLabelEnd(text, i + 1);
      if (labelEnd !== -1) {
        const tail = parseLinkTail(text, i + 1, labelEnd, state.refs);
        if (tail) {
          const alt = inlineToText(nested(text.slice(i + 2, labelEnd)));
          push({ type: 'image', src: safeImageUrl(tail.url), alt });
          i = labelEnd + 1 + tail.length;
          continue;
        }
      }
    }

    // links
    if (c === '[' && !state.inLink) {
      const labelEnd = findLabelEnd(text, i);
      if (labelEnd !== -1) {
        const tail = parseLinkTail(text, i, labelEnd, state.refs);
        if (tail) {
          push({
            type: 'link',
            href: safeLinkUrl(tail.url),
            children: nested(text.slice(i + 1, labelEnd), true),
          });
          i = labelEnd + 1 + tail.length;
          continue;
        }
      }
    }

    if (c === '<') {
      // autolinks
      const autolink = AUTOLINK_RE.exec(text.slice(i, i + MAX_TAG_LENGTH));
      if (autolink) {
        const url = decodeEntities(autolink[1]);
        if (state.inLink) {
          buffer += url;
        } else {
          push({
            type: 'link',
            href: safeLinkUrl(url),
            children: [{ type: 'text', text: url }],
          });
        }
        i += autolink[0].length;
        continue;
      }
      // comments, removed
      if (text.startsWith('<!--', i)) {
        const close = text.indexOf('-->', i + 4);
        i = close === -1 ? text.length : close + 3;
        continue;
      }
      // other markup declarations and processing instructions, removed
      if (text[i + 1] === '!' || text[i + 1] === '?') {
        const close = text.indexOf('>', i + 2);
        if (close !== -1) {
          i = close + 1;
          continue;
        }
      }
      const tag = matchTag(text, i);
      if (tag) {
        i = handleTag(tag, text, i, state, push, nested);
        continue;
      }
    }

    // emphasis and strikethrough
    if (c === '*' || c === '_' || c === '~') {
      let run = 1;
      while (text[i + run] === c) run++;
      const next = text[i + run];
      const prev = text[i - 1];
      const canOpen =
        !isWhitespace(next) && (c !== '_' || !isAlphanumeric(prev));
      if (canOpen) {
        const attempts: Array<[string, (inner: MdInline[]) => MdInline]> =
          c === '~'
            ? run === 2
              ? [['~~', (children) => ({ type: 'del', children })]]
              : []
            : [
                ...(run >= 3
                  ? [
                      [
                        c.repeat(3),
                        (children: MdInline[]) =>
                          ({
                            type: 'strong',
                            children: [{ type: 'em', children }],
                          }) as MdInline,
                      ] as [string, (inner: MdInline[]) => MdInline],
                    ]
                  : []),
                ...(run >= 2
                  ? [
                      [
                        c.repeat(2),
                        (children: MdInline[]) =>
                          ({ type: 'strong', children }) as MdInline,
                      ] as [string, (inner: MdInline[]) => MdInline],
                    ]
                  : []),
                [
                  c,
                  (children: MdInline[]) =>
                    ({ type: 'em', children }) as MdInline,
                ],
              ];
        let matched = false;
        for (const [delimiter, make] of attempts) {
          const close = findEmphasisCloser(text, delimiter, i + run);
          if (close !== -1) {
            // the opening run's extra delimiters stay literal
            buffer += c.repeat(run - delimiter.length);
            push(make(nested(text.slice(i + run, close))));
            i = close + delimiter.length;
            matched = true;
            break;
          }
        }
        if (matched) continue;
      }
      buffer += c.repeat(run);
      i += run;
      continue;
    }

    // bare URLs
    if (
      (c === 'h' || c === 'H') &&
      !state.inLink &&
      !isAlphanumeric(text[i - 1])
    ) {
      const match = BARE_URL_RE.exec(text.slice(i, i + SCAN_WINDOW));
      if (match) {
        const url = trimBareUrl(match[0]);
        const decoded = decodeEntities(url);
        push({
          type: 'link',
          href: safeLinkUrl(decoded),
          children: [{ type: 'text', text: decoded }],
        });
        i += url.length;
        continue;
      }
    }

    buffer += c;
    i++;
  }
  flush();
  return mergeText(nodes);
};

/**
 * Handles an HTML tag found at pos, and returns the index to continue at.
 * `<a href>` takes its content up to `</a>` as the link text, `<img>` and
 * `<br>` become nodes, and every other tag is dropped.
 */
const handleTag = (
  tag: HtmlTag,
  text: string,
  pos: number,
  state: InlineState,
  push: (node: MdInline) => void,
  nested: (inner: string, inLink?: boolean) => MdInline[]
): number => {
  const after = pos + tag.length;
  if (tag.closing) return after;
  if (DROP_CONTENT_TAGS.includes(tag.name)) {
    const close = findClosingTag(text, tag.name, after);
    if (close === -1) return text.length;
    return close + `</${tag.name}>`.length;
  }
  if (tag.name === 'br') {
    push({ type: 'br' });
    return after;
  }
  if (tag.name === 'img') {
    const src = tag.attributes.src;
    if (src !== undefined)
      push({
        type: 'image',
        src: safeImageUrl(src),
        alt: tag.attributes.alt ?? '',
      });
    return after;
  }
  if (tag.name === 'a' && !state.inLink) {
    const close = findClosingTag(text, 'a', after);
    const href = tag.attributes.href;
    if (close !== -1 && href !== undefined) {
      push({
        type: 'link',
        href: safeLinkUrl(href),
        children: nested(text.slice(after, close), true),
      });
      const closeTag = matchTag(text, close);
      return close + (closeTag ? closeTag.length : 4);
    }
  }
  return after;
};

/** Removes anything tag-like from text, for content nested too deeply. */
const stripTagsShallow = (text: string) =>
  decodeEntities(text.replace(/<[^>]{0,2000}>/g, ''));

const mergeText = (nodes: MdInline[]): MdInline[] => {
  const merged: MdInline[] = [];
  for (const node of nodes) {
    const last = merged[merged.length - 1];
    if (node.type === 'text' && last?.type === 'text') {
      merged[merged.length - 1] = { type: 'text', text: last.text + node.text };
    } else {
      merged.push(node);
    }
  }
  return merged;
};

/** Parses inline Markdown (emphasis, code, links, images, inline HTML). */
export function parseInline(text: string, refs: RefMap = emptyRefs()) {
  return parseInlineWith(text, {
    refs,
    depth: 0,
    inLink: false,
    noCodeCloser: new Map(),
  });
}

/* ------------------------------------------------------------------------ */
/* Block parsing                                                            */
/* ------------------------------------------------------------------------ */

const FENCE_RE = /^( {0,3})(`{3,}|~{3,})(.*)$/;
const ATX_RE = /^ {0,3}(#{1,6})(?:[ \t]+(.*?))?(?:[ \t]+#+)?[ \t]*$/;
const HR_RE = /^ {0,3}(?:(?:-[ \t]*){3,}|(?:\*[ \t]*){3,}|(?:_[ \t]*){3,})$/;
const QUOTE_RE = /^ {0,3}> ?/;
const LIST_RE = /^( {0,3})([-*+]|\d{1,9}[.)])([ \t]+|$)(.*)$/;
const SETEXT_RE = /^ {0,3}(=+|-+)[ \t]*$/;
const REF_DEF_RE =
  /^ {0,3}\[([^\]]{1,999})\]:[ \t]*<?([^\s>]+)>?(?:[ \t]+(?:"[^"]*"|'[^']*'|\([^)]*\)))?[ \t]*$/;
const INDENTED_RE = /^ {4}/;

const TABLE_DELIMITER_RE =
  /^ {0,3}\|?[ \t]*:?-+:?[ \t]*(?:\|[ \t]*:?-+:?[ \t]*)*\|?[ \t]*$/;
/** At most this many columns are shown. */
const MAX_TABLE_COLUMNS = 64;

const isBlank = (line: string) => line.trim() === '';

/** Splits a table row into cells, on `|` that are not escaped. */
const splitTableRow = (line: string): string[] => {
  let row = line.trim();
  if (row.startsWith('|')) row = row.slice(1);
  if (row.endsWith('|') && !row.endsWith('\\|')) row = row.slice(0, -1);
  const cells: string[] = [];
  let cell = '';
  for (let i = 0; i < row.length; i++) {
    if (row[i] === '\\' && row[i + 1] === '|') {
      cell += '|';
      i++;
    } else if (row[i] === '|') {
      cells.push(cell.trim());
      cell = '';
    } else {
      cell += row[i];
    }
  }
  cells.push(cell.trim());
  return cells;
};

/** Whether lines[i] and lines[i + 1] start a table. */
const startsTable = (lines: string[], i: number) =>
  i + 1 < lines.length &&
  lines[i].includes('|') &&
  TABLE_DELIMITER_RE.test(lines[i + 1]) &&
  splitTableRow(lines[i]).length === splitTableRow(lines[i + 1]).length;

/** Whether a line starts a block that interrupts a paragraph. */
const startsBlock = (line: string) =>
  FENCE_RE.test(line) ||
  ATX_RE.test(line) ||
  HR_RE.test(line) ||
  QUOTE_RE.test(line) ||
  LIST_RE.test(line);

const expandTabs = (line: string) =>
  line.replace(/^[ \t]+/, (indent) => indent.replace(/\t/g, '    '));

const leadingSpaces = (line: string) => line.length - line.trimStart().length;

/** Removes up to n leading spaces. */
const dedent = (line: string, n: number) =>
  line.slice(Math.min(n, leadingSpaces(line)));

const paragraphBlock = (text: string, refs: RefMap): MdBlock | null => {
  const children = parseInline(text.trim(), refs);
  const empty = children.every(
    (node) => node.type === 'text' && node.text.trim() === ''
  );
  return empty ? null : { type: 'paragraph', children };
};

const parseBlocks = (
  lines: string[],
  refs: RefMap,
  depth: number
): MdBlock[] => {
  const blocks: MdBlock[] = [];
  const pushBlock = (block: MdBlock | null) => {
    if (block) blocks.push(block);
  };
  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    if (isBlank(line)) {
      i++;
      continue;
    }

    // fenced code
    const fence = FENCE_RE.exec(line);
    if (fence && !(fence[2][0] === '`' && fence[3].includes('`'))) {
      const indent = fence[1].length;
      const marker = fence[2];
      const closing = new RegExp(
        `^ {0,3}${marker[0] === '`' ? '`' : '~'}{${marker.length},}[ \\t]*$`
      );
      const content: string[] = [];
      i++;
      while (i < lines.length) {
        if (closing.test(lines[i])) {
          i++;
          break;
        }
        content.push(dedent(lines[i], indent));
        i++;
      }
      blocks.push({ type: 'code', text: content.join('\n') });
      continue;
    }

    // indented code
    if (INDENTED_RE.test(line)) {
      const content: string[] = [];
      while (
        i < lines.length &&
        (INDENTED_RE.test(lines[i]) || isBlank(lines[i]))
      ) {
        content.push(dedent(lines[i], 4));
        i++;
      }
      while (content.length > 0 && isBlank(content[content.length - 1]))
        content.pop();
      blocks.push({ type: 'code', text: content.join('\n') });
      continue;
    }

    // ATX headings
    const atx = ATX_RE.exec(line);
    if (atx) {
      blocks.push({
        type: 'heading',
        level: atx[1].length as 1 | 2 | 3 | 4 | 5 | 6,
        children: parseInline((atx[2] ?? '').trim(), refs),
      });
      i++;
      continue;
    }

    if (HR_RE.test(line)) {
      blocks.push({ type: 'hr' });
      i++;
      continue;
    }

    // block quotes
    if (QUOTE_RE.test(line)) {
      const content: string[] = [];
      while (i < lines.length) {
        const current = lines[i];
        if (QUOTE_RE.test(current)) {
          content.push(current.replace(QUOTE_RE, ''));
        } else if (
          !isBlank(current) &&
          content.length > 0 &&
          !isBlank(content[content.length - 1]) &&
          !startsBlock(current)
        ) {
          // lazy continuation of a paragraph in the quote
          content.push(current);
        } else {
          break;
        }
        i++;
      }
      blocks.push(
        depth >= MAX_BLOCK_DEPTH
          ? {
              type: 'paragraph',
              children: [{ type: 'text', text: content.join(' ') }],
            }
          : {
              type: 'blockquote',
              children: parseBlocks(content, refs, depth + 1),
            }
      );
      continue;
    }

    // tables
    if (startsTable(lines, i)) {
      const header = splitTableRow(line).slice(0, MAX_TABLE_COLUMNS);
      const align = splitTableRow(lines[i + 1])
        .slice(0, MAX_TABLE_COLUMNS)
        .map((cell) => {
          const left = cell.startsWith(':');
          const right = cell.endsWith(':');
          if (left && right) return 'center' as const;
          if (right) return 'right' as const;
          if (left) return 'left' as const;
          return null;
        });
      const rows: MdInline[][][] = [];
      i += 2;
      while (i < lines.length && !isBlank(lines[i]) && !startsBlock(lines[i])) {
        const cells = splitTableRow(lines[i]);
        rows.push(
          header.map((_, column) => parseInline(cells[column] ?? '', refs))
        );
        i++;
      }
      blocks.push({
        type: 'table',
        align,
        header: header.map((cell) => parseInline(cell, refs)),
        rows,
      });
      continue;
    }

    // lists
    const item = LIST_RE.exec(line);
    if (item) {
      const [list, next] = parseList(lines, i, refs, depth);
      blocks.push(list);
      i = next;
      continue;
    }

    // paragraphs, and setext headings
    const content: string[] = [line];
    i++;
    let heading: 1 | 2 | null = null;
    while (i < lines.length && !isBlank(lines[i])) {
      const setext = SETEXT_RE.exec(lines[i]);
      if (setext) {
        heading = setext[1][0] === '=' ? 1 : 2;
        i++;
        break;
      }
      if (startsBlock(lines[i]) || startsTable(lines, i)) break;
      content.push(lines[i]);
      i++;
    }
    if (heading) {
      blocks.push({
        type: 'heading',
        level: heading,
        children: parseInline(content.join('\n').trim(), refs),
      });
    } else {
      pushBlock(paragraphBlock(content.join('\n'), refs));
    }
  }
  return blocks;
};

/** Parses the list that starts at lines[start]. Returns it and the next index. */
const parseList = (
  lines: string[],
  start: number,
  refs: RefMap,
  depth: number
): [MdBlock, number] => {
  const first = LIST_RE.exec(lines[start]);
  if (!first) throw new Error('parseList called on a line that is no list');
  const ordered = /\d/.test(first[2][0]);
  // items of one list share the bullet, or the ordered delimiter
  const markerKind = (marker: string) =>
    /\d/.test(marker[0]) ? marker[marker.length - 1] : marker;
  const kind = markerKind(first[2]);
  const startNumber = ordered ? parseInt(first[2], 10) : 1;

  const items: string[][] = [];
  let i = start;
  while (i < lines.length) {
    const match = LIST_RE.exec(lines[i]);
    if (!match || markerKind(match[2]) !== kind || HR_RE.test(lines[i])) break;
    const indent = match[1].length;
    const spaces = match[3].length;
    const contentIndent =
      indent + match[2].length + (spaces === 0 || spaces > 4 ? 1 : spaces);
    const itemLines = [
      spaces > 4 ? ' '.repeat(spaces - 1) + match[4] : match[4],
    ];
    i++;
    while (i < lines.length) {
      const current = lines[i];
      if (isBlank(current)) {
        // a blank line continues the item only if indented content follows
        let next = i + 1;
        while (next < lines.length && isBlank(lines[next])) next++;
        if (
          next < lines.length &&
          leadingSpaces(lines[next]) >= contentIndent
        ) {
          for (; i < next; i++) itemLines.push('');
          continue;
        }
        break;
      }
      if (leadingSpaces(current) >= contentIndent) {
        itemLines.push(dedent(current, contentIndent));
      } else if (
        !startsBlock(current) &&
        !isBlank(itemLines[itemLines.length - 1])
      ) {
        // lazy continuation of the item's paragraph
        itemLines.push(current.trimStart());
      } else {
        break;
      }
      i++;
    }
    items.push(itemLines);
    // a blank line between items keeps the list going
    let next = i;
    while (next < lines.length && isBlank(lines[next])) next++;
    const following = next < lines.length ? LIST_RE.exec(lines[next]) : null;
    if (following && markerKind(following[2]) === kind) i = next;
    else break;
  }

  const list: MdBlock = {
    type: 'list',
    ordered,
    start: startNumber,
    items: items.map((itemLines) =>
      depth >= MAX_BLOCK_DEPTH
        ? [
            {
              type: 'paragraph',
              children: [{ type: 'text', text: itemLines.join(' ') }],
            },
          ]
        : parseBlocks(itemLines, refs, depth + 1)
    ),
  };
  return [list, i];
};

/**
 * Collects link reference definitions (`[label]: url`) outside code fences,
 * and blanks their lines.
 */
const collectReferences = (lines: string[]): [string[], RefMap] => {
  const refs: RefMap = new Map();
  const result: string[] = [];
  let fence: string | null = null;
  for (const line of lines) {
    const fenceMatch = FENCE_RE.exec(line);
    if (fence !== null) {
      if (
        fenceMatch &&
        fenceMatch[2][0] === fence[0] &&
        fenceMatch[2].length >= fence.length &&
        fenceMatch[3].trim() === ''
      )
        fence = null;
      result.push(line);
      continue;
    }
    if (fenceMatch) {
      fence = fenceMatch[2];
      result.push(line);
      continue;
    }
    const def = REF_DEF_RE.exec(line);
    if (def) {
      const key = normalizeLabel(def[1]);
      if (!refs.has(key)) refs.set(key, decodeEntities(def[2]));
      result.push('');
      continue;
    }
    result.push(line);
  }
  return [result, refs];
};

/** Parses a Markdown document into blocks. Never throws on any input. */
export function parseMarkdown(source: string): MdBlock[] {
  const text = source
    .slice(0, MAX_MARKDOWN_LENGTH)
    .replace(/\r\n?/g, '\n')
    .replace(/\u0000/g, '�');
  const [lines, refs] = collectReferences(text.split('\n').map(expandTabs));
  return parseBlocks(lines, refs, 0);
}
