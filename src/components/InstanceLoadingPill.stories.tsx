import { ComponentStory, ComponentMeta } from '@storybook/react';
import InstanceLoadingCard from './InstanceLoadingPill';

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
  focus: false,
  progress_percent: 0.5,
};
