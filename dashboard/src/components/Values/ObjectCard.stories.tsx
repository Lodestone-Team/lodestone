import { faPuzzlePiece } from '@fortawesome/free-solid-svg-icons';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import GameIcon from 'components/Atoms/GameIcon';
import { MonoText, StatusBadge } from 'components/Atoms/Values';
import { Fact, FactLine } from './Fact';
import { ObjectCard } from './ObjectCard';

const meta: Meta<typeof ObjectCard> = {
  title: 'Values/ObjectCard',
  component: ObjectCard,
  decorators: [
    (Story) => (
      <div className="w-96 bg-gray-850 p-4">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof ObjectCard>;

export const Server: Story = {
  args: {
    icon: (
      <GameIcon
        game_type={{ type: 'MinecraftJava', variant: { type: 'Fabric' } }}
        className="size-8"
      />
    ),
    name: 'Survival SMP',
    facts: (
      <FactLine>
        <span>Minecraft 1.21.4</span>
        <span>Fabric 0.16.9</span>
      </FactLine>
    ),
    badge: <StatusBadge tone="neutral">Stopped</StatusBadge>,
  },
};

export const Mod: Story = {
  args: {
    icon: <FontAwesomeIcon icon={faPuzzlePiece} className="w-5" />,
    name: 'Sodium',
    version: 'mc1.21.1-0.6.0-fabric',
    facts: (
      <FactLine>
        <Fact label="File">
          <MonoText text="sodium-fabric-0.6.0+mc1.21.1.jar" max={30} />
        </Fact>
      </FactLine>
    ),
  },
};

export const Compact: Story = {
  args: { compact: true, name: 'Home core' },
};
