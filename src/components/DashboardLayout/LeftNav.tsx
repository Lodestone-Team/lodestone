import SystemStat from './SystemStat';
import InstanceList from './InstanceList';
import { useState } from 'react';
import { useIntervalImmediate } from 'utils/hooks';
import { useClientInfo } from 'data/SystemInfo';
import Button from 'components/Atoms/Button';
import { faPlus } from '@fortawesome/free-solid-svg-icons';
import { Dialog } from '@headlessui/react';
import CreateInstanceFlow from 'components/Minecraft/MinecraftCreateForm';
import { useUserAuthorized } from 'data/UserInfo';

export default function LeftNav({ className }: { className?: string }) {
  const { data: clientInfo, isLoading: clientInfoLoading } = useClientInfo();
  const [showCreateInstance, setShowCreateInstance] = useState(false);
  const canCreateInstance = useUserAuthorized('can_create_instance');

  const systemName = clientInfoLoading ? '...' : clientInfo?.client_name;
  const cpu = clientInfoLoading ? '...' : clientInfo?.cpu;
  const os = clientInfoLoading ? '...' : clientInfo?.os;
  const up_since = clientInfoLoading ? 0 : clientInfo?.up_since;

  const [uptime, setUptime] = useState(0);
  useIntervalImmediate(() => {
    setUptime(up_since ? Date.now() / 1000 - up_since : 0);
  }, 1000);

  return (
    <div className={`flex w-full flex-col items-center px-4 ${className}`}>
      <div className="flex w-full grow flex-col">
        <Dialog
          open={showCreateInstance}
          onClose={() => setShowCreateInstance(false)}
          className="relative z-10"
        >
          <div className="fixed inset-0 bg-[#000]/80" />
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
        <InstanceList className="pt-8">
          <div className="items-begin mb-4 flex w-full flex-row items-center justify-center gap-4">
            {/* <h1 className="mr-1 text-large font-bold tracking-tight">
              Instances
            </h1> */}
            <Button
              label="Add"
              className="w-fit"
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
