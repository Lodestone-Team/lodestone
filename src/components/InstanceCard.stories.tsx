import { ComponentStory, ComponentMeta } from '@storybook/react';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { setupWorker, rest } from 'msw';
import InstanceCard from './InstanceCard';

const mockedUserInfo = {
  uid: '109bdc1d-f7ab-4186-a9c2-d1a9aaf8d937',
  username: 'owner',
  is_owner: true,
  is_admin: false,
  permissions: {
    can_view_instance: [],
    can_start_instance: [],
    can_stop_instance: [],
    can_access_instance_console: [],
    can_access_instance_setting: [],
    can_read_instance_resource: [],
    can_write_instance_resource: [],
    can_access_instance_macro: [],
    can_read_instance_file: [],
    can_write_instance_file: [],
    can_create_instance: false,
    can_delete_instance: false,
    can_read_global_file: false,
    can_write_global_file: false,
    can_manage_permission: false,
  },
};

const worker = setupWorker(
  rest.get('/user/info', (req, res, ctx) => {
    return res(ctx.json(mockedUserInfo));
  })
);

worker.start();

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
  <QueryClientProvider client={new QueryClient()}>
    <div className="flex flex-col child:w-full w-60">
      <InstanceCard {...args} />
    </div>
  </QueryClientProvider>
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
