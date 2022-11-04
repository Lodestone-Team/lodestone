import { createContext } from 'react';

interface LodestoneContext {
  address: string;
  port: string;
  protocol: string;
  apiVersion: string;
  isReady: boolean;
  /** The JWT token string, where no token is an empty string */
  token: string;
  /** Sets the JWT token in state and localStorage, where no token is an empty string */
  setToken: (token: string) => void;
}

export const LodestoneContext = createContext<LodestoneContext>({
  address: 'localhost',
  port: '16662',
  protocol: 'http',
  apiVersion: 'v1',
  isReady: false,
  token: '',
  setToken: () => {
    console.error('setToken not implemented');
  },
});
