import { createContext } from 'react';
import { InstanceInfo } from './../bindings/InstanceInfo';

interface InstanceContextType {
  instanceList: { [uuid: string]: InstanceInfo };
  selectedInstance?: InstanceInfo;
  selectInstance: (instance?: InstanceInfo) => void;
}

export const InstanceContext = createContext<InstanceContextType>({
  instanceList: {},
  selectedInstance: undefined,
  selectInstance: () => {
    throw new Error('InstanceContext not initialized');
  },
});
