import { createContext } from 'react';
import { InstanceInfo } from './../bindings/InstanceInfo';

interface InstanceContextType {
  instanceList: { [uuid: string]: InstanceInfo };
  selectedInstance: InstanceInfo | null;
  selectInstance: (instance: InstanceInfo | null) => void;
  isReady: boolean;
  showCreateInstance: boolean;
  setShowCreateInstance: (show: boolean) => void;
}

export const InstanceContext = createContext<InstanceContextType>({
  instanceList: {},
  selectedInstance: null,
  selectInstance: () => {
    throw new Error('InstanceContext not initialized');
  },
  isReady: false,
  showCreateInstance: false,
  setShowCreateInstance: () => {
    throw new Error('InstanceContext not initialized');
  }
});
