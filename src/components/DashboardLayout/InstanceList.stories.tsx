import { ComponentStory, ComponentMeta } from '@storybook/react';
import InstanceList from './InstanceList';
import { InstanceInfo } from 'data/InstanceList';
import Split from 'react-split';
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { setupWorker, rest } from 'msw';
import { LodestoneContext } from 'data/LodestoneContext';

const mockedInstanceInfo: InstanceInfo[] = [
  {
    uuid: '64417721-930f-4009-8e02-377bfd477355',
    name: 'CON',
    port: 25565,
    description: 'Pizza time',
    game_type: 'minecraft',
    flavour: 'vanilla',
    state: 'Stopped',
    player_count: 0,
    max_player_count: 20,
    creation_time: 1662770937,
    path: "/home/runner/work/lodestone/lodestone/instances/64417721-930f-4009-8e02-377bfd477355",
  },
  {
    uuid: '1f2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d',
    name: 'A very long instance name',
    port: 25566,
    description: 'Pizza time',
    game_type: 'minecraft',
    flavour: 'vanilla',
    state: 'Starting',
    player_count: 0,
    max_player_count: 20,
    creation_time: 1662770937,
    path: "C:\\Users\\runneradmin\\AppData\\Local\\Temp\\lodestone\\instances\\1f2b3c4d-5e6f-7a8b-9c0d-1e2f3a4b5c6d",
  },
  {
    uuid: '1f2asdasd-5e6f-7a8b-9c0d-1e2f3a4b5c6d',
    name: 'A',
    port: 25566,
    description: 'Pizza time',
    game_type: 'minecraft',
    flavour: 'fabric',
    state: 'Running',
    player_count: 12,
    max_player_count: 20,
    creation_time: 1662770937,
    path: "usr/ubuntu/client/.lodestone/instances/1f2asdasd-5e6f-7a8b-9c0d-1e2f3a4b5c6d",
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
