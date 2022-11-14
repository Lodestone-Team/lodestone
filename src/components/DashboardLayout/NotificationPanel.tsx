import ProgressBar from 'components/Atoms/ProgressBar';
import {
  NotificationContext,
  NotificationStatus,
  OngoingState,
} from 'data/NotificationContext';
import { useContext } from 'react';
import { GridLoader } from 'react-spinners';
import { formatNotificationTime } from 'utils/util';
import NotificationCard from 'components/Atoms/NotificationCard';

const ongoingStateMapToNotificationStatus: Record<
  OngoingState,
  NotificationStatus
> = {
  ongoing: 'info',
  done: 'success',
  error: 'error',
};

export default function NotificationPanel() {
  const { notifications, ongoingNotifications } =
    useContext(NotificationContext);
  return (
    <div className="flex w-full flex-col border-l-2 border-gray-faded/30 bg-gray-800">
      <div className="p-4 font-sans text-large font-black tracking-tight">
        Notifications
      </div>
      <div className="border-y-2 border-gray-faded/30 px-4 py-3 font-sans text-smaller font-bold">
        In Progress
      </div>
      <div className="max-h-[50%] shrink-0 space-y-4 overflow-y-auto p-4 pb-12">
        {ongoingNotifications.length > 0 ? (
          ongoingNotifications
            .map((notification) => (
              <NotificationCard
                key={notification.key}
                status={ongoingStateMapToNotificationStatus[notification.state]}
                title={notification.title}
                message={notification.message ?? 'Loading...'}
                progress_percent={
                  notification.total
                    ? notification.progress / notification.total
                    : undefined
                }
                timestamp={notification.timestamp}
              />
            ))
            .reverse()
        ) : (
          <div
            className={`justify-stretch flex flex-col items-start px-4 py-3 text-white`}
          >
            <p className="w-full text-base">
              No tasks in progress at the moment!
            </p>
          </div>
        )}
      </div>
      <div className="border-y-2 border-gray-faded/30 px-4 py-3 font-sans text-smaller font-bold">
        Silent
      </div>
      <div className="grow space-y-4 overflow-y-auto p-4">
        {notifications.length > 0 ? (
          notifications
            .map((notification) => (
              <NotificationCard
                key={notification.key}
                status={notification.status}
                title={notification.message}
                timestamp={notification.timestamp}
              />
            ))
            .reverse()
        ) : (
          <div
            className={`justify-stretch flex flex-col items-start px-4 py-3 text-white`}
          >
            <p className="w-full text-base">No notifications at the moment!</p>
          </div>
        )}
      </div>
    </div>
  );
}
