import { faExpand } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { RadioGroup } from '@headlessui/react';
import InstanceLoadingPill from 'components/InstanceLoadingPill';
import InstancePill from 'components/InstancePill';
import { InstanceContext } from 'data/InstanceContext';
import { NotificationContext } from 'data/NotificationContext';
import { useUserLoggedIn } from 'data/UserInfo';
import { useContext, useEffect } from 'react';
import useAnalyticsEventTracker from 'utils/hooks';
import { match, otherwise } from 'variant';
import { BrowserLocationContext } from 'data/BrowserLocationContext';

export default function IntegrationsList({
  className = '',
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) {
  const gaEventTracker = useAnalyticsEventTracker('Integrations List');
  const { ongoingNotifications } = useContext(NotificationContext);
  const userLoggedIn = useUserLoggedIn();
  const {
    location: { pathname },
  } = useContext(BrowserLocationContext);

  return (
    <RadioGroup
      className={`mx-1 flex min-h-0 flex-col gap-y-1 overflow-y-auto px-1 child:w-full ${className}`}
      value="test"
    >
      {userLoggedIn ? (
          <>
            <RadioGroup.Label className="text-small font-bold leading-snug text-gray-faded/30">
              INTEGRATIONS
            </RadioGroup.Label>
          </>
        ) : (
          <></>
        )
      }
    </RadioGroup>
  );
}
