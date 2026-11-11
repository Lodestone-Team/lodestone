import { createContext } from 'react';
import { LODESTONE_PORT } from 'utils/util';

export type CoreConnectionStatus = 'loading' | 'error' | 'degraded' | 'success';

interface LodestoneContext {
  core: CoreConnectionInfo;
  setCore: (core: CoreConnectionInfo) => void;
  addCore: (core: CoreConnectionInfo) => void;
  coreList: CoreConnectionInfo[];
  coreConnectionStatus: CoreConnectionStatus;
  setCoreConnectionStatus: (status: CoreConnectionStatus) => void;
  /** The session's token: a JWT from signing in, or an API key (`lsk_…`). No token
   * is an empty string */
  token: string;
  uid: string;
  /** Sets the token in state and localStorage, where no token is an empty string.
   * An API key carries no claims, so signing in with one also gives the uid of the
   * user it acts as. */
  setToken: (token: string, coreSocket: string, uid?: string) => void;
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
  uid: '',
  setToken: () => {
    console.error('setToken not implemented');
  },
  tokens: {},
});
