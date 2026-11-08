import { cn } from 'utils/util';

/**
 * A quiet placeholder block, sized by `className` to match the content it
 * stands in for, so nothing moves when the content arrives.
 */
export function Skeleton({ className }: { className?: string }) {
  return (
    // gray-800 is white in the light theme, the colour of the cards and of the
    // side panels around them, so light placeholders use the hover gray
    <div
      className={cn(
        'animate-pulse rounded-md bg-gray-800 light:bg-gray-700',
        className
      )}
    />
  );
}

/**
 * Wraps placeholders so they appear only after 200ms: a load that finishes
 * sooner shows nothing instead of flashing the skeleton.
 */
export function SkeletonGroup({
  className,
  children,
}: {
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div className={cn('loading-delayed', className)} aria-busy="true">
      {children}
    </div>
  );
}
