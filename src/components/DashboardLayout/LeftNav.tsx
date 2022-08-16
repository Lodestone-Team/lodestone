import SystemStat from './SystemStat';
import InstanceList from './InstanceList';
import { useState } from 'react';
import { useIntervalImmediate } from 'utils/hooks';

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
  const [systemName, setSystemName] = useState<string>('PLACEHOLDER');
  const [systemCpu, setSystemCpu] = useState<string>('PLACEHOLDER');
  const [systemOs, setSystemOs] = useState<string>('PLACEHOLDER');
  const [systemStartTime, setSystemStartTime] = useState<Date>(new Date());
  const [systemUptime, setSystemUptime] = useState<string>('00:00:00:00');

  useIntervalImmediate(() => {
    // calculate system uptime in DD:HH:MM:SS format
    const uptime = Math.floor(
      (new Date().getTime() - systemStartTime.getTime()) / 1000
    );

    setSystemUptime(formatDuration(uptime));
  }, 1000);

  return (
    <div className="flex flex-col items-center w-2/12 h-full px-8 pt-10 bg-gray-700 border-r border-gray-500">
      <div className="w-full px-6 mb-5">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img src="/logo.svg" alt="logo" className="w-full" />
        <SystemStat name="system name" value={systemName} />
        <SystemStat name="cpu" value={systemCpu} />
        <SystemStat name="os" value={systemOs} />
        <SystemStat name="uptime" value={systemUptime} />
      </div>
      <div className="flex flex-col w-full grow">
        <h1 className="font-bold text-medium">Server Instances</h1>
        <InstanceList />
      </div>
    </div>
  );
}
