import { Tab } from '@headlessui/react';
import ClipboardTextfield from 'components/ClipboardTextfield';
import InputBox from 'components/Atoms/Config/InputBox';
import GameConsole from 'components/GameConsole';
import DashboardCard from 'components/DashboardCard';
import DashboardLayout from 'components/DashboardLayout';
import Label from 'components/Atoms/Label';
import { updateInstance } from 'data/InstanceList';
import { LodestoneContext } from 'data/LodestoneContext';
import { ReactElement, ReactNode, useContext, useState } from 'react';
import {
  axiosPutSingleValue,
  axiosWrapper,
  catchAsyncToString,
  stateToLabelColor,
} from 'utils/util';
import { NextPageWithLayout } from './_app';
import EditableTextfield from 'components/EditableTextfield';
import { useQueryClient } from '@tanstack/react-query';
import MinecraftGeneralCard from 'components/Minecraft/MinecraftGeneralCard';
import MinecraftSettingCard from 'components/Minecraft/MinecraftSettingCard';
import Button from 'components/Atoms/Button';
import { useRouter } from 'next/router';
import MinecraftPerformanceCard from 'components/Minecraft/MinecraftPerformanceCard';
import FileViewer from 'components/FileViewer';
import { useUserAuthorized } from 'data/UserInfo';
import { InstanceContext } from 'data/InstanceContext';
import GameIcon from 'components/Atoms/GameIcon';
import SettingField from 'components/SettingField';
import ToggleBox from 'components/Atoms/Config/ToggleBox';

const SettingsPage: NextPageWithLayout = () => {
  const { address } = useContext(LodestoneContext);
  const queryClient = useQueryClient();
  const router = useRouter();

  const nameField = (
    <InputBox
      label="Name"
      value={'PLACEHOLDER'}
      disabled={false}
      onSubmit={async (name) => {
        // TODO
      }}
    />
  );

  const unsafeModeField = (
    <ToggleBox
      label={'Unsafe Mode'}
      value={true} // TODO
      onChange={async (value) => {
        //  TODO
      }}
    />
  );

  return (
    // used to possibly center the content
    <div className="gutter-stable relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10 @container">
      <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-6">
        <div className="flex min-w-0 flex-row items-center gap-4">
          <h1 className="dashboard-instance-heading">Dashboard Settings</h1>
        </div>
        <div className="flex w-full flex-col gap-4 @4xl:flex-row">
          <div className="w-[28rem]">
            <h1 className="text-large font-black"> General Settings </h1>
            <h2 className="text-base font-medium italic tracking-tight text-white/50">
              Most commonly used settings for your server
            </h2>
          </div>
          <div className="w-full rounded-lg border border-gray-faded/30 child:w-full child:border-b child:border-gray-faded/30 first:child:rounded-t-lg last:child:rounded-b-lg last:child:border-b-0">
            {nameField}
            {unsafeModeField}
          </div>
        </div>
      </div>
    </div>
  );
};

SettingsPage.getLayout = (page: ReactElement): ReactNode => (
  <DashboardLayout>{page}</DashboardLayout>
);

export default SettingsPage;
