import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import NotificationCard from './NotificationCard';

const meta: Meta<typeof NotificationCard> = {
  title: 'library/NotificationCard',
  component: NotificationCard,
  argTypes: {
    progress_percent: {
      control: {
        type: 'range',
        min: 0,
        max: 1,
        step: 0.01,
      },
    },
  },
  decorators: [
    (Story) => (
      <div className="w-80">
        <Story />
      </div>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof NotificationCard>;

/** Players joined: the server leads, the players are chips */
export const Default: Story = {
  args: {
    level: 'Info',
    content: {
      type: 'players',
      server: 'Test Server',
      joined: ['Dream', 'Technoblade'],
      left: [],
    },
    timestamp: 1620000000000,
  },
};

export const State: Story = {
  args: {
    level: 'Info',
    content: { type: 'state', server: 'Test Server', state: 'Running' },
    timestamp: 1620000000000,
  },
};

export const Warning: Story = {
  args: {
    level: 'Warning',
    content: { type: 'warning', server: 'Test Server' },
    timestamp: 1620000000000,
  },
};

export const Error: Story = {
  args: {
    level: 'Error',
    content: { type: 'state', server: 'Test Server', state: 'Error' },
    timestamp: 1620000000000,
  },
};

export const Chat: Story = {
  args: {
    level: 'Info',
    content: {
      type: 'chat',
      server: 'Test Server',
      player: 'Dream',
      message: 'anyone up for the nether?',
    },
    timestamp: 1620000000000,
  },
};

export const ModpackUpdate: Story = {
  args: {
    level: 'Info',
    content: {
      type: 'modpack',
      server: 'Test Server',
      pack: 'Fabulously Optimized',
      version: '6.5.0',
    },
    timestamp: 1620000000000,
  },
};

export const Progress: Story = {
  args: {
    level: 'Info',
    state: 'ongoing',
    title: 'Setting up Minecraft Server Manhunt',
    message: '1/4 Downloading JRE',
    progress_percent: 0.25,
    timestamp: 1620000000000,
  },
};

export const ProgressSuccess: Story = {
  args: {
    level: 'Info',
    state: 'done',
    title: 'Setting up Minecraft Server Manhunt',
    message: 'Server Creation Success',
    progress_percent: 1,
    timestamp: 1620000000000,
  },
};

export const ProgressError: Story = {
  args: {
    level: 'Error',
    state: 'error',
    title: 'Setting up Minecraft Server Manhunt',
    message: 'Server Creation Failed',
    progress_percent: 0.5,
    timestamp: 1620000000000,
  },
};
