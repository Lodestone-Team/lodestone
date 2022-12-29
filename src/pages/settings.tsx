import { Tab } from '@headlessui/react';
import InputBox from 'components/Atoms/Config/InputBox';
import { LodestoneContext } from 'data/LodestoneContext';
import { useContext, useState } from 'react';
import { axiosPutSingleValue, errorToString } from 'utils/util';
import { useQueryClient } from '@tanstack/react-query';
import { useUserInfo } from 'data/UserInfo';
import ToggleBox from 'components/Atoms/Config/ToggleBox';
import { useGlobalSettings } from 'data/GlobalSettings';
import { useCoreInfo } from 'data/SystemInfo';

const SettingsPage = () => {
  const { core } = useContext(LodestoneContext);
  const queryClient = useQueryClient();
  const [selectedTabIndex, setSelectedTabIndex] = useState(0);
  const { data: globalSettings, isLoading, error } = useGlobalSettings();
  const { data: coreInfo } = useCoreInfo();
  const { data: userInfo, isLoading: userLoading } = useUserInfo();
  const can_change_core_settings = userInfo?.is_owner ?? false;

  const errorString = errorToString(error);

  const nameField = (
    <InputBox
      label="Core Name"
      value={globalSettings?.core_name}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={
        'A nickname for this core. This is what you and others will see when you connect to this core.'
      }
      validate={async (name) => {
        // don't be empty
        if (name === '') throw new Error('Name cannot be empty');
        // don't be too long
        if (name.length > 32)
          throw new Error('Name cannot be longer than 32 characters');
      }}
      onSubmit={async (name) => {
        await axiosPutSingleValue('/global_settings/name', name);
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          core_name: name,
        });
        queryClient.setQueryData(['systeminfo', 'CoreInfo'], {
          ...coreInfo,
          core_name: name,
        });
      }}
    />
  );

  const domainField = (
    <InputBox
      label="Public Domain/IP"
      value={globalSettings?.domain ?? ''}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={
        //TODO: more info needed once we add more functionality
        'The domain or public IP address of this core.'
      }
      placeholder={`${core?.address}`}
      validate={async (domain) => {
        // can be empty
        if (domain === '') return;
        // don't be too long
        if (domain.length > 253)
          throw new Error('Domain cannot be longer than 253 characters');
      }}
      onSubmit={async (domain) => {
        await axiosPutSingleValue('/global_settings/domain', domain);
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          domain: domain,
        });
      }}
    />
  );

  const unsafeModeField = (
    <ToggleBox
      label={'Safe Mode'}
      value={globalSettings?.safe_mode ?? false}
      isLoading={isLoading}
      error={errorString}
      disabled={!can_change_core_settings}
      canRead={userInfo !== undefined}
      description={
        'Safe mode limits non-owner users to only relatively safe commands. Unsafe mode allows users to potentially take over your server/computer.'
      }
      onChange={async (value) => {
        await axiosPutSingleValue('/global_settings/safe_mode', value);
        queryClient.setQueryData(['global_settings'], {
          ...globalSettings,
          safe_mode: value,
        });
      }}
    />
  );

  const tabList = [
    {
      title: 'Settings',
      content: (
        <div className="flex w-full flex-col gap-4 @4xl:flex-row">
          <div className="w-[28rem]">
            <h1 className="text-large font-black"> Owner Settings </h1>
            <h2 className="text-base font-medium italic tracking-tight text-white/50">
              Settings that only the owner can change
            </h2>
          </div>
          <div className="w-full rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
            {nameField}
            {domainField}
            {unsafeModeField}
          </div>
        </div>
      ),
    },
  ];

  return (
    // used to possibly center the content
    <div className="gutter-stable relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10 @container">
      <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
        <div className="flex min-w-0 flex-row items-center gap-4">
          <h1 className="dashboard-instance-heading">Core Settings</h1>
        </div>
        <Tab.Group
          selectedIndex={selectedTabIndex}
          onChange={setSelectedTabIndex}
        >
          <Tab.List className="mb-6 flex w-full flex-row flex-wrap items-center gap-4 border-b-2 border-gray-700">
            {tabList.map((tab) => (
              <Tab
                key={tab.title}
                className={({ selected }) =>
                  `text-medium font-semibold tracking-tight focus-visible:outline-none ${
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
          <Tab.Panels className="flex w-full grow flex-row items-stretch">
            {tabList.map((tab) => (
              <Tab.Panel
                className="flex w-full flex-col gap-16 focus:outline-none"
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
