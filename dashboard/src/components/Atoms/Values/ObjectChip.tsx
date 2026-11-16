import { ReactNode } from 'react';
import { cn } from 'utils/util';
import { KindIcon, ObjectKind } from './KindIcon';

/**
 * The object a toast, notification or fact is about: a kind icon and the
 * name, no fill. The name is cut at `max` with the full name in the title.
 * `mono` is for exact names (files); `href` makes it a link inside the app.
 */
export function ObjectChip({
  kind,
  icon,
  name,
  mono = false,
  max = '14rem',
  href,
  muted = false,
  className,
}: {
  kind?: ObjectKind;
  icon?: ReactNode;
  name: string;
  mono?: boolean;
  max?: string;
  href?: string;
  muted?: boolean;
  className?: string;
}) {
  const content = (
    <>
      {icon ?? (kind && <KindIcon kind={kind} />)}
      <span
        className={cn(
          'min-w-0 truncate',
          mono ? 'font-mono text-small' : 'font-mediumbold',
          muted ? 'text-gray-400' : 'text-gray-300'
        )}
        style={{ maxWidth: max }}
      >
        {name}
      </span>
    </>
  );
  const classes = cn(
    'inline-flex h-5 max-w-full min-w-0 items-center gap-1 align-middle',
    className
  );
  return href ? (
    <a
      href={href}
      className={cn(classes, 'hover:underline focus-visible:underline')}
      title={name}
    >
      {content}
    </a>
  ) : (
    <span className={classes} title={name}>
      {content}
    </span>
  );
}
