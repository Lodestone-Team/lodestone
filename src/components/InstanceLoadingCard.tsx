import Button from './Atoms/Button';
import Label from './Atoms/Label';
import GameIcon from './Atoms/GameIcon';
import LoadingStatusIcon from './Atoms/LoadingStatusIcon';
import { EventLevel } from 'bindings/EventLevel';
import { OngoingState } from 'data/NotificationContext';
import ProgressBar from './Atoms/ProgressBar';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import clsx from 'clsx';
import { faCircle } from '@fortawesome/free-solid-svg-icons';
import { stateToColor } from 'utils/util';
import CircularProgress from './CircularProgress';

const NotificationLevelToBgColorClass = (
  level: EventLevel,
  state?: OngoingState
) => {
  switch (level) {
    case 'Warning':
      return 'bg-yellow';
    case 'Error':
      return 'bg-red';
    default:
      switch (state) {
        case 'done':
          return 'bg-green';
        case 'error':
          return 'bg-red';
        case 'ongoing':
        default:
          return 'bg-blue-200/50';
      }
  }
};

export default function InstanceLoadingCard({
  progress_percent = 0,
}: {
  level: EventLevel;
  state: OngoingState;
  focus?: boolean;
  progress_percent?: number;
}) {
  const stateColor = stateToColor['Starting'];

  return (
    // TODO
    // - increase stroke width
    // - change hardcoded 14px to rem (note: circluar progress bar fills the container it's in, read docs)
    // - test it more

    <button
      className={clsx(
        'flex flex-row items-center gap-x-1.5',
        'cursor-pointer rounded-md py-1 px-2',
        'text-medium font-bold leading-5 tracking-medium',
        'text-white/50 ui-checked:text-gray-300',
        'ui-checked:bg-gray-800 ui-checked:outline ui-checked:outline-1 ui-checked:outline-fade-700 ui-not-checked:hover:bg-gray-800',
        'focus-visible:outline-none enabled:focus-visible:ring-4 enabled:focus-visible:ring-blue-faded/50'
      )}
    >
      <div className="h-[14px] w-[14px]">
        <CircularProgress
          progress_percent={progress_percent}
        ></CircularProgress>
      </div>

      <p className="grow truncate text-left italic">Setting up...</p>
      <FontAwesomeIcon
        icon={faCircle}
        className={`select-none ${stateColor} text-[8px]`}
      />
    </button>
  );
}
