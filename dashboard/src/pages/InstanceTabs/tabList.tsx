import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
  faChartLine,
  faCodeCompare,
  faCog,
  faFolder,
  faInbox,
  faPuzzlePiece,
  faServer,
} from '@fortawesome/free-solid-svg-icons';
import type { ReactNode } from 'react';
import type { MessageKey } from 'i18n';

// The instance tabs' names, paths and icons, without their content. The
// sidebar (SelectedInstanceInfo) imports this; importing InstanceTabs there
// would pull every tab's components into the main bundle and defeat the lazy
// loading of InstanceTabs.

export type InstanceTabPath =
  | 'overview'
  | 'settings'
  | 'console'
  | 'files'
  | 'mods'
  | 'macros'
  | 'logs';

/** a tab's name, a message without placeholders */
export type TabTitleKey = Extract<MessageKey, `server.tab.${string}`>;

export interface InstanceTabInfo {
  /** the name in the sidebar, translated when rendered: t(tab.titleKey) */
  titleKey: TabTitleKey;
  /** heading shown above the tab content, none if null */
  displayTitleKey: TabTitleKey | null;
  path: InstanceTabPath;
  /** max-width class of the tab content */
  width: string;
  icon: ReactNode;
}

export const tabs: InstanceTabInfo[] = [
  {
    titleKey: 'server.tab.overview',
    displayTitleKey: null,
    path: 'overview',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faChartLine} />,
  },
  {
    titleKey: 'server.tab.settings',
    displayTitleKey: 'server.tab.settings',
    path: 'settings',
    // the list of sections (11rem) and the content column (680px)
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faCog} />,
  },
  {
    titleKey: 'server.tab.console',
    displayTitleKey: 'server.tab.console',
    path: 'console',
    width: 'max-w-6xl',
    icon: <FontAwesomeIcon icon={faServer} />,
  },
  {
    titleKey: 'server.tab.files',
    displayTitleKey: 'server.tab.files',
    path: 'files',
    width: 'max-w-6xl',
    icon: <FontAwesomeIcon icon={faFolder} />,
  },
  {
    // "Plugins" on plugin instances, see utils/mods.ts; only shown on
    // instances with mod or plugin support
    titleKey: 'server.tab.mods',
    displayTitleKey: 'server.tab.mods',
    path: 'mods',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faPuzzlePiece} />,
  },
  {
    titleKey: 'server.tab.macros',
    displayTitleKey: 'server.tab.macros',
    path: 'macros',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faCodeCompare} />,
  },
  {
    titleKey: 'server.tab.logs',
    displayTitleKey: 'server.tab.logs',
    path: 'logs',
    width: 'max-w-4xl',
    icon: <FontAwesomeIcon icon={faInbox} />,
  },
];
