import { faArrowRight } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { useT } from 'i18n';
import { ReactNode } from 'react';
import { cn } from 'utils/util';

/** A change: the old value, an arrow (mirrored right to left), the new one */
export function DiffValue({
  from,
  to,
  className,
}: {
  from: ReactNode;
  to: ReactNode;
  className?: string;
}) {
  const t = useT();
  return (
    <span className={cn('inline-flex min-w-0 items-center gap-1.5', className)}>
      {from}
      <FontAwesomeIcon
        icon={faArrowRight}
        className="w-3 shrink-0 text-gray-500 rtl:-scale-x-100"
        aria-hidden="true"
      />
      <span className="sr-only">{t('values.diff.to')}</span>
      {to}
    </span>
  );
}
