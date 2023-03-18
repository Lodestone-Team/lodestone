import { Tab } from '@headlessui/react';
import { useContext, useEffect, useState } from 'react';
import CoreSettings from 'components/Settings/CoreSettings';
import UserSettings from 'components/Settings/UserSettings';
import { SettingsContext } from 'data/SettingsContext';
import { useDocumentTitle } from 'usehooks-ts';
import { UserState } from 'components/UserMenu';
import { BrowserLocationContext } from 'data/BrowserLocationContext';
import { LodestoneContext } from 'data/LodestoneContext';
import { useUid, useUserInfo } from 'data/UserInfo';
import { useLocation } from 'react-router-dom';
import InputBox from 'components/Atoms/Config/InputBox';

const ProfilePage = () => {
  useDocumentTitle('Profile - Lodestone');
  const { token, setToken, core } = useContext(LodestoneContext);
  const { setPathname, setSearchParam } = useContext(BrowserLocationContext);
  const location = useLocation();
  const uid = useUid();
  const { isLoading, isError, data: user } = useUserInfo();
  const [userState, setUserState] = useState<UserState>('logged-out');
  const { address, port } = core;
  const socket = `${address}:${port}`;

  useEffect(() => {
    if (!token) {
      setUserState('logged-out');
    } else if (isLoading) {
      setUserState('loading');
      return;
    } else if (isError) {
      setUserState('logged-out');
      return;
    } else {
      setUserState('logged-in');
    }
  }, [token, isLoading, isError, user]);

  return (
    // used to possibly center the content
    <div className="relative mx-auto flex h-full w-full max-w-2xl flex-row justify-center @container">
      <div className="flex w-full grow flex-col items-stretch gap-2 px-4 pt-8">
        <div className="flex min-w-0 flex-row items-center gap-4">
          <h1 className="dashboard-instance-heading text-gray-300">Profile</h1>
        </div>
        <InputBox
          label={'Username'}
          value={user?.username ?? ''}
          type="text"
          // isLoading={isLoading}
          onSubmit={async (value) => {
            console.log('hey');
          }}
        />

        <div className="mt-4 flex w-full flex-row items-center text-h3 font-extrabold text-gray-300">
          Password and Authentication
        </div>

        {/* <Tab.Group
          selectedIndex={tabIndex}
          onChange={(i) => {
            setTabIndex(i);
            if (i !== 1) {
              selectUser(undefined);
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
        </Tab.Group> */}
      </div>
    </div>
  );
};

export default ProfilePage;
