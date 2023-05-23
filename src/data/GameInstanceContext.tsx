import { GenericHandlerGameType } from 'components/Instance/InstanceCreateForm';
import { createContext } from 'react';

type GameInstanceContextValue = {
  gameType: GenericHandlerGameType;
  setGameType: (gameType: GenericHandlerGameType) => void;
  urlValid: boolean;
  setUrlValid: (value: boolean) => void;
  url: string;
  setUrl: (value: string) => void;
  genericFetchReady: boolean;
  setGenericFetchReady: (value: boolean) => void;
};

export const GameInstanceContext = createContext<GameInstanceContextValue>({
  gameType: 'Generic',
  setGameType: () => {
    throw new Error('CreateGameInstanceContext not initialized');
  },
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
