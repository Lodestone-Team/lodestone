// The zoom levels and shortcuts of the desktop app (see zoom.ts), without
// imports so tests can load it.

/** The levels + and − step through, as in browsers. */
export const ZOOM_LEVELS = [
  0.5, 0.67, 0.75, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2,
] as const;

/** The level after `current` in the direction of `step` (+1 or −1). */
export const nextZoomLevel = (current: number, step: 1 | -1): number => {
  if (step > 0)
    return ZOOM_LEVELS.find((level) => level > current + 1e-3) ?? current;
  return (
    [...ZOOM_LEVELS].reverse().find((level) => level < current - 1e-3) ??
    current
  );
};

/** What a key press does to the zoom, or null if it isn't a zoom shortcut. */
export const zoomShortcut = (event: {
  key: string;
  code: string;
  ctrlKey: boolean;
  metaKey: boolean;
  altKey: boolean;
}): 'in' | 'out' | 'reset' | null => {
  if (!(event.ctrlKey || event.metaKey) || event.altKey) return null;
  if (
    ['=', '+'].includes(event.key) ||
    ['Equal', 'NumpadAdd'].includes(event.code)
  )
    return 'in';
  if (
    ['-', '_'].includes(event.key) ||
    ['Minus', 'NumpadSubtract'].includes(event.code)
  )
    return 'out';
  if (event.key === '0' || ['Digit0', 'Numpad0'].includes(event.code))
    return 'reset';
  return null;
};
