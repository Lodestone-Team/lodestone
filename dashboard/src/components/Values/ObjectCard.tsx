import { VersionTag } from 'components/Atoms/Values';
import { ReactNode } from 'react';
import { cn } from 'utils/util';

/**
 * The object of a dialog, under its title: its icon, its name (and version),
 * a few facts and a status badge. The title names only the action, so the
 * name appears here once. `compact` is for confirmations that don't destroy
 * anything.
 */
export function ObjectCard({
  icon,
  name,
  version,
  facts,
  badge,
  compact = false,
  className,
}: {
  icon?: ReactNode;
  name: string;
  version?: string | null;
  facts?: ReactNode;
  badge?: ReactNode;
  compact?: boolean;
  className?: string;
}) {
  return (
    <div
      className={cn(
        'flex min-w-0 items-center gap-3 rounded-lg border border-line bg-gray-800 text-left',
        compact ? 'px-2.5 py-2' : 'px-3 py-2.5',
        className
      )}
    >
      {icon && (
        <div
          className={cn(
            'flex shrink-0 items-center justify-center overflow-hidden rounded-md text-gray-500',
            compact ? 'size-6' : 'size-8'
          )}
        >
          {icon}
        </div>
      )}
      <div className="flex min-w-0 grow flex-col gap-0.5">
        <div className="flex min-w-0 items-center gap-1.5">
          <span
            className="min-w-0 truncate text-medium font-mediumbold text-gray-300"
            title={name}
          >
            {name}
          </span>
          {version && <VersionTag version={version} />}
        </div>
        {facts}
      </div>
      {badge && <div className="shrink-0">{badge}</div>}
    </div>
  );
}
