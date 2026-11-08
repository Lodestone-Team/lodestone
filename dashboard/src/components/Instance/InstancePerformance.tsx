import DashboardCard from 'components/DashboardCard';
import PerformanceGraph from 'components/Graphs/PerformanceGraph';
import { InstanceContext } from 'data/InstanceContext';
import { usePerformanceStream } from 'data/PerformanceStream';
import { useCoreInfo } from 'data/SystemInfo';
import { useContext } from 'react';
import { round } from 'utils/util';
import { useResolvedTheme } from 'utils/theme';

const bytesInGigabyte = 1024 * 1024 * 1024;

// the light theme uses a darker green, so the line stands out on white
const graphColors = {
  dark: { color: '#62DD76', backgroundColor: '#61AE3240' },
  light: { color: '#2E9A49', backgroundColor: '#3DAE5E33' },
};

export default function InstancePerformance() {
  const { selectedInstance: instance } = useContext(InstanceContext);
  if (!instance) throw new Error('No instance selected');
  const {
    buffer: performanceBuffer,
    counter,
    latency_s,
  } = usePerformanceStream(instance.uuid);
  const { data } = useCoreInfo();
  const total_ram = data?.total_ram ?? 32;
  // the buffer is oldest-first, so the newest sample is the last one
  const latestSample = performanceBuffer.at(-1);
  const { color, backgroundColor } = graphColors[useResolvedTheme()];

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
            CPU LOAD: {latestSample?.cpu_usage?.toFixed(1) ?? 0}%
          </h3>

          <DashboardCard className="border-0 bg-gray-900">
            <PerformanceGraph
              title="CPU Usage"
              color={color}
              backgroundColor={backgroundColor}
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
              Number(latestSample?.memory_usage ?? 0) / bytesInGigabyte
            ).toFixed(1)}{' '}
            GiB
          </h3>

          <DashboardCard className="border-0 bg-gray-900">
            <PerformanceGraph
              title="Memory Usage"
              color={color}
              backgroundColor={backgroundColor}
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
