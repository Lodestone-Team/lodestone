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

export const Default: Story = {
  args: {
    level: 'Info',
    title: 'Dream joined "Test Instance"',
    progress_percent: undefined,
    timestamp: 1620000000000,
  },
};

export const Warning: Story = {
  args: {
    level: 'Warning',
    title: 'Instance "Test Instance" started',
    progress_percent: undefined,
    timestamp: 1620000000000,
  },
};

export const Error: Story = {
  args: {
    level: 'Error',
    title: 'Instance "Test Instance" crashed',
    progress_percent: undefined,
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
    message: 'Instance Creation Success',
    progress_percent: 1,
    timestamp: 1620000000000,
  },
};

export const ProgressError: Story = {
  args: {
    level: 'Error',
    state: 'error',
    title: 'Setting up Minecraft Server Manhunt',
    message: 'Instance Creation Failed',
    progress_percent: 0.5,
    timestamp: 1620000000000,
  },
};
