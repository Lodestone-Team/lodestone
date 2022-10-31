import { ComponentStory, ComponentMeta } from '@storybook/react';
import SystemStat from './SystemStat';

export default {
  title: 'library/SystemStat',
  component: SystemStat,
} as ComponentMeta<typeof SystemStat>;

const Template: ComponentStory<typeof SystemStat> = (args) => (
  <SystemStat {...args} />
);

export const Default = Template.bind({});
Default.args = {
  name: 'Memory',
  value: '100%',
};
