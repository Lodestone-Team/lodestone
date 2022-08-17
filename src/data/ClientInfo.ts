import { createSlice } from '@reduxjs/toolkit';
import type { PayloadAction } from '@reduxjs/toolkit';
import type { RootState } from 'data/store';

export interface ClientInfoState {
    apiUrl: string;
    loading: boolean;
}

const initialState: ClientInfoState = {
    apiUrl: '',
    loading: true,
};

export const clientInfoSlice = createSlice({
    name: 'clientInfo',
    initialState,
    reducers: {
        setapiUrl: (state, action: PayloadAction<string>) => {
            state.apiUrl = action.payload;
        },
        setLoading: (state, action: PayloadAction<boolean>) => {
            state.loading = action.payload;
        }
    }
});

export const { setapiUrl, setLoading } = clientInfoSlice.actions;

export const selectClientInfo = (state: RootState) => state.clientInfo;

export default clientInfoSlice.reducer;
