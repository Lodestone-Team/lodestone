import {
  IconDefinition,
  faBell,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Popover } from '@headlessui/react';
import { NotificationContext, NotificationItem } from 'data/NotificationContext';

import NotificationPanel from './NotificationPanel';
import { forwardRef, useContext, useEffect, useState } from 'react';
import clsx from 'clsx';

const IconWithBadge = forwardRef((
  { 
    icon, 
    onClick,
    className,
  } : {
    icon: IconDefinition,
    onClick: () => void,
    className: string,
  }, 
  ref: React.Ref<HTMLDivElement>) => {

  return (
    <div ref={ref}>
      <FontAwesomeIcon icon={icon} className={className} onClick={onClick} />
      <div className="absolute top-[3px] right-[0px] h-2 w-2 rounded-full bg-red" />
    </div>
  )
});

IconWithBadge.displayName = 'IconWithBadge';

export const NotificationPopover = () => {
  const { notifications, ongoingNotifications } = useContext(NotificationContext);
  const [ newNotifications, setNewNotifications ] = useState<boolean>(false);
  const [ previousNotifications, setPreviousNotifications ] = useState<NotificationItem[]>([]);

  const shouldShowPopup = () => {
    return newNotifications || 
      ongoingNotifications.length > 0 || 
      previousNotifications.map(n => n.fresh && !(notifications.includes(n))).length > 0;
  };

  useEffect(() => {
    console.log(notifications, ongoingNotifications)
    setPreviousNotifications(notifications);
    setNewNotifications(shouldShowPopup());
  }, [notifications, ongoingNotifications]);
    

  return (
    <Popover className="relative">
      <Popover.Button
        as={newNotifications ? IconWithBadge : FontAwesomeIcon}
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