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

export default function LeftNav() {
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
    <div className="flex w-full flex-col items-center border-r border-gray-faded/30 bg-gray-700 px-4 pt-4">
      {/* <div className="w-full max-w-xs px-6 pb-6 mb-5 border-b border-gray-faded/30">
        <img src="/logo.svg" alt="logo" className="w-full" />
        <SystemStat
          name="client&nbsp;name"
          value={clientInfoLoading ? '...' : systemName}
        />
        <SystemStat name="cpu" value={clientInfoLoading ? '...' : cpu} />
        <SystemStat name="os" value={clientInfoLoading ? '...' : os} />
        <SystemStat
          name="uptime"
          value={clientInfoLoading ? '...' : formatDuration(uptime)}
        />
      </div> */}
      <div className="flex w-full grow flex-col">
        <div className="items-begin mb-4 flex w-full flex-row items-center justify-start gap-4">
          <h1 className="mr-1 text-large font-bold tracking-tight">
            Instances
          </h1>
          <Button
            label="Add"
            className="w-fit"
            icon={faPlus}
            disabled={!canCreateInstance}
            onClick={() => setShowCreateInstance(true)}
          />
        </div>
        <Dialog
          open={showCreateInstance}
          onClose={() => setShowCreateInstance(false)}
          className="relative z-10"
        >
          <div className="fixed inset-0 bg-gray-800/70" />
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
        <InstanceList />
      </div>
    </div>
  );
}
