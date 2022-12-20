import { ComponentStory, ComponentMeta } from '@storybook/react';
import InstanceList from './InstanceList';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { setupWorker, rest } from 'msw';
import { LodestoneContext } from 'data/LodestoneContext';
import { InstanceInfo } from 'bindings/InstanceInfo';
import { LODESTONE_PORT } from 'utils/util';

const mockedInstanceInfo: InstanceInfo[] = [
  {
    uuid: '123',
    name: 'Test Instance',
    flavour: 'vanilla',
    game_type: 'minecraft',
    cmd_args: [],
    description: 'This is a test instance',
    port: 25565,
    min_ram: 1024,
    max_ram: 2048,
    creation_time: BigInt(24234234234),
    path: 'C:\\Users\\User\\AppData\\Lodestone\\instances\\123',
    auto_start: false,
    restart_on_crash: false,
    backup_period: 0,
    state: 'Stopped',
    player_count: 0,
    max_player_count: 0,
  },
  {
    uuid: '456',
    name: 'Minecraft manhunt 1.16.5',
    flavour: 'vanilla',
    game_type: 'minecraft',
    cmd_args: [],
    description: 'This is a test instance',
    port: 25565,
    min_ram: 1024,
    max_ram: 2048,
    creation_time: BigInt(24234234234),
    path: 'C:\\Users\\User\\AppData\\Lodestone\\instances\\123',
    auto_start: false,
    restart_on_crash: false,
    backup_period: 0,
    state: 'Stopped',
    player_count: 0,
    max_player_count: 0,
  },
  {
    uuid: '789',
    name: 'My Minecraft Server',
    flavour: 'vanilla',
    game_type: 'minecraft',
    cmd_args: [],
    description: 'This is a test instance',
    port: 25565,
    min_ram: 1024,
    max_ram: 2048,
    creation_time: BigInt(24234234234),
    path: 'C:\\Users\\User\\AppData\\Lodestone\\instances\\123',
    auto_start: false,
    restart_on_crash: false,
    backup_period: 0,
    state: 'Stopped',
    player_count: 0,
    max_player_count: 0,
  },
];

const worker = setupWorker(
  rest.get('/instance/list', (req, res, ctx) => {
    return res(ctx.json(mockedInstanceInfo));
  })
);

worker.start();

export default {
  title: 'components/InstanceList',
  component: InstanceList,
  parameters: {
    backgrounds: {
      default: 'gray-700',
    },
  },
} as ComponentMeta<typeof InstanceList>;

const queryClient = new QueryClient();

const Template: ComponentStory<typeof InstanceList> = () => (
  <LodestoneContext.Provider
    value={{
      address: 'example.com',
      port: LODESTONE_PORT.toString(),
      socket: `example.com:${LODESTONE_PORT}`,
      protocol: 'http',
      apiVersion: 'v1',
      isReady: true,
      token: 'example-token',
      setToken: () => {
        console.error('setToken not implemented');
      },
      tokens: {},
      setAddress: () => {
        console.error('setAddress not implemented');
      },
      setPort: () => {
        console.error('setPort not implemented');
      },
      setProtocol: () => {
        console.error('setProtocol not implemented');
      },
      setApiVersion: () => {
        console.error('setApiVersion not implemented');
      },
    }}
  >
    <QueryClientProvider client={queryClient}>
      <InstanceList />
    </QueryClientProvider>
  </LodestoneContext.Provider>
);

export const Default = Template.bind({});
Default.args = {
  
};
