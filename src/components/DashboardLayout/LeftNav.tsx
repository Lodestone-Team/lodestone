import SystemStat from './SystemStat';
import InstanceList from './InstanceList';
import { Fragment, useState } from 'react';
import useAnalyticsEventTracker, { useIntervalImmediate } from 'utils/hooks';
import { useCoreInfo } from 'data/SystemInfo';
import Button from 'components/Atoms/Button';
import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { Dialog, Transition } from '@headlessui/react';
import CreateInstanceFlow from 'components/Minecraft/MinecraftCreateForm';
import { useUserAuthorized } from 'data/UserInfo';

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
      <div className="flex h-full w-full grow flex-col">
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
        <InstanceList className="pt-12 pb-4">
          <div className="items-begin mb-4 flex w-full flex-row items-center justify-center gap-4">
            <Button
              label="Add"
              className="w-fit"
              variant="text"
              icon={faPlus}
              disabled={!canCreateInstance}
              onClick={() => setShowCreateInstance(true)}
            />
          </div>
        </InstanceList>
      </div>
    </div>
  );
}
