import { InstanceSettingCard } from 'components/Instance';
import GameConsole from 'components/GameConsole';
import FileViewer from 'components/FileViewer';
import DashboardCard from 'components/DashboardCard';
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
import { HandlerGameType } from 'bindings/HandlerGameType';

type InstanceTab = {
  title: string;
  displayTitle: string | null;
  path: string;
  width: string;
  icon: JSX.Element;
  content: JSX.Element;
};

export const gameIcons: { [key: string]: { [key: string]: string } } = {
  MinecraftJava: {
    Vanilla: '/assets/minecraft-vanilla.png',
    Fabric: '/assets/minecraft-fabric.png',
    Forge: '/assets/minecraft-forge.png',
    Paper: '/assets/minecraft-paper.png',
  },
};

export const gameTypeInfoFromHandlerType: Record<HandlerGameType, any> = {
  MinecraftJavaVanilla: {
    title: 'Minecraft',
    description: 'Standard vanilla Minecraft server from Mojang.',
    game_type: { MinecraftJava: { variant: 'Vanilla' } },
  },
  MinecraftFabric: {
    title: 'Fabric (Minecraft)',
    description: 'Lightweight modding toolchain for Minecraft.',
    game_type: { MinecraftJava: { variant: 'Fabric' } },
  },
  MinecraftForge: {
    title: 'Forge (Minecraft)',
    description:
      'Modding framework that allows you to install mods and customize your Minecraft experience.',
    game_type: { MinecraftJava: { variant: 'Forge' } },
  },
  MinecraftPaper: {
    title: 'Paper (Minecraft)',
    description: 'High-performance Spigot fork that aims to fix gameplay and mechanics inconsistencies.',
    game_type: { MinecraftJava: { variant: 'Paper' } },
  },
};

export const spanMap: { [key: string]: { [key: string]: string } } = {
  MinecraftJava: {
    Vanilla: 'Minecraft Vanilla',
    Fabric: 'Minecraft Fabric',
    Forge: 'Minecraft Forge',
    Paper: 'Minecraft Paper',
  },
};

export const InstanceTabListMap: Record<string, InstanceTab[]> = {
  MinecraftJava: [
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
            style={{ imageRendering: 'pixelated' }}
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
            style={{ imageRendering: 'pixelated' }}
          />
          <p className="text-center font-medium text-white/50">
            Coming soon to a dashboard near you!
          </p>
        </DashboardCard>
      ),
    },
  ],
};
