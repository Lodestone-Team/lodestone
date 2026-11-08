import { cn } from 'utils/util';

/**
 * A placeholder block with a shimmer (see the `skeleton` utility), sized and
 * shaped by `className` to match the content it stands in for, so nothing moves
 * when the content arrives. Use it inside a SkeletonGroup.
 */
export function Skeleton({ className }: { className?: string }) {
  // a span, so it is also valid inside buttons and text
  return (
    <span aria-hidden className={cn('block skeleton rounded-md', className)} />
  );
}

/** Placeholder lines of text: the last one is shorter, like a paragraph */
export function SkeletonText({
  lines = 1,
  className,
  lineClassName = 'h-3.5',
}: {
  lines?: number;
  className?: string;
  lineClassName?: string;
}) {
  return (
    <div className={cn('flex w-full flex-col gap-2', className)}>
      {Array.from({ length: lines }, (_, i) => (
        <Skeleton
          key={i}
          className={cn(
            'rounded-sm',
            lineClassName,
            i === lines - 1 && lines > 1 ? 'w-3/5' : 'w-full'
          )}
        />
      ))}
    </div>
  );
}

// Line widths that look like varied text, used in turn by the shapes below
const lineWidths = ['w-3/5', 'w-2/5', 'w-1/2', 'w-3/4', 'w-1/3', 'w-2/3'];

/**
 * List rows: an optional icon (square or round) and a line of text of varying
 * width, optionally with a second, smaller line and a short value at the end.
 */
export function SkeletonRows({
  rows,
  icon = 'none',
  secondLine = false,
  trailing = false,
  className,
  rowClassName,
  trailingClassName,
}: {
  rows: number;
  icon?: 'none' | 'square' | 'round' | 'large';
  secondLine?: boolean;
  trailing?: boolean;
  className?: string;
  rowClassName?: string;
  /** sizes the value at the end, a short line by default */
  trailingClassName?: string;
}) {
  return (
    <div className={cn('flex flex-col', className)}>
      {Array.from({ length: rows }, (_, i) => (
        <div
          key={i}
          className={cn('flex flex-row items-center gap-3', rowClassName)}
        >
          {icon !== 'none' && (
            <Skeleton
              className={cn(
                'shrink-0',
                icon === 'large' ? 'size-10' : 'size-4',
                icon === 'round' ? 'rounded-full' : 'rounded-sm'
              )}
            />
          )}
          <div className="flex min-w-0 grow flex-col gap-1.5">
            <Skeleton
              className={cn(
                'h-3.5 rounded-sm',
                lineWidths[i % lineWidths.length]
              )}
            />
            {secondLine && (
              <Skeleton
                className={cn(
                  'h-3 rounded-sm',
                  lineWidths[(i + 3) % lineWidths.length]
                )}
              />
            )}
          </div>
          {trailing && (
            <Skeleton
              className={cn(
                'h-3.5 w-12 shrink-0 rounded-sm',
                trailingClassName
              )}
            />
          )}
        </div>
      ))}
    </div>
  );
}

/** Lines of code or log output, indented now and then */
export function SkeletonCode({
  lines = 14,
  className,
}: {
  lines?: number;
  className?: string;
}) {
  const indents = ['', '', 'ml-6', 'ml-6', 'ml-12', 'ml-6', ''];
  return (
    <div className={cn('flex flex-col gap-2.5', className)}>
      {Array.from({ length: lines }, (_, i) => (
        <Skeleton
          key={i}
          className={cn(
            'h-3 rounded-sm',
            indents[i % indents.length],
            lineWidths[(i * 2) % lineWidths.length]
          )}
        />
      ))}
    </div>
  );
}

/**
 * Wraps placeholders so they appear only after 200ms: a load that finishes
 * sooner shows nothing instead of flashing the skeleton.
 */
export function SkeletonGroup({
  className,
  children,
  label = 'Loading',
}: {
  className?: string;
  children: React.ReactNode;
  label?: string;
}) {
  return (
    <div
      className={cn('loading-delayed', className)}
      role="status"
      aria-busy="true"
      aria-label={label}
    >
      {children}
    </div>
  );
}
