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
import { useT } from 'i18n';

const NotificationPanel = forwardRef(
  (
    {
      className = '',
    }: {
      className?: string;
    },
    ref: React.Ref<HTMLDivElement>
  ) => {
    const t = useT();
    const gaEventTracker = useAnalyticsEventTracker('Notification Panel');
    const { notifications, ongoingNotifications, historyLoaded } =
      useContext(NotificationContext);
    useEffectOnce(() => {
      gaEventTracker('Notification Panel Opened');
    });
    return (
      <div
        ref={ref}
        className={`flex w-full flex-col border-l border-gray-faded/30 bg-gray-800 ${className}`}
      >
        <div className="px-4 py-3 font-sans text-h2 font-extrabold tracking-tight">
          {t('shell.notifications.title')}
        </div>
        {ongoingNotifications.length > 0 && (
          <>
            <div className="border-y border-gray-faded/30 px-4 py-3 font-sans text-medium font-bold">
              {t('shell.notifications.inProgress')}
            </div>
            <div className="space-y-4 overflow-y-auto p-4">
              {ongoingNotifications
                .map((notification) => (
                  <NotificationCard
                    key={notification.key}
                    level={notification.level}
                    state={notification.state}
                    title={notification.title}
                    message={notification.message ?? t('common.loading')}
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
        <div className="border-y border-gray-faded/30 px-4 py-3 font-sans text-medium font-bold">
          {t('shell.notifications.all')}
        </div>
        <div className="grow basis-1 space-y-2 overflow-y-auto p-4">
          {notifications.length === 0 && !historyLoaded ? (
            // notification cards while the past events load (after a moment)
            <SkeletonGroup
              label={t('shell.notifications.loading')}
              className="space-y-2"
            >
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
              className={`flex flex-col items-start justify-stretch px-4 py-3 text-gray-300`}
            >
              <p className="w-full text-h3">{t('shell.notifications.empty')}</p>
            </div>
          )}
        </div>
      </div>
    );
  }
);

NotificationPanel.displayName = 'NotificationPanel';

export default NotificationPanel;
