import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import clsx from 'clsx';
import { faCircle } from '@fortawesome/free-solid-svg-icons';
import { stateToColor } from 'utils/util';
import CircularProgress from './Atoms/CircularProgress';

export default function InstanceLoadingPill({
  progress_percent = 0,
}: {
  progress_percent?: number;
}) {
  const stateColor = stateToColor['Starting'];

  return (
    <button
      className={clsx(
        'flex flex-row items-center gap-x-1.5',
        'rounded-md py-1 px-2',
        'text-medium font-bold leading-5 tracking-medium',
        'text-white/50 ui-checked:text-gray-300',
        'ui-checked:bg-gray-800 ui-checked:outline ui-checked:outline-1 ui-checked:outline-fade-700 ui-not-checked:hover:bg-gray-800',
        'focus-visible:outline-none enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50'
      )}
    >
      <div className="h-4 w-4">
        <CircularProgress
          progress_percent={progress_percent}
        ></CircularProgress>
      </div>

      <p className="grow truncate text-left italic">
        Setting up... ({Math.round(progress_percent * 100)}%)
      </p>
      <FontAwesomeIcon
        icon={faCircle}
        className={`select-none ${stateColor} text-[8px]`}
      />
    </button>
  );
}
