import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import GameIcon from './GameIcon';

const meta: Meta<typeof GameIcon> = {
  title: 'library/GameIcon',
  component: GameIcon,
};
export default meta;

type Story = StoryObj<typeof GameIcon>;

export const MinecraftVanilla: Story = {
  args: {
    game_type: { type: 'MinecraftJava', variant: { type: 'Vanilla' } },
  },
};
