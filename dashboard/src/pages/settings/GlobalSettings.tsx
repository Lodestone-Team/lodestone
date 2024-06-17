
import { cn } from 'utils/util';
import { CommandHistoryContextProvider } from 'data/CommandHistoryContext';
import { FontAwesomeIcon } from '@fortawesome/react-fontawesome';
import {
    faGears,
    faJarWheat,
} from '@fortawesome/free-solid-svg-icons';
import CoreSettings from './CoreSettings';
import GenericSetting from './GenericSettings';

export const tabs = [
    {
        title: 'Core Settings',
        displayTitle: null,
        path: 'core-settings',
        width: 'max-w-4xl',
        icon: <FontAwesomeIcon icon={faGears} />,
        content: <CoreSettings />,
    },
    {
        title: 'Verion Info',
        displayTitle: null,
        path: 'version',
        width: 'max-w-4xl',
        icon: <FontAwesomeIcon icon={faJarWheat} />,
        content: <GenericSetting />,
    }
];

const GlobalTabs = () => {
  return (
    <CommandHistoryContextProvider>
    {tabs.map((tab, index) => {
        return (
            <div
            className={cn(
                'relative mx-auto flex h-full w-full flex-row justify-center @container',
                tab.width
            )}
            key={index}
            >
            <div
                className="gutter-stable -mx-3 flex grow flex-row items-stretch overflow-y-auto pl-4 pr-2"
            >
                <div className="flex h-fit min-h-full w-full flex-col gap-2 pt-6 pb-10 focus:outline-none">
                {tab.displayTitle && (
                    <div className="flex font-title text-h1 font-bold leading-tight text-gray-300">
                    {tab.displayTitle}
                    </div>
                )}
                {tab.content}
                </div>
            </div>
            </div>
        )
    })}
    </CommandHistoryContextProvider>
  );
};

export default GlobalTabs;
