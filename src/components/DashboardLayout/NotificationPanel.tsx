import ProgressBar from 'components/Atoms/ProgressBar';
import {
  NotificationContext,
  NotificationStatus,
  OngoingState,
} from 'data/NotificationContext';
import { useContext } from 'react';
import { GridLoader } from 'react-spinners';
import { formatNotificationTime } from 'utils/util';
import NotificationCard from './NotificationCard';
import NotificationItem from './NotificationCard';

const ongoingStateMapToNotificationType: Record<
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
    <div className="flex flex-col w-full bg-gray-800 border-l-2 border-gray-faded/30">
      <div className="p-4 font-sans font-black tracking-tight text-large">
        Notifications
      </div>
      <div className="px-4 py-3 font-sans font-bold border-y-2 border-gray-faded/30 text-smaller">
        In Progress
      </div>
      <div className="flex flex-col gap-2 p-4 overflow-y-auto">
        {ongoingNotifications.length > 0 ? (
          ongoingNotifications
            .map((notification) => (
              <NotificationCard
                key={notification.key}
                type={ongoingStateMapToNotificationType[notification.state]}
                title={notification.title}
                message={notification.message}
                progress={notification.progress}
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
      <div className="px-4 py-3 mt-12 font-sans font-bold border-y-2 border-gray-faded/30 text-smaller">
        Silent
      </div>
      <div className="flex flex-col gap-2 p-4 overflow-y-auto grow">
        {notifications.length > 0 ? (
          notifications
            .map((notification) => (
              <NotificationCard
                key={notification.key}
                type={notification.status}
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
