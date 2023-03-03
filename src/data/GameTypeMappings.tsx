import GameConsole from 'components/GameConsole';
import MinecraftGeneralCard from 'components/Minecraft/MinecraftGeneralCard';
import MinecraftSettingCard from 'components/Minecraft/MinecraftSettingCard';
import FileViewer from 'components/FileViewer';
import MinecraftPerformanceCard from 'components/Minecraft/MinecraftPerformanceCard';
import DashboardCard from 'components/DashboardCard';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';

import {
  faChartLine,
  faCodeCompare,
  faCog,
  faFolder,
  faInbox,
  faServer,
} from '@fortawesome/free-solid-svg-icons';
import { HandlerGameType } from 'bindings/HandlerGameType';
import { GameType } from 'bindings/InstanceInfo';

type InstanceTab = {
  title: string;
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
    description: 'A standard vanilla Minecraft server.',
    game_type: { MinecraftJava: { variant: 'Vanilla' } },
  },
  MinecraftFabric: {
    title: 'Paper (Minecraft)',
    description: 'A high performance fork of the Spigot Minecraft Server.',
    game_type: { MinecraftJava: { variant: 'Fabric' } },
  },
  MinecraftForge: {
    title: 'Forge (Minecraft)',
    description:
      'Modifications to the Minecraft base files to assist in compatibility between mods.',
    game_type: { MinecraftJava: { variant: 'Forge' } },
  },
  MinecraftPaper: {
    title: 'Fabric (Minecraft)',
    description: 'A lightweight, experimental modding toolchain for Minecraft.',
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
      path: 'overview',
      width: 'max-w-4xl',
      icon: <FontAwesomeIcon icon={faChartLine} />,
      content: (
        <>
          <p>Work in Progress</p>
          <MinecraftPerformanceCard />
        </>
      ),
    },
    {
      title: 'Settings',
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
      path: 'console',
      width: 'max-w-6xl',
      icon: <FontAwesomeIcon icon={faServer} />,
      content: <GameConsole />,
    },
    {
      title: 'Files',
      path: 'files',
      width: 'max-w-6xl',
      icon: <FontAwesomeIcon icon={faFolder} />,
      content: <FileViewer />,
    },
    {
      title: 'Tasks',
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
