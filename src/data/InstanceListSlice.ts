import { createSlice } from '@reduxjs/toolkit';
import type { PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from 'data/store';

enum InstanceStatus {
  Stopped = 'Stopped',
  Starting = 'Starting',
  Running = 'Running',
  Stopping = 'Stopping',
  Error = 'Error',
}

interface InstanceState {
  id: string;
  name: string;
  playerCount: number;
  maxPlayerCount: number;
  port: number;
  ip: string;
  status: InstanceStatus;
}

// instance list state is a map of id to instance state
interface InstanceListState {
  instances: { [id: string]: InstanceState };
}

export const instanceListSlice = createSlice({
  name: 'instanceList',
  initialState: {
    instances: {},
  } as InstanceListState,
  reducers: {
    addInstance(state, action: PayloadAction<InstanceState>) {
      state.instances[action.payload.id] = action.payload;
    },
    removeInstance(state, action: PayloadAction<string>) {
      delete state.instances[action.payload];
    },
    updateInstance(state, action: PayloadAction<InstanceState>) {
      state.instances[action.payload.id] = action.payload;
    },
  },
});

export const { addInstance, removeInstance, updateInstance } =
  instanceListSlice.actions;

export const selectInstanceList = (state: RootState) => state.instanceList;

export default instanceListSlice.reducer;
