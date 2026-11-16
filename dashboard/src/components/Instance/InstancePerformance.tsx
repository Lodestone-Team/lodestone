import DashboardCard from 'components/DashboardCard';
import PerformanceGraph from 'components/Graphs/PerformanceGraph';
import { InstanceContext } from 'data/InstanceContext';
import { usePerformanceStream } from 'data/PerformanceStream';
import { useCoreInfo } from 'data/SystemInfo';
import { useContext } from 'react';
import { round } from 'utils/util';
import { useResolvedTheme } from 'utils/theme';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';
import { useI18n } from 'i18n';
import { Num } from 'components/Atoms/Values';
import { StatTile } from 'components/Values';

const bytesInGigabyte = 1024 * 1024 * 1024;

// the light theme uses a darker green, so the line stands out on white
const graphColors = {
  dark: { color: '#62DD76', backgroundColor: '#61AE3240' },
  light: { color: '#1D7A3A', backgroundColor: '#1D7A3A1F' },
};

export default function InstancePerformance() {
  const { t, formatNumber } = useI18n();
  const { selectedInstance: instance } = useContext(InstanceContext);
  if (!instance) throw new Error('No server selected');
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
  // no report yet from a running instance: the graph area is a placeholder
  // (a stopped instance may send none, so its empty graph shows as before)
  const waitingForData =
    counter < 0 &&
    (instance.state === 'Running' || instance.state === 'Starting');
  const oneDecimal = (value: number) =>
    formatNumber(value, { minimumFractionDigits: 1, maximumFractionDigits: 1 });

  return (
    <div>
      <h2 className="text-h2 font-extrabold tracking-medium">
        {t('server.performance.title')}
      </h2>
      <div className="mt-4 grid grid-cols-1 gap-10 @3xl:grid-cols-2">
        <div>
          <StatTile
            className="mb-2"
            label={t('values.key.cpuLoad')}
            value={
              <Num
                value={
                  latestSample?.cpu_usage != null
                    ? oneDecimal(latestSample.cpu_usage)
                    : formatNumber(0)
                }
                unit={t('server.performance.unitPercent')}
                minWidth="4.5rem"
                className="text-left"
              />
            }
          />

          <DashboardCard className="border-0 bg-gray-900 light:border light:bg-gray-800">
            {waitingForData ? (
              <SkeletonGroup label={t('server.performance.loadingGraph')}>
                <Skeleton className="aspect-2 w-full" />
              </SkeletonGroup>
            ) : (
              <PerformanceGraph
                title={t('server.performance.cpuUsage')}
                color={color}
                backgroundColor={backgroundColor}
                data={performanceBuffer.map((p) =>
                  p.cpu_usage !== null ? round(p.cpu_usage, 1) : NaN
                )}
                max={100}
                counter={counter}
                unit={t('server.performance.unitPercent')}
              />
            )}
          </DashboardCard>
        </div>
        <div>
          <StatTile
            className="mb-2"
            label={t('values.key.memoryUsed')}
            value={
              <Num
                value={oneDecimal(
                  Number(latestSample?.memory_usage ?? 0) / bytesInGigabyte
                )}
                unit={t('server.performance.unitGiB')}
                minWidth="5.5rem"
                className="text-left"
              />
            }
          />

          <DashboardCard className="border-0 bg-gray-900 light:border light:bg-gray-800">
            {waitingForData ? (
              <SkeletonGroup label={t('server.performance.loadingGraph')}>
                <Skeleton className="aspect-2 w-full" />
              </SkeletonGroup>
            ) : (
              <PerformanceGraph
                title={t('server.performance.memoryUsage')}
                color={color}
                backgroundColor={backgroundColor}
                data={performanceBuffer.map((p) =>
                  p.memory_usage !== null
                    ? round(Number(p.memory_usage) / bytesInGigabyte, 1)
                    : NaN
                )}
                max={round(total_ram / bytesInGigabyte, 1)}
                counter={counter}
                unit={t('server.performance.unitGiB')}
              />
            )}
          </DashboardCard>
        </div>
      </div>
    </div>
  );
}
