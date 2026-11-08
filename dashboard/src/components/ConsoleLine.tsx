import { CSSProperties, memo } from 'react';
import {
  BADGE_PALETTE,
  BOLD,
  ConsoleColor,
  ConsoleSegment,
  ConsoleTheme,
  DIM,
  INVERSE,
  ITALIC,
  ParsedConsoleLine,
  RGB_BASE,
  STRIKE,
  UNDERLINE,
  luminance,
  readableColor,
  toHex,
  xterm256,
} from 'utils/consoleFormat';

// Marks the console's own text and background colour after an inverse swap
const DEFAULT_FG = -1;
const DEFAULT_BG = -2;

/** A text colour on the console background, readable in the theme */
const textColor = (color: ConsoleColor, theme: ConsoleTheme) => {
  // the 16 theme colours are CSS variables (--console-0 ... in globals.css)
  if (color < 16) return `var(--console-${color})`;
  const value = color < RGB_BASE ? xterm256(color) : color - RGB_BASE;
  return toHex(readableColor(value, theme));
};

/** The 0xRRGGBB value of a colour on a badge, the same in both themes */
const badgeValue = (color: ConsoleColor) =>
  color < 16
    ? BADGE_PALETTE[color]
    : color < RGB_BASE
      ? xterm256(color)
      : color - RGB_BASE;

const segmentStyle = (
  segment: ConsoleSegment,
  theme: ConsoleTheme
): CSSProperties => {
  const style: CSSProperties = {};
  let fg: number | null = segment.fg;
  let bg: number | null = segment.bg;
  if (segment.flags & INVERSE) {
    fg = segment.bg ?? DEFAULT_BG;
    bg = segment.fg ?? DEFAULT_FG;
  }

  let color: string | undefined;
  if (bg === DEFAULT_FG) {
    // inverse without colours: the console's text and background swapped
    style.backgroundColor = 'var(--gray-300)';
    color = fg === DEFAULT_BG ? 'var(--gray-900)' : toHex(badgeValue(fg!));
  } else if (bg !== null) {
    // text on a background colour: a badge, readable on its own
    const background = badgeValue(bg);
    style.backgroundColor = toHex(background);
    color =
      fg === null || fg === DEFAULT_BG
        ? luminance(background) > 0.179
          ? '#000000'
          : '#ffffff'
        : toHex(badgeValue(fg));
  } else if (fg !== null) {
    color = textColor(fg, theme);
  }

  if (segment.flags & DIM)
    color = `color-mix(in srgb, ${color ?? 'currentColor'} 65%, transparent)`;
  if (color) style.color = color;
  if (segment.flags & BOLD) style.fontWeight = 700;
  if (segment.flags & ITALIC) style.fontStyle = 'italic';
  const decoration = [
    segment.flags & UNDERLINE ? 'underline' : '',
    segment.flags & STRIKE ? 'line-through' : '',
  ]
    .filter(Boolean)
    .join(' ');
  if (decoration) style.textDecorationLine = decoration;
  return style;
};

const levelColors = {
  warn: 'var(--console-warn)',
  error: 'var(--console-error)',
};

/**
 * One console line. With `colored` off it shows the plain text. Lines are
 * memoized: a new line only renders itself, not the lines before it.
 */
export const ConsoleLine = memo(function ConsoleLine({
  line,
  colored,
  theme,
}: {
  line: ParsedConsoleLine;
  colored: boolean;
  theme: ConsoleTheme;
}) {
  return (
    <li
      className="px-4 py-[0.125rem] hover:bg-gray-800"
      style={
        colored && line.level ? { color: levelColors[line.level] } : undefined
      }
    >
      {!colored || !line.styled
        ? line.plain
        : line.segments.map((segment, index) =>
            segment.fg === null &&
            segment.bg === null &&
            segment.flags === 0 ? (
              segment.text
            ) : (
              <span key={index} style={segmentStyle(segment, theme)}>
                {segment.text}
              </span>
            )
          )}
    </li>
  );
});
