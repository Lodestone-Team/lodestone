import type { JSX } from 'react';
import { InstanceSettingCard } from 'components/Instance';
import GameConsole from 'components/GameConsole';
import FileViewer from 'components/FileViewer';
import DashboardCard from 'components/DashboardCard';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import InstanceOverview from 'components/Instance/InstanceOverview';
import { match, otherwise } from 'variant';

const unknown_icon = asset('/assets/minecraft-missing-texture.svg');

import {
  faChartLine,
  faCodeCompare,
  faCog,
  faFolder,
  faInbox,
  faServer,
} from '@fortawesome/free-solid-svg-icons';
import { HandlerGameType } from 'bindings/HandlerGameType';
import { Game } from 'bindings/Game';
import { t } from 'i18n';
import { asset } from 'utils/basePath';

type InstanceTab = {
  title: string;
  displayTitle: string | null;
  path: string;
  width: string;
  icon: JSX.Element;
  content: JSX.Element;
};

export const game_to_game_icon = (game: Game) =>
  match(game, {
    MinecraftJava: ({ variant }) =>
      match(
        variant,
        otherwise(
          {
            Vanilla: () => asset('/assets/minecraft-vanilla.png'),
            Fabric: () => asset('/assets/minecraft-fabric.png'),
            Forge: () => asset('/assets/minecraft-forge.png'),
            Paper: () => asset('/assets/minecraft-paper.png'),
          },
          () => unknown_icon
        )
      ),
    MinecraftBedrock: () => unknown_icon,
    Generic: () => asset('/assets/GenericIcon.svg'),
  });

// Titles and descriptions in the current language (call them at render).
// Product names (Minecraft, Forge, NeoForge, Fabric, Paper, Spigot) are not translated.
export const game_to_game_title = (game: Game) =>
  match(game, {
    MinecraftJava: ({ variant }) =>
      match(variant, {
        Vanilla: () => 'Minecraft',
        Forge: () => 'Forge (Minecraft)',
        NeoForge: () => 'NeoForge (Minecraft)',
        Fabric: () => 'Fabric (Minecraft)',
        Paper: () => 'Paper (Minecraft)',
        Spigot: () => 'Spigot (Minecraft)',
        Other: ({ name }) => `${name} (Minecraft)`,
      }),
    MinecraftBedrock: () => 'Minecraft Bedrock',
    Generic: ({ game_name }) =>
      t('create.game.genericTitle', { name: game_name }),
  });

export const game_to_description = (game: Game) =>
  match(game, {
    MinecraftJava: ({ variant }) =>
      match(variant, {
        Vanilla: () => t('create.game.vanillaDescription'),
        Forge: () => t('create.game.forgeDescription'),
        NeoForge: () => t('create.game.neoforgeDescription'),
        Fabric: () => t('create.game.fabricDescription'),
        Paper: () => t('create.game.paperDescription'),
        Spigot: () => t('create.game.spigotDescription'),
        Other: ({ name }) => t('create.game.otherDescription', { name }),
      }),
    MinecraftBedrock: () => t('create.game.bedrockDescription'),
    Generic: ({ game_name }) =>
      t('create.game.genericDescription', { name: game_name }),
  });

export const HandlerGameType_to_Game: Record<HandlerGameType, Game> = {
  MinecraftJavaVanilla: {
    type: 'MinecraftJava',
    variant: {
      type: 'Vanilla',
    },
  },
  MinecraftFabric: {
    type: 'MinecraftJava',
    variant: {
      type: 'Fabric',
    },
  },
  MinecraftForge: {
    type: 'MinecraftJava',
    variant: {
      type: 'Forge',
    },
  },
  MinecraftNeoForge: {
    type: 'MinecraftJava',
    variant: {
      type: 'NeoForge',
    },
  },
  MinecraftPaper: {
    type: 'MinecraftJava',
    variant: {
      type: 'Paper',
    },
  },
  // Not offered by the core's game list yet
  MinecraftBedrock: {
    type: 'MinecraftBedrock',
  },
};
