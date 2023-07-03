import { useContext, useEffect, useState } from 'react';
import { InstanceContext } from 'data/InstanceContext';
import { useDocumentTitle } from 'usehooks-ts';
import { useLocation } from 'react-router-dom';
import Label from 'components/Atoms/Label';
import { cn, stateToLabelColor } from 'utils/util';
import Spinner from 'components/DashboardLayout/Spinner';
import { CommandHistoryContextProvider } from 'data/CommandHistoryContext';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import InstanceOverview from 'components/Instance/InstanceOverview';
import {
  faChartLine,
  faCodeCompare,
  faCog,
  faFolder,
  faInbox,
  faServer,
} from '@fortawesome/free-solid-svg-icons';
import GameConsole from 'components/GameConsole';
import FileViewer from 'components/FileViewer';
import DashboardCard from 'components/DashboardCard';
import { InstanceSettingCard } from 'components/Instance';
import Macros from 'pages/macros';
import { PlayitggSignup } from 'components/PlayitggSignup';
import { TunnelList } from 'components/TunnelList';

export const tabs = [
  {
    title: 'Overview',
    displayTitle: null,
    path: 'overview',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faChartLine} />,
    content: <InstanceOverview />,
  },
  {
    title: 'Settings',
    displayTitle: 'Settings',
    path: 'settings',
    width: 'max-w-2xl',
    icon: <FontAwesomeIcon icon={faCog} />,
    content: (
      <div className="flex flex-col gap-8">
        <InstanceSettingCard />
      </div>
    ),
  },
  {
    title: 'Console',
    displayTitle: 'Console',
    path: 'console',
    width: 'max-w-6xl',
    icon: <FontAwesomeIcon icon={faServer} />,
    content: <GameConsole />,
  },
  {
    title: 'Files',
    displayTitle: 'Files',
    path: 'files',
    width: 'max-w-6xl',
    icon: <FontAwesomeIcon icon={faFolder} />,
    content: <FileViewer />,
  },
  {
    title: 'Macros',
    displayTitle: 'Macros',
    path: 'macros',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faCodeCompare} />,
    content: <Macros />,
  },
  {
    title: 'Event Logs',
    displayTitle: 'Event Logs',
    path: 'logs',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faInbox} />,
    content: (
      <DashboardCard className="grow justify-center gap-4">
        <img
          src="/assets/placeholder-cube.png"
          alt="placeholder"
          className="mx-auto w-20"
          style={{ imageRendering: 'pixelated' }}
        />
        <p className="text-center font-medium text-white/50">
          Coming soon to a dashboard near you!
        </p>
      </DashboardCard>
    ),
  },
  {
    title: 'Playitgg',
    displayTitle: 'Playitgg',
    path: 'playitgg',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faInbox} />,
    content: (
      <PlayitggSignup />
    ),
  },
  {
    title: 'Tunnels',
    displayTitle: 'Tunnels',
    path: 'tunnels',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faInbox} />,
    content: (
      <TunnelList />
    ),
  },
];

const InstanceTabs = () => {
  useDocumentTitle('Dashboard - Lodestone');
  const location = useLocation();
  const [path, setPath] = useState(location.pathname.split('/')[2]);
  const { selectedInstance: instance } = useContext(InstanceContext);
  const uuid = instance?.uuid;
  const [loading, setLoading] = useState(true);
  useEffect(() => {
    setPath(location.pathname.split('/')[2]);
  }, [location.pathname]);

  useEffect(() => {
    //give time for instance to load
    setTimeout(() => {
      setLoading(false);
    }, 1000);
  }, []);

  if (!instance || !uuid) {
    if (loading) {
      return <Spinner />;
    } else {
      return (
        <div
          className="relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10 @container"
          key={uuid}
        >
          <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
            <div className="flex min-w-0 flex-row items-center gap-4">
              <h1 className="dashboard-instance-heading truncate whitespace-pre">
                Instance not found
              </h1>
            </div>
          </div>
        </div>
      );
    }
  }

  const tab = tabs.find((tab) => tab.path === path);
  if (!tab) {
    return (
      <div
        className="relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10 @container"
        key={uuid}
      >
        <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
          <div className="flex min-w-0 flex-row items-center gap-4">
            <h1 className="dashboard-instance-heading truncate whitespace-pre">
              {path} not found
            </h1>
          </div>
        </div>
      </div>
    );
  }

  return (
    <CommandHistoryContextProvider>
      <div
        className={cn(
          'relative mx-auto flex h-full w-full flex-row justify-center @container',
          tab.width
        )}
        key={uuid}
      >
        <div
          className="gutter-stable -mx-3 flex grow flex-row items-stretch overflow-y-auto pl-4 pr-2"
          key={`${instance.name}-${tab.title}`}
        >
          <div className="flex h-fit min-h-full w-full flex-col gap-12 pt-6 pb-10 focus:outline-none">
            {tab.displayTitle && (
              <div className="flex font-title text-h1 font-bold leading-tight text-gray-300">
                {tab.displayTitle}
                {tab.displayTitle === 'Console' && (
                  <Label
                    size="medium"
                    className="ml-2 mt-[6px]"
                    color={stateToLabelColor[instance.state]}
                  >
                    {instance.state}
                  </Label>
                )}
              </div>
            )}
            {tab.content}
          </div>
        </div>
      </div>
    </CommandHistoryContextProvider>
  );
};

export default InstanceTabs;
