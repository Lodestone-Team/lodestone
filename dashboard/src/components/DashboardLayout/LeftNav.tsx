import InstanceList from './InstanceList';
import { Fragment, lazy, Suspense, useContext } from 'react';
import Button from 'components/Atoms/Button';
import { faSquarePlus } from '@fortawesome/free-solid-svg-icons';
import { Dialog, Transition } from '@headlessui/react';
import Spinner from './Spinner';
import { useUserAuthorized, useUserLoggedIn } from 'data/UserInfo';
import clsx from 'clsx';
import { SelectedInstanceInfo } from './SelectedInstanceInfo';
import { InstanceContext } from 'data/InstanceContext';
import IntegrationsList from './IntegrationsList';
import SidebarFooter from './SidebarFooter';

// the create form (formik, yup, setup manifests) loads when it is first opened
const CreateInstanceFlow = lazy(
  () => import('components/Instance/InstanceCreateForm')
);

export default function LeftNav({ className }: { className?: string }) {
  const {
    showCreateInstance,
    setShowCreateInstance,
    modpackPreset,
    setModpackPreset,
  } = useContext(InstanceContext);
  const closeCreateInstance = () => {
    setShowCreateInstance(false);
    setModpackPreset(null);
  };
  const canCreateInstance = useUserAuthorized('can_create_instance');
  const userLoggedIn = useUserLoggedIn();
  return (
    <div
      // overflow-y-auto is the fallback for browsers without overflow: overlay
      // eslint-disable-next-line tailwindcss/no-contradicting-classname
      className={`overflow-y-overlay flex w-full flex-col items-center overflow-y-auto px-2 ${className}`}
    >
      <div className="mt-8 flex w-full grow flex-col">
        <Transition
          appear
          show={showCreateInstance}
          as={Fragment}
          enter="ease-out duration-200"
          enterFrom="opacity-0"
          enterTo="opacity-100"
          leave="ease-in duration-150"
          leaveFrom="opacity-100"
          leaveTo="opacity-0"
        >
          <Dialog onClose={closeCreateInstance} className="z-10">
            <div className="fixed inset-0 bg-scrim" />
            <div className="fixed inset-0">
              <div className="overflow-y-overlay flex min-h-full items-center justify-center p-4 text-center">
                <Dialog.Panel>
                  <Suspense fallback={<Spinner />}>
                    <CreateInstanceFlow
                      onComplete={closeCreateInstance}
                      modpackPreset={modpackPreset}
                    />
                  </Suspense>
                </Dialog.Panel>
              </div>
            </div>
          </Dialog>
        </Transition>

        <div className="h-full">
          <SelectedInstanceInfo />

          <InstanceList className="mt-6">
            {userLoggedIn && (
              <div className="flex w-full flex-row items-center justify-center gap-4 pb-8">
                <Button
                  label={canCreateInstance ? "New instance..." : "No permission"}
                  className={clsx(
                    'w-full text-medium font-medium tracking-normal',
                    showCreateInstance
                      ? 'bg-gray-850 text-gray-300 outline-solid outline-1 outline-fade-700/10'
                      : 'text-white/50',
                    'disabled:text-white/30',
                    'enabled:hover:bg-gray-800 enabled:focus-visible:outline-hidden',
                    'enabled:active:bg-gray-850 enabled:active:text-gray-300 enabled:active:outline-solid enabled:active:outline-1 enabled:active:outline-fade-700/10'
                  )}
                  intention="none"
                  variant="text"
                  icon={faSquarePlus}
                  disabled={!canCreateInstance}
                  onClick={() => setShowCreateInstance(true)}
                  align="start"
                />
              </div>
            )}
          </InstanceList>
        </div>
      </div>
      <SidebarFooter />
    </div>
  );
}
