import { ComponentStory, ComponentMeta } from '@storybook/react';
import NotificationCard from './NotificationCard';

export default {
  title: 'library/NotificationCard',
  component: NotificationCard,
} as ComponentMeta<typeof NotificationCard>;

const Template: ComponentStory<typeof NotificationCard> = (args) => (
  <div className="w-80">
    <NotificationCard {...args} />
  </div>
);

export const Default = Template.bind({});
Default.args = {
  type: 'success',
  title: 'Setting up Minecraft Server Manhunt',
  message: 'Instance Creation Success',
  progress_percent: 50,
  timestamp: 1620000000000,
};
