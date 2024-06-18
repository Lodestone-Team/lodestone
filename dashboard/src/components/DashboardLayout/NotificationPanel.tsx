import { NotificationContext } from 'data/NotificationContext';
import { forwardRef, Fragment, useContext } from 'react';
import NotificationCard from 'components/Atoms/NotificationCard';
import { Transition } from '@headlessui/react';
import { useEffectOnce } from 'usehooks-ts';
import useAnalyticsEventTracker from 'utils/hooks';

const NotificationPanel = forwardRef(
  (
    {
      className = '',
    }: {
      className?: string;
    },
    ref: React.Ref<HTMLDivElement>
  ) => {
    const gaEventTracker = useAnalyticsEventTracker('Notification Panel');
    const { notifications, ongoingNotifications } =
      useContext(NotificationContext);
    useEffectOnce(() => {
      gaEventTracker('Notification Panel Opened');
    });
    return (
      <div
        ref={ref}
        className={`border-gray-faded/30 flex w-full flex-col border-l bg-gray-800 ${className}`}
      >
        <div className="text-h2 px-4 py-3 font-sans font-extrabold tracking-tight">
          Notifications
        </div>
        {ongoingNotifications.length > 0 && (
          <>
            <div className="border-gray-faded/30 text-medium border-y px-4 py-3 font-sans font-bold">
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
        <div className="border-gray-faded/30 text-medium border-y px-4 py-3 font-sans font-bold">
          All Notifications
        </div>
        <div className="grow basis-1 space-y-2 overflow-y-auto p-4">
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
              className={`justify-stretch flex flex-col items-start px-4 py-3 text-gray-300`}
            >
              <p className="text-h3 w-full">No notifications at the moment!</p>
            </div>
          )}
        </div>
      </div>
    );
  }
);

NotificationPanel.displayName = 'NotificationPanel';

export default NotificationPanel;
