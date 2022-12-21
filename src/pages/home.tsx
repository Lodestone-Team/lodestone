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

const Home = () => {
  return (
    // used to possibly center the content
    <div className="relative flex h-full w-full flex-row justify-center overflow-y-scroll px-4 pt-8 pb-10 @container">
      {/* main content container */}
      <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
        <h1 className="font-heading text-2xlarge font-semibold tracking-tight text-gray-300">
          Home
        </h1>
        <p>Display some general information here maybe</p>
        <DashboardCard>
          <h1 className="text-medium font-bold"> Performance </h1>
          <div className="mb-10 grid grid-cols-2 gap-10">
            <div>
              <PerformanceGraph
                title="CPU Usage"
                color="#62DD76"
                backgroundColor="#61AE3240"
                getter={getCpuUsage}
                unit="%"
              />
            </div>
            <div>
              <PerformanceGraph
                title="Memory Usage"
                color="#62DD76"
                backgroundColor="#61AE3240"
                getter={getRamUsage}
                unit="GiB"
              />
            </div>
          </div>
        </DashboardCard>
      </div>
    </div>
  );
};

export default Home;
