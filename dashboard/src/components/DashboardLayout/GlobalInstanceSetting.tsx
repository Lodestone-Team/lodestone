import { useEffect, useMemo, useState } from 'react';
import { RadioGroup } from '@headlessui/react';
import { useContext } from 'react';
import clsx from 'clsx';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import { faAngleRight, faAngleDown } from '@fortawesome/free-solid-svg-icons';
import { tabs } from 'pages/settings/GlobalSettings';
import { useGlobalSettings } from 'data/GlobalSettings';

export const GlobalInstanceSetting = ({
  className = '',
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) => {

  const { setPathname } = useContext(BrowserLocationContext);
  const setActive = useMemo(() => location.pathname.split('/')[2], [location.pathname]);
  const { data: globalSettings } = useGlobalSettings();

  const [ expand, setExpand ] = useState(false);

  useEffect(() => {
    if (location.pathname == '/dashboard/core-settings' || location.pathname == '/dashboard/version') {
        setExpand(true);
    }
  }, [])

  return (
    <RadioGroup
      className={`child:w-full mx-1 flex min-h-0 flex-col gap-y-1 overflow-x-hidden px-1 pb-1 ${className}`}
      onChange={setPathname}
    >
      <RadioGroup.Label 
        className="text-small font-bold leading-snug text-gray-faded/30 flex justify-between items-center hover:cursor-pointer"
        onClick={() => setExpand(!expand)}>
        GLOBAL SETTINGS
        <FontAwesomeIcon icon={expand ? faAngleDown : faAngleRight}/>
      </RadioGroup.Label>

      {expand &&
        tabs.map((tab) => (
          (tab.title !== 'Playitgg' || globalSettings?.playit_enabled) &&
          <RadioGroup.Option
            key={tab.path}
            value={`/dashboard/${tab.path}`}
            className="rounded-md outline-none focus-visible:bg-gray-800 child:w-full"
          >
            <button
              className={clsx(
                'flex flex-row items-center gap-x-1.5',
                'cursor-pointer rounded-md px-2 py-1',
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
        ))}
      {children}
    </RadioGroup>
  );
};
