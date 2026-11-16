import { useI18n } from 'i18n';
import { cn } from 'utils/util';

/**
 * A count after a heading, tab or button label, instead of "(7)". Large
 * counts are compact (12.3K); the exact number is in the title and in the
 * text screen readers get.
 */
export function CountBadge({
  count,
  active = false,
  plus = false,
  title,
  className,
}: {
  count: number;
  active?: boolean;
  /** "+3": how many more there are than shown */
  plus?: boolean;
  /** replaces the exact count as the title (e.g. the names not shown) */
  title?: string;
  className?: string;
}) {
  const { formatCompact, formatNumber } = useI18n();
  const sign = plus ? '+' : '';
  const compact = sign + formatCompact(count);
  const exact = sign + formatNumber(count);
  return (
    <span
      className={cn(
        'inline-flex h-4 min-w-4 shrink-0 items-center justify-center rounded-full px-1.25 text-caption leading-none font-bold tabular-nums',
        active ? 'bg-pill-blue text-blue-200' : 'bg-gray-700 text-gray-400',
        className
      )}
      title={title ?? (compact === exact ? undefined : exact)}
    >
      {compact === exact ? (
        exact
      ) : (
        <>
          <span aria-hidden="true">{compact}</span>
          <span className="sr-only">{exact}</span>
        </>
      )}
    </span>
  );
}
