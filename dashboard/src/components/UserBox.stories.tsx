import type { Meta, StoryObj } from '@storybook/nextjs-vite';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import UserBox from './UserBox';

const meta: Meta<typeof UserBox> = {
  title: 'library/UserBox',
  component: UserBox,
  decorators: [
    (Story) => (
      <QueryClientProvider client={new QueryClient()}>
        <Story />
      </QueryClientProvider>
    ),
  ],
};
export default meta;

type Story = StoryObj<typeof UserBox>;

export const Primary: Story = {
  args: {
    user: {
      uid: '123',
      username: 'John Doe',
      is_owner: true,
      is_admin: true,
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
        can_install_extension: false,
      },
    },
    onClick: () => {
      console.log('clicked');
    },
  },
};
