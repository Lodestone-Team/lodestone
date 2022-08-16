import { createSlice } from '@reduxjs/toolkit';
import type { PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from 'data/store';

export interface ClientInfoState {
    address: string;
    port: number;
    loading: boolean;
}

const initialState: ClientInfoState = {
    address: 'localhost',
    port: 3000,
    loading: true,
};

export const clientInfoSlice = createSlice({
    name: 'clientInfo',
    initialState,
    reducers: {
        setAddress: (state, action: PayloadAction<string>) => {
            state.address = action.payload;
        },
        setPort: (state, action: PayloadAction<number>) => {
            state.port = action.payload;
        },
        setLoading: (state, action: PayloadAction<boolean>) => {
            state.loading = action.payload;
        }
    }
});

export const { setAddress, setPort, setLoading } = clientInfoSlice.actions;

export const selectClientInfo = (state: RootState) => state.clientInfo;

export default clientInfoSlice.reducer;
