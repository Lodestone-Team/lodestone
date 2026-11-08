import clsx from 'clsx';
import { lazyWithPreload } from 'utils/lazyWithPreload';

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
}) => (
  <div
    aria-busy="true"
    className={clsx(
      'flex w-203 rounded-2xl border-2 border-gray-faded/10 bg-gray-850 drop-shadow-lg',
      modpack ? 'h-175 max-h-[calc(100vh-2rem)]' : 'min-h-140'
    )}
  >
    <div className="w-45 shrink-0 border-r border-gray-700" />
  </div>
);
