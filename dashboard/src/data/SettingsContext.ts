import { createContext } from 'react';
import { PublicUser } from '../@bindings/PublicUser';

interface SettingsContextType {
  userList: { [uuid: string]: PublicUser };
  selectedUser?: PublicUser | null;
  selectUser: (user: PublicUser|null) => void;
  tabIndex: number;
  setTabIndex: (index: number) => void;
}

export const SettingsContext = createContext<SettingsContextType>({
  userList: {},
  selectedUser: undefined,
  selectUser: () => {
    throw new Error('SettingsContext not initialized');
  },
  tabIndex: 0,
  setTabIndex: () => {
    throw new Error('SettingsContext not initialized');
  },
});
