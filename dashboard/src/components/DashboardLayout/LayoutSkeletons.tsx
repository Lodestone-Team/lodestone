import {
  Skeleton,
  SkeletonGroup,
  SkeletonRows,
} from 'components/Atoms/Skeleton';
import { useLocalStorage } from 'usehooks-ts';
import { cn } from 'utils/util';
import { isTauriMacOS } from 'utils/tauriUtil';

// Placeholders shaped like the dashboard's layout, shown while its code or data
// loads. They follow the real layout's sizes, so nothing jumps when it arrives.

/** The left navigation: a section label, the selected-instance card and tabs, the instance list */
export function SidebarSkeleton() {
  return (
    <div className="flex flex-col gap-6 px-3 pt-8">
      <div className="flex flex-col gap-2">
        <Skeleton className="h-3 w-28 rounded-sm" />
        <Skeleton className="h-16 w-full rounded-lg" />
        <SkeletonRows rows={5} icon="square" rowClassName="h-7 px-2" />
      </div>
      <div className="flex flex-col gap-2">
        <Skeleton className="h-3 w-24 rounded-sm" />
        <SkeletonRows rows={4} icon="square" rowClassName="h-7 px-2" />
      </div>
    </div>
  );
}

/**
 * A page of the main area: a title, a subtitle and a card of setting rows.
 * Used for instance tabs and settings pages before their content is known.
 */
export function PageSkeleton({
  rows = 5,
  className,
}: {
  rows?: number;
  className?: string;
}) {
  return (
    <div className={cn('flex w-full flex-col gap-2 pt-6', className)}>
      <Skeleton className="h-8 w-56 max-w-full" />
      <Skeleton className="h-4 w-80 max-w-full rounded-sm" />
      <div className="mt-6 flex flex-col divide-y divide-gray-faded/30 overflow-hidden rounded-lg border border-gray-faded/30">
        {Array.from({ length: rows }, (_, i) => (
          <div
            key={i}
            className="flex h-17 flex-row items-center justify-between gap-4 bg-gray-800 px-4"
          >
            <div className="flex grow flex-col gap-2">
              <Skeleton className="h-3.5 w-40 rounded-sm" />
              <Skeleton className="h-3 w-64 max-w-full rounded-sm" />
            </div>
            <Skeleton className="h-8 w-5/12 shrink-0" />
          </div>
        ))}
      </div>
    </div>
  );
}

/** The area under the top bar: the left navigation and a page */
export function DashboardBodySkeleton() {
  const [leftNavSize] = useLocalStorage('leftNavSize', 220);
  return (
    <SkeletonGroup className="flex min-h-0 w-full grow flex-row">
      <div
        className="h-full shrink-0 border-r border-fade-700 bg-gray-850"
        style={{ width: leftNavSize }}
      >
        <SidebarSkeleton />
      </div>
      <div className="mx-8 min-w-0 grow">
        <PageSkeleton className="mx-auto max-w-4xl" />
      </div>
    </SkeletonGroup>
  );
}

/**
 * The whole window, for the first load of the dashboard (its code is loading).
 * Only the dashboard's routes are loaded lazily, so this never stands in for a
 * login page.
 */
export function AppShellSkeleton() {
  const dragRegion = isTauriMacOS || undefined;
  return (
    <div className="flex h-screen flex-col bg-gray-875">
      <div
        data-tauri-drag-region={dragRegion}
        className="flex h-10 w-full shrink-0 items-center justify-center border-b border-gray-faded/30 bg-gray-800 light:bg-gray-850"
      >
        <SkeletonGroup>
          <Skeleton className="h-5 w-28" />
        </SkeletonGroup>
      </div>
      <DashboardBodySkeleton />
    </div>
  );
}

/** A loading page in the main area (appears after 200ms) */
export function PageLoading({ className }: { className?: string }) {
  return (
    <SkeletonGroup className={cn('mx-auto w-full max-w-4xl px-4', className)}>
      <PageSkeleton />
    </SkeletonGroup>
  );
}
