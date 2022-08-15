import SystemStat from "./SystemStat"
import Image from "next/image"
import InstanceList from "./InstanceList"
import { useState } from "react";
import { useInterval } from "usehooks-ts";

export default function LeftNav() {
  const [systemName, setSystemName] = useState<string>("PLACEHOLDER");
  const [systemCpu, setSystemCpu] = useState<string>("PLACEHOLDER");
  const [systemOs, setSystemOs] = useState<string>("PLACEHOLDER");
  const [systemStartTime, setSystemStartTime] = useState<Date>(new Date());
  const [systemUptime, setSystemUptime] = useState<string>("");

  useInterval(() => {
    // calculate system uptime in DD:HH:MM:SS format
    const uptime = Math.floor(
      (new Date().getTime() - systemStartTime.getTime()) / 1000
    );
    const days = Math.floor(uptime / 86400);
    const hours = Math.floor((uptime % 86400) / 3600);
    const minutes = Math.floor((uptime % 3600) / 60);
    const seconds = Math.floor(uptime % 60);
    setSystemUptime(`${days}:${hours}:${minutes}:${seconds}`);
  }, 1000);

  return (
    <div className="flex flex-col items-center w-2/12 h-full px-8 pt-10 border-r bg-dark-background-accent border-fade">
      <div className="w-full px-6 mb-5">
        {/* eslint-disable-next-line @next/next/no-img-element */}
        <img src="/logo.svg" alt="logo" className="w-full" />
        <SystemStat name="system name" value={systemName} />
        <SystemStat name="cpu" value={systemCpu} />
        <SystemStat name="os" value={systemOs} />
        <SystemStat name="uptime" value={systemUptime} />
      </div>
      <InstanceList />
    </div>
  )
}
