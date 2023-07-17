import { Tab } from '@headlessui/react';
import { useContext, useState } from 'react';
import { useUserInfo } from 'data/UserInfo';
import CoreSettings from 'pages/settings/CoreSettings';
import UserSettings from 'pages/settings/UserSettings';
import { SettingsContext } from 'data/SettingsContext';
import { useDocumentTitle } from 'usehooks-ts';
//------------------------------!!!!!! NO LONGER IN USE !!!!!!!-------------------------------------
const SettingsPage = () => {
  useDocumentTitle('Core Settings - Lodestone');
  const { tabIndex, setTabIndex, selectUser } = useContext(SettingsContext);

  const tabList = [
    {
      title: 'General',
      content: <CoreSettings />,
    },
    {
      title: 'Users',
      content: <UserSettings />,
    },
  ];

  return (
    // used to possibly center the content
    <div className="relative mx-auto flex h-full w-full max-w-2xl flex-row justify-center @container">
      <div className="flex w-full grow flex-col items-stretch gap-2 px-4 pt-8">
        <div className="flex min-w-0 flex-row items-center gap-4">
          <h1 className="dashboard-instance-heading">Core Settings</h1>
        </div>
        <Tab.Group
          selectedIndex={tabIndex}
          onChange={(i) => {
            setTabIndex(i);
            if (i !== 1) {
              selectUser(null);
            }
          }}
        >
          <Tab.List className="flex w-full flex-row flex-wrap items-center gap-4 border-b-2 border-gray-700">
            {tabList.map((tab) => (
              <Tab
                key={tab.title}
                className={({ selected }) =>
                  `text-h3 font-bold tracking-medium focus-visible:outline-none ${
                    selected
                      ? 'border-b-2 border-blue-200 text-blue-200'
                      : 'mb-0.5 text-gray-500'
                  }`
                }
              >
                {tab.title}
              </Tab>
            ))}
          </Tab.List>
          <Tab.Panels className="gutter-stable -mx-4 flex grow flex-row items-stretch overflow-y-auto pl-4 pr-2">
            {tabList.map((tab) => (
              <Tab.Panel
                className="flex h-fit min-h-full w-full flex-col gap-8 pt-10 pb-8 focus:outline-none"
                key={tab.title}
              >
                {tab.content}
              </Tab.Panel>
            ))}
          </Tab.Panels>
        </Tab.Group>
      </div>
    </div>
  );
};

export default SettingsPage;
