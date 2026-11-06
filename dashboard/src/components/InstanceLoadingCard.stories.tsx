import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import InstanceLoadingCard from './InstanceLoadingCard';

const meta: Meta<typeof InstanceLoadingCard> = {
  title: 'library/InstanceLoadingCard',
  component: InstanceLoadingCard,
  decorators: [
    (Story) => (
      <div className="flex w-60 flex-col child:w-full">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof InstanceLoadingCard>;

export const Default: Story = {
  args: {
    uuid: 'bruh moment',
    name: 'My Minecraft Server',
    port: 25565,
    game_type: {
      type: 'MinecraftJava',
      variant: {
        type: 'Vanilla',
      },
    },
    focus: false,
    progress_percent: 0.5,
    progress_title: 'Setting up...',
  },
};
