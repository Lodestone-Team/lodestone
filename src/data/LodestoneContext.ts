import { createContext } from 'react';
import { LODESTONE_PORT } from 'utils/util';

export type CoreConnectionStatus = 'loading' | 'error' | 'success';

interface LodestoneContext {
  core: CoreConnectionInfo;
  setCore: (core: CoreConnectionInfo) => void;
  addCore: (core: CoreConnectionInfo) => void;
  coreList: CoreConnectionInfo[];
  coreConnectionStatus: CoreConnectionStatus;
  setCoreConnectionStatus: (status: CoreConnectionStatus) => void;
  /** The JWT token string, where no token is an empty string */
  token: string;
  /** Sets the JWT token in state and localStorage, where no token is an empty string */
  setToken: (token: string, coreSocket: string) => void;
  /** All the tokens, a record from CoreSocket to token */
  tokens: Record<string, string>;
}

export interface CoreConnectionInfo {
  address: string;
  port: string;
  protocol: string;
  apiVersion: string;
}

export const LodestoneContext = createContext<LodestoneContext>({
  core: {
    address: '',
    port: LODESTONE_PORT.toString(),
    protocol: 'http',
    apiVersion: 'v1',
  } as CoreConnectionInfo,
  coreConnectionStatus: 'loading',
  addCore: () => {
    console.error('addCore not implemented');
  },
  setCoreConnectionStatus: () => {
    console.error('setCoreConnectionStatus not implemented');
  },
  setCore: () => {
    console.error('setCore not implemented');
  },
  coreList: [],
  token: '',
  setToken: () => {
    console.error('setToken not implemented');
  },
  tokens: {},
});
