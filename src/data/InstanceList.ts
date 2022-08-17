import { ClientInfoState } from './ClientInfo';
import { createAsyncThunk, createSlice } from '@reduxjs/toolkit';
import type { PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from 'data/store';
import { Stats } from 'fs';

export type InstanceStatus =
  | 'stopped'
  | 'running'
  | 'starting'
  | 'stopping'
  | 'crashed'
  | 'error'
  | 'loading';
export type InstanceType =
  | 'minecraft'
  | 'minecraft-fabric'
  | 'minecraft-paper'
  | 'minecraft-forge';

export interface InstanceState {
  id: string;
  name: string;
  type: InstanceType;
  playerCount: number;
  maxPlayerCount: number;
  port: number;
  ip: string;
  status: InstanceStatus;
}

// instance list state is a map of id to instance state
export interface InstanceListState {
  instances: { [id: string]: InstanceState };
  loading: boolean;
  error: string | null;
}

const initialState: InstanceListState = {
  instances: {},
  loading: false,
  error: null,
};

// fetch instance using the client info
export const fetchInstanceList = createAsyncThunk(
  'instanceList/fetch',
  async ({ apiUrl }: ClientInfoState, thunkAPI) => {
    return fetch(`${apiUrl}/list`)
      .then((response) => {
        if (!response.ok) {
          throw new Error(response.statusText);
        }
        return response.json();
      })
      .then((data) => {
        const instances: { [id: string]: InstanceState } = {};
        for (let i = 0; i < data.length; i++) {
          instances[data[i].uuid] = {
            // TODO fix the any type here
            id: data[i].uuid,
            name: data[i].name,
            playerCount: 0, //TODO: update backend to return this value
            maxPlayerCount: 0, //TODO: update backend to return this value
            port: data[i].port,
            ip: 'idkman', //TODO: update backend to return this value
            status: 'stopped', //TODO: update backend to return this value
            type: data[i].type,
          };
        }
        return instances;
      })
      .catch((error) => {
        if (error instanceof Error) {
          return thunkAPI.rejectWithValue(error.message);
        } else {
          return thunkAPI.rejectWithValue('Unknown error');
        }
      });
  }
);

export const instanceListSlice = createSlice({
  name: 'instanceList',
  initialState,
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
    updateStatus(
      state,
      action: PayloadAction<{ id: string, status: InstanceStatus }>
    ) {
      state.instances[action.payload.id].status = action.payload.status;
    },
    overwriteInstanceList(state, action: PayloadAction<InstanceListState>) {
      state.instances = action.payload.instances;
    },
  },
  extraReducers: (builder) => {
    builder
      .addCase(fetchInstanceList.pending, (state) => {
        state.loading = true;
      })
      .addCase(fetchInstanceList.fulfilled, (state, action) => {
        state.loading = false;
        state.error = null;
        state.instances = action.payload;
      })
      .addCase(fetchInstanceList.rejected, (state, action) => {
        state.loading = false;
        // if action is a string, it's an error message
        if (typeof action.payload === 'string') {
          state.error = action.payload;
        } else {
          state.error = 'Unknown error';
        }
      });
  },
});

export const {
  addInstance,
  removeInstance,
  updateInstance,
  updateStatus,
  overwriteInstanceList,
} = instanceListSlice.actions;

export const selectInstanceList = (state: RootState) => state.instanceList;

export default instanceListSlice.reducer;
