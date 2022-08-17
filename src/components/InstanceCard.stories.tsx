import { configureStore, createSlice } from '@reduxjs/toolkit';
import { ComponentStory, ComponentMeta } from '@storybook/react';
import { ClientInfoState } from 'data/ClientInfo';
import { InstanceListState } from 'data/InstanceList';
import { Provider } from 'react-redux';
import Split from 'react-split';
import InstanceCard from './InstanceCard';

const mockedClientInfo = {
  loading: false,
  apiUrl: 'https://mocked-address.com:3000',
} as ClientInfoState;

const Mockstore = ({ children }: { children: React.ReactNode }) => {
  const store = configureStore({
    reducer: {
      clientInfo: createSlice({
        name: 'clientInfo',
        initialState: mockedClientInfo,
        reducers: {},
      }).reducer,
      instanceList: createSlice({
        name: 'instanceList',
        initialState: {} as InstanceListState,
        reducers: {},
      }).reducer,
    },
  });
  return <Provider store={store}>{children}</Provider>;
};

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
  <Mockstore>
    <Split sizes={[25, 75]} snapOffset={0} className="flex flex-row">
      <div className="flex flex-col child:w-full">
        <InstanceCard {...args} />
      </div>
      <div>Other Stuff</div>
    </Split>
  </Mockstore>
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
