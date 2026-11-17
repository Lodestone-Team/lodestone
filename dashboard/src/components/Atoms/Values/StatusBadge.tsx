import { IconDefinition } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { InstanceState } from 'bindings/InstanceState';
import { ReactNode } from 'react';
import { cn } from 'utils/util';

export type StatusTone = 'info' | 'success' | 'warning' | 'danger' | 'neutral';

/**
 * A state (Running, Expired, Update available) as a small pill. Colour means
 * status and nothing else, so tags that aren't a state use Label instead.
 */
export function StatusBadge({
  tone,
  icon,
  children,
  className,
}: {
  tone: StatusTone;
  icon?: IconDefinition;
  children: ReactNode;
  className?: string;
}) {
  return (
    <span
      className={cn(
        'inline-flex h-4.5 shrink-0 items-center gap-1 rounded-full px-1.5 text-caption leading-none font-bold whitespace-nowrap select-none',
        {
          info: 'bg-pill-blue text-blue-150',
          success: 'bg-pill-green text-green',
          warning: 'bg-pill-yellow text-yellow',
          danger: 'bg-pill-red text-red-200',
          neutral: 'bg-gray-700 text-gray-400',
        }[tone],
        className
      )}
    >
      {icon && (
        <FontAwesomeIcon icon={icon} className="w-2.5" aria-hidden="true" />
      )}
      {children}
    </span>
  );
}

/** The badge tone of a server's state */
export const stateTone = (state: InstanceState): StatusTone =>
  (
    ({
      Running: 'success',
      Starting: 'warning',
      Stopping: 'warning',
      Stopped: 'neutral',
      Error: 'danger',
    }) as const
  )[state] ?? 'neutral';
