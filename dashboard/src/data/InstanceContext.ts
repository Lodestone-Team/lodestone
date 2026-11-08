import { createContext } from 'react';
import { InstanceInfo } from './../bindings/InstanceInfo';

/** A Modrinth modpack to open the create-instance flow with. */
export type ModpackPreset = { projectId: string; versionId: string | null };

interface InstanceContextType {
  instanceList: { [uuid: string]: InstanceInfo };
  selectedInstance: InstanceInfo | null;
  selectInstance: (instance: InstanceInfo | null) => void;
  isReady: boolean;
  showCreateInstance: boolean;
  setShowCreateInstance: (show: boolean) => void;
  /** set before showing the create flow to start it on this modpack */
  modpackPreset: ModpackPreset | null;
  setModpackPreset: (preset: ModpackPreset | null) => void;
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
  },
  modpackPreset: null,
  setModpackPreset: () => {
    throw new Error('InstanceContext not initialized');
  },
});
