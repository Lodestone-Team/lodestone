import ProgressBar from 'components/Atoms/ProgressBar';
import { NotificationContext } from 'data/NotificationContext';
import { useContext } from 'react';
import { GridLoader } from 'react-spinners';
import { formatNotificationTime } from 'utils/util';

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
      <div className="overflow-y-auto">
        {ongoingNotifications.length > 0 ? (
          ongoingNotifications
            .map((notification) => (
              <div key={notification.key}>
                {notification.progress ? (
                  <ProgressBar progress={notification.progress} />
                ) : (
                  <div className="h-1"></div>
                )}
                <div
                  className={`justify-stretch flex flex-row items-center justify-between px-4 py-3 text-white hover:bg-gray-900`}
                >
                  <div className="flex flex-col items-start">
                    <span className="whitespace-nowrap text-smaller text-white/50">
                      {formatNotificationTime(Number(notification.timestamp))}
                      {/* TODO: make error and success messages look different */}
                    </span>
                    <p className="w-full text-base">{notification.title}</p>
                    <p className="w-full text-smaller">{notification.message}</p>
                  </div>
                  {/* TODO: fix loading animation flickering on resize */}
                  <GridLoader size={5} margin={1} color="#E3E3E4" key={notification.key} />
                </div>
              </div>
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
      <div className="overflow-y-auto">
        {notifications.length > 0 ? (
          notifications
            .map((notification) => (
              <div
                key={notification.key}
                className={`justify-stretch flex flex-col items-start px-4 py-3 text-white hover:bg-gray-900`}
              >
                <span className="whitespace-nowrap text-smaller text-white/50">
                  {formatNotificationTime(Number(notification.timestamp))}
                  {/* TODO: make error and success messages look different */}
                </span>
                <p className="w-full text-base">{notification.message}</p>
              </div>
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
