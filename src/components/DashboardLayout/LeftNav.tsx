import SystemStat from './SystemStat';
import InstanceList from './InstanceList';
import { Fragment, useState } from 'react';
import useAnalyticsEventTracker, { useIntervalImmediate } from 'utils/hooks';
import { useCoreInfo } from 'data/SystemInfo';
import Button from 'components/Atoms/Button';
import { faSquarePlus } from '@fortawesome/free-solid-svg-icons';
import { Dialog, Transition } from '@headlessui/react';
import CreateInstanceFlow from 'components/Minecraft/MinecraftCreateForm';
import { useUserAuthorized } from 'data/UserInfo';
import UserMenu from 'components/Atoms/UserMenu';
import clsx from 'clsx';
import { InstanceNestedBarStates } from './InstanceNestedBarStates';
export default function LeftNav({ className }: { className?: string }) {
  const { data: clientInfo, isLoading: clientInfoLoading } = useCoreInfo();
  const [showCreateInstance, setShowCreateInstance] = useState(false);
  const canCreateInstance = useUserAuthorized('can_create_instance');

  const systemName = clientInfoLoading ? '...' : clientInfo?.core_name;
  const cpu = clientInfoLoading ? '...' : clientInfo?.cpu;
  const os = clientInfoLoading ? '...' : clientInfo?.os;
  const up_since = clientInfoLoading ? 0 : clientInfo?.up_since;

  const [uptime, setUptime] = useState(0);
  useIntervalImmediate(() => {
    setUptime(up_since ? Date.now() / 1000 - up_since : 0);
  }, 1000);

  return (
    <div className={`flex w-full flex-col items-center px-4 ${className}`}>
      <div className="mt-12 flex h-full w-full grow flex-col ">
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
          <Dialog
            onClose={() => setShowCreateInstance(false)}
            className="relative z-10"
          >
            <div className="fixed inset-0 bg-gray-900/60" />
            <div className="fixed inset-0 overflow-y-auto">
              <div className="flex min-h-full items-center justify-center p-4 text-center">
                <Dialog.Panel>
                  <CreateInstanceFlow
                    onComplete={() => setShowCreateInstance(false)}
                  />
                </Dialog.Panel>
              </div>
            </div>
          </Dialog>
        </Transition>
        <div className="overflow-y-auto">
          <InstanceNestedBarStates></InstanceNestedBarStates>
          <InstanceList className="mt-6">
            <div className="items-begin mb-4 flex w-full flex-row items-center justify-center gap-4">
              <Button
                label="New instance..."
                className={
                  'w-full text-medium text-gray-faded/30 hover:bg-gray-800 focus-visible:outline-none active:bg-gray-850 active:text-gray-300 active:outline active:outline-1 active:outline-fade-700/10' +
                  clsx(
                    showCreateInstance &&
                      'bg-gray-850 text-gray-300 outline outline-1 outline-fade-700/10 '
                  )
                }
                intention="none"
                variant="text"
                icon={faSquarePlus}
                disabled={!canCreateInstance}
                onClick={() => setShowCreateInstance(true)}
              />
            </div>
          </InstanceList>
        </div>
      </div>
    </div>
  );
}
