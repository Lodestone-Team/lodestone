import InstanceList from './InstanceList';
import { Fragment, Suspense, useContext } from 'react';
import Button from 'components/Atoms/Button';
import { faSquarePlus } from '@fortawesome/free-solid-svg-icons';
import { Dialog, Transition } from '@headlessui/react';
import { ModalBackdrop, ModalPanel } from 'components/Atoms/Modal';
import {
  CreateInstanceFlow,
  CreateInstanceFlowSkeleton,
  preloadCreateInstanceProps,
} from './CreateInstanceFlow';
import { useUserAuthorized, useUserLoggedIn } from 'data/UserInfo';
import clsx from 'clsx';
import { SelectedInstanceInfo } from './SelectedInstanceInfo';
import { InstanceContext } from 'data/InstanceContext';
import IntegrationsList from './IntegrationsList';
import SidebarFooter from './SidebarFooter';

export default function LeftNav({ className }: { className?: string }) {
  const {
    showCreateInstance,
    setShowCreateInstance,
    modpackPreset,
    setModpackPreset,
    isReady,
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
        <Transition appear show={showCreateInstance} as={Fragment}>
          <Dialog onClose={closeCreateInstance} className="z-10">
            <ModalBackdrop />
            <div className="fixed inset-0">
              <div className="overflow-y-overlay flex min-h-full items-center justify-center p-4 text-center">
                <ModalPanel>
                  <Suspense
                    fallback={
                      <CreateInstanceFlowSkeleton modpack={!!modpackPreset} />
                    }
                  >
                    <CreateInstanceFlow
                      onComplete={closeCreateInstance}
                      modpackPreset={modpackPreset}
                    />
                  </Suspense>
                </ModalPanel>
              </div>
            </div>
          </Dialog>
        </Transition>

        <div className="h-full">
          <SelectedInstanceInfo />

          <InstanceList className="mt-6">
            {/* after the list has loaded, so the instances don't push it down */}
            {userLoggedIn && isReady && (
              <div
                className="flex w-full flex-row items-center justify-center gap-4 pb-8"
                {...preloadCreateInstanceProps}
              >
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
