import { ComponentStory, ComponentMeta } from '@storybook/react';
import InstanceLoadingCard from './InstanceLoadingCard';

export default {
  title: 'library/InstanceLoadingCard',
  component: InstanceLoadingCard,
} as ComponentMeta<typeof InstanceLoadingCard>;

const Template: ComponentStory<typeof InstanceLoadingCard> = (args) => (
  <div className="flex w-60 flex-col child:w-full">
    <InstanceLoadingCard {...args} />
  </div>
);

export const Default = Template.bind({});
Default.args = {
  uuid: 'bruh moment',
  name: 'My Minecraft Server',
  port: 25565,
  flavour: 'vanilla',
  game_type: 'minecraft',
  focus: false,
  progress_percent: 0.5,
  progress_title: 'Setting up...',
};
