import {
  faBell,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Popover } from '@headlessui/react';
import { NotificationContext } from 'data/NotificationContext';

import NotificationPanel from './NotificationPanel';
import { useContext, useEffect, useState } from 'react';
import clsx from 'clsx';

export const NotificationPopover = () => {
  const { notifications, ongoingNotifications } = useContext(NotificationContext);
  const [ newNotifications, setNewNotifications ] = useState<boolean>(false);

  useEffect(() => {
    setNewNotifications(notifications.length > 0 || ongoingNotifications.length > 0);
  }, [notifications, ongoingNotifications]);
    
  return (
    <Popover className="relative">
      <Popover.Button
        as={FontAwesomeIcon}
        icon={faBell}
        className={clsx(
          "w-4 select-none hover:cursor-pointer ui-open:text-gray-300",
          newNotifications ? "ui-not-open:text-white/75 ui-not-open:hover:text-white/90" :
           "ui-not-open:text-white/50 ui-not-open:hover:text-white/75")}
        onClick={() => newNotifications && setNewNotifications(false)}
      />
      <Popover.Panel className="absolute right-0 z-40 mt-1 h-[80vh] w-[480px] rounded-lg drop-shadow-lg child:h-full">
        <NotificationPanel className="rounded-lg border" />
      </Popover.Panel>
    </Popover>
  );
}