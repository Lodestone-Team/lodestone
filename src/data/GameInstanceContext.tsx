import { createContext } from 'react';

type GameInstanceContextValue = {
  urlValid: boolean;
  setUrlValid: (value: boolean) => void;
  url: string;
  setUrl: (value: string) => void;
  genericFetchReady: boolean;
  setGenericFetchReady: (value: boolean) => void;
};

export const GameInstanceContext = createContext<GameInstanceContextValue>({
  urlValid: false,
  setUrlValid: () => {
    throw new Error('CreateGameInstanceContext not initialized');
  },
  url: '',
  setUrl: () => {
    throw new Error('CreateGameInstanceContext not initialized');
  },
  genericFetchReady: false,
  setGenericFetchReady: () => {
    throw new Error('CreateGameInstanceContext not initialized');
  },
});
