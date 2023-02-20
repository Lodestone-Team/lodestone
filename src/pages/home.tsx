import axios from 'axios';
import DashboardCard from 'components/DashboardCard';
import PerformanceGraph from 'components/Graphs/PerformanceGraph';
import { useDocumentTitle } from 'usehooks-ts';
import { round } from 'utils/util';
import { useUserInfo } from 'data/UserInfo';
import { LodestoneContext } from 'data/LodestoneContext';
import { useContext } from 'react';

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
  const { token } = useContext(LodestoneContext);
  const { isLoading, isError, data: user } = useUserInfo();

  useDocumentTitle('Home - Lodestone');
  return (
    // used to possibly center the content
    <div className="relative flex h-full w-full flex-row justify-center overflow-y-scroll px-4 pt-8 pb-10 @container">
      {/* main content container */}
      <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
        <h1 className="font-title text-2xlarge font-bold tracking-tight text-gray-300">
          {`Welcome, ${
            token && !isLoading && !isError && user
              ? `${user.username}`
              : 'Guest'
          }!`}
        </h1>
        <h3 className="mb-4 text-h3 font-medium italic tracking-medium text-white/50">
          {' '}
          Select or create an instance to continue.
        </h3>
        <DashboardCard>
          <div className="my-8 grid grid-cols-2 gap-10">
            <div>
              <PerformanceGraph
                title="CPU load"
                color="#62DD76"
                backgroundColor="#61AE3240"
                getter={getCpuUsage}
                unit="%"
              />
            </div>
            <div>
              <PerformanceGraph
                title="Memory load"
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
