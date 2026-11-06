import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { http, HttpResponse } from 'msw';
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

const meta: Meta<typeof InstanceCard> = {
  title: 'library/InstanceCard',
  component: InstanceCard,
  argTypes: {
    onClick: { table: { disable: true } },
    focus: { control: 'boolean' },
  },
  parameters: {
    // Served by the MSW worker that mswLoader (.storybook/preview.ts) starts
    msw: {
      handlers: [
        http.get('/user/info', () => HttpResponse.json(mockedUserInfo)),
      ],
    },
  },
  globals: {
    backgrounds: { value: 'gray-850' },
  },
  decorators: [
    (Story) => (
      <QueryClientProvider client={new QueryClient()}>
        <div className="flex w-60 flex-col child:w-full">
          <Story />
        </div>
      </QueryClientProvider>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof InstanceCard>;

export const Default: Story = {
  args: {
    uuid: '1',
    name: 'Test Instance',
    game_type: { type: 'MinecraftJava', variant: { type: 'Vanilla' } },
    state: 'Running',
    player_count: 1,
    max_player_count: 12,
    port: 25565,
    focus: false,
  },
};

export const InvestigatingCrash: Story = {
  args: {
    uuid: '2',
    name: 'Crashed Instance',
    game_type: { type: 'MinecraftJava', variant: { type: 'Vanilla' } },
    state: 'Error',
    player_count: 0,
    max_player_count: 12,
    port: 25565,
    focus: true,
  },
};
