import { ComponentStory, ComponentMeta } from '@storybook/react';
import Split from 'react-split';
import InstanceCard from './InstanceCard';

export default {
  title: 'Library/InstanceCard',
  component: InstanceCard,
  argTypes: {
    onClick: { table: { disable: true } },
    id: { table: { disable: true } },
    type: { control: 'select' },
    status: { control: 'inline-radio' },
    focus: { control: 'boolean' },
  },
  parameters: {
    backgrounds: {
      default: 'gray-700',
    },
  },
} as ComponentMeta<typeof InstanceCard>;

const Template: ComponentStory<typeof InstanceCard> = (args) => (
  <Split
    sizes={[25, 75]}
    minSize={100}
    className="flex flex-row"
  >
    <div className="flex flex-col child:w-full">
      <InstanceCard {...args} />
    </div>
    <div>Other Stuff</div>
  </Split>
);

export const Default = Template.bind({});
Default.args = {
  id: '1',
  name: 'Test Instance',
  type: 'minecraft',
  status: 'running',
  playerCount: 1,
  maxPlayerCount: 12,
  ip: '123.345.456.678',
  port: 25565,
  focus: false,
};

export const InvestigatingCrash = Template.bind({});
InvestigatingCrash.args = {
  id: '2',
  name: 'Crashed Instance',
  type: 'minecraft',
  status: 'crashed',
  playerCount: 0,
  maxPlayerCount: 12,
  ip: '123.345.456.678',
  port: 25565,
  focus: true,
};
