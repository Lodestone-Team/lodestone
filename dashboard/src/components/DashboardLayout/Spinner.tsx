import { cn } from 'utils/util';

/**
 * A loading spinner. It stays invisible for the first 200ms (see the
 * `loading-delayed` utility), so a load that is over quickly shows nothing.
 *
 * By default it fills and centers in its parent, so it only covers the part of
 * the page that is loading. `fullScreen` covers the whole window; use it only
 * when nothing else is on screen yet (the first load of the app).
 */
const Spinner = ({
  fullScreen = false,
  className,
}: {
  fullScreen?: boolean;
  className?: string;
}) => {
  return (
    <div
      role="status"
      aria-label="Loading"
      className={cn(
        'flex loading-delayed items-center justify-center',
        fullScreen ? 'fixed inset-0 z-50 bg-gray-900/75' : 'size-full min-h-32',
        className
      )}
    >
      <div
        className={cn(
          'animate-spin rounded-full border-blue-400 border-b-transparent',
          fullScreen ? 'size-24 border-4' : 'size-10 border-3'
        )}
      />
    </div>
  );
};

export default Spinner;
