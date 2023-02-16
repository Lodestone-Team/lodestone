import { useState } from 'react';
import { RadioGroup } from '@headlessui/react';
import { InstanceContext } from 'data/InstanceContext';
import { useContext, useEffect } from 'react';
import useAnalyticsEventTracker from 'utils/hooks';
import clsx from 'clsx';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import InstanceCard from 'components/InstanceCard';
import InstanceTabListMap from '../../data/InstanceTabListMap';

export const InstanceNestedBarStates = ({
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
    return <div></div>;
  }

  return (
    <RadioGroup
      className={`gap mx-1 flex min-h-0 flex-col gap-y-1 overflow-x-hidden px-1 pb-1 child:w-full ${className}`}
      onChange={setPathname}
    >
      <RadioGroup.Label className="text-small font-bold leading-snug text-gray-faded/30">
        SELECTED INSTANCE
      </RadioGroup.Label>

      <InstanceCard {...selectedInstance} />

      {selectedInstance &&
        InstanceTabListMap[selectedInstance.game_type].map((tab) => (
          <RadioGroup.Option
            key={tab.path}
            value={`/dashboard/${tab.path}`}
            className="rounded-md outline-none focus-visible:bg-gray-800 child:w-full"
          >
            <button
              className={clsx(
                'flex flex-row items-center gap-x-1.5',
                'cursor-pointer rounded-md py-1 px-2',
                'text-medium font-bold leading-5 tracking-medium',
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
        ))}
      {children}
    </RadioGroup>
  );
};
