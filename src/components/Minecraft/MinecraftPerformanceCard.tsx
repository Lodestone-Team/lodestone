import axios from 'axios';
import DashboardCard from 'components/DashboardCard';
import PerformanceGraph from 'components/Graphs/PerformanceGraph';
import { round } from 'utils/util';

type CpuUsageReply = {
  cpu_speed: number;
  cpu_load: number;
};

type RamUsageReply = {
  total: number;
  free: number;
};

const bytesInGigabyte = 1073741824;

const getCpuUsage = async (): Promise<[number, number]> => {
  return await axios.get<CpuUsageReply>('/system/cpu').then((res) => {
    return [round(res.data.cpu_load, 1), 100];
  });
};

const getRamUsage = async (): Promise<[number, number]> => {
  return await axios.get<RamUsageReply>('/system/ram').then((res) => {
    return [
      round((res.data.total - res.data.free) / bytesInGigabyte, 1),
      round(res.data.total / bytesInGigabyte, 1),
    ];
  });
};

export default function MinecraftPerformanceCard() {
  return (
    <DashboardCard>
      <h1 className="font-bold text-medium"> Performance </h1>
      <div className="flex flex-row gap-10 mb-10">
        <div className="w-1/2 h-80">
          <PerformanceGraph
            title="CPU"
            color="#62DD76"
            backgroundColor="#61AE3240"
            getter={getCpuUsage}
            unit="%"
          />
        </div>
        <div className="w-1/2 h-80">
          <PerformanceGraph
            title="Memory"
            color="#62DD76"
            backgroundColor="#61AE3240"
            getter={getRamUsage}
            unit="GB"
          />
        </div>
      </div>
    </DashboardCard>
  );
}
