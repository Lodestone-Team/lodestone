import clsx from 'clsx';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { lazyWithPreload } from 'utils/lazyWithPreload';
import { useT } from 'i18n';

// the create form (formik, yup, setup manifests) loads when it is first
// opened, or ahead of that when a button that opens it is hovered or focused
export const CreateInstanceFlow = lazyWithPreload(
  () => import('components/Instance/InstanceCreateForm')
);

/** Props that load the create form when the user gets close to opening it. */
export const preloadCreateInstanceProps = {
  onPointerEnter: () => void CreateInstanceFlow.preload().catch(() => {}),
  onFocus: () => void CreateInstanceFlow.preload().catch(() => {}),
};

/** The create dialog's frame, while its code loads (same size, no spinner). */
export const CreateInstanceFlowSkeleton = ({
  modpack,
}: {
  modpack: boolean;
}) => {
  const t = useT();
  return (
    <div
      aria-busy="true"
      className={clsx(
        'flex w-203 rounded-2xl border-2 border-gray-faded/10 bg-gray-850 drop-shadow-lg',
        modpack ? 'h-175 max-h-[calc(100vh-2rem)]' : 'min-h-140'
      )}
    >
      <div className="w-45 shrink-0 border-r border-gray-700" />
      {/* the steps and the first step's cards, appearing after a moment */}
      <SkeletonGroup
        label={t('create.loadingForm')}
        className="-ml-45 flex grow flex-row"
      >
        <div className="flex w-45 shrink-0 flex-col gap-4 px-6 pt-8">
          {Array.from({ length: modpack ? 3 : 4 }, (_, i) => (
            <Skeleton key={i} className="h-4 w-24 rounded-sm" />
          ))}
        </div>
        <div className="flex grow flex-col gap-6 p-8">
          <Skeleton className="h-6 w-56" />
          <Skeleton className="h-4 w-80 max-w-full rounded-sm" />
          <div className="grid grid-cols-2 gap-9 pt-3">
            <Skeleton className="col-span-2 h-14" />
            {Array.from({ length: 4 }, (_, i) => (
              <Skeleton key={i} className="h-32" />
            ))}
          </div>
        </div>
      </SkeletonGroup>
    </div>
  );
};
