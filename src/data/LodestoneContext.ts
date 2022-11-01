import { createContext } from 'react';
import { DashboardNotification } from './EventStream';

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
  /** notifications */
  notifications: DashboardNotification[];
  /** add a notification */
  pushNotification: (notification: DashboardNotification) => void;
}

export const LodestoneContext = createContext<LodestoneContext>({
  address: 'localhost',
  port: '3000',
  protocol: 'http',
  apiVersion: 'v1',
  isReady: false,
  token: '',
  setToken: () => {
    console.error('setToken not implemented');
  },
  notifications: [],
  pushNotification: () => {
    console.error('pushNotification not implemented');
  },
});
