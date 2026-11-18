import { ReactNode } from 'react';
import { cn } from 'utils/util';

export type ListColumnWidth = 'size' | 'date';

const WIDTHS: Record<ListColumnWidth, string> = {
  size: 'w-18',
  date: 'w-28',
};

/**
 * The header of a list's right-aligned value columns (Size, Added), shown
 * once instead of a key on every row. Like the cells, it hides below the
 * list's @2xl width, where each row's FactLine shows the same facts. Put it
 * in the list's @container; `trailing` is the width of the rows' buttons.
 */
export function ListHeader({
  columns,
  trailing,
  className,
}: {
  columns: { label: string; width: ListColumnWidth }[];
  trailing?: string;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'hidden items-center gap-3 border-b border-line pb-1 text-small font-medium text-ink-muted @2xl:flex',
        className
      )}
      aria-hidden="true"
    >
      <span className="grow" />
      {columns.map(({ label, width }) => (
        <span key={label} className={cn(WIDTHS[width], 'shrink-0 text-right')}>
          {label}
        </span>
      ))}
      {trailing && <span className={cn(trailing, 'shrink-0')} />}
    </div>
  );
}

/** A right-aligned, tabular value under a ListHeader column */
export function ListCell({
  width,
  children,
  title,
  className,
}: {
  width: ListColumnWidth;
  children: ReactNode;
  title?: string;
  className?: string;
}) {
  return (
    <span
      className={cn(
        WIDTHS[width],
        'hidden shrink-0 truncate text-right text-small tabular-nums @2xl:block',
        width === 'date' ? 'text-ink-muted' : 'text-gray-400',
        className
      )}
      title={title}
    >
      {children}
    </span>
  );
}
