import { useState } from 'react';
import { RadioGroup } from '@headlessui/react';
import { InstanceContext } from 'data/InstanceContext';
import { useContext, useEffect } from 'react';
import useAnalyticsEventTracker from 'utils/hooks';
import clsx from 'clsx';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import InstanceCard from 'components/InstanceCard';
import { InstanceTabListMap } from '../../data/GameTypeMappings';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faExpand } from '@fortawesome/free-solid-svg-icons';

export const SelectedInstanceInfo = ({
  className = '',
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) => {
  const {
    instanceList: instances,
    selectedInstance,
    isReady,
  } = useContext(InstanceContext);

  const gaEventTracker = useAnalyticsEventTracker('Instance List');
  const { setPathname } = useContext(BrowserLocationContext);
  const [setActive, setActiveTab] = useState(location.pathname.split('/')[2]);

  useEffect(() => {
    if (!isReady) return;
    gaEventTracker(
      'View',
      'Instance List',
      true,
      Object.keys(instances).length
    );
  }, [isReady, instances]);

  useEffect(() => {
    setActiveTab(location.pathname.split('/')[2]);
  }, [location.pathname]);

  const uuid = selectedInstance?.uuid;
  if (!selectedInstance || !uuid) {
    return (
      <div className="mx-1 text-gray-faded/30">
        <div className="text-small font-bold leading-snug text-gray-faded/30">
          SELECTED INSTANCE
        </div>
        <div className="mt-[0.5rem] flex h-[17.61rem] justify-center rounded-md border border-dashed text-center text-gray-faded/30">
          <div className="mt-16 w-[5.5rem]">
            <div className="text-h1">
              <FontAwesomeIcon icon={faExpand} />
            </div>
            <div className="mt-4 text-medium font-bold">
              Any selected instance will appear here
            </div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <RadioGroup
      className={`mx-1 flex min-h-0 flex-col gap-y-1 overflow-x-hidden px-1 pb-1 child:w-full ${className}`}
      onChange={setPathname}
    >
      <RadioGroup.Label className="text-small font-bold leading-snug text-gray-faded/30">
        SELECTED INSTANCE
      </RadioGroup.Label>

      <InstanceCard {...selectedInstance} />

      {selectedInstance &&
        InstanceTabListMap[Object.keys(selectedInstance.game_type)[0]].map(
          (tab) => (
            <RadioGroup.Option
              key={tab.path}
              value={`/dashboard/${tab.path}`}
              className="rounded-md outline-none focus-visible:bg-gray-800 child:w-full"
            >
              <button
                className={clsx(
                  'flex flex-row items-center gap-x-1.5',
                  'cursor-pointer rounded-md py-1 px-2',
                  'text-medium font-medium leading-5 tracking-normal',
                  'hover:bg-gray-800',
                  'focus-visible:outline-none focus-visible:ring-4 focus-visible:ring-blue-faded/50',
                  setActive === tab.path
                    ? 'bg-gray-800 outline outline-1 outline-fade-700'
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
                <div className="text-gray-300">{tab.title}</div>
              </button>
            </RadioGroup.Option>
          )
        )}
      {children}
    </RadioGroup>
  );
};
