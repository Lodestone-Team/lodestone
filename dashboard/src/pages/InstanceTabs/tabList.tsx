import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faCarTunnel,
  faChartLine,
  faCodeCompare,
  faCog,
  faFolder,
  faInbox,
  faServer,
} from '@fortawesome/free-solid-svg-icons';
import type { ReactNode } from 'react';

// The instance tabs' names, paths and icons, without their content. The
// sidebar (SelectedInstanceInfo) imports this; importing InstanceTabs there
// would pull every tab's components into the main bundle and defeat the lazy
// loading of InstanceTabs.

export type InstanceTabPath =
  | 'overview'
  | 'settings'
  | 'console'
  | 'files'
  | 'macros'
  | 'logs'
  | 'playitgg';

export interface InstanceTabInfo {
  title: string;
  /** heading shown above the tab content, none if null */
  displayTitle: string | null;
  path: InstanceTabPath;
  /** max-width class of the tab content */
  width: string;
  icon: ReactNode;
}

export const tabs: InstanceTabInfo[] = [
  {
    title: 'Overview',
    displayTitle: null,
    path: 'overview',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faChartLine} />,
  },
  {
    title: 'Settings',
    displayTitle: 'Settings',
    path: 'settings',
    width: 'max-w-2xl',
    icon: <FontAwesomeIcon icon={faCog} />,
  },
  {
    title: 'Console',
    displayTitle: 'Console',
    path: 'console',
    width: 'max-w-6xl',
    icon: <FontAwesomeIcon icon={faServer} />,
  },
  {
    title: 'Files',
    displayTitle: 'Files',
    path: 'files',
    width: 'max-w-6xl',
    icon: <FontAwesomeIcon icon={faFolder} />,
  },
  {
    title: 'Macros',
    displayTitle: 'Macros',
    path: 'macros',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faCodeCompare} />,
  },
  {
    title: 'Event Logs',
    displayTitle: 'Event Logs',
    path: 'logs',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faInbox} />,
  },
  {
    title: 'Playitgg',
    displayTitle: 'Playitgg',
    path: 'playitgg',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faCarTunnel} />,
  },
];
