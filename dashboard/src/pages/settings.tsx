import { Tab } from '@headlessui/react';
import { Fragment, useContext, useState } from 'react';
import { useUserInfo } from 'data/UserInfo';
import CoreSettings from 'pages/settings/CoreSettings';
import UserSettings from 'pages/settings/UserSettings';
import { SettingsContext } from 'data/SettingsContext';
import { useDocumentTitle } from 'usehooks-ts';
import { useT } from 'i18n';
//------------------------------!!!!!! NO LONGER IN USE !!!!!!!-------------------------------------
const SettingsPage = () => {
  const t = useT();
  useDocumentTitle(t('common.pageTitle', { page: t('settings.legacy.title') }));
  const { tabIndex, setTabIndex, selectUser } = useContext(SettingsContext);

  const tabList = [
    {
      id: 'general',
      title: t('settings.core.title'),
      content: <CoreSettings />,
    },
    {
      id: 'users',
      title: t('settings.users.title'),
      content: <UserSettings />,
    },
  ];

  return (
    // used to possibly center the content
    <div className="@container relative mx-auto flex h-full w-full flex-row justify-center">
      <div className="flex w-full grow flex-col items-stretch gap-2 px-4 pt-8">
        <div className="flex min-w-0 flex-row items-center gap-4">
          <h1 className="dashboard-instance-heading">
            {t('settings.legacy.title')}
          </h1>
        </div>
        {/* Headless UI 2 renders a div by default; keep the v1 Fragment so that the
            tab list and panels stay direct flex children */}
        <Tab.Group
          as={Fragment}
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
                key={tab.id}
                className={({ selected }) =>
                  `text-h3 font-bold tracking-medium focus-visible:outline-hidden ${
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
          <Tab.Panels className="-mx-4 flex grow gutter-stable flex-row items-stretch overflow-y-auto pr-2 pl-4">
            {tabList.map((tab) => (
              <Tab.Panel
                className="flex h-fit min-h-full w-full flex-col gap-8 pt-10 pb-8 focus:outline-hidden"
                key={tab.id}
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
