import { ReactNode, useContext } from 'react';
import { InstanceContext } from 'data/InstanceContext';
import { useLocation } from 'react-router';
import Label from 'components/Atoms/Label';
import { cn, stateToLabelColor } from 'utils/util';
import Spinner from 'components/DashboardLayout/Spinner';
import { CommandHistoryContextProvider } from 'data/CommandHistoryContext';
import InstanceOverview from 'components/Instance/InstanceOverview';
import GameConsole from 'components/GameConsole';
import FileViewer from 'components/FileViewer';
import DashboardCard from 'components/DashboardCard';
import { InstanceSettingCard } from 'components/Instance';
import Macros from 'pages/macros';
import { PlayitggOverview } from 'components/PlayitggOverview';
import ModsTab from 'components/Mods/ModsTab';
import { getModSupport, modTabTitle } from 'utils/mods';
import { InstanceTabPath, tabs } from './tabList';

// The content of each tab; their names, paths and icons are in tabList.
const tabContent: Record<InstanceTabPath, ReactNode> = {
  overview: <InstanceOverview />,
  settings: (
    <div className="flex flex-col gap-8">
      <InstanceSettingCard />
    </div>
  ),
  console: <GameConsole />,
  files: <FileViewer />,
  mods: <ModsTab />,
  macros: <Macros />,
  logs: (
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
  playitgg: <PlayitggOverview />,
};

const InstanceTabs = () => {
  const location = useLocation();
  // read from the URL on every render: a copy kept in state by an effect was a
  // render behind, so switching tabs first rendered the old tab again
  const path = location.pathname.split('/')[2];
  const {
    selectedInstance: instance,
    instanceList,
    isReady,
  } = useContext(InstanceContext);
  const uuid = instance?.uuid;

  if (!instance || !uuid) {
    // Wait for the instance list to load. Keep waiting if the instance named
    // in the URL exists: after back/forward the selection follows the URL a
    // render later.
    const queryUuid = new URLSearchParams(location.search).get('instance');
    if (!isReady || (queryUuid && queryUuid in instanceList)) {
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
  const modSupport = getModSupport(instance.game_type);
  const displayTitle =
    tab?.path === 'mods' && modSupport
      ? modTabTitle(modSupport.kind)
      : tab?.displayTitle;
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
          // not keyed by name: renaming must not remount the tab (and e.g.
          // reconnect the performance stream)
          key={`${uuid}-${tab.path}`}
        >
          <div className="flex h-fit min-h-full w-full flex-col gap-2 pt-6 pb-10 focus:outline-hidden">
            {displayTitle && (
              <div className="flex font-title text-h1 font-bold leading-tight text-gray-300">
                {displayTitle}
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
            {tabContent[tab.path]}
          </div>
        </div>
      </div>
    </CommandHistoryContextProvider>
  );
};

export default InstanceTabs;
