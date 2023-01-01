import { createContext } from 'react';
import { PublicUser } from '../bindings/PublicUser';

interface SettingsContextType {
  userList: { [uuid: string]: PublicUser };
  selectedUser?: PublicUser;
  selectUser: (user?: PublicUser) => void;
}

export const SettingsContext = createContext<SettingsContextType>({
  userList: {},
  selectedUser: undefined,
  selectUser: () => {
    throw new Error('SettingsContext not initialized');
  },
});
