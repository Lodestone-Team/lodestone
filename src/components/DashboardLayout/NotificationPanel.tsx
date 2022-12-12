import { NotificationContext } from 'data/NotificationContext';
import { useContext } from 'react';
import NotificationCard from 'components/Atoms/NotificationCard';
import { useUserAuthorized } from 'data/UserInfo';

export default function NotificationPanel({
  className = '',
}: {
  className?: string;
}) {
  const { notifications, ongoingNotifications } =
    useContext(NotificationContext);
  return (
    <div
      className={`flex w-full flex-col border-l border-gray-faded/30 bg-gray-800 ${className}`}
    >
      <div className="p-4 font-sans text-large font-black tracking-tight">
        Notifications
      </div>
      {ongoingNotifications.length > 0 && (
        <>
          <div className="border-y border-gray-faded/30 px-4 py-3 font-sans text-smaller font-bold">
            In progress
          </div>
          <div className="space-y-4 overflow-y-auto p-4">
            {ongoingNotifications
              .map((notification) => (
                <NotificationCard
                  key={notification.key}
                  level={notification.level}
                  state={notification.state}
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
              .reverse()}
          </div>
        </>
      )}
      <div className="border-y border-gray-faded/30 px-4 py-3 font-sans text-smaller font-bold">
        Notifications
      </div>
      <div className="grow basis-1 space-y-4 overflow-y-auto p-4">
        {notifications.length > 0 ? (
          notifications
            .map((notification) => (
              <NotificationCard
                key={notification.key}
                level={notification.level}
                title={notification.title}
                message={notification.message}
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
