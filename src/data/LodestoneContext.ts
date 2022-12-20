import { createContext } from 'react';
import { LODESTONE_PORT } from 'utils/util';

interface LodestoneContext {
  core: CoreConnectionInfo;
  setCore: (core: CoreConnectionInfo, pathname?: string) => void;
  isReady: boolean;
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
  setCore: () => {
    console.error('setCore not implemented');
  },
  isReady: false,
  token: '',
  setToken: () => {
    console.error('setToken not implemented');
  },
  tokens: {},
});
