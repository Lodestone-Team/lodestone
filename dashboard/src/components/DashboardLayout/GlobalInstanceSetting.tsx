import { useContext, useState } from 'react';
import { RadioGroup } from '@headlessui/react';
import clsx from 'clsx';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faAngleDown,
  faAngleRight,
  faGears,
  faJarWheat,
} from '@fortawesome/free-solid-svg-icons';
import {
  GlobalSettingsPath,
  globalSettingsPage,
} from 'utils/globalSettingsRoutes';

const tabs: {
  title: string;
  path: GlobalSettingsPath;
  icon: typeof faGears;
}[] = [
  { title: 'Core Settings', path: 'core-settings', icon: faGears },
  { title: 'Version Info', path: 'version', icon: faJarWheat },
];

export const GlobalInstanceSetting = ({
  className = '',
  children,
}: {
  className?: string;
  children?: React.ReactNode;
}) => {
  const { setPathname, location } = useContext(BrowserLocationContext);
  const activePage = globalSettingsPage(location.pathname);

  // open while on one of its pages, even if it was closed to begin with
  const [expandedByUser, setExpandedByUser] = useState<boolean | null>(null);
  const [lastActivePage, setLastActivePage] = useState(activePage);
  if (lastActivePage !== activePage) {
    setLastActivePage(activePage);
    if (activePage) setExpandedByUser(null);
  }
  const expand = expandedByUser ?? activePage !== null;

  return (
    <RadioGroup
      className={`mx-1 flex min-h-0 flex-col gap-y-1 overflow-x-hidden px-1 pb-1 child:w-full ${className}`}
      onChange={setPathname}
    >
      <button
        type="button"
        aria-expanded={expand}
        className="flex items-center justify-between text-small leading-snug font-bold text-gray-faded/30 hover:cursor-pointer light:text-ink-muted"
        onClick={() => setExpandedByUser(!expand)}
      >
        GLOBAL SETTINGS
        <FontAwesomeIcon icon={expand ? faAngleDown : faAngleRight} />
      </button>

      {expand &&
        tabs.map((tab) => (
          <RadioGroup.Option
            key={tab.path}
            value={`/dashboard/${tab.path}`}
            className="rounded-md outline-hidden focus-visible:bg-gray-800 child:w-full"
          >
            <button
              className={clsx(
                'flex flex-row items-center gap-x-1.5',
                'cursor-pointer rounded-md px-2 py-1',
                'text-medium leading-5 font-medium tracking-normal',
                'hover:bg-gray-800',
                'focus-visible:ring-4 focus-visible:ring-blue-faded/50 focus-visible:outline-hidden',
                activePage === tab.path
                  ? 'bg-gray-800 outline-1 outline-fade-700 outline-solid'
                  : ''
              )}
              onClick={() => setPathname(`/dashboard/${tab.path}`)}
            >
              <div
                className={clsx(
                  activePage === tab.path
                    ? 'text-white/50 light:text-blue-200'
                    : 'text-gray-faded/30'
                )}
              >
                <FontAwesomeIcon icon={tab.icon} />
              </div>
              <div className="text-gray-300">{tab.title}</div>
            </button>
          </RadioGroup.Option>
        ))}
      {children}
    </RadioGroup>
  );
};
