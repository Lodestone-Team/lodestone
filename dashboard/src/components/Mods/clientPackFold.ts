/**
 * The client pack's mods shown while it is folded: about three rows of chips
 * at the tab's width (a chip with a name and a version is 150 to 250 px wide).
 */
export const PACK_CHIPS_FOLDED = 12;
/**
 * The client pack folds only above this many mods, so that "Show all" always
 * shows a row or more that was hidden. The Host folds it the same way.
 */
export const PACK_FOLDS_ABOVE = 16;

/**
 * How many of the client pack's `count` mods it shows, and whether it has the
 * control that shows them all (`open`) or folds them again.
 */
export const packChips = (
  count: number,
  open: boolean
): { shown: number; folds: boolean } => {
  if (count <= PACK_FOLDS_ABOVE) return { shown: count, folds: false };
  return { shown: open ? count : PACK_CHIPS_FOLDED, folds: true };
};
