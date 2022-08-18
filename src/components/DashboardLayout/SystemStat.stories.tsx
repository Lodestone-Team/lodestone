import { ComponentStory, ComponentMeta } from '@storybook/react';
import Split from 'react-split';
import SystemStat from './SystemStat';

export default {
  title: 'library/SystemStat',
  component: SystemStat,
} as ComponentMeta<typeof SystemStat>;

const Template: ComponentStory<typeof SystemStat> = (args) => (
  <Split
    sizes={[10, 90]}
    snapOffset={0}
    gutterSize={0}
    minSize={0}
    className="flex flex-row"
  >
    <SystemStat {...args} />
    <div className="pl-4 text-gray-300 border-l">{'   <-Try drag here'}</div>
  </Split>
);

export const Default = Template.bind({});
Default.args = {
  name: 'Memory',
  value: '100%',
};
