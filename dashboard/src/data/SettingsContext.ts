import { createContext } from 'react';
import { PublicUser } from '../bindings/PublicUser';

interface SettingsContextType {
  userList: { [uuid: string]: PublicUser };
  /** true until it is known whether there is a user list to show */
  userListLoading: boolean;
  selectedUser?: PublicUser | null;
  selectUser: (user: PublicUser|null) => void;
  tabIndex: number;
  setTabIndex: (index: number) => void;
}

export const SettingsContext = createContext<SettingsContextType>({
  userList: {},
  userListLoading: false,
  selectedUser: undefined,
  selectUser: () => {
    throw new Error('SettingsContext not initialized');
  },
  tabIndex: 0,
  setTabIndex: () => {
    throw new Error('SettingsContext not initialized');
  },
});
