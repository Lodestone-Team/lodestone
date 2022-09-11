import { createContext } from 'react';

interface LodestoneContext {
  address: string;
  port: string;
  protocol: string;
  apiVersion: string;
  isReady: boolean;
}

export const LodestoneContext = createContext<LodestoneContext>({
    address: 'localhost',
    port: '3000',
    protocol: 'http',
    apiVersion: 'v1',
    isReady: false,
});
