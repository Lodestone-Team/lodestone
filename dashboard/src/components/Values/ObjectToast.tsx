import { ReactNode } from 'react';

/**
 * A toast that leads with its object: the object chip (and its status) on
 * line 1, the outcome as a short phrase under it, then the reason (an error
 * from the core, shown as it is) and any list or counts.
 */
export function ObjectToast({
  object,
  status,
  outcome,
  reason,
  children,
}: {
  object?: ReactNode;
  status?: ReactNode;
  outcome: string;
  reason?: string | null;
  children?: ReactNode;
}) {
  return (
    <div className="flex min-w-0 flex-col gap-0.5">
      {(object || status) && (
        <div className="flex min-w-0 items-center gap-2 text-medium">
          {object}
          {status}
        </div>
      )}
      <span
        className={
          object || status
            ? 'text-small font-medium text-ink-muted'
            : 'text-medium font-mediumbold text-gray-300'
        }
      >
        {outcome}
      </span>
      {reason && (
        <span className="text-small wrap-break-word whitespace-pre-line text-gray-400">
          {reason}
        </span>
      )}
      {children && <div className="mt-1 min-w-0">{children}</div>}
    </div>
  );
}
