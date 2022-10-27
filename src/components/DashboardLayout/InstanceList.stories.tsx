import { ComponentStory, ComponentMeta } from '@storybook/react';
import InstanceList from './InstanceList';
import Split from 'react-split';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { setupWorker, rest } from 'msw';
import { LodestoneContext } from 'data/LodestoneContext';
import { InstanceInfo } from 'bindings/InstanceInfo';

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
    path: "C:\\Users\\User\\AppData\\Lodestone\\instances\\123",
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
    path: "C:\\Users\\User\\AppData\\Lodestone\\instances\\123",
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
    path: "C:\\Users\\User\\AppData\\Lodestone\\instances\\123",
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
      port: '3000',
      protocol: 'http',
      apiVersion: 'v1',
      isReady: true,
      token: 'example-token',
    }}
  >
    <QueryClientProvider client={queryClient}>
      <Split
        sizes={[80, 20]}
        snapOffset={0}
        gutterSize={0}
        minSize={0}
        className="flex flex-row"
      >
        <Split
          sizes={[80, 20]}
          snapOffset={0}
          gutterSize={0}
          minSize={0}
          direction="vertical"
          className="flex flex-col"
        >
          <InstanceList />
          <div>
            <div className="text-gray-300">{'⬆Try drag here'}</div>
          </div>
        </Split>
        <div className="pl-4 text-gray-300">{'⬅Try drag here'}</div>
      </Split>
    </QueryClientProvider>
  </LodestoneContext.Provider>
);

export const Default = Template.bind({});
Default.args = {
  instanceInfo: mockedInstanceInfo,
};
