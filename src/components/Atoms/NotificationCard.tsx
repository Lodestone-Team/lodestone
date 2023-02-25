import ProgressBar from 'components/Atoms/ProgressBar';
import { formatNotificationTime } from 'utils/util';
import LoadingStatusIcon from './LoadingStatusIcon';
import { EventLevel } from 'bindings/EventLevel';
import { OngoingState } from 'data/NotificationContext';

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
        case 'ongoing':
          return 'bg-gray-500';
        case 'done':
          return 'bg-green';
        case 'error':
          return 'bg-red';
        default:
          return 'bg-gray-500';
      }
  }
};

export default function NotificationCard({
  level,
  state,
  title = '',
  message = '',
  progress_percent = 0,
  timestamp,
}: {
  level: EventLevel;
  state?: OngoingState;
  title?: string;
  message?: string;
  progress_percent?: number; // progress in percentage
  timestamp: number;
}) {
  progress_percent = Math.min(0.99, progress_percent); // Prevents the progress bar from being full when the instance is still loading

  return (
    <div className="overflow-hidden rounded-md bg-gray-900">
      <div
        className={`flex flex-row items-center justify-start gap-3 px-4 pt-3 pb-2.5 text-gray-300 hover:bg-gray-900`}
      >
        <LoadingStatusIcon level={level} state={state} />
        <div className="flex flex-col items-start">
          <p className="w-full text-medium font-bold tracking-medium">
            {title}
          </p>
          <p className="w-full text-medium font-medium tracking-medium">
            {message}
          </p>
          {!message && (
            <span className="whitespace-nowrap text-medium font-medium tracking-medium text-white/50">
              {formatNotificationTime(timestamp)}
            </span>
          )}
        </div>
      </div>
      {state && state !== 'done' ? (
        <ProgressBar
          progress_percent={progress_percent}
          colorClass={NotificationLevelToBgColorClass(level, state)}
        />
      ) : (
        <div className="h-1"></div>
      )}
    </div>
  );
}
