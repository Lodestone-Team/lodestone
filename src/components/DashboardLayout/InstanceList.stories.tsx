import { ComponentStory, ComponentMeta } from '@storybook/react';
import InstanceList from './InstanceList';
import { Provider } from 'react-redux';
import { configureStore, createSlice } from '@reduxjs/toolkit';
import { InstanceListState } from 'data/InstanceList';
import { ClientInfoState } from 'data/ClientInfo';
import Split from 'react-split';

const mockedClientInfo = {
  loading: false,
  apiUrl: 'mocked-address.com:3000',
} as ClientInfoState;

const mockedInstanceList = {
  loading: false,
  error: null,
  instances: {
    '1': {
      id: '1',
      name: 'Test Instance',
      type: 'minecraft',
      status: 'running',
      playerCount: 1,
      maxPlayerCount: 12,
      ip: '123.345.456.678',
      port: 25565,
    },
    '2': {
      id: '2',
      name: 'Crashed Instance',
      type: 'minecraft',
      status: 'crashed',
      playerCount: 0,
      maxPlayerCount: 12,
      ip: '123.345.456.678',
      port: 25565,
    },
    '3': {
      id: '3',
      name: 'This is a really long name that should wrap',
      type: 'minecraft',
      status: 'stopped',
      playerCount: 0,
      maxPlayerCount: 12,
      ip: '123.345.456.678',
      port: 25565,
    },
  },
} as InstanceListState;

const Mockstore = ({
  instanceList,
  children,
}: {
  instanceList: InstanceListState;
  children: React.ReactNode;
}) => {
  const store = configureStore({
    reducer: {
      clientInfo: createSlice({
        name: 'clientInfo',
        initialState: mockedClientInfo,
        reducers: {},
      }).reducer,
      instanceList: createSlice({
        name: 'instanceList',
        initialState: instanceList,
        reducers: {},
      }).reducer,
    },
  });
  return <Provider store={store}>{children}</Provider>;
};

export default {
  title: 'components/InstanceList',
  component: InstanceList,
  parameters: {
    backgrounds: {
      default: 'gray-700',
    },
  },
} as ComponentMeta<typeof InstanceList>;

const Template: ComponentStory<typeof InstanceList> = () => (
  <Split
    sizes={[25, 75]}
    snapOffset={0}
    gutterSize={0}
    minSize={0}
    className="flex flex-row"
  >
    <div className="flex flex-col">
      <InstanceList />
    </div>
    <div className="pl-4 text-gray-300">{'   <-Try drag here'}</div>
  </Split>
);

export const Default = Template.bind({});
Default.decorators = [
  (story) => <Mockstore instanceList={mockedInstanceList}>{story()}</Mockstore>,
];
