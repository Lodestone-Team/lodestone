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

// format duration in seconds to DD:HH:MM:SS
const formatDuration = (duration: number) => {
  const days = Math.floor(duration / 86400);
  const hours = Math.floor((duration % 86400) / 3600);
  const minutes = Math.floor((duration % 3600) / 60);
  const seconds = Math.floor(duration % 60);
  return `${days < 10 ? '0' + days : days}:${
    hours < 10 ? '0' + hours : hours
  }:${minutes < 10 ? '0' + minutes : minutes}:${
    seconds < 10 ? '0' + seconds : seconds
  }`;
};

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
    <div className="flex flex-col items-center w-full px-4 pt-4 overflow-x-visible bg-gray-700 border-r border-gray-faded/30">
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
      <div className="flex flex-col w-full overflow-x-visible grow">
        <div className="flex flex-row items-center justify-start w-full gap-4 mb-4 items-begin">
          <h1 className="mr-1 font-bold tracking-tight text-large">
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
            <div className="flex items-center justify-center min-h-full p-4 text-center">
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
