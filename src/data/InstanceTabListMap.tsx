import GameConsole from 'components/GameConsole';
import MinecraftGeneralCard from 'components/Minecraft/MinecraftGeneralCard';
import MinecraftSettingCard from 'components/Minecraft/MinecraftSettingCard';
import FileViewer from 'components/FileViewer';
import DashboardCard from 'components/DashboardCard';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import MinecraftOverview from 'components/Minecraft/MinecraftOverview';
import MinecraftPerformanceCard from 'components/Minecraft/MinecraftPerformanceCard';

import {
  faChartLine,
  faCodeCompare,
  faCog,
  faFolder,
  faInbox,
  faServer,
} from '@fortawesome/free-solid-svg-icons';

const InstanceTabListMap = {
  minecraft: [
    {
      title: 'Overview',
      path: 'overview',
      icon: <FontAwesomeIcon icon={faChartLine} />,
      content: <MinecraftOverview/>,
    },
    {
      title: 'Settings',
      displayTitle: 'Settings',
      path: 'settings',
      icon: <FontAwesomeIcon icon={faCog} />,
      content: (
        <>
          <MinecraftGeneralCard />
          <MinecraftSettingCard />
        </>
      ),
    },
    {
      title: 'Console',
      displayTitle: 'Console',
      path: 'console',
      icon: <FontAwesomeIcon icon={faServer} />,
      content: <GameConsole />,
    },
    {
      title: 'Files',
      displayTitle: 'Files',
      path: 'files',
      icon: <FontAwesomeIcon icon={faFolder} />,
      content: <FileViewer />,
    },
    {
      title: 'Tasks',
      displayTitle: 'Tasks',
      path: 'tasks',
      icon: <FontAwesomeIcon icon={faCodeCompare} />,
      content: (
        <DashboardCard className="grow !justify-center !gap-4">
          <img
            src="/assets/placeholder-cube.png"
            alt="placeholder"
            className="mx-auto w-20"
          />
          <p className="text-xl text-center font-medium text-white/50">
            Coming soon to a dashboard near you!
          </p>
        </DashboardCard>
      ),
    },
    {
      title: 'Event Logs',
      displayTitle: 'Event Logs',
      path: 'logs',
      icon: <FontAwesomeIcon icon={faInbox} />,
      content: (
        <DashboardCard className="grow !justify-center !gap-4">
          <img
            src="/assets/placeholder-cube.png"
            alt="placeholder"
            className="mx-auto w-20"
          />
          <p className="text-xl text-center font-medium text-white/50">
            Coming soon to a dashboard near you!
          </p>
        </DashboardCard>
      ),
    },
  ],
};

export default InstanceTabListMap;
