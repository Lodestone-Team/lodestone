import InstanceList from './InstanceList';
import { Fragment, useContext, useState } from 'react';
import Button from 'components/Atoms/Button';
import { faSquarePlus } from '@fortawesome/free-solid-svg-icons';
import { Dialog, Transition } from '@headlessui/react';
import CreateInstanceFlow from 'components/Instance/InstanceCreateForm';
import { useUserAuthorized, useUserLoggedIn } from 'data/UserInfo';
import UserMenu from 'components/UserMenu';
import clsx from 'clsx';
import { SelectedInstanceInfo } from './SelectedInstanceInfo';
import { InstanceContext } from 'data/InstanceContext';
export default function LeftNav({ className }: { className?: string }) {
  const { showCreateInstance, setShowCreateInstance } =
    useContext(InstanceContext);
  const canCreateInstance = useUserAuthorized('can_create_instance');
  const userLoggedIn = useUserLoggedIn();
  return (
    <div
      className={`overflow-y-overlay flex w-full flex-col items-center overflow-y-auto px-2 ${className}`}
    >
      <div className="mt-10 flex h-full w-full grow flex-col ">
        <UserMenu />
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
          <Dialog onClose={() => setShowCreateInstance(false)} className="z-10">
            <div className="fixed inset-0 bg-gray-900/60" />
            <div className="fixed inset-0">
              <div className="overflow-y-overlay flex min-h-full items-center justify-center p-4 text-center">
                <Dialog.Panel>
                  <CreateInstanceFlow
                    onComplete={() => setShowCreateInstance(false)}
                  />
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
                      ? 'bg-gray-850 text-gray-300 outline outline-1 outline-fade-700/10'
                      : 'text-white/50',
                    'disabled:text-white/30',
                    'enabled:hover:bg-gray-800 enabled:focus-visible:outline-none',
                    'enabled:active:bg-gray-850 enabled:active:text-gray-300 enabled:active:outline enabled:active:outline-1 enabled:active:outline-fade-700/10'
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
    </div>
  );
}
