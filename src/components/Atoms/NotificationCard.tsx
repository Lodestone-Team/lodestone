import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faCircleCheck,
  faCircleXmark,
  faCircleNotch,
} from '@fortawesome/free-solid-svg-icons';
import ProgressBar from 'components/Atoms/ProgressBar';
import { NotificationStatus } from 'data/NotificationContext';
import { formatNotificationTime } from 'utils/util';
import StatusIcon from './StatusIcon';

const NotificationStatusToBgColorClass: Record<NotificationStatus, string> = {
  info: 'bg-gray-500',
  success: 'bg-green',
  error: 'bg-red',
};

const NotificationStatusToFgColorClass: Record<NotificationStatus, string> = {
  info: 'text-gray-500',
  success: 'text-green',
  error: 'text-red',
};

export default function NotificationCard({
  status,
  title = '',
  message = '',
  progress_percent,
  timestamp,
}: {
  status: NotificationStatus;
  title?: string;
  message?: string;
  progress_percent?: number; // progress in percentage
  timestamp: number;
}) {
  return (
    <div className="overflow-hidden rounded-md bg-gray-900">
      <div
        className={`flex flex-row items-center justify-start gap-3 px-4 pt-3 pb-2.5 text-white hover:bg-gray-900`}
      >
        {!!progress_percent && (
          <StatusIcon status={status} />
        )}
        <div className="flex flex-col items-start">
          <p className="w-full text-base font-bold tracking-medium">{title}</p>
          <p className="w-full text-small font-medium tracking-medium">
            {message}
          </p>
          {!message && (
            <span className="whitespace-nowrap text-smaller font-medium tracking-medium text-white/50">
              {formatNotificationTime(timestamp)}
            </span>
          )}
        </div>
      </div>
      {progress_percent && status !== 'success' ? (
        <ProgressBar
          progress_percent={progress_percent}
          colorClass={NotificationStatusToBgColorClass[status]}
        />
      ) : (
        <div className="h-1"></div>
      )}
    </div>
  );
}
