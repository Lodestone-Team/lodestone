import { faExpand } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { RadioGroup } from '@headlessui/react';
import InstanceLoadingPill from 'components/InstanceLoadingPill';
import InstancePill from 'components/InstancePill';
import { InstanceContext } from 'data/InstanceContext';
import { NotificationContext } from 'data/NotificationContext';
import { useUserState } from 'components/useUserState';
import { useContext, useEffect, useRef } from 'react';
import useAnalyticsEventTracker from 'utils/hooks';
import { match, otherwise } from 'variant';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { SkeletonGroup, SkeletonRows } from 'components/Atoms/Skeleton';
import { useT } from 'i18n';

export default function InstanceList({
  className = '',
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) {
  const t = useT();
  const gaEventTracker = useAnalyticsEventTracker('Instance List');
  const {
    instanceList: instances,
    selectedInstance,
    selectInstance,
    isReady,
  } = useContext(InstanceContext);
  const { ongoingNotifications } = useContext(NotificationContext);
  // not "logged out" while the user info loads, or the sign-in hint flashes
  const { userState } = useUserState();
  const {
    location: { pathname },
  } = useContext(BrowserLocationContext);

  // once per mount, when the list has loaded (not on every instance update)
  const viewTracked = useRef(false);
  useEffect(() => {
    if (!isReady || viewTracked.current) return;
    viewTracked.current = true;
    gaEventTracker(
      'View',
      'Instance List',
      true,
      Object.keys(instances).length
    );
  }, [isReady, instances]);

  useEffect(() => {
    if (pathname == '/') {
      selectInstance(null);
    }
  }, [pathname]);

  return (
    <RadioGroup
      className={`mx-1 flex min-h-0 flex-col gap-y-1 overflow-y-auto px-1 child:w-full ${className}`}
      value={selectedInstance}
      onChange={selectInstance}
    >
      <RadioGroup.Label className="text-small leading-snug font-bold text-gray-faded/30 uppercase light:text-ink-muted">
        {t('shell.nav.allInstances')}
      </RadioGroup.Label>
      {userState === 'loading' || (userState === 'logged-in' && !instances) ? (
        // instance pills (icon, name, status dot) while the list loads
        <SkeletonGroup label={t('shell.nav.loadingServers')}>
          <SkeletonRows
            rows={4}
            icon="square"
            trailing
            className="gap-y-1"
            rowClassName="h-7 px-2"
            trailingClassName="size-2 rounded-full"
          />
        </SkeletonGroup>
      ) : userState === 'logged-in' ? (
        instances &&
        Object.values(instances).map((instance) => (
          <RadioGroup.Option
            key={instance.uuid}
            value={instance}
            className="rounded-md outline-hidden child:w-full"
          >
            <InstancePill {...instance} />
          </RadioGroup.Option>
        ))
      ) : (
        <div
          className={`mt-2 flex w-fit flex-col items-stretch gap-4 rounded-xl border-2 border-dashed border-gray-faded/10 px-6 py-4 text-medium font-bold tracking-tight select-none`}
        >
          <FontAwesomeIcon
            icon={faExpand}
            className="text-h1 text-gray-faded/30"
          />
          <p className="text-center text-medium text-gray-faded/30 italic light:text-ink-muted">
            {t('shell.nav.logInToView')}
          </p>
        </div>
      )}
      {ongoingNotifications &&
        ongoingNotifications
          .map((notification) => {
            if (!notification.start_value) return null;
            if (notification.state === 'done') return null;
            return match(
              notification.start_value,
              otherwise(
                {
                  InstanceCreation: ({ instance_uuid }) => (
                    <InstanceLoadingPill
                      key={instance_uuid}
                      progress_percent={
                        notification.total
                          ? notification.progress / notification.total
                          : undefined
                      }
                    />
                  ),
                },
                (_) => null
              )
            );
          })
          .reverse()}
      {children}
    </RadioGroup>
  );
}
