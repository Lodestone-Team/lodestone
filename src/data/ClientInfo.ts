import { createSlice } from '@reduxjs/toolkit';
import type { PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from 'data/store';

export interface ClientInfoState {
    protocol: string;
    ip: string;
    port: number;
    loading: boolean;
}

const initialState: ClientInfoState = {
    protocol: 'http://',
    ip: '',
    port: 3000,
    loading: true,
};

export const clientInfoSlice = createSlice({
    name: 'clientInfo',
    initialState,
    reducers: {
        setIp(state, action: PayloadAction<string>) {
            state.ip = action.payload;
        },
        setPort(state, action: PayloadAction<number>) {
            state.port = action.payload;
        },
        setLoading: (state, action: PayloadAction<boolean>) => {
            state.loading = action.payload;
        }
    }
});

export const { setIp, setPort, setLoading } = clientInfoSlice.actions;

export const selectClientInfo = (state: RootState) => state.clientInfo;

export default clientInfoSlice.reducer;
