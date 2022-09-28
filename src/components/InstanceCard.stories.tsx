import { ComponentStory, ComponentMeta } from '@storybook/react';
import Split from 'react-split';
import InstanceCard from './InstanceCard';

export default {
  title: 'Components/InstanceCard',
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
    sizes={[60, 40]}
    snapOffset={0}
    gutterSize={0}
    minSize={0}
    className="flex flex-row w-96"
  >
    <div className="flex flex-col child:w-full">
      <InstanceCard {...args} />
    </div>
    <div className="pl-4 text-gray-300">{'   <-Try drag here'}</div>
  </Split>
);

export const Default = Template.bind({});
Default.args = {
  uuid: '1',
  name: 'Test Instance',
  game_type: 'minecraft',
  state: 'Running',
  player_count: 1,
  max_player_count: 12,
  port: 25565,
  focus: false,
};

export const InvestigatingCrash = Template.bind({});
InvestigatingCrash.args = {
  uuid: '2',
  name: 'Crashed Instance',
  game_type: 'minecraft',
  state: 'Error',
  player_count: 0,
  max_player_count: 12,
  port: 25565,
  focus: true,
};
