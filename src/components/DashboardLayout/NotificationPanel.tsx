// NotificationPanel

import { LodestoneContext } from 'data/LodestoneContext';
import { useContext } from 'react';
import { formatNotificationTime } from 'utils/util';

export default function NotificationPanel() {
  const { notifications } = useContext(LodestoneContext);

  return (
    <div className="flex flex-col w-full bg-gray-800 border-l-2 border-gray-faded/30">
      <div className="p-4 font-sans font-black tracking-tight border-b-2 border-gray-faded/30 text-large">
        Notifications
      </div>
      {/* <div className="px-4 py-3 font-sans font-bold border-b-2 border-gray-faded/30 text-smaller">
        Silent
      </div> */}
      <div className="overflow-y-auto">
        {notifications.length > 0 ? notifications
          .map((notification) => (
            <div
              key={notification.key}
              className={`justify-stretch flex flex-col items-start px-4 py-3 text-white hover:bg-gray-900`}
            >
              <span className="whitespace-nowrap text-smaller text-white/50">
                {formatNotificationTime(Number(notification.timestamp))}
              </span>
              <p className="w-full text-base">{notification.message}</p>
            </div>
          ))
          .reverse() : (
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
