import DashboardCard from 'components/DashboardCard';
import PerformanceGraph from 'components/Graphs/PerformanceGraph';
import { InstanceContext } from 'data/InstanceContext';
import { usePerformanceStream } from 'data/PerformanceStream';
import { useCoreInfo } from 'data/SystemInfo';
import { useContext } from 'react';
import { round } from 'utils/util';

const bytesInGigabyte = 1073741824;

export default function InstancePerformanceCard() {
  const { selectedInstance: instance } = useContext(InstanceContext);
  if (!instance) throw new Error('No instance selected');
  const {
    buffer: performanceBuffer,
    counter,
    latency_s,
  } = usePerformanceStream(instance.uuid);
  const { data } = useCoreInfo();
  const total_ram = data?.total_ram ?? 32;

  return (
    <div>
      <h2 className="text-h2 font-extrabold tracking-medium">
        Instance Monitors
      </h2>
      <h3 className="text-h3 font-medium italic tracking-medium text-white/50">
        Instance’s usage of your Lodestone Core
      </h3>
      <div className="mt-4 grid grid-cols-1 gap-10 @3xl:grid-cols-2">
        <div>
          <h3 className="mb-2 text-small font-mediumbold text-gray-faded/50">
            CPU LOAD: {performanceBuffer[0]?.cpu_usage?.toFixed(1) ?? 0}%
          </h3>

          <DashboardCard className="border-0 bg-gray-900">
            <PerformanceGraph
              title="CPU Usage"
              color="#62DD76"
              backgroundColor="#61AE3240"
              data={performanceBuffer.map((p) =>
                p.cpu_usage !== null ? round(p.cpu_usage, 1) : NaN
              )}
              max={100}
              counter={counter}
              unit="%"
            />
          </DashboardCard>
        </div>
        <div>
          <h3 className="mb-2 text-small font-mediumbold text-gray-faded/50">
            MEMORY USED:{' '}
            {(
              Number(performanceBuffer[0]?.memory_usage ?? 0) / bytesInGigabyte
            ).toFixed(1)}{' '}
            GiB
          </h3>

          <DashboardCard className="border-0 bg-gray-900">
            <PerformanceGraph
              title="Memory Usage"
              color="#62DD76"
              backgroundColor="#61AE3240"
              data={performanceBuffer.map((p) =>
                p.memory_usage !== null
                  ? round(Number(p.memory_usage) / bytesInGigabyte, 1)
                  : NaN
              )}
              max={round(total_ram / bytesInGigabyte, 1)}
              counter={counter}
              unit="GiB"
            />
          </DashboardCard>
        </div>
      </div>
    </div>
  );
}
