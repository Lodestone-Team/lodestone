import { ReactNode } from 'react';
import { cn } from 'utils/util';

/**
 * The single number a reading depends on: the value large (a Num, so the
 * unit is lighter), its label under it. `warning`/`danger` mark a shortfall.
 */
export function StatTile({
  label,
  value,
  tone = 'neutral',
  className,
}: {
  label: string;
  value: ReactNode;
  tone?: 'neutral' | 'warning' | 'danger';
  className?: string;
}) {
  return (
    <div className={cn('flex min-w-0 flex-col gap-0.5', className)}>
      <span
        className={cn(
          'w-fit text-h2 leading-tight font-mediumbold tabular-nums [&_.text-gray-400]:text-inherit',
          {
            neutral: 'text-gray-300',
            warning: 'rounded-sm bg-pill-yellow px-1 text-yellow',
            danger: 'rounded-sm bg-pill-red px-1 text-red-200',
          }[tone]
        )}
      >
        {value}
      </span>
      <span className="text-small font-medium text-ink-muted">{label}</span>
    </div>
  );
}

/** Two or three StatTiles side by side */
export function StatRow({
  children,
  className,
}: {
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={cn('flex flex-wrap items-start gap-6', className)}>
      {children}
    </div>
  );
}
