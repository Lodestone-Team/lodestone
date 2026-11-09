import { ReactNode, useContext } from 'react';
import { InstanceContext } from 'data/InstanceContext';
import { useLocation } from 'react-router';
import Label from 'components/Atoms/Label';
import { cn, stateToLabelColor } from 'utils/util';
import { PageLoading } from 'components/DashboardLayout/LayoutSkeletons';
import { CommandHistoryContextProvider } from 'data/CommandHistoryContext';
import InstanceOverview from 'components/Instance/InstanceOverview';
import GameConsole from 'components/GameConsole';
import FileViewer from 'components/FileViewer';
import DashboardCard from 'components/DashboardCard';
import { InstanceSettingCard } from 'components/Instance';
import Macros from 'pages/macros';
import ModsTab from 'components/Mods/ModsTab';
import { getModSupport, modTabTitle } from 'utils/mods';
import { InstanceTabPath, tabs } from './tabList';
import { useT } from 'i18n';
import { asset } from 'utils/basePath';

/** The Event Logs tab, not made yet */
const LogsPlaceholder = () => {
  const t = useT();
  return (
    <DashboardCard className="grow justify-center gap-4">
      <img
        src={asset('/assets/placeholder-cube.png')}
        alt=""
        className="mx-auto w-20"
        style={{ imageRendering: 'pixelated' }}
      />
      <p className="text-center font-medium text-white/50">
        {t('server.logs.comingSoon')}
      </p>
    </DashboardCard>
  );
};

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
  logs: <LogsPlaceholder />,
};

const InstanceTabs = () => {
  const t = useT();
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
      return <PageLoading />;
    } else {
      return (
        <div
          className="@container relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10"
          key={uuid}
        >
          <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
            <div className="flex min-w-0 flex-row items-center gap-4">
              <h1 className="dashboard-instance-heading truncate whitespace-pre">
                {t('server.notFound')}
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
      : tab?.displayTitleKey && t(tab.displayTitleKey);
  if (!tab) {
    return (
      <div
        className="@container relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10"
        key={uuid}
      >
        <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
          <div className="flex min-w-0 flex-row items-center gap-4">
            <h1 className="dashboard-instance-heading truncate whitespace-pre">
              {t('server.tabNotFound', { path })}
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
          '@container relative mx-auto flex h-full w-full flex-row justify-center',
          tab.width
        )}
        key={uuid}
      >
        <div
          className="-mx-3 flex grow gutter-stable flex-row items-stretch overflow-y-auto pr-2 pl-4"
          // not keyed by name: renaming must not remount the tab (and e.g.
          // reconnect the performance stream)
          key={`${uuid}-${tab.path}`}
        >
          <div className="flex h-fit min-h-full w-full flex-col gap-2 pt-6 pb-10 focus:outline-hidden">
            {displayTitle && (
              <div className="flex min-w-0 flex-wrap font-title text-h1 leading-tight font-bold text-gray-300">
                {displayTitle}
                {tab.path === 'console' && (
                  <Label
                    size="medium"
                    className="mt-[6px] ml-2"
                    color={stateToLabelColor[instance.state]}
                  >
                    {t(`common.state.${instance.state}`)}
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
