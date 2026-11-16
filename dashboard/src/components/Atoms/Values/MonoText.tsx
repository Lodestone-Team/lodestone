import { ReactNode } from 'react';
import { cn } from 'utils/util';
import { middleTruncate, shortenPath } from 'utils/values';

/**
 * Exact text (a file name, path, address or id) in the mono font. `middle`
 * keeps the end (".jar") visible, `path` shows the home folder as "~" and
 * keeps the last two parts, `end` lets CSS cut it at the end. The full value
 * is always in the title and in the text screen readers get.
 */
export function MonoText({
  text,
  truncate = 'middle',
  max = 40,
  className,
}: {
  text: string;
  truncate?: 'end' | 'middle' | 'path';
  max?: number;
  className?: string;
}) {
  const shown =
    truncate === 'path'
      ? shortenPath(text, { max })
      : truncate === 'middle'
        ? middleTruncate(text, max)
        : text;
  return (
    <span
      className={cn(
        'font-mono',
        truncate === 'end' && 'inline-block max-w-full truncate align-bottom',
        className
      )}
      title={text}
    >
      {shown === text ? (
        text
      ) : (
        <>
          <span aria-hidden="true">{shown}</span>
          <span className="sr-only">{text}</span>
        </>
      )}
    </span>
  );
}

/** A short exact value inside a sentence (a rich() param): `mods/`, a port */
export function MonoValue({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <span
      className={cn(
        'rounded-xs bg-gray-900 px-1 font-mono text-small break-all text-gray-400',
        className
      )}
    >
      {children}
    </span>
  );
}
