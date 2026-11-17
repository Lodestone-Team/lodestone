import { cn } from 'utils/util';

/**
 * A 4px meter (the progress bar's look) for a value out of a maximum, such as
 * memory used. `marker` draws a tick at another value (what is needed).
 */
export function MeterBar({
  value,
  max,
  tone = 'info',
  marker,
  label,
  className,
}: {
  value: number;
  max: number;
  tone?: 'info' | 'warning' | 'danger';
  marker?: number;
  /** what is measured, for screen readers */
  label: string;
  className?: string;
}) {
  const fraction = (v: number) =>
    max > 0 ? Math.min(1, Math.max(0, v / max)) : 0;
  return (
    <div
      role="meter"
      aria-label={label}
      aria-valuenow={value}
      aria-valuemin={0}
      aria-valuemax={max}
      className={cn('relative h-1 rounded-full bg-gray-700', className)}
    >
      <div
        className={cn(
          'h-full rounded-full transition-width duration-100 ease-in',
          { info: 'bg-blue', warning: 'bg-yellow', danger: 'bg-red' }[tone]
        )}
        style={{ width: `${fraction(value) * 100}%` }}
      />
      {marker !== undefined && (
        <div
          className="absolute -top-0.5 h-2 w-0.5 rounded-full bg-gray-300"
          style={{ left: `calc(${fraction(marker) * 100}% - 1px)` }}
        />
      )}
    </div>
  );
}
