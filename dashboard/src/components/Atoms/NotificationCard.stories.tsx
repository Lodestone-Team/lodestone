import { ComponentStory, ComponentMeta } from '@storybook/react';
import NotificationCard from './NotificationCard';

export default {
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
} as ComponentMeta<typeof NotificationCard>;

const Template: ComponentStory<typeof NotificationCard> = (args) => (
  <div className="w-80">
    <NotificationCard {...args} />
  </div>
);

export const Default = Template.bind({});
Default.args = {
  level: 'Info',
  title: 'Dream joined "Test Instance"',
  progress_percent: undefined,
  timestamp: 1620000000000,
};

export const Warning = Template.bind({});
Warning.args = {
  level: 'Warning',
  title: 'Instance "Test Instance" started',
  progress_percent: undefined,
  timestamp: 1620000000000,
};

export const Error = Template.bind({});
Error.args = {
  level: 'Error',
  title: 'Instance "Test Instance" crashed',
  progress_percent: undefined,
  timestamp: 1620000000000,
};

export const Progress = Template.bind({});
Progress.args = {
  level: 'Info',
  state: 'ongoing',
  title: 'Setting up Minecraft Server Manhunt',
  message: '1/4 Downloading JRE',
  progress_percent: 0.25,
  timestamp: 1620000000000,
};

export const ProgressSuccess = Template.bind({});
ProgressSuccess.args = {
  level: 'Info',
  state: 'done',
  title: 'Setting up Minecraft Server Manhunt',
  message: 'Instance Creation Success',
  progress_percent: 1,
  timestamp: 1620000000000,
};

export const ProgressError = Template.bind({});
ProgressError.args = {
  level: 'Error',
  state: 'error',
  title: 'Setting up Minecraft Server Manhunt',
  message: 'Instance Creation Failed',
  progress_percent: 0.5,
  timestamp: 1620000000000,
};

