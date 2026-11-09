import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import clsx from 'clsx';
import { faCircle } from '@fortawesome/free-solid-svg-icons';
import { stateToColor } from 'utils/util';
import CircularProgress from './Atoms/CircularProgress';
import { useI18n } from 'i18n';

export default function InstanceLoadingPill({
  progress_percent = 0,
}: {
  progress_percent?: number;
}) {
  const { t, formatNumber } = useI18n();
  const stateColor = stateToColor['Starting'];
  progress_percent = Math.min(0.99, Math.max(0, progress_percent));

  return (
    <button
      className={clsx(
        'flex flex-row items-center gap-x-1.5',
        'rounded-md px-2 py-1',
        'text-medium leading-5 font-bold tracking-medium',
        'text-white/50 ui-checked:text-gray-300',
        'ui-checked:bg-gray-800 ui-checked:outline-1 ui-checked:outline-fade-700 ui-checked:outline-solid ui-not-checked:hover:bg-gray-800',
        'focus-visible:outline-hidden enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50'
      )}
    >
      <div className="h-4 w-4">
        <CircularProgress
          progress_percent={progress_percent}
        ></CircularProgress>
      </div>

      <p className="grow truncate text-left italic">
        {t('server.loading.settingUp', {
          percent: formatNumber(Math.round(progress_percent * 100) / 100, {
            style: 'percent',
          }),
        })}
      </p>
      <FontAwesomeIcon
        icon={faCircle}
        className={`select-none ${stateColor} text-[8px]`}
      />
    </button>
  );
}
