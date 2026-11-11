import { HandlerGameType } from 'bindings/HandlerGameType';
import { createContext } from 'react';

type GameInstanceContextValue = {
  /** null until a game is picked */
  gameType: HandlerGameType | null;
  setGameType: (gameType: HandlerGameType) => void;
};

export const GameInstanceContext = createContext<GameInstanceContextValue>({
  gameType: null,
  setGameType: () => {
    throw new Error('CreateGameInstanceContext not initialized');
  },
});
