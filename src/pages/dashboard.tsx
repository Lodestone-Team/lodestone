import { Tab } from '@headlessui/react';
import ClipboardTextfield from 'components/ClipboardTextfield';
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

const Dashboard: NextPageWithLayout = () => {
  const { address } = useContext(LodestoneContext);
  const { selectedInstance: instance } = useContext(InstanceContext);
  const queryClient = useQueryClient();
  const uuid = instance?.uuid;
  const router = useRouter();
  const [selectedTabIndex, setSelectedTabIndex] = useState(0);
  const canDeleteInstance = useUserAuthorized('can_delete_instance');

  if (!instance || !uuid) {
    return (
      <div
        className="relative flex h-full w-full flex-row justify-center overflow-y-auto px-4 pt-8 pb-10 @container"
        key={uuid}
      >
        <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
          <div className="flex min-w-0 flex-row items-center gap-4">
            <h1 className="truncate whitespace-pre dashboard-instance-heading">
              Instance not found
            </h1>
          </div>
        </div>
      </div>
    );
  }

  const labelColor = stateToLabelColor[instance.state];

  // tablist is map from GameType to tabs
  const tabList = {
    minecraft: [
      {
        title: 'Settings',
        content: (
          <>
            <MinecraftGeneralCard />
            <MinecraftSettingCard />
          </>
        ),
      },
      {
        title: 'Console',
        content: <GameConsole />,
      },
      {
        title: 'Files',
        content: <FileViewer />,
      },
      {
        title: 'Overview',
        content: (
          <DashboardCard>
            <h1 className="text-medium font-bold"> Placeholder </h1>
          </DashboardCard>
        ),
      },
      {
        title: 'Resources',
        content: (
          <DashboardCard>
            <h1 className="text-medium font-bold"> Placeholder </h1>
          </DashboardCard>
        ),
      },
      {
        title: 'Macro',
        content: (
          <DashboardCard>
            <h1 className="text-medium font-bold"> Placeholder </h1>
          </DashboardCard>
        ),
      },
      {
        title: 'Monitor',
        content: <MinecraftPerformanceCard />,
      },
    ],
  };

  const setInstanceName = async (name: string) => {
    await axiosPutSingleValue<void>(`/instance/${uuid}/name`, name);
    updateInstance(uuid, queryClient, (oldData) => ({
      ...oldData,
      name,
    }));
  };

  const setInstanceDescription = async (description: string) => {
    await axiosPutSingleValue<void>(
      `/instance/${uuid}/description`,
      description
    );
    updateInstance(uuid, queryClient, (oldData) => ({
      ...oldData,
      description,
    }));
  };

  return (
    // used to possibly center the content
    <div
      className="relative flex h-full w-full flex-row justify-center overflow-y-auto gutter-stable px-4 pt-8 pb-10 @container"
      key={uuid}
    >
      {/* main content container */}
      <div className="flex h-fit min-h-full w-full grow flex-col items-start gap-2">
        <div className="flex min-w-0 flex-row items-center gap-4">
          {/* TODO: create a universal "text with edit button" component */}
          <EditableTextfield
            initialText={instance.name}
            type={'heading'}
            onSubmit={setInstanceName}
            placeholder="No name"
            containerClassName="min-w-0"
          />
        </div>
        <div className="-mt-2 flex flex-row flex-wrap items-center gap-4">
          <GameIcon
            game_type={instance.game_type}
            game_flavour={instance.flavour}
            className="h-6 w-6"
          />
          <Label size="large" color={labelColor}>
            {instance.state}
          </Label>
          <Label size="large" color={labelColor}>
            Player Count {instance.player_count}/{instance.max_player_count}
          </Label>
          <Label
            size="large"
            color="gray"
            className="flex flex-row items-center gap-3"
          >
            <ClipboardTextfield text={`${address}:${instance.port}`} />
          </Label>
          <Button
            label="Delete"
            disabled={!canDeleteInstance}
            onClick={() => {
              axiosWrapper({
                method: 'DELETE',
                url: `/instance/${uuid}`,
              }).then(() => {
                queryClient.invalidateQueries(['instances', 'list']);
                router.push(
                  {
                    pathname: '/',
                    query: {
                      ...router.query,
                      uuid: null,
                    },
                  },
                  undefined,
                  { shallow: true }
                );
              });
            }}
          />
        </div>
        <div className="flex w-full flex-row items-center gap-2">
          <EditableTextfield
            initialText={instance.description}
            type={'description'}
            onSubmit={setInstanceDescription}
            placeholder="No description"
            containerClassName="min-w-0"
          />
        </div>
        <Tab.Group
          selectedIndex={selectedTabIndex}
          onChange={setSelectedTabIndex}
        >
          <Tab.List className="mb-6 flex w-full flex-row flex-wrap items-center gap-4 border-b-2 border-gray-700">
            {tabList[instance.game_type].map((tab) => (
              <Tab
                key={tab.title}
                className={({ selected }) =>
                  `text-medium font-semibold tracking-tight focus-visible:outline-none ${
                    selected
                      ? 'border-b-2 border-blue-accent text-blue-accent'
                      : 'mb-0.5 text-gray-500'
                  }`
                }
              >
                {tab.title}
              </Tab>
            ))}
          </Tab.List>
          <Tab.Panels className="flex w-full grow flex-row items-stretch focus:outline-none">
            {tabList[instance.game_type].map((tab) => (
              <Tab.Panel
                className="flex w-full flex-col gap-16"
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

Dashboard.getLayout = (page: ReactElement): ReactNode => (
  <DashboardLayout>{page}</DashboardLayout>
);

export default Dashboard;
