import type { ClientFile } from 'bindings/ClientFile';

export type FileSortKey = 'name' | 'date_modified' | 'size';
export type FileSortDirection = 'asc' | 'desc';
export type FileSort = { key: FileSortKey; direction: FileSortDirection };

const nameCollator = new Intl.Collator(undefined, {
  numeric: true,
  sensitivity: 'base',
});

/** -1, 0 or 1; equal values compare as 0 */
export function compareValues(a: string | number, b: string | number): number {
  if (a < b) return -1;
  if (a > b) return 1;
  return 0;
}

/**
 * Orders by `direction`, except that a missing value (null or undefined)
 * sorts after every present one in both directions and equals another
 * missing value. 0 is a present value.
 */
export function compareNullable(
  a: string | number | null | undefined,
  b: string | number | null | undefined,
  direction: FileSortDirection
): number {
  if (a == null && b == null) return 0;
  if (a == null) return 1;
  if (b == null) return -1;
  const result = compareValues(a, b);
  return direction === 'asc' ? result : -result;
}

/** the timestamp the file list shows */
export function fileTime(file: ClientFile): number | null {
  return file.modification_time ?? file.creation_time;
}

function compareNames(a: ClientFile, b: ClientFile): number {
  return (
    nameCollator.compare(a.name, b.name) ||
    compareValues(a.name, b.name) ||
    compareValues(a.path, b.path)
  );
}

function compareByKey(
  a: ClientFile,
  b: ClientFile,
  { key, direction }: FileSort
): number {
  switch (key) {
    case 'name': {
      const result = compareNames(a, b);
      return direction === 'asc' ? result : -result;
    }
    case 'date_modified':
      return compareNullable(fileTime(a), fileTime(b), direction);
    case 'size':
      // the list shows no size for folders
      return compareNullable(
        a.file_type === 'Directory' ? null : a.size,
        b.file_type === 'Directory' ? null : b.size,
        direction
      );
  }
}

/**
 * Returns a sorted copy and leaves `files` as it was. Folders come first
 * whichever the direction. Files that tie on the key are ordered by name,
 * ascending, so the order is the same on every render. Without a sort the
 * list is returned as it is.
 */
export function sortFiles(
  files: ClientFile[],
  sort: FileSort | null
): ClientFile[] {
  if (!sort) return files;
  return [...files].sort((a, b) => {
    const aFolder = a.file_type === 'Directory';
    const bFolder = b.file_type === 'Directory';
    if (aFolder !== bFolder) return aFolder ? -1 : 1;
    return compareByKey(a, b, sort) || compareNames(a, b);
  });
}

/** clicking a column sorts by it ascending, and flips it if it is already the sort */
export function nextSort(current: FileSort | null, key: FileSortKey): FileSort {
  if (current?.key === key) {
    return { key, direction: current.direction === 'asc' ? 'desc' : 'asc' };
  }
  return { key, direction: 'asc' };
}
