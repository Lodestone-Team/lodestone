import { ReactNode } from 'react';
import { cn } from 'utils/util';

export type FactRow = { label: string; value: ReactNode; id?: string };

/**
 * Facts in a dialog or panel: keys in a column as wide as the widest key
 * (6rem to 40%, wrapping before they truncate), values beside them, a line
 * between rows. `framed` puts it on a card.
 */
export function FactList({
  rows,
  framed = false,
  compact = false,
  className,
}: {
  rows: FactRow[];
  framed?: boolean;
  compact?: boolean;
  className?: string;
}) {
  return (
    <dl
      className={cn(
        'grid grid-cols-[minmax(6rem,min(max-content,40%))_minmax(0,1fr)] gap-x-4 text-medium',
        framed && 'rounded-lg border border-line bg-gray-800 px-3 py-1',
        className
      )}
    >
      {rows.map(({ label, value, id }, i) => {
        const last = i === rows.length - 1;
        const cell = cn(
          'flex items-center',
          compact ? 'min-h-6 py-0.5' : 'min-h-7 py-1',
          !last && 'border-b border-line'
        );
        return (
          <div key={id ?? label} className="contents">
            <dt
              className={cn(
                cell,
                'text-small font-medium wrap-break-word text-ink-muted'
              )}
            >
              {label}
            </dt>
            <dd
              className={cn(
                cell,
                'min-w-0 flex-wrap gap-x-1.5 text-gray-300 tabular-nums'
              )}
            >
              {value}
            </dd>
          </div>
        );
      })}
    </dl>
  );
}
