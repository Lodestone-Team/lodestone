import { NotificationContext } from 'data/NotificationContext';
import { forwardRef, Fragment, useContext } from 'react';
import NotificationCard from 'components/Atoms/NotificationCard';
import { Transition } from '@headlessui/react';
import useAnalyticsEventTracker, { useEffectOnce } from 'utils/hooks';
import {
  Skeleton,
  SkeletonGroup,
  SkeletonText,
} from 'components/Atoms/Skeleton';

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
    const { notifications, ongoingNotifications, historyLoaded } =
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
          {notifications.length === 0 && !historyLoaded ? (
            // notification cards while the past events load (after a moment)
            <SkeletonGroup label="Loading notifications" className="space-y-2">
              {Array.from({ length: 4 }, (_, i) => (
                <div
                  key={i}
                  className="flex flex-row items-center gap-3 rounded-md bg-gray-900 px-4 py-3"
                >
                  <Skeleton className="size-5 shrink-0 rounded-full" />
                  <SkeletonText lines={2} />
                </div>
              ))}
            </SkeletonGroup>
          ) : notifications.length > 0 ? (
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
