import { ComponentStory, ComponentMeta } from '@storybook/react';
import InstanceLoadingPill from './InstanceLoadingPill';

export default {
  title: 'library/InstanceLoadingPill',
  component: InstanceLoadingPill,
} as ComponentMeta<typeof InstanceLoadingPill>;

const Template: ComponentStory<typeof InstanceLoadingPill> = (args) => (
  <div className="flex w-60 flex-col child:w-full">
    <InstanceLoadingPill {...args} />
  </div>
);

export const Default = Template.bind({});
Default.args = {
  focus: false,
  progress_percent: 0.5,
};
