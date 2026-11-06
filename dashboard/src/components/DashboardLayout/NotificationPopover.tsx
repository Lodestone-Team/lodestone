import {
  IconDefinition,
  faBell,
} from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { Popover } from '@headlessui/react';
import { NotificationContext } from 'data/NotificationContext';

import NotificationPanel from './NotificationPanel';
import { forwardRef, useContext, useEffect, useRef, useState } from 'react';
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
  // keys of the notifications seen in the previous update
  const seenKeys = useRef<Set<string>>(new Set());

  // show the badge for an ongoing task, or for a live (not replayed) notification
  // that wasn't there before; it stays until the bell is clicked
  useEffect(() => {
    const hasNewFresh = notifications.some(
      (n) => n.fresh && !seenKeys.current.has(n.key)
    );
    seenKeys.current = new Set(notifications.map((n) => n.key));
    if (hasNewFresh || ongoingNotifications.length > 0)
      setNewNotifications(true);
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
