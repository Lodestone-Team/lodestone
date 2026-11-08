// simplified version of a ClientEvent with just InstanceOutput
export type ConsoleEvent = {
  timestamp: number;
  snowflake: string;
  detail: string;
  uuid: string;
  name: string;
  message: string;
};

/** Only the newest lines are kept, older ones are dropped. */
export const MAX_CONSOLE_LINES = 5000;

/** Most lines kept once older history has been loaded as well. */
export const MAX_LOADED_LINES = 20000;

// snowflakes are 64-bit, so compare them as BigInt
export const compareSnowflakes = (a: string, b: string) => {
  const x = BigInt(a);
  const y = BigInt(b);
  return x < y ? -1 : x > y ? 1 : 0;
};

/**
 * Merges new lines into the log, ordered by snowflake (time), without
 * duplicates, keeping at most `maxLines` of the newest.
 * Live lines are almost always newer than the last line, which is checked in
 * O(1). Only an overlapping batch (e.g. the buffer arriving after live lines,
 * or older history) needs the full merge.
 */
export const mergeConsoleEvents = (
  oldLog: ConsoleEvent[],
  newEvents: ConsoleEvent[],
  maxLines: number = MAX_CONSOLE_LINES
): ConsoleEvent[] => {
  if (newEvents.length === 0) return oldLog;
  let merged: ConsoleEvent[];
  let last = oldLog.length > 0 ? oldLog[oldLog.length - 1].snowflake : null;
  const appendable = newEvents.every((event) => {
    if (last !== null && compareSnowflakes(event.snowflake, last) <= 0)
      return false;
    last = event.snowflake;
    return true;
  });
  if (appendable) {
    merged = [...oldLog, ...newEvents];
  } else {
    const seen = new Set<string>();
    merged = [...oldLog, ...newEvents]
      .filter((event) => {
        if (seen.has(event.snowflake)) return false;
        seen.add(event.snowflake);
        return true;
      })
      .sort((a, b) => compareSnowflakes(a.snowflake, b.snowflake));
  }
  return merged.length > maxLines
    ? merged.slice(merged.length - maxLines)
    : merged;
};

/** The older of two snowflakes (`null` stands for none yet). */
export const olderSnowflake = (a: string | null, b: string | null) => {
  if (a === null) return b;
  if (b === null) return a;
  return compareSnowflakes(a, b) <= 0 ? a : b;
};
