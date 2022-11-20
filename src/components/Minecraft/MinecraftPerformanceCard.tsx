import axios from 'axios';
import { InstanceInfo } from 'bindings/InstanceInfo';
import DashboardCard from 'components/DashboardCard';
import PerformanceGraph from 'components/Graphs/PerformanceGraph';
import { InstanceContext } from 'data/InstanceContext';
import { usePerformanceStream } from 'data/PerformanceStream';
import { useClientInfo } from 'data/SystemInfo';
import { useContext } from 'react';
import { round } from 'utils/util';

const bytesInGigabyte = 1073741824;

export default function MinecraftPerformanceCard() {
  const { selectedInstance: instance } = useContext(InstanceContext);
  if (!instance) throw new Error('No instance selected');
  const { buffer: performanceBuffer, latency_s } = usePerformanceStream(
    instance.uuid
  );
  const { data } = useClientInfo();
  const total_ram = data?.total_ram ?? 32;

  return (
    <DashboardCard>
      <h1 className="text-medium font-bold"> Performance </h1>
      <div className="mb-10 grid grid-cols-1 gap-10 @3xl:grid-cols-2">
        <div>
          <PerformanceGraph
            title="CPU Usage"
            color="#62DD76"
            backgroundColor="#61AE3240"
            data={performanceBuffer.map((p) =>
              p.cpu_usage !== null ? round(p.cpu_usage, 1) : NaN
            )}
            max={100}
            unit="%"
          />
        </div>
        <div>
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
            unit="GiB"
          />
        </div>
      </div>
    </DashboardCard>
  );
}
