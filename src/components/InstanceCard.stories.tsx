import { ComponentStory, ComponentMeta } from '@storybook/react';
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
  <div className="flex flex-col child:w-full">
    <InstanceCard {...args} />
  </div>
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
