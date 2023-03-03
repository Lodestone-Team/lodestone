import { 
  MinecraftPerformanceCard,
  MinecraftPlayerList,
  MinecraftGeneralCard,
  MinecraftSettingCard
} from 'components/Minecraft';

import GameConsole from 'components/GameConsole';
import FileViewer from 'components/FileViewer';
import DashboardCard from 'components/DashboardCard';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
// import MinecraftOverview from 'components/Minecraft/MinecraftOverview';

import {
  faChartLine,
  faCodeCompare,
  faCog,
  faFolder,
  faInbox,
  faServer,
} from '@fortawesome/free-solid-svg-icons';

import ErrorGraphic from 'components/ErrorGraphic';

const InstanceTabListMap = {
  minecraft: [
    {
      title: 'Overview',
      path: 'overview',
      width: 'max-w-4xl',
      icon: <FontAwesomeIcon icon={faChartLine} />,
      content: (
        <>
          <MinecraftPerformanceCard />
          <MinecraftPlayerList />
        </>
      ),
    },
    {
      title: 'Settings',
      displayTitle: 'Settings',
      path: 'settings',
      width: 'max-w-2xl',
      icon: <FontAwesomeIcon icon={faCog} />,
      content: (
        <div className="flex flex-col gap-8">
          <MinecraftGeneralCard />
          <MinecraftSettingCard />
        </div>
      ),
    },
    {
      title: 'Console',
      displayTitle: 'Console',
      path: 'console',
      width: 'max-w-6xl',
      icon: <FontAwesomeIcon icon={faServer} />,
      content: (
        <>
          <GameConsole />
          <ErrorGraphic
            iconProp={faServer}
            message="You don't have permission to access this console"
          />
        </>

      )
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
      title: 'Tasks',
      displayTitle: 'Tasks',
      path: 'tasks',
      width: 'max-w-4xl',
      icon: <FontAwesomeIcon icon={faCodeCompare} />,
      content: (
        <DashboardCard className="grow justify-center gap-4">
          <img
            src="/assets/placeholder-cube.png"
            alt="placeholder"
            className="mx-auto w-20"
          />
          <p className="text-center font-medium text-white/50">
            Coming soon to a dashboard near you!
          </p>
        </DashboardCard>
      ),
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
          />
          <p className="text-center font-medium text-white/50">
            Coming soon to a dashboard near you!
          </p>
        </DashboardCard>
      ),
    },
  ],
};

export default InstanceTabListMap;
