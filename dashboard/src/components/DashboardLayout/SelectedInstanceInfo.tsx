import { useMemo } from 'react';
import { RadioGroup } from '@headlessui/react';
import { InstanceContext } from 'data/InstanceContext';
import { useContext } from 'react';
import clsx from 'clsx';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import InstanceCard from 'components/InstanceCard';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faExpand } from '@fortawesome/free-solid-svg-icons';
import { tabs } from 'pages/InstanceTabs/tabList';
import { useQueryClient } from '@tanstack/react-query';
import { GlobalSettingsData } from 'bindings/GlobalSettingsData';
import { useGlobalSettings } from 'data/GlobalSettings';
import { useCodeExecution } from 'data/CodeExecution';
import { isUserAuthorized, useUserInfo } from 'data/UserInfo';
import { getModSupport, modTabTitle } from 'utils/mods';
import { Skeleton, SkeletonGroup } from 'components/Atoms/Skeleton';

export const SelectedInstanceInfo = ({
  className = '',
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) => {
  // the "Instance List" view event is sent by InstanceList, once per mount
  const { selectedInstance, isReady } = useContext(InstanceContext);

  const { setPathname, searchParams } = useContext(BrowserLocationContext);
  const setActive = useMemo(() => location.pathname.split('/')[2], [location.pathname]);
  const { data: globalSettings } = useGlobalSettings();
  // macros are owner only
  const { canManage } = useCodeExecution();
  // the Mods tab: only for instances that support mods or plugins
  const modSupport = selectedInstance
    ? getModSupport(selectedInstance.game_type)
    : null;
  const { data: user } = useUserInfo();
  const canReadFiles =
    !!selectedInstance &&
    isUserAuthorized(user, 'can_read_instance_file', selectedInstance.uuid);

  const visibleTabs = tabs.filter(
    (tab) =>
      (tab.title !== 'Playitgg' || globalSettings?.playit_enabled) &&
      (tab.path !== 'macros' || canManage) &&
      (tab.path !== 'mods' || (modSupport && canReadFiles))
  );

  const uuid = selectedInstance?.uuid;
  if (
    (!selectedInstance || !uuid) &&
    !isReady &&
    searchParams.get('instance')
  ) {
    // The URL names an instance but the list is still loading: hold its place
    // instead of flashing the "no instance selected" box.
    return (
      <SkeletonGroup className="mx-1 flex flex-col gap-y-1 px-1 pb-1">
        <div className="text-small leading-snug font-bold text-gray-faded/30">
          SELECTED INSTANCE
        </div>
        <div className="my-2 flex flex-col gap-3 border-b-2 border-gray-faded/30 pb-2">
          <Skeleton className="h-7 w-full" />
          <Skeleton className="h-6 w-full" />
        </div>
        {visibleTabs.map((tab) => (
          <Skeleton key={tab.path} className="h-7 w-full bg-gray-800/60" />
        ))}
      </SkeletonGroup>
    );
  }
  if (!selectedInstance || !uuid) {
    return (
      <div className="mx-1 px-1 text-gray-faded/30">
        <div className="text-small font-bold leading-snug text-gray-faded/30">
          SELECTED INSTANCE
        </div>
        <div className="mt-2 flex h-[17.625rem] justify-center rounded-md border border-dashed text-center text-gray-faded/30">
          <div className="mt-20 w-[5.5rem]">
            <div className="text-h1">
              <FontAwesomeIcon icon={faExpand} />
            </div>
            <div className="mt-2 text-medium font-bold">
              Any selected instance will appear here
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <RadioGroup
      className={`child:w-full mx-1 flex min-h-0 flex-col gap-y-1 overflow-x-hidden px-1 pb-1 ${className}`}
      onChange={setPathname}
    >
      <RadioGroup.Label className="text-small font-bold leading-snug text-gray-faded/30">
        SELECTED INSTANCE
      </RadioGroup.Label>

      <InstanceCard {...selectedInstance} key={selectedInstance.uuid} />

      {selectedInstance &&
        visibleTabs.map((tab) => (
          <RadioGroup.Option
            key={tab.path}
            value={`/dashboard/${tab.path}`}
            className="rounded-md outline-hidden focus-visible:bg-gray-800 child:w-full"
          >
            <button
              className={clsx(
                'flex flex-row items-center gap-x-1.5',
                'cursor-pointer rounded-md px-2 py-1',
                'text-medium font-medium leading-5 tracking-normal',
                'hover:bg-gray-800',
                'focus-visible:outline-hidden focus-visible:ring-4 focus-visible:ring-blue-faded/50',
                setActive === tab.path
                  ? 'bg-gray-800 outline-solid outline-1 outline-fade-700'
                  : ''
              )}
              onClick={() => setPathname(`/dashboard/${tab.path}`)}
            >
              <div
                className={clsx(
                  setActive === tab.path
                    ? 'text-white/50'
                    : 'text-gray-faded/30'
                )}
              >
                {tab.icon}
              </div>
              <div className="text-gray-300">
                {tab.path === 'mods' && modSupport
                  ? modTabTitle(modSupport.kind)
                  : tab.title}
              </div>
            </button>
          </RadioGroup.Option>
        ))}
      {children}
    </RadioGroup>
  );
};
