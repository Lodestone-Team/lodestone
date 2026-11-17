import { ReactNode } from 'react';
import { cn } from 'utils/util';

/**
 * The facts under a page or card title: each key over its value, 24px
 * apart with a thin divider, wrapping (never truncating) when space runs out.
 */
export function FactStrip({
  items,
  className,
}: {
  items: { label: string; value: ReactNode }[];
  className?: string;
}) {
  return (
    <dl className={cn('flex flex-wrap gap-x-6 gap-y-2', className)}>
      {items.map(({ label, value }) => (
        <div
          key={label}
          className="relative flex min-w-0 flex-col gap-0.5 not-first:before:absolute not-first:before:top-1/2 not-first:before:-left-3 not-first:before:h-4 not-first:before:w-px not-first:before:-translate-y-1/2 not-first:before:bg-line"
        >
          <dt className="text-small font-medium text-ink-muted">{label}</dt>
          <dd className="flex min-w-0 items-center gap-1.5 text-medium font-mediumbold text-gray-300 tabular-nums">
            {value}
          </dd>
        </div>
      ))}
    </dl>
  );
}
