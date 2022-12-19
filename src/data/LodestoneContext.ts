import { createContext } from 'react';

interface LodestoneContext {
  address: string;
  port: string;
  socket: string;
  protocol: string;
  apiVersion: string;
  isReady: boolean;
  /** The JWT token string, where no token is an empty string */
  token: string;
  /** Sets the JWT token in state and localStorage, where no token is an empty string */
  setToken: (token: string, coreSocket: string) => void;
  /** All the tokens, a record from CoreSocket to token */
  tokens: Record<string, string>;
  setAddress: (address: string) => void;
  setPort: (port: string) => void;
  setProtocol: (protocol: string) => void;
  setApiVersion: (apiVersion: string) => void;
}

export const LodestoneContext = createContext<LodestoneContext>({
  address: 'localhost',
  port: '16662',
  socket: 'localhost:16662',
  protocol: 'http',
  apiVersion: 'v1',
  isReady: false,
  token: '',
  setToken: () => {
    console.error('setToken not implemented');
  },
  tokens: {},
  setAddress: () => {
    console.error('setAddress not implemented');
  },
  setPort: () => {
    console.error('setPort not implemented');
  },
  setProtocol: () => {
    console.error('setProtocol not implemented');
  },
  setApiVersion: () => {
    console.error('setApiVersion not implemented');
  },
});
