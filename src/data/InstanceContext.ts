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
  // eslint-disable-next-line @typescript-eslint/no-empty-function
  selectInstance: () => {},
});
