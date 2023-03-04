import { createContext } from 'react';
import { InstanceInfo } from './../bindings/InstanceInfo';

interface InstanceContextType {
  instanceList: { [uuid: string]: InstanceInfo };
  selectedInstance: InstanceInfo | null;
  selectInstance: (instance: InstanceInfo | null) => void;
  isReady: boolean;
}

export const InstanceContext = createContext<InstanceContextType>({
  instanceList: {},
  selectedInstance: null,
  selectInstance: () => {
    throw new Error('InstanceContext not initialized');
  },
  isReady: false,
});
